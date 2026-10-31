//! Port of `CefAppBrowser`: one HTML5 app bundle in an offscreen browser.
//!
//! The bundle is served at `sp://<app>/` (see [`super::sp_scheme_handler`]),
//! and the shared API bundle at `sp://import/`.

use std::collections::BTreeSet;
use std::rc::Rc;

use bungalow::platform_ui;
use space::sp_app_bundle::SPAppBundle;
use space::sp_app_manager::SPAppManager;
use space::sp_embedded_browser::{
    SPEmbeddedBrowser, SPEmbeddedBrowserCallbacks, SPEmbeddedBrowserOptions, SHARED_API_BUNDLE,
};

use super::browser_window::{BrowserClient, BrowserEvent, BrowserWindow};
use super::sp_scheme_handler::Mount;
use super::{is_bungalow_uri, is_web_url, Host};

/// Defines `window.navigate(path)` for apps that do not bring their own (the
/// host calls it to route the running app: bungalow:app:<id>:a:b -> "/a/b"),
/// `window.bungalow.navigate(uri)`, and hands bungalow: links to the host.
pub const BRIDGE_SCRIPT: &str = r#"
(function () {
    if (window.__bungalowBridge) return;
    window.__bungalowBridge = true;
    var hostNavigate = function (uri) {
        if (typeof uri !== 'string') throw new TypeError('bungalow.navigate(uri) expects a bungalow: URI string');
        if (window.jsBridge && window.jsBridge.navigate) window.jsBridge.navigate(uri);
    };
    try {
        Object.defineProperty(window, 'bungalow', { value: Object.freeze({ navigate: hostNavigate }), writable: false });
    } catch (e) {}
    if (typeof window.navigate !== 'function') {
        window.navigate = function (path) {
            if (location.pathname + location.search + location.hash === path) return;
            history.pushState(null, '', path);
            window.dispatchEvent(new PopStateEvent('popstate', { state: null }));
        };
    }
    document.addEventListener('click', function (e) {
        var a = e.target && e.target.closest ? e.target.closest('a[href]') : null;
        if (!a) return;
        var href = a.getAttribute('href') || '';
        if (/^(bungalow|space|musik):/i.test(href)) {
            e.preventDefault();
            hostNavigate(href);
        }
    }, true);
})();
"#;

/// The C++ `AppClient`: keeps the app inside its bundle.
struct AppClient {
    origin: String,
    callbacks: SPEmbeddedBrowserCallbacks,
}

impl AppClient {
    /// sp://app/a/b?x#y -> "/a/b"; None outside the bundle.
    fn route_of(&self, url: &str) -> Option<String> {
        let rest = url.strip_prefix(&self.origin)?;
        let path = rest.split(['?', '#']).next().unwrap_or_default();
        let path = format!("/{path}");
        Some(if path == "/index.html" { "/".to_string() } else { path })
    }

    fn leave_bundle(&self, url: &str) {
        if is_bungalow_uri(url) {
            if let Some(cb) = &self.callbacks.navigate_requested {
                cb(url);
            }
        } else if is_web_url(url) {
            if let Some(cb) = &self.callbacks.external_url_requested {
                cb(url);
            }
        }
    }
}

impl BrowserClient for AppClient {
    /// The app stays inside its bundle.
    fn allow_navigation(&self, url: &str, _is_redirect: bool) -> bool {
        self.route_of(url).is_some()
    }

    fn on_event(&self, window: &BrowserWindow, event: BrowserEvent) {
        match event {
            BrowserEvent::AddressChanged { url, main_frame: true } => {
                if let (Some(route), Some(cb)) = (self.route_of(&url), &self.callbacks.route_changed) {
                    cb(&route);
                }
            }
            BrowserEvent::NavigationBlocked(url) => self.leave_bundle(&url),
            BrowserEvent::TitleChanged(title) => {
                if let Some(cb) = &self.callbacks.title_changed {
                    cb(&title);
                }
            }
            BrowserEvent::PopupRequested(url) => self.leave_bundle(&url),
            BrowserEvent::NavigateRequested(uri) => {
                if is_bungalow_uri(&uri) {
                    if let Some(cb) = &self.callbacks.navigate_requested {
                        cb(&uri);
                    }
                }
            }
            // Right-clicking an app shows the Develop menu, like the caption bar's.
            BrowserEvent::ContextMenu(_) => {
                let name = window.name().to_string();
                let id = window.id;
                platform_ui::show_popup_menu(
                    &vec![(1, "Show Inspector".to_string()), (2, "Reload Page".to_string())],
                    Box::new(move |choice| {
                        let Some(window) = super::find_window(id) else { return };
                        match choice {
                            1 => window.show_dev_tools(&name),
                            2 => window.reload(),
                            _ => {}
                        }
                    }),
                );
            }
            _ => {}
        }
    }
}

