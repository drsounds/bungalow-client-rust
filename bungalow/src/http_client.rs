//! Port of `WinHttpClient`: a blocking HTTP request for worker threads,
//! implemented with ureq instead of WinHTTP.

use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Default)]
pub struct HttpResult {
    /// Transport succeeded and a response was read.
    pub ok: bool,
    pub cancelled: bool,
    pub status: i32,
    pub content_length: i64,
    pub content_type: String,
    pub content_disposition: String,
    pub error: String,
}

/// Performs a request. `on_data` receives body chunks and returns false to
/// abort. Follows redirects.
pub fn http_request(
    url: &str,
    method: &str,
    mut on_data: Option<&mut dyn FnMut(&[u8]) -> bool>,
    cancel: Option<&AtomicBool>,
    on_headers: Option<&mut dyn FnMut(&HttpResult)>,
) -> HttpResult {
    let mut result = HttpResult { content_length: -1, ..Default::default() };
    let response = match ureq::request(method, url).set("User-Agent", "Bungalow/1.0").call() {
        Ok(response) => response,
        Err(ureq::Error::Status(_, response)) => response,
        Err(e) => {
            result.error = e.to_string();
            return result;
        }
    };
    result.ok = true;
    result.status = response.status() as i32;
    result.content_type = response.header("Content-Type").unwrap_or_default().to_string();
    result.content_disposition = response.header("Content-Disposition").unwrap_or_default().to_string();
    result.content_length = response
        .header("Content-Length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(-1);
    if let Some(on_headers) = on_headers {
        on_headers(&result);
    }
    let Some(on_data) = on_data.as_deref_mut() else {
        return result;
    };
    let mut reader = response.into_reader();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            result.cancelled = true;
            break;
        }
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if !on_data(&buf[..n]) {
                    result.cancelled = true;
                    break;
                }
            }
            Err(e) => {
                result.ok = false;
                result.error = e.to_string();
                break;
            }
        }
    }
    result
}
