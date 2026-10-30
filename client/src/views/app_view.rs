//! Port of `AppView` (wxwidgets/app/): like SiteView but handles
//! bungalow:app: URIs.
//!
//! Dev mode: full URL bar always shown. Non-dev mode: a nav bar with an
//! "Add app" button, shown only while the app is not in favourites.
//!
//! Installed bundles with "AppType": "Native" run as Lua MVC apps rendered
//! with native elements (see SPNativeApp), one per app, kept alive while
//! switching so controllers keep their state.
//!
//! With an embedded browser backend (the C++ `BUNGALOW_CEF` build; here the
//! `wef` feature), other installed bundles that contain an index.html run
//! locally in an embedded browser (one per app, kept alive while switching),
//! served from the app's own origin. bungalow:app:<app_id>:a:b maps to the
//! in-app route /a/b. Unknown app ids show an error view.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;

use bungalow::bungalow_app_handle;

use space::impl_element_base;
use space::libspace::SPMargin;
use space::sp_app_bundle::SPAppBundle;
use space::sp_app_manager::SPAppManager;
use space::sp_button_element::SPButtonElement;
use space::sp_element::{construct, ElementRef, ElementRefExt, SPElementBase, SPElementClass, SP_COLOR_SCHEME_DARK};
use space::sp_embedded_browser::{
    create_embedded_app_browser, has_backend, SPEmbeddedBrowser, SPEmbeddedBrowserCallbacks, SPEmbeddedBrowserOptions,
    SHARED_API_BUNDLE,
};
use space::sp_graphics_context::SPGraphicsContext;
use space::sp_hbox_element::SPHBoxElement;
use space::sp_native_app::SPNativeApp;
use space::sp_native_app_element::SPNativeAppElement;
use space::sp_text_element::SPTextElement;
use space::sp_vbox_element::SPVBoxElement;
use space::sp_view_element::{SPViewClass, SPViewElement};
use space::sp_window_element::SPWindowElement;

use super::site_view::{hosted_uri_to_url, hosted_url_to_uri, SiteView, SiteViewClass, NAV_BAR_HEIGHT};

const APP_PREFIX: &str = "bungalow:app:";
const APPS_DOMAIN: &str = ".apps.bungalow.se";

fn get_dev_mode() -> bool {
    bungalow_app_handle::current().is_some_and(|a| a.is_developer_mode())
}

/// bungalow:app:<id>:a:b -> ("<id>", "/a/b")
fn split_app_uri(uri: &str) -> Option<(String, String)> {
    let rest = uri.strip_prefix(APP_PREFIX)?;
    let (app_id, route) = match rest.find(':') {
        Some(colon) => (&rest[..colon], &rest[colon + 1..]),
        None => (rest, ""),
    };
    let mut path = format!("/{}", route.replace(':', "/"));
    while path.len() > 1 && path.ends_with('/') {
        path.pop();
    }
    (!app_id.is_empty()).then(|| (app_id.to_string(), path))
}

/// ("<id>", "/a/b") -> bungalow:app:<id>:a:b
fn app_uri(app_id: &str, path: &str) -> String {
    let uri = format!("{APP_PREFIX}{app_id}{}", path.replace('/', ":"));
    uri.trim_end_matches(':').to_string()
}

fn has_index_html(bundle: &SPAppBundle) -> bool {
    Path::new(&bundle.path).join("index.html").is_file()
}

fn find_bundle(identifier: &str) -> Option<SPAppBundle> {
    SPAppManager::with(|m| m.find_bundle(identifier).cloned())
}

/// The Apps API bootstrap (getSpotifyApi, sp.require) from the shared API
/// bundle, run in every app page. Read per app browser, so edits apply to
/// apps opened afterwards. Empty if the bundle is not installed.
fn shared_api_script() -> String {
    find_bundle(SHARED_API_BUNDLE)
        .and_then(|shared| std::fs::read_to_string(Path::new(&shared.path).join("scripts").join("sp.js")).ok())
        .unwrap_or_default()
}

struct LocalApp {
    browser: Rc<dyn SPEmbeddedBrowser>,
    /// In-app path last loaded or reported, e.g. "/a/b".
    route: String,
    title: String,
}

#[derive(Default)]
struct AppParts {
    add_app_button: Option<Rc<SPButtonElement>>,
    dev_tools_button: Option<Rc<SPButtonElement>>,
    error_view: Option<Rc<SPVBoxElement>>,
    error_text: Option<Rc<SPTextElement>>,
}

