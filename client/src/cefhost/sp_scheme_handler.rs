//! Port of `SpSchemeHandler`: serves app bundles to the embedded browsers
//! at `sp://<bundle>/`.
//!
//! `sp` is registered as a standard, secure scheme in every process (see
//! `cef_runtime`), so each bundle is an origin of its own and keeps its
//! storage apart from the others. All browsers share one factory, which
//! serves the bundles mounted here; CEF calls it on its IO thread.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use space::sp_app_manager::SPAppManager;

pub const SCHEME: &str = "sp";

/// One bundle served at `sp://<host>/`.
#[derive(Clone, Default)]
pub struct Mount {
    pub bundle_root: String,
    /// Lets any origin read the files (the shared API bundle; sp.require()
    /// uses XHR).
    pub allow_any_origin: bool,
    /// Script run in every HTML page before the page's own scripts.
    pub inject_script: Option<String>,
    /// Content-Security-Policy for the bundle's pages: keeps the app's
    /// requests to its bundle and its allowed hosts, like the C++ request
    /// filter.
    pub content_security_policy: Option<String>,
}

type Mounts = Arc<RwLock<HashMap<String, Mount>>>;

#[derive(Default)]
pub struct SpSchemeHandler {
    mounts: Mounts,
}

impl SpSchemeHandler {
    pub fn new() -> Self {
        Self::default()
    }

