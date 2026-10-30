//! Port of `SpSchemeHandler`: serves app bundles to the embedded browsers.
//!
//! CEF let the C++ client register an `sp://` scheme per request context;
//! Wef has no custom schemes, so bundles are served over loopback HTTP
//! instead, one origin per bundle: `http://<bundle>.localhost:<port>/`.
//! Chromium resolves every `*.localhost` name to the loopback address, and
//! the separate origins keep each app's storage apart like the per-app
//! profiles did.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use space::sp_app_manager::SPAppManager;

/// One bundle served at `http://<host>.localhost:<port>/`.
#[derive(Clone, Default)]
pub struct Mount {
    pub bundle_root: String,
    /// Lets any origin read the files (the shared API bundle; sp.require()
    /// uses XHR).
    pub allow_any_origin: bool,
    /// Script run in every HTML page before the page's own scripts.
    pub inject_script: Option<String>,
    /// Content-Security-Policy for the bundle's pages: stands in for the
    /// C++ request filter that kept apps inside their bundle.
    pub content_security_policy: Option<String>,
}

type Mounts = Arc<RwLock<HashMap<String, Mount>>>;

pub struct SpSchemeServer {
    port: u16,
    mounts: Mounts,
}

impl SpSchemeServer {
    /// Starts serving on a free loopback port.
    pub fn start() -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let port = listener.local_addr()?.port();
        let mounts: Mounts = Arc::default();
        let served = mounts.clone();
        std::thread::Builder::new().name("sp-scheme-server".into()).spawn(move || {
            for stream in listener.incoming().flatten() {
                let mounts = served.clone();
                std::thread::spawn(move || {
                    if let Err(e) = handle_connection(stream, &mounts) {
                        log::debug!("sp:// request failed: {e}");
                    }
                });
            }
        })?;
        Ok(Self { port, mounts })
    }

    /// `http://<host>.localhost:<port>/`, the stand-in for `sp://<host>/`.
    pub fn origin(&self, host: &str) -> String {
        format!("http://{host}.localhost:{}/", self.port)
    }

    pub fn mount(&self, host: &str, mount: Mount) {
        self.mounts.write().unwrap().insert(host.to_ascii_lowercase(), mount);
    }

    pub fn unmount(&self, host: &str) {
        self.mounts.write().unwrap().remove(&host.to_ascii_lowercase());
    }

    pub fn is_mounted(&self, host: &str) -> bool {
        self.mounts.read().unwrap().contains_key(&host.to_ascii_lowercase())
    }
}

struct Response {
    status: u16,
    status_text: &'static str,
    content_type: String,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
}

impl Response {
    fn error(status: u16, status_text: &'static str) -> Self {
        Self { status, status_text, content_type: "text/plain".into(), headers: Vec::new(), body: Vec::new() }
    }
}

fn handle_connection(stream: TcpStream, mounts: &Mounts) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut stream = stream;
    // Keep-alive: serve requests until the browser closes the connection.
    loop {
        let mut request_line = String::new();
        if reader.read_line(&mut request_line)? == 0 {
            return Ok(());
        }
        let mut host = String::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 {
                return Ok(());
            }
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some((name, value)) = line.split_once(':') {
                if name.eq_ignore_ascii_case("host") {
                    host = value.trim().to_string();
                }
            }
        }
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or_default().to_string();
        let target = parts.next().unwrap_or_default().to_string();
        let response = match method.as_str() {
            "GET" | "HEAD" => respond(mounts, &host, &target),
            _ => Response::error(405, "Method Not Allowed"),
        };
        write_response(&mut stream, &response, method == "HEAD")?;
    }
}

fn write_response(stream: &mut TcpStream, response: &Response, head_only: bool) -> std::io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\n",
        response.status,
        response.status_text,
        response.content_type,
        response.body.len()
    );
    for (name, value) in &response.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    if !head_only {
        stream.write_all(&response.body)?;
    }
    stream.flush()
}

/// "<bundle>.localhost:<port>" -> "<bundle>".
fn bundle_host(host_header: &str) -> Option<String> {
    let host = host_header.rsplit_once(':').map_or(host_header, |(h, _)| h);
    host.strip_suffix(".localhost").filter(|h| !h.is_empty()).map(str::to_ascii_lowercase)
}

fn respond(mounts: &Mounts, host_header: &str, target: &str) -> Response {
    let Some(host) = bundle_host(host_header) else { return Response::error(400, "Bad Request") };
    let Some(mount) = mounts.read().unwrap().get(&host).cloned() else {
        return Response::error(404, "Not Found");
    };
    let path = target.split(['?', '#']).next().unwrap_or_default();
    let Some(path) = percent_decode_path(path) else { return Response::error(400, "Bad Request") };
    let Some(file) = find_file(&mount.bundle_root, path.trim_start_matches('/')) else {
        return Response::error(404, "Not Found");
    };
    let Ok(mut body) = std::fs::read(&file) else { return Response::error(404, "Not Found") };

    let extension = file.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    let is_html = extension == "html" || extension == "htm";
    if is_html {
        if let Some(script) = &mount.inject_script {
            body = inject_script(&body, script);
        }
    }
    let mut headers = Vec::new();
    if mount.allow_any_origin {
        headers.push(("Access-Control-Allow-Origin", "*".to_string()));
    }
    if is_html {
        if let Some(csp) = &mount.content_security_policy {
            headers.push(("Content-Security-Policy", csp.clone()));
        }
    }
    Response { status: 200, status_text: "OK", content_type: mime_type(&extension).to_string(), headers, body }
}