#[derive(Default)]
pub struct AppView {
    pub site: SiteView,
    dev_mode: std::cell::Cell<bool>,
    parts: RefCell<AppParts>,
    /// By app id; children of this view.
    native_apps: RefCell<BTreeMap<String, Rc<SPNativeAppElement>>>,
    active_native_app: RefCell<Option<Rc<SPNativeAppElement>>>,
    /// By app id.
    local_apps: RefCell<BTreeMap<String, LocalApp>>,
    active_app: RefCell<Option<String>>,
    /// This view, for the methods that create children and need the `Rc`.
    me: RefCell<std::rc::Weak<AppView>>,
}

impl_element_base!(AppView, site);

impl AppView {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        *rc.me.borrow_mut() = Rc::downgrade(&rc);
        let dev_mode = get_dev_mode();
        rc.dev_mode.set(dev_mode);
        let outer: Rc<dyn SiteViewClass> = rc.clone();
        // No bookmark button: "Add app" is added instead.
        rc.site.init_site(&outer, APP_PREFIX, true, dev_mode, false);
        let this = rc.element().this();

        let error_view = SPVBoxElement::new(Some(&this));
        error_view.element().set_flex(1);
        error_view.element().set_padding(SPMargin::new(24, 24, 24, 24));
        error_view.element().set_gap(8);
        let error_ref: ElementRef = error_view.clone();
        let heading = SPTextElement::new(Some(&error_ref));
        heading.element().set_inner_text("App unavailable");
        heading.element().set_string_attribute("size", "20");
        heading.element().set_height(28);
        error_ref.append_child(&(heading as ElementRef));
        let error_text = SPTextElement::new(Some(&error_ref));
        error_text.element().set_height(20);
        error_ref.append_child(&(error_text.clone() as ElementRef));
        error_view.set_visible(false);
        this.append_child(&error_ref);
        {
            let mut parts = rc.parts.borrow_mut();
            parts.error_view = Some(error_view);
            parts.error_text = Some(error_text);
        }

        let Some(nav_row) = rc.site.nav_row.borrow().clone() else { return rc };
        let row: ElementRef = nav_row;

        if !dev_mode {
            // Non-dev mode: a flexible spacer pushes "Add app" to the right.
            let spacer = SPHBoxElement::new(Some(&row));
            spacer.element().set_flex(1);
            row.append_child(&(spacer as ElementRef));
        }

        let add_app = SPButtonElement::new(Some(&row));
        add_app.element().set_inner_text("Add app");
        add_app.element().set_height(NAV_BAR_HEIGHT - 8);
        let weak = Rc::downgrade(&rc);
        add_app.element().add_event_listener(
            "click",
            Rc::new(move |_, _| {
                if let Some(this) = weak.upgrade() {
                    this.on_add_app_clicked();
                }
                0
            }),
        );
        row.append_child(&(add_app.clone() as ElementRef));
        rc.parts.borrow_mut().add_app_button = Some(add_app);