/// A Content-Security-Policy that keeps the app's requests to its bundle,
/// the shared API bundle, data:, blob: and the allowed https/wss hosts (an
/// allowed host also covers its subdomains), like the C++ resource filter.
fn content_security_policy(shared_origin: &str, allowed_hosts: &BTreeSet<String>) -> String {
    let mut sources = vec![
        "'self'".to_string(),
        shared_origin.trim_end_matches('/').to_string(),
        "data:".into(),
        "blob:".into(),
        "'unsafe-inline'".into(),
        "'unsafe-eval'".into(),
    ];
    for host in allowed_hosts {
        for scheme in ["https", "wss"] {
            sources.push(format!("{scheme}://{host}"));
            sources.push(format!("{scheme}://*.{host}"));
        }
    }
    format!("default-src {}", sources.join(" "))
}

pub struct CefAppBrowser {
    host: Rc<Host>,
    app_id: String,
    origin: String,
    window: Rc<BrowserWindow>,
}

impl CefAppBrowser {
    pub fn new(
        host: &Rc<Host>,
        bundle: &SPAppBundle,
        options: SPEmbeddedBrowserOptions,
        callbacks: SPEmbeddedBrowserCallbacks,
    ) -> Self {
        // Hosts are canonicalized to lower case.
        let app_id = bundle.identifier.to_ascii_lowercase();
        let origin = host.scheme_handler.origin(&app_id);
        let shared_origin = host.scheme_handler.origin(SHARED_API_BUNDLE);

        host.scheme_handler.mount(
            &app_id,
            Mount {
                bundle_root: bundle.path.clone(),
                allow_any_origin: app_id == SHARED_API_BUNDLE,
                inject_script: Some(format!("{BRIDGE_SCRIPT}\n{}", options.startup_script)),
                content_security_policy: Some(content_security_policy(&shared_origin, &options.allowed_hosts)),
            },
        );
        if app_id != SHARED_API_BUNDLE && !host.scheme_handler.is_mounted(SHARED_API_BUNDLE) {
            let shared_root = SPAppManager::with(|m| m.find_bundle(SHARED_API_BUNDLE).map(|b| b.path.clone()));
            if let Some(root) = shared_root {
                host.scheme_handler.mount(
                    SHARED_API_BUNDLE,
                    Mount { bundle_root: root, allow_any_origin: true, ..Default::default() },
                );
            }
        }

        let window = host.new_window(&app_id, options.dark_color_scheme);
        window.set_client(Rc::new(AppClient { origin: origin.clone(), callbacks }));
        Self { host: host.clone(), app_id, origin, window }
    }
}

impl Drop for CefAppBrowser {
    fn drop(&mut self) {
        self.window.close();
        // Other browsers of the same app keep the bundle mounted.
        if !self.host.has_window_named(&self.app_id, self.window.id) && self.app_id != SHARED_API_BUNDLE {
            self.host.scheme_handler.unmount(&self.app_id);
        }
    }
}

impl SPEmbeddedBrowser for CefAppBrowser {
    fn set_bounds(&self, x: i32, y: i32, width: i32, height: i32) {
        self.window.set_bounds(x, y, width, height);
    }

    fn set_visible(&self, visible: bool) {
        self.window.set_visible(visible);
    }

    fn load_path(&self, path: &str) {
        let url = format!("{}{}", self.origin, path.strip_prefix('/').unwrap_or(path));
        if !self.window.load(&url) {
            self.window.create(&url);
        }
    }

    fn navigate_route(&self, path: &str) {
        if self.window.is_running() {
            let path = serde_json::Value::String(path.to_string());
            self.execute_script(&format!("window.navigate({path});"));
        } else {
            // Not running yet: the route becomes the initial page.
            self.load_path(path);
        }
    }

    fn execute_script(&self, js: &str) {
        self.window.execute_script(js);
    }

    fn reload(&self) {
        self.window.reload();
    }

    fn show_dev_tools(&self) {
        self.window.show_dev_tools(&self.app_id);
    }

    fn set_dark_color_scheme(&self, dark: bool) {
        self.window.set_dark_color_scheme(dark);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_stay_inside_the_bundle() {
        let client = AppClient { origin: "sp://app/".into(), callbacks: Default::default() };
        assert_eq!(client.route_of("sp://app/a/b?x#y").as_deref(), Some("/a/b"));
        assert_eq!(client.route_of("sp://app/index.html").as_deref(), Some("/"));
        assert_eq!(client.route_of("sp://app/").as_deref(), Some("/"));
        assert_eq!(client.route_of("sp://other/a"), None);
        assert_eq!(client.route_of("sp://application/a"), None);
        assert!(client.allow_navigation("sp://app/x", false));
        assert!(!client.allow_navigation("https://example.com/", false));
        assert!(!client.allow_navigation("bungalow:app:other", false));
    }

    #[test]
    fn policy_allows_hosts_and_subdomains() {
        let hosts = BTreeSet::from(["api.example.com".to_string()]);
        let csp = content_security_policy("sp://import/", &hosts);
        assert!(csp.starts_with("default-src 'self' sp://import data: blob:"));
        assert!(csp.contains("https://*.api.example.com"));
        assert!(csp.contains("wss://api.example.com"));
    }
}