/// Maps a request path inside the bundle to the file to serve.
fn find_file(bundle_root: &str, relative_path: &str) -> Option<PathBuf> {
    let target = PathBuf::from(SPAppManager::resolve_in_bundle(bundle_root, relative_path)?);
    if target.is_file() {
        return Some(target);
    }
    if target.is_dir() && target.join("index.html").is_file() {
        return Some(target.join("index.html"));
    }
    // Client-side route of a single-page app: serve the app shell.
    if target.extension().is_none() {
        let shell = Path::new(bundle_root).join("index.html");
        if shell.is_file() {
            return Some(shell);
        }
    }
    None
}

/// Decodes %XX escapes, but keeps %2F and %5C escaped so they cannot
/// introduce path separators. None for malformed escapes or non-UTF-8.
fn percent_decode_path(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            let value = u8::from_str_radix(hex, 16).ok()?;
            if value == b'/' || value == b'\\' {
                out.extend_from_slice(&bytes[i..i + 3]);
            } else {
                out.push(value);
            }
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Inserts `<script>` right after `<head>` (or at the start of the page).
fn inject_script(html: &[u8], script: &str) -> Vec<u8> {
    let tag = format!("<script>{}</script>", script.replace("</", "<\\/"));
    let text = String::from_utf8_lossy(html);
    let lower = text.to_ascii_lowercase();
    let at = lower.find("<head").and_then(|start| lower[start..].find('>').map(|end| start + end + 1)).unwrap_or(0);
    let mut out = String::with_capacity(text.len() + tag.len());
    out.push_str(&text[..at]);
    out.push_str(&tag);
    out.push_str(&text[at..]);
    out.into_bytes()
}

/// `CefGetMimeType` for the file types bundles ship.
fn mime_type(extension: &str) -> &'static str {
    match extension {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "xml" => "application/xml",
        "txt" | "md" => "text/plain; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "wasm" => "application/wasm",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_map_to_bundles() {
        assert_eq!(bundle_host("testapp.localhost:4321").as_deref(), Some("testapp"));
        assert_eq!(bundle_host("Import.localhost").as_deref(), Some("import"));
        assert_eq!(bundle_host("example.com:80"), None);
        assert_eq!(bundle_host(".localhost:1"), None);
    }

    #[test]
    fn escaped_separators_stay_escaped() {
        assert_eq!(percent_decode_path("/a%20b/c").as_deref(), Some("/a b/c"));
        assert_eq!(percent_decode_path("/..%2Fsecret").as_deref(), Some("/..%2Fsecret"));
        assert_eq!(percent_decode_path("/bad%zz"), None);
    }

    #[test]
    fn scripts_go_first_in_head() {
        let html = b"<!doctype html><html><HEAD lang=en><title>x</title></head></html>";
        let out = String::from_utf8(inject_script(html, "a()</script>")).unwrap();
        assert_eq!(
            out,
            "<!doctype html><html><HEAD lang=en><script>a()<\\/script></script><title>x</title></head></html>"
        );
        assert_eq!(String::from_utf8(inject_script(b"<p>", "b()")).unwrap(), "<script>b()</script><p>");
    }

    #[test]
    fn serves_bundle_files_and_spa_routes() {
        let root = std::env::temp_dir().join(format!("sp-scheme-test-{}", std::process::id()));
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("index.html"), "<head></head>shell").unwrap();
        std::fs::write(root.join("app.js"), "x").unwrap();
        let mounts: Mounts = Arc::default();
        mounts.write().unwrap().insert(
            "app".into(),
            Mount {
                bundle_root: root.to_string_lossy().into_owned(),
                inject_script: Some("i()".into()),
                ..Default::default()
            },
        );
        let js = respond(&mounts, "app.localhost:1", "/app.js?v=1");
        assert_eq!((js.status, js.body.as_slice()), (200, b"x".as_slice()));
        let route = respond(&mounts, "app.localhost:1", "/some/route");
        assert_eq!(String::from_utf8(route.body).unwrap(), "<head><script>i()</script></head>shell");
        assert_eq!(respond(&mounts, "app.localhost:1", "/missing.png").status, 404);
        assert_eq!(respond(&mounts, "other.localhost:1", "/").status, 404);
        std::fs::remove_dir_all(root).ok();
    }
}