        if dev_mode && has_backend() {
            let dev_tools = SPButtonElement::new(Some(&row));
            dev_tools.element().set_inner_text("DevTools");
            dev_tools.element().set_height(NAV_BAR_HEIGHT - 8);
            let weak = Rc::downgrade(&rc);
            dev_tools.element().add_event_listener(
                "click",
                Rc::new(move |_, _| {
                    if let Some(this) = weak.upgrade() {
                        if let Some(app) = this.active_browser() {
                            app.show_dev_tools();
                        }
                    }
                    0
                }),
            );
            row.append_child(&(dev_tools.clone() as ElementRef));
            rc.parts.borrow_mut().dev_tools_button = Some(dev_tools);
        }
        rc
    }

    /// `AppView(SPElement* parent, bool showNavBar)`.
    pub fn new_with_nav_bar(parent: Option<&ElementRef>, show_nav_bar: bool) -> Rc<Self> {
        let rc = Self::new(parent);
        if !show_nav_bar {
            rc.site.set_nav_bar_visible(false);
        }
        rc
    }

    fn window_navigate(&self, uri: &str) {
        if let Some(window) = self.element().get_window_element() {
            if let Some(window) = window.downcast::<SPWindowElement>() {
                window.navigate(uri);
            }
        }
    }

    fn active_browser(&self) -> Option<Rc<dyn SPEmbeddedBrowser>> {
        let id = self.active_app.borrow().clone()?;
        self.local_apps.borrow().get(&id).map(|a| a.browser.clone())
    }

    fn update_app_nav_bar(&self, uri: &str) {
        let Some(db) = bungalow_app_handle::current().and_then(|a| a.get_database()) else { return };
        let favorited = db.is_bookmarked(uri);
        self.site.set_nav_bar_visible(!favorited);
    }

    fn on_add_app_clicked(&self) {
        let uri = self.site.get_current_uri();
        if uri.is_empty() {
            return;
        }
        let mut title = uri.clone(); // fallback
        let page_title = self.site.get_page_title();
        if !page_title.is_empty() {
            title = page_title;
        }
        if let Some(id) = self.active_app.borrow().as_ref() {
            if let Some(app) = self.local_apps.borrow().get(id) {
                if !app.title.is_empty() {
                    title = app.title.clone();
                }
            }
        }
        self.site.notify_bookmark_toggled(&uri, &title);
        self.site.set_nav_bar_visible(false);
    }

    // ── Installed app bundles ─────────────────────────────────────────────

    /// Shows `uri` in an installed app bundle (or the error view). Returns
    /// false, with no app content shown, when `uri` belongs to the remote
    /// app path.
    fn navigate_bundle(self: &Rc<Self>, uri: &str) -> bool {
        let mut shown = self.navigate_native_app(uri, false);
        if !shown && has_backend() {
            shown = self.navigate_local_app(uri);
        }
        if !shown {
            self.show_native_app(None);
            return false;
        }
        if !self.dev_mode.get() {
            self.update_app_nav_bar(uri);
        }
        self.element().invalidate(); // a local app browser is positioned on the next paint
        true
    }

    /// Shows `uri` in a native app bundle, restarting the app first if
    /// asked. Returns false if `uri` is not in a native app.
    fn navigate_native_app(self: &Rc<Self>, uri: &str, restart: bool) -> bool {
        let Some((app_id, _path)) = split_app_uri(uri) else { return false };
        let Some(bundle) = find_bundle(&app_id) else { return false };
        if !SPNativeApp::is_native(&bundle) {
            return false;
        }
        self.site.set_current_uri(uri);
        let app = self.native_app_for(&bundle, restart);
        self.show_native_app(Some(app.clone()));
        app.element().load_src(uri); // errors replace it with the error view
        true
    }

    /// An app whose Lua state failed to start reports it through the error
    /// handler on its first request, which shows the error view.
    fn native_app_for(self: &Rc<Self>, bundle: &SPAppBundle, restart: bool) -> Rc<SPNativeAppElement> {
        if let Some(existing) = self.native_apps.borrow().get(&bundle.identifier) {
            if restart {
                existing.restart();
            }
            return existing.clone();
        }
        let this = self.element().this();
        let app = SPNativeAppElement::new(Some(&this), bundle);
        app.element().set_flex(1);
        app.set_visible(false);
        let weak = Rc::downgrade(self);
        app.set_error_handler(Some(Rc::new(move |message: &str| {
            if let Some(this) = weak.upgrade() {
                this.show_error(message);
            }
        })));
        this.append_child(&(app.clone() as ElementRef));
        self.native_apps.borrow_mut().insert(bundle.identifier.clone(), app.clone());
        app
    }

    /// `None` hides all app content (and the site browser).
    fn show_native_app(&self, app: Option<Rc<SPNativeAppElement>>) {
        self.site.hide_site_browser();
        if let Some(browser) = self.active_browser() {
            browser.set_visible(false);
        }
        *self.active_app.borrow_mut() = None;
        let previous = self.active_native_app.borrow().clone();
        if let Some(previous) = previous {
            if app.as_ref().is_none_or(|a| !Rc::ptr_eq(a, &previous)) {
                previous.set_visible(false);
            }
        }
        if let Some(app) = &app {
            app.set_visible(true);
        }
        *self.active_native_app.borrow_mut() = app;
        if let Some(error_view) = self.parts.borrow().error_view.clone() {
            error_view.set_visible(false);
        }
        self.element().invalidate();
    }

    fn show_error(&self, message: &str) {
        self.show_native_app(None);
        let parts = self.parts.borrow();
        if let Some(text) = &parts.error_text {
            text.element().set_inner_text(message);
        }
        if let Some(view) = &parts.error_view {
            view.set_visible(true);
        }
        drop(parts);
        self.element().invalidate();
    }

    // ── Local HTML5 app bundles ───────────────────────────────────────────

    /// Shows `uri` in a local app browser or the error view. Returns false
    /// when `uri` belongs to the remote (non-bundle) app path.
    fn navigate_local_app(self: &Rc<Self>, uri: &str) -> bool {
        let Some((app_id, path)) = split_app_uri(uri) else { return false };
        let bundle = find_bundle(&app_id);
        if bundle.as_ref().is_some_and(|b| !has_index_html(b)) {
            // Not an HTML5 bundle: keep the remote https://<id>.apps.bungalow.se path.
            self.show_local_app(None);
            return false;
        }

        self.site.set_current_uri(uri);
        let Some(bundle) = bundle else {
            self.show_error(&format!("No app with the id \"{app_id}\" is installed."));
            return true;
        };
        if !self.ensure_local_app(&bundle) {
            self.show_error("Apps cannot be shown because the embedded browser failed to start.");
            return true;
        }

        self.show_local_app(Some(bundle.identifier.clone()));
        let mut apps = self.local_apps.borrow_mut();
        let Some(app) = apps.get_mut(&bundle.identifier) else { return true };
        if app.route.is_empty() {
            app.browser.load_path(&path);
        } else if app.route != path {
            app.browser.navigate_route(&path);
        }
        app.route = path;
        true
    }

    fn ensure_local_app(self: &Rc<Self>, bundle: &SPAppBundle) -> bool {
        let app_id = bundle.identifier.clone();
        if self.local_apps.borrow().contains_key(&app_id) {
            return true;
        }

        let mut options = SPEmbeddedBrowserOptions {
            developer_mode: self.dev_mode.get(),
            dark_color_scheme: self.element().get_color_scheme() == SP_COLOR_SCHEME_DARK,
            startup_script: self.site.startup_theme_script(),
            ..Default::default()
        };
        // The shared bundle itself has no app pages to bootstrap.
        if app_id != SHARED_API_BUNDLE {
            options.startup_script += "\n";
            options.startup_script += &shared_api_script();
        }
        // Besides bungalow.se, apps may fetch from the https hosts their
        // manifest lists under AllowedHosts, once the user has accepted each.
        options.allowed_hosts.insert("bungalow.se".into());
        if let Some(hosts) = bundle.manifest.get("AllowedHosts").and_then(|h| h.as_array()) {
            for host in hosts.iter().filter_map(|h| h.as_str()) {
                if self.site.is_domain_allowed(&format!("https://{host}")) || self.site.show_third_party_warning(host) {
                    options.allowed_hosts.insert(host.to_string());
                }
            }
        }

        let weak = Rc::downgrade(self);
        let id = app_id.clone();
        let route_changed: Rc<dyn Fn(&str)> = Rc::new(move |path: &str| {
            if let Some(this) = weak.upgrade() {
                this.on_route_changed(&id, path);
            }
        });
        let weak = Rc::downgrade(self);
        let navigate_requested: Rc<dyn Fn(&str)> = Rc::new(move |target: &str| {
            if let Some(this) = weak.upgrade() {
                this.window_navigate(target);
            }
        });
        let weak = Rc::downgrade(self);
        let external_url_requested: Rc<dyn Fn(&str)> = Rc::new(move |url: &str| {
            if let Some(this) = weak.upgrade() {
                this.site.open_external_url(url);
            }
        });
        let weak = Rc::downgrade(self);
        let id = app_id.clone();
        let title_changed: Rc<dyn Fn(&str)> = Rc::new(move |title: &str| {
            if let Some(this) = weak.upgrade() {
                if let Some(app) = this.local_apps.borrow_mut().get_mut(&id) {
                    app.title = title.to_string();
                }
            }
        });
        let callbacks = SPEmbeddedBrowserCallbacks {
            route_changed: Some(route_changed),
            navigate_requested: Some(navigate_requested),
            external_url_requested: Some(external_url_requested),
            title_changed: Some(title_changed),
        };

        let Some(browser) = create_embedded_app_browser(bundle, &self.element().this(), options, callbacks) else {
            return false;
        };
        self.local_apps
            .borrow_mut()
            .insert(app_id, LocalApp { browser: Rc::from(browser), route: String::new(), title: String::new() });
        true
    }

    /// The app changed its own route: record it in the view stack history.
    fn on_route_changed(&self, app_id: &str, path: &str) {
        {
            let mut apps = self.local_apps.borrow_mut();
            let Some(app) = apps.get_mut(app_id) else { return };
            if app.route == path {
                return;
            }
            app.route = path.to_string();
        }
        let is_active = self.active_app.borrow().as_deref() == Some(app_id);
        if is_active && self.element().is_visible() {
            self.window_navigate(&app_uri(app_id, path));
        }
    }

    /// `None` when showing the error view or before falling back to the
    /// remote path, which shows the site browser again.
    fn show_local_app(&self, app_id: Option<String>) {
        self.site.hide_site_browser();
        if let Some(native) = self.active_native_app.borrow_mut().take() {
            native.set_visible(false);
        }
        let previous = self.active_app.borrow().clone();
        if previous.is_some() && previous != app_id {
            if let Some(browser) = self.active_browser() {
                browser.set_visible(false);
            }
        }
        *self.active_app.borrow_mut() = app_id;
        if let Some(error_view) = self.parts.borrow().error_view.clone() {
            error_view.set_visible(false);
        }
        self.element().invalidate();
    }
}

