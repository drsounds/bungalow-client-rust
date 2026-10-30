//! Port of `SPEmbeddedBrowser.h`: backend-neutral interfaces for the native
//! browsers that host HTML5 app bundles and web sites.
//!
//! The C++ backends defined `createEmbeddedAppBrowser()` and
//! `createEmbeddedSiteBrowser()` at link time; here the active UI backend
//! registers an [`SPEmbeddedBrowserBackend`] with [`set_backend`].

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use crate::sp_app_bundle::SPAppBundle;
use crate::sp_element::ElementRef;

/// Identifier of the bundle with the shared Apps JavaScript API
/// (Apps/import). Every app browser also serves it at sp://import/, and
/// its files may be read cross-origin (sp.require() uses XHR).
pub const SHARED_API_BUNDLE: &str = "import";

#[derive(Debug, Clone, Default)]
pub struct SPEmbeddedBrowserOptions {
    pub developer_mode: bool,
    /// Initial color scheme; see `set_dark_color_scheme()`.
    pub dark_color_scheme: bool,
    /// JavaScript run in every app page before its own scripts.
    pub startup_script: String,
    /// https/wss hosts the app may fetch from, besides its own bundle.
    pub allowed_hosts: BTreeSet<String>,
}

pub type StringCallback = Option<Rc<dyn Fn(&str)>>;

#[derive(Clone, Default)]
pub struct SPEmbeddedBrowserCallbacks {
    /// The app changed its route (pushState, hash change, page load).
    /// path is the in-app path, e.g. "/a/b".
    pub route_changed: StringCallback,
    /// The app asked the host to open a bungalow: URI
    /// (window.bungalow.navigate or a bungalow: link).
    pub navigate_requested: StringCallback,
    /// The app tried to open a page outside its bundle.
    pub external_url_requested: StringCallback,
    pub title_changed: StringCallback,
}

/// A native browser that hosts one HTML5 app bundle (a bundle with an
/// index.html) and serves it from sp://<bundle>/, plus the shared API
/// bundle ([`SHARED_API_BUNDLE`]). Each instance has its own isolated storage.
///
/// All methods must be called on the UI thread, and all callbacks are
/// delivered on the UI thread. Callbacks stop once the browser is dropped.
pub trait SPEmbeddedBrowser {
    /// Window-relative bounds in pixels.
    fn set_bounds(&self, x: i32, y: i32, width: i32, height: i32);
    fn set_visible(&self, visible: bool);
    /// Loads an in-app path ("/a/b" -> sp://<bundle>/a/b).
    fn load_path(&self, path: &str);
    /// Routes the running app to an in-app path via window.navigate(path).
    fn navigate_route(&self, path: &str);
    fn execute_script(&self, js: &str);
    fn reload(&self);
    fn show_dev_tools(&self);
    /// Follows the host's light/dark mode.
    fn set_dark_color_scheme(&self, dark: bool);
}

#[derive(Debug, Clone, Default)]
pub struct SPEmbeddedSiteBrowserOptions {
    pub developer_mode: bool,
    pub dark_color_scheme: bool,
    /// JavaScript run in every top-level page before its own scripts.
    pub startup_script: String,
}

#[derive(Clone, Default)]
pub struct SPEmbeddedSiteBrowserCallbacks {
    /// The main frame's URL changed (page load, pushState, hash change).
    pub address_changed: StringCallback,
    /// The page asked to open a bungalow: URI (link or redirect).
    pub navigate_requested: StringCallback,
    /// The page tried to open an http(s) page on a host that has not
    /// been allowed, or in a new window. The navigation was cancelled.
    pub navigation_requested: StringCallback,
    /// The page started a download. It was cancelled in the browser.
    pub download_requested: StringCallback,
    pub title_changed: StringCallback,
}

/// A native browser that shows ordinary web sites (SiteView). All sites
/// share one persistent profile. Top-level navigations stay in the browser
/// only for hosts the host element has allowed; anything else is handed
/// back through `navigation_requested` so the host can ask for consent.
pub trait SPEmbeddedSiteBrowser {
    fn set_bounds(&self, x: i32, y: i32, width: i32, height: i32);
    fn set_visible(&self, visible: bool);
    /// Lets top-level navigations to host (exact match) stay in the browser.
    fn allow_host(&self, host: &str);
    /// Loads url, allowing its host first.
    fn load_url(&self, url: &str);
    fn execute_script(&self, js: &str);
    fn reload(&self);
    fn show_dev_tools(&self);
    fn set_dark_color_scheme(&self, dark: bool);
}

/// Implemented by the active UI backend.
pub trait SPEmbeddedBrowserBackend {
    fn create_app_browser(
        &self,
        bundle: &SPAppBundle,
        host: &ElementRef,
        options: SPEmbeddedBrowserOptions,
        callbacks: SPEmbeddedBrowserCallbacks,
    ) -> Option<Box<dyn SPEmbeddedBrowser>>;

    fn create_site_browser(
        &self,
        host: &ElementRef,
        options: SPEmbeddedSiteBrowserOptions,
        callbacks: SPEmbeddedSiteBrowserCallbacks,
    ) -> Option<Box<dyn SPEmbeddedSiteBrowser>>;
}

thread_local! {
    static BACKEND: RefCell<Option<Rc<dyn SPEmbeddedBrowserBackend>>> = const { RefCell::new(None) };
}

pub fn set_backend(backend: Option<Rc<dyn SPEmbeddedBrowserBackend>>) {
    BACKEND.with(|b| *b.borrow_mut() = backend);
}

/// True when a UI backend with embedded browsers is registered (the C++
/// `BUNGALOW_CEF` build).
pub fn has_backend() -> bool {
    BACKEND.with(|b| b.borrow().is_some())
}

/// `createEmbeddedAppBrowser`: None if no embedded browser is available.
pub fn create_embedded_app_browser(
    bundle: &SPAppBundle,
    host: &ElementRef,
    options: SPEmbeddedBrowserOptions,
    callbacks: SPEmbeddedBrowserCallbacks,
) -> Option<Box<dyn SPEmbeddedBrowser>> {
    let backend = BACKEND.with(|b| b.borrow().clone())?;
    backend.create_app_browser(bundle, host, options, callbacks)
}

/// `createEmbeddedSiteBrowser`: None if no embedded browser is available.
pub fn create_embedded_site_browser(
    host: &ElementRef,
    options: SPEmbeddedSiteBrowserOptions,
    callbacks: SPEmbeddedSiteBrowserCallbacks,
) -> Option<Box<dyn SPEmbeddedSiteBrowser>> {
    let backend = BACKEND.with(|b| b.borrow().clone())?;
    backend.create_site_browser(host, options, callbacks)
}
