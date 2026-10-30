//! Port of `FaviconCache` (windows/): fetches and caches favicons from the
//! network on worker threads. `on_ready` runs on the UI thread.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use bungalow::http_client::http_request;
use bungalow::platform_ui;
use bungalow::ui_dispatcher::post_to_ui;

pub struct FaviconCache {
    /// Cleared on drop so late results are ignored, like the C++ m_alive token.
    alive: Rc<()>,
    /// UI thread only.
    in_flight: Rc<RefCell<BTreeSet<String>>>,
}

thread_local! {
    /// Results handed from worker threads to the UI thread: (domain, success).
    static PENDING_CALLBACKS: RefCell<Vec<(u64, Box<dyn FnOnce(bool)>)>> = const { RefCell::new(Vec::new()) };
    static NEXT_ID: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
}

impl Default for FaviconCache {
    fn default() -> Self {
        Self::new()
    }
}

impl FaviconCache {
    pub fn new() -> Self {
        Self { alive: Rc::new(()), in_flight: Rc::new(RefCell::new(BTreeSet::new())) }
    }

    pub fn cache_dir() -> String {
        let dir = format!("{}/favicons", platform_ui::user_data_dir());
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    pub fn get_cached_path(&self, domain: &str) -> String {
        let path = format!("{}/{domain}.ico", Self::cache_dir());
        if Path::new(&path).exists() {
            path
        } else {
            String::new()
        }
    }

    pub fn fetch_if_needed(&self, domain: &str, favicon_url: &str, on_ready: Box<dyn Fn()>) {
        if !self.get_cached_path(domain).is_empty() || self.in_flight.borrow().contains(domain) {
            return;
        }
        self.in_flight.borrow_mut().insert(domain.to_string());

        // The completion runs on the UI thread; it is parked here (it is not
        // Send) and looked up by id when the worker posts its result.
        let id = NEXT_ID.with(|n| {
            let id = n.get();
            n.set(id + 1);
            id
        });
        let alive = Rc::downgrade(&self.alive);
        let in_flight = Rc::downgrade(&self.in_flight);
        let domain_owned = domain.to_string();
        PENDING_CALLBACKS.with(|p| {
            p.borrow_mut().push((
                id,
                Box::new(move |success| {
                    if alive.upgrade().is_none() {
                        return;
                    }
                    if let Some(in_flight) = in_flight.upgrade() {
                        in_flight.borrow_mut().remove(&domain_owned);
                    }
                    if success {
                        on_ready();
                    }
                }),
            ))
        });

        let dest_path = format!("{}/{domain}.ico", Self::cache_dir());
        let url = favicon_url.to_string();
        std::thread::spawn(move || {
            let tmp_path = format!("{dest_path}.part");
            let mut saved = false;
            if let Ok(file) = std::fs::File::create(&tmp_path) {
                let file = Arc::new(Mutex::new(file));
                let writer = file.clone();
                let mut on_data = move |data: &[u8]| {
                    use std::io::Write;
                    writer.lock().map(|mut f| f.write_all(data).is_ok()).unwrap_or(false)
                };
                let result = http_request(&url, "GET", Some(&mut on_data), None, None);
                saved = result.ok && (200..300).contains(&result.status);
            }
            let success = if saved {
                std::fs::rename(&tmp_path, &dest_path).is_ok()
            } else {
                let _ = std::fs::remove_file(&tmp_path);
                false
            };
            post_to_ui(move || {
                let callback = PENDING_CALLBACKS.with(|p| {
                    let mut pending = p.borrow_mut();
                    let index = pending.iter().position(|(i, _)| *i == id)?;
                    Some(pending.remove(index).1)
                });
                if let Some(callback) = callback {
                    callback(success);
                }
            });
        });
    }
}