impl SiteViewClass for AppView {
    fn site(&self) -> &SiteView {
        &self.site
    }

    fn accepts_site_uri(&self, uri: &str) -> bool {
        uri.starts_with(APP_PREFIX)
    }

    /// bungalow:app:myapp -> https://myapp.apps.bungalow.se,
    /// bungalow:app:example.com:page -> https://example.com/page
    fn uri_to_url(&self, uri: &str) -> String {
        hosted_uri_to_url(uri, APP_PREFIX, APPS_DOMAIN)
    }

    fn url_to_uri(&self, url: &str) -> String {
        hosted_url_to_uri(url, APP_PREFIX, APPS_DOMAIN)
    }

    /// Native apps restart with a fresh Lua state and reloaded routes.
    /// Keyed on the current URI rather than the shown app, so a native app
    /// that is showing an error can be reloaded once the error is fixed.
    fn reload(&self) {
        if let Some(rc) = self.me.borrow().upgrade() {
            if rc.navigate_native_app(&self.site.get_current_uri(), true) {
                return;
            }
        }
        if let Some(browser) = self.active_browser() {
            browser.reload();
            return;
        }
        self.site.reload_default();
    }

    fn show_inspector(&self) {
        match self.active_browser() {
            Some(browser) => browser.show_dev_tools(),
            None => self.site.show_inspector_default(),
        }
    }
}