    /// `sp://<host>/`.
    pub fn origin(&self, host: &str) -> String {
        format!("{SCHEME}://{host}/")
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

/// A complete response, read before CEF asks for the headers.
struct Resource {
    status: i32,
    status_text: &'static str,
    mime_type: &'static str,
    charset: Option<&'static str>,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
}

impl Resource {
    fn error(status: i32, status_text: &'static str) -> Self {
        Self { status, status_text, mime_type: "text/plain", charset: None, headers: Vec::new(), body: Vec::new() }
    }
}

/// "sp://Bundle/a/b?x#y" -> ("bundle", "/a/b").
fn split_url(url: &str) -> Option<(String, &str)> {
    let rest = url.strip_prefix(SCHEME)?.strip_prefix("://")?;
    let (host, path) = rest.find(['/', '?', '#']).map_or((rest, ""), |at| rest.split_at(at));
    let path = path.split(['?', '#']).next().unwrap_or_default();
    (!host.is_empty()).then(|| (host.to_ascii_lowercase(), path))
}

fn respond(mounts: &Mounts, url: &str) -> Resource {
    let Some((host, path)) = split_url(url) else { return Resource::error(400, "Bad Request") };
    let Some(mount) = mounts.read().unwrap().get(&host).cloned() else {
        return Resource::error(404, "Not Found");
    };
    let Some(path) = percent_decode_path(path) else { return Resource::error(400, "Bad Request") };
    let Some(file) = find_file(&mount.bundle_root, path.trim_start_matches('/')) else {
        return Resource::error(404, "Not Found");
    };
    let Ok(mut body) = std::fs::read(&file) else { return Resource::error(404, "Not Found") };

    let extension = file.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    let is_html = extension == "html" || extension == "htm";
    if is_html {
        if let Some(script) = &mount.inject_script {
            body = inject_script(&body, script);
        }
    }
    let mut headers = vec![("Cache-Control", "no-cache".to_string())];
    if mount.allow_any_origin {
        headers.push(("Access-Control-Allow-Origin", "*".to_string()));
    }
    if is_html {
        if let Some(csp) = &mount.content_security_policy {
            headers.push(("Content-Security-Policy", csp.clone()));
        }
    }
    let (mime_type, charset) = mime_type(&extension);
    Resource { status: 200, status_text: "OK", mime_type, charset, headers, body }
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

/// `CefGetMimeType` for the file types bundles ship, with the charset of
/// text types.
fn mime_type(extension: &str) -> (&'static str, Option<&'static str>) {
    const UTF8: Option<&str> = Some("utf-8");
    match extension {
        "html" | "htm" => ("text/html", UTF8),
        "js" | "mjs" => ("text/javascript", UTF8),
        "css" => ("text/css", UTF8),
        "json" | "map" => ("application/json", None),
        "xml" => ("application/xml", None),
        "txt" | "md" => ("text/plain", UTF8),
        "svg" => ("image/svg+xml", None),
        "png" => ("image/png", None),
        "jpg" | "jpeg" => ("image/jpeg", None),
        "gif" => ("image/gif", None),
        "webp" => ("image/webp", None),
        "ico" => ("image/x-icon", None),
        "woff" => ("font/woff", None),
        "woff2" => ("font/woff2", None),
        "ttf" => ("font/ttf", None),
        "otf" => ("font/otf", None),
        "wasm" => ("application/wasm", None),
        "mp3" => ("audio/mpeg", None),
        "ogg" => ("audio/ogg", None),
        "wav" => ("audio/wav", None),
        "mp4" => ("video/mp4", None),
        "webm" => ("video/webm", None),
        _ => ("application/octet-stream", None),
    }
}

#[cfg(feature = "cef")]
pub use factory::register_scheme;

#[cfg(feature = "cef")]
mod factory {
    use std::os::raw::c_int;
    use std::sync::Mutex;

    use cef::*;

    use super::*;

    /// Options of the `sp` scheme: standard (origins and relative URLs),
    /// secure (pages may use https and wss), and readable by XHR and fetch.
    fn scheme_options() -> c_int {
        use sys::cef_scheme_options_t as O;
        [O::CEF_SCHEME_OPTION_STANDARD, O::CEF_SCHEME_OPTION_SECURE, O::CEF_SCHEME_OPTION_CORS_ENABLED, O::CEF_SCHEME_OPTION_FETCH_ENABLED]
            .into_iter()
            .fold(0, |options, option| options | option as c_int)
    }

    /// Declares the scheme. Every process must, from `on_register_custom_schemes`.
    pub fn register_scheme(registrar: &mut SchemeRegistrar) {
        registrar.add_custom_scheme(Some(&SCHEME.into()), scheme_options());
    }

    impl SpSchemeHandler {
        /// Serves the mounted bundles to every browser. Call once, after CEF
        /// has started.
        pub fn register(&self) -> bool {
            let mut factory = BundleFactory::new(self.mounts.clone());
            register_scheme_handler_factory(Some(&SCHEME.into()), None, Some(&mut factory)) == 1
        }
    }

    wrap_scheme_handler_factory! {
        struct BundleFactory {
            mounts: Mounts,
        }

        impl SchemeHandlerFactory {
            fn create(
                &self,
                _browser: Option<&mut Browser>,
                _frame: Option<&mut Frame>,
                _scheme_name: Option<&CefString>,
                request: Option<&mut Request>,
            ) -> Option<ResourceHandler> {
                let url = request.map(|r| CefString::from(&r.url()).to_string()).unwrap_or_default();
                Some(BundleResource::new(Arc::new(respond(&self.mounts, &url)), Arc::default()))
            }
        }
    }

    wrap_resource_handler! {
        struct BundleResource {
            resource: Arc<Resource>,
            offset: Arc<Mutex<usize>>,
        }

        impl ResourceHandler {
            /// The response is already complete.
            fn open(
                &self,
                _request: Option<&mut Request>,
                handle_request: Option<&mut c_int>,
                _callback: Option<&mut Callback>,
            ) -> c_int {
                if let Some(handle_request) = handle_request {
                    *handle_request = 1;
                }
                1
            }

            fn response_headers(
                &self,
                response: Option<&mut Response>,
                response_length: Option<&mut i64>,
                _redirect_url: Option<&mut CefString>,
            ) {
                let resource = &self.resource;
                if let Some(response) = response {
                    response.set_status(resource.status);
                    response.set_status_text(Some(&resource.status_text.into()));
                    response.set_mime_type(Some(&resource.mime_type.into()));
                    if let Some(charset) = resource.charset {
                        response.set_charset(Some(&charset.into()));
                    }
                    for (name, value) in &resource.headers {
                        response.set_header_by_name(Some(&(*name).into()), Some(&value.as_str().into()), 1);
                    }
                }
                if let Some(length) = response_length {
                    *length = resource.body.len() as i64;
                }
            }

            /// Media seeks skip ahead.
            fn skip(
                &self,
                bytes_to_skip: i64,
                bytes_skipped: Option<&mut i64>,
                _callback: Option<&mut ResourceSkipCallback>,
            ) -> c_int {
                let mut offset = self.offset.lock().unwrap();
                let skipped = (bytes_to_skip.max(0) as usize).min(self.resource.body.len() - *offset);
                *offset += skipped;
                if let Some(bytes_skipped) = bytes_skipped {
                    // ERR_FAILED when there is nothing left to skip.
                    *bytes_skipped = if skipped > 0 { skipped as i64 } else { -2 };
                }
                (skipped > 0).into()
            }

            fn read(
                &self,
                data_out: *mut u8,
                bytes_to_read: c_int,
                bytes_read: Option<&mut c_int>,
                _callback: Option<&mut ResourceReadCallback>,
            ) -> c_int {
                let mut offset = self.offset.lock().unwrap();
                let body = &self.resource.body;
                let count = (bytes_to_read.max(0) as usize).min(body.len() - *offset);
                if count > 0 && !data_out.is_null() {
                    // SAFETY: CEF passes a buffer of bytes_to_read bytes, valid for the call.
                    unsafe { std::ptr::copy_nonoverlapping(body[*offset..].as_ptr(), data_out, count) };
                    *offset += count;
                }
                if let Some(bytes_read) = bytes_read {
                    *bytes_read = count as c_int;
                }
                // 0 bytes read and false: the response is complete.
                (count > 0).into()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_map_to_bundles() {
        assert_eq!(split_url("sp://testapp/a/b?x#y"), Some(("testapp".into(), "/a/b")));
        assert_eq!(split_url("sp://Import"), Some(("import".into(), "")));
        assert_eq!(split_url("sp://app?x"), Some(("app".into(), "")));
        assert_eq!(split_url("https://example.com/"), None);
        assert_eq!(split_url("sp:///x"), None);
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
        let handler = SpSchemeHandler::new();
        handler.mount(
            "app",
            Mount {
                bundle_root: root.to_string_lossy().into_owned(),
                inject_script: Some("i()".into()),
                ..Default::default()
            },
        );
        let js = respond(&handler.mounts, "sp://app/app.js?v=1");
        assert_eq!((js.status, js.mime_type, js.body.as_slice()), (200, "text/javascript", b"x".as_slice()));
        let route = respond(&handler.mounts, "sp://app/some/route");
        assert_eq!(String::from_utf8(route.body).unwrap(), "<head><script>i()</script></head>shell");
        assert_eq!(respond(&handler.mounts, "sp://app/missing.png").status, 404);
        assert_eq!(respond(&handler.mounts, "sp://other/").status, 404);
        std::fs::remove_dir_all(root).ok();
    }
}