impl SPElementClass for AppView {
    fn get_type(&self) -> String {
        "view".into()
    }
    view_element_overrides!();

    fn set_visible(&self, value: bool) {
        self.site.set_visible_site(value);
        let Some(browser) = self.active_browser() else { return };
        if value {
            // Pick up theme changes made while hidden (e.g. in the settings view).
            browser.set_dark_color_scheme(self.element().get_color_scheme() == SP_COLOR_SCHEME_DARK);
            browser.execute_script(&self.site.theme_script());
            self.element().invalidate();
        } else {
            browser.set_visible(false); // hidden views are not painted
        }
    }

    /// Absolute bounds are only known once painted, so the app browser is
    /// positioned and shown from here.
    fn paint(&self, x: i32, y: i32, gc: &dyn SPGraphicsContext) {
        self.site.paint_site(x, y, gc);
        let Some(browser) = self.active_browser() else { return };
        if !self.element().is_visible() {
            return;
        }
        let bounds = self.element().get_absolute_bounds();
        let nav_offset = self.site.get_nav_offset();
        browser.set_bounds(bounds.x, bounds.y + nav_offset, bounds.width, bounds.height - nav_offset);
        browser.set_visible(true);
    }
}

impl SPViewClass for AppView {
    fn view(&self) -> &SPViewElement {
        &self.site.view
    }

    fn accepts_uri(&self, uri: &str) -> bool {
        self.accepts_site_uri(uri)
    }

    fn navigate(&self, uri: &str) {
        let Some(rc) = self.me.borrow().upgrade() else { return };
        if rc.navigate_bundle(uri) {
            return;
        }
        self.site.navigate_site(uri);
        if !self.dev_mode.get() {
            self.update_app_nav_bar(uri);
        }
    }

    fn navigate_back(&self, uri: &str) {
        let Some(rc) = self.me.borrow().upgrade() else { return };
        if !rc.navigate_bundle(uri) {
            self.site.navigate_back_site(uri);
        }
    }

    fn navigate_forward(&self, uri: &str) {
        self.navigate_back(uri);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_uris_map_to_routes() {
        assert_eq!(split_app_uri("bungalow:app:finder"), Some(("finder".into(), "/".into())));
        assert_eq!(split_app_uri("bungalow:app:finder:a:b:"), Some(("finder".into(), "/a/b".into())));
        assert_eq!(split_app_uri("bungalow:app:"), None);
        assert_eq!(app_uri("finder", "/a/b"), "bungalow:app:finder:a:b");
        assert_eq!(app_uri("finder", "/"), "bungalow:app:finder");
    }
}
