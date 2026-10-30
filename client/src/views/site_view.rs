//! Port of the Win32 `SiteView` (windows/): the Space-rendered navigation
//! bar, URI mapping, third-party-domain consent and download detection.
//!
//! With an embedded browser backend (the `wef` feature), pages are shown in
//! an embedded browser below the nav bar; in-page navigations become view
//! stack history entries, and pages on hosts the user has not accepted yet
//! go through the third-party consent first. Without one (or if the browser
//! cannot start) pages open in the system browser.
//!
//! C++ subclasses override `uriToUrl`, `urlToUri`, `reload` and
//! `showInspector`; here those are [`SiteViewClass`] methods, and the
//! embedded [`SiteView`] calls them through a weak reference to the
//! outermost view.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use bungalow::bungalow_app_handle;
use bungalow::platform_ui;

use space::impl_element_base;
use space::libspace::SPMargin;
use space::sp_bookmark_event_args::SPBookmarkEventArgs;
use space::sp_button_element::SPButtonElement;
use space::sp_element::{construct, ElementRef, ElementRefExt, SPElementBase, SPElementClass, SP_COLOR_SCHEME_DARK};
use space::sp_embedded_browser::{
    create_embedded_site_browser, SPEmbeddedSiteBrowser, SPEmbeddedSiteBrowserCallbacks, SPEmbeddedSiteBrowserOptions,
};
use space::sp_event_args::downcast_args;
use space::sp_graphics_context::SPGraphicsContext;
use space::sp_hbox_element::SPHBoxElement;
use space::sp_input_element::SPInputElement;
use space::sp_input_event_args::SPInputEventArgs;
use space::sp_object::SPObject;
use space::sp_view_element::{SPViewClass, SPViewElement};
use space::sp_view_stack_element::SPViewStackElement;
use space::sp_window_element::SPWindowElement;

use crate::platform::open_in_system_browser;

pub const NAV_BAR_HEIGHT: i32 = 36;

/// The virtual methods `SiteView` subclasses override.
pub trait SiteViewClass: SPElementClass {
    /// The embedded `SiteView` base.
    fn site(&self) -> &SiteView;

    fn accepts_site_uri(&self, uri: &str) -> bool {
        uri.starts_with("bungalow:site:")
    }
    fn uri_to_url(&self, uri: &str) -> String {
        self.site().uri_to_url_default(uri)
    }
    fn url_to_uri(&self, url: &str) -> String {
        self.site().url_to_uri_default(url)
    }
    fn show_inspector(&self) {
        self.site().show_inspector_default();
    }
    fn reload(&self) {
        self.site().reload_default();
    }
}

thread_local! {
    static ACTIVE_INSTANCE: RefCell<Option<Weak<dyn SiteViewClass>>> = const { RefCell::new(None) };
}

fn is_bungalow_scheme(uri: &str) -> bool {
    uri.starts_with("bungalow:") || uri.starts_with("space:") || uri.starts_with("musik:")
}

/// Trailing slashes do not make a different page.
pub fn normalize_url(url: &str) -> &str {
    url.strip_suffix('/').unwrap_or(url)
}

pub fn extract_hostname(url: &str) -> String {
    let Some(start) = url.find("://").map(|i| i + 3) else { return url.to_string() };
    let rest = &url[start..];
    let host_port = rest.split(['/', '?', '#']).next().unwrap_or_default();
    match host_port.rfind(':') {
        Some(colon) => host_port[..colon].to_string(),
        None => host_port.to_string(),
    }
}

/// Maps `bungalow:<kind>:<name>[:path]` to `https://<name>[<hosted>]/path`,
/// adding `hosted` to bare names (no dot). Shared by the app, curator and
/// store views.
pub fn hosted_uri_to_url(uri: &str, prefix: &str, hosted: &str) -> String {
    if uri.len() <= prefix.len() || !uri.starts_with(prefix) {
        return String::new();
    }
    let remainder = &uri[prefix.len()..];
    let (domain, rest) = match remainder.find(':') {
        Some(colon) => (&remainder[..colon], &remainder[colon..]),
        None => (remainder, ""),
    };
    let mut domain = domain.to_string();
    if !domain.contains('.') && !domain.is_empty() {
        domain += hosted;
    }
    format!("https://{domain}{}", rest.replace(':', "/"))
}

/// The inverse of [`hosted_uri_to_url`].
pub fn hosted_url_to_uri(url: &str, prefix: &str, hosted: &str) -> String {
    let Some(rest) = url.strip_prefix("https://") else { return String::new() };
    let (domain, path) = match rest.find('/') {
        Some(slash) => (&rest[..slash], &rest[slash..]),
        None => (rest, ""),
    };
    // Collapse the hosted domain back to the bare name.
    let domain = match domain.strip_suffix(hosted) {
        Some(bare) if !bare.is_empty() => bare,
        _ => domain,
    };
    let uri = format!("{prefix}{domain}{}", path.replace('/', ":"));
    uri.trim_end_matches(':').to_string()
}

/// The runtime state of a site view (the C++ private members).
#[derive(Default)]
struct SiteState {
    uri_prefix: String,
    nav_offset: i32,
    current_uri: String,
    pending_url: String,
    checking_url: bool,
    previous_uri: String,
    loaded_url: String,
    page_title: String,
    browser_failed: bool,
    /// The site browser is this view's content.
    show_browser: bool,
    /// A load we started has not committed yet.
    loading_url: bool,
}

#[derive(Default)]
pub struct SiteView {
    pub view: SPViewElement,
    outer: RefCell<Option<Weak<dyn SiteViewClass>>>,
    state: RefCell<SiteState>,
    pub nav_row: RefCell<Option<Rc<SPHBoxElement>>>,
    bookmark_button: RefCell<Option<Rc<SPButtonElement>>>,
    url_input: RefCell<Option<Rc<SPInputElement>>>,
    go_button: RefCell<Option<Rc<SPButtonElement>>>,
    browser: RefCell<Option<Rc<dyn SPEmbeddedSiteBrowser>>>,
}

impl_element_base!(SiteView, view);

impl SiteView {
    /// `SiteView(SPElement* parent)`: the plain bungalow:site: view.
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        let outer: Rc<dyn SiteViewClass> = rc.clone();
        rc.init_site(&outer, "bungalow:site:", true, true, true);
        rc
    }

    /// The protected `SiteView(parent, uriPrefix, showNavBar, showUrlBar,
    /// showBookmarkButton)` constructor body. `outer` is the view this
    /// `SiteView` is embedded in (or itself).
    pub fn init_site(
        &self,
        outer: &Rc<dyn SiteViewClass>,
        uri_prefix: &str,
        show_nav_bar: bool,
        show_url_bar: bool,
        show_bookmark_button: bool,
    ) {
        SPViewElement::init_view(&self.view);
        *self.outer.borrow_mut() = Some(Rc::downgrade(outer));
        {
            let mut state = self.state.borrow_mut();
            state.uri_prefix = uri_prefix.to_string();
            state.nav_offset = if show_nav_bar { NAV_BAR_HEIGHT } else { 0 };
        }
        let el = self.element();
        el.set_clip_view(false);
        if !show_nav_bar {
            return;
        }
        let this = el.this();
        let nav_row = SPHBoxElement::new(Some(&this));
        nav_row.element().set_height(NAV_BAR_HEIGHT);
        nav_row.element().set_padding(SPMargin::new(4, 4, 4, 4));
        nav_row.element().set_gap(6);
        this.append_child(&(nav_row.clone() as ElementRef));
        let row: ElementRef = nav_row.clone();

        if show_bookmark_button {
            let button = SPButtonElement::new(Some(&row));
            button.element().set_inner_text("\u{2606}");
            button.element().set_width(NAV_BAR_HEIGHT - 8);
            button.element().set_height(NAV_BAR_HEIGHT - 8);
            let weak = Rc::downgrade(outer);
            button.element().add_event_listener(
                "click",
                Rc::new(move |_, _| {
                    if let Some(outer) = weak.upgrade() {
                        outer.site().on_bookmark_clicked();
                    }
                    0
                }),
            );
            row.append_child(&(button.clone() as ElementRef));
            *self.bookmark_button.borrow_mut() = Some(button);
        }

        if show_url_bar {
            let input = SPInputElement::new(Some(&row));
            input.element().set_flex(1);
            input.element().set_height(NAV_BAR_HEIGHT - 8);
            let weak = Rc::downgrade(outer);
            input.element().add_event_listener(
                "enter",
                Rc::new(move |_, e| {
                    if let (Some(outer), Some(args)) = (weak.upgrade(), downcast_args::<SPInputEventArgs>(e)) {
                        outer.site().navigate_to_input_url(&args.get_text());
                    }
                    0
                }),
            );
            row.append_child(&(input.clone() as ElementRef));
            *self.url_input.borrow_mut() = Some(input);

            let go = SPButtonElement::new(Some(&row));
            go.element().set_inner_text("Go");
            go.element().set_width(48);
            go.element().set_height(NAV_BAR_HEIGHT - 8);
            let weak = Rc::downgrade(outer);
            go.element().add_event_listener(
                "click",
                Rc::new(move |_, _| {
                    if let Some(outer) = weak.upgrade() {
                        let site = outer.site();
                        let value = site.url_input.borrow().as_ref().map(|i| i.get_value());
                        if let Some(value) = value {
                            site.navigate_to_input_url(&value);
                        }
                    }
                    0
                }),
            );
            row.append_child(&(go.clone() as ElementRef));
            *self.go_button.borrow_mut() = Some(go);
        }
        *self.nav_row.borrow_mut() = Some(nav_row);
    }

    pub fn get_active_instance() -> Option<Rc<dyn SiteViewClass>> {
        ACTIVE_INSTANCE.with(|a| a.borrow().as_ref().and_then(Weak::upgrade))
    }

    fn outer(&self) -> Option<Rc<dyn SiteViewClass>> {
        self.outer.borrow().as_ref().and_then(Weak::upgrade)
    }

    fn uri_to_url(&self, uri: &str) -> String {
        match self.outer() {
            Some(outer) => outer.uri_to_url(uri),
            None => self.uri_to_url_default(uri),
        }
    }

    fn url_to_uri(&self, url: &str) -> String {
        match self.outer() {
            Some(outer) => outer.url_to_uri(url),
            None => self.url_to_uri_default(url),
        }
    }

    fn window(&self) -> Option<ElementRef> {
        self.element().get_window_element()
    }

    fn window_navigate(&self, uri: &str) {
        if let Some(window) = self.window() {
            if let Some(window) = window.downcast::<SPWindowElement>() {
                window.navigate(uri);
            }
        }
    }

    fn is_visible(&self) -> bool {
        self.element().is_visible()
    }

    // ── URI mapping ───────────────────────────────────────────────────────

    pub fn uri_to_url_default(&self, uri: &str) -> String {
        let prefix_len = self.state.borrow().uri_prefix.len();
        if uri.len() <= prefix_len {
            return String::new();
        }
        format!("https://{}", uri[prefix_len..].replace(':', "/"))
    }

    pub fn url_to_uri_default(&self, url: &str) -> String {
        let Some(rest) = url.strip_prefix("https://") else { return String::new() };
        let uri = format!("{}{}", self.state.borrow().uri_prefix, rest.replace('/', ":"));
        uri.trim_end_matches(':').to_string()
    }

    // ── Accessors for subclasses ──────────────────────────────────────────

    pub fn get_current_uri(&self) -> String {
        self.state.borrow().current_uri.clone()
    }

    pub fn set_current_uri(&self, uri: &str) {
        self.state.borrow_mut().current_uri = uri.to_string();
    }

    pub fn get_nav_offset(&self) -> i32 {
        self.state.borrow().nav_offset
    }

    pub fn get_page_title(&self) -> String {
        self.state.borrow().page_title.clone()
    }

    pub fn set_nav_bar_visible(&self, visible: bool) {
        let Some(row) = self.nav_row.borrow().clone() else { return };
        self.state.borrow_mut().nav_offset = if visible { NAV_BAR_HEIGHT } else { 0 };
        row.set_visible(visible);
    }

    // ── Consent ───────────────────────────────────────────────────────────

    pub fn is_domain_allowed(&self, url: &str) -> bool {
        let hostname = extract_hostname(url);
        if hostname.is_empty() || hostname == "bungalow.se" || hostname.ends_with(".bungalow.se") {
            return true;
        }
        bungalow_app_handle::current().and_then(|a| a.get_database()).is_some_and(|db| db.is_domain_accepted(&hostname))
    }

    pub fn show_third_party_warning(&self, hostname: &str) -> bool {
        let message = format!(
            "The site \"{hostname}\" is a third party outside of the Bungalow developer's responsibility.\n\n\
             The developers of Bungalow, including but not limited to its contributors, take no \
             responsibility for the content, data, or financial transactions on that site and \
             cannot be held liable for any losses caused by using this third-party website.\n\n\
             If you agree, click Yes to proceed. If you do not agree, click No and you \
             will be returned to the previous view."
        );
        if !platform_ui::ask_yes_no("Third-Party Website", &message) {
            return false;
        }
        if let Some(db) = bungalow_app_handle::current().and_then(|a| a.get_database()) {
            db.add_accepted_domain(hostname);
        }
        true
    }

    /// Opens `url` in the system browser after the third-party consent check.
    pub fn open_external_url(&self, url: &str) {
        if !self.is_domain_allowed(url) && !self.show_third_party_warning(&extract_hostname(url)) {
            return;
        }
        open_in_system_browser(url);
    }

    // ── Page scripting ────────────────────────────────────────────────────

    /// Runs `js` in the current page; a no-op without an embedded browser.
    pub fn run_script(&self, js: &str) {
        if let Some(browser) = self.browser.borrow().clone() {
            browser.execute_script(js);
        }
    }

    /// Exposes the app theme to pages as CSS variables and body colors.
    pub fn theme_script(&self) -> String {
        let el = self.element();
        let bg = el.get_background_color().to_html_color();
        let fg = el.get_foreground_color().to_html_color();
        let scheme = if el.get_color_scheme() == SP_COLOR_SCHEME_DARK { "dark" } else { "light" };
        format!(
            "(function(){{var e=document.getElementById('sp-theme');\
             if(!e){{e=document.createElement('style');e.id='sp-theme';\
             (document.head||document.documentElement).appendChild(e);}}\
             e.textContent=':root{{--sp-bg-color:{bg};--sp-fg-color:{fg};--sp-color-scheme:{scheme}}}\
             body{{background-color:{bg};color:{fg}}}';}})();"
        )
    }

    /// Runs the theme as soon as the document element exists, before page scripts.
    pub fn startup_theme_script(&self) -> String {
        format!(
            "(function(apply){{if(document.documentElement){{apply();return;}}\
             new MutationObserver(function(_,o){{if(document.documentElement){{o.disconnect();apply();}}}})\
             .observe(document,{{childList:true}});}})(function(){{{}}});",
            self.theme_script()
        )
    }

    // ── Nav bar ───────────────────────────────────────────────────────────

    fn update_url_bar(&self, url: &str) {
        if let Some(input) = self.url_input.borrow().clone() {
            input.set_value(url);
        }
        let uri = self.url_to_uri(url);
        if !uri.is_empty() {
            self.update_bookmark_button(&uri);
        }
    }

    pub fn update_bookmark_button(&self, uri: &str) {
        let Some(button) = self.bookmark_button.borrow().clone() else { return };
        let Some(db) = bungalow_app_handle::current().and_then(|a| a.get_database()) else { return };
        let bookmarked = db.is_bookmarked(uri);
        button.element().set_inner_text(if bookmarked { "\u{2605}" } else { "\u{2606}" });
        button.element().invalidate();
    }

    fn on_bookmark_clicked(&self) {
        let (uri, title) = {
            let state = self.state.borrow();
            let title = if state.page_title.is_empty() { state.current_uri.clone() } else { state.page_title.clone() };
            (state.current_uri.clone(), title)
        };
        if uri.is_empty() {
            return;
        }
        self.notify_bookmark_toggled(&uri, &title);
    }

    pub fn notify_bookmark_toggled(&self, uri: &str, title: &str) {
        let Some(window) = self.window() else { return };
        let args = SPBookmarkEventArgs::new(uri, title);
        window.notify("bookmarktoggled", Some(&SPObject::Element(self.element().this())), &args);
    }

    fn navigate_to_input_url(&self, text: &str) {
        if text.is_empty() {
            return;
        }
        let mut resolved = text.to_string();
        if !resolved.contains("://") && !is_bungalow_scheme(&resolved) {
            resolved = format!("https://{resolved}");
        }
        let bungalow_uri = if resolved.starts_with("https://") {
            self.url_to_uri(&resolved)
        } else if is_bungalow_scheme(&resolved) {
            resolved
        } else {
            String::new()
        };
        if !bungalow_uri.is_empty() {
            self.window_navigate(&bungalow_uri);
        }
    }

    // ── Navigation ────────────────────────────────────────────────────────

    pub fn navigate_site(&self, uri: &str) {
        {
            let mut state = self.state.borrow_mut();
            state.previous_uri = std::mem::take(&mut state.current_uri);
            state.current_uri = uri.to_string();
        }
        self.update_bookmark_button(uri);

        let url = self.uri_to_url(uri);
        if url.is_empty() {
            return;
        }
        self.update_url_bar(&url);

        // Already showing it, e.g. the page navigated itself and reported it.
        if self.browser.borrow().is_some() && normalize_url(&url) == normalize_url(&self.state.borrow().loaded_url) {
            self.state.borrow_mut().show_browser = true;
            self.element().invalidate();
            return;
        }

        if let Some(dm) = bungalow_app_handle::current().and_then(|a| a.get_download_manager()) {
            {
                let mut state = self.state.borrow_mut();
                state.pending_url = url.clone();
                state.checking_url = true;
            }
            let weak = self.outer.borrow().clone();
            dm.check_url(
                &url,
                Box::new(move |checked_url, is_download, filename, content_type| {
                    let Some(outer) = weak.as_ref().and_then(Weak::upgrade) else { return };
                    let site = outer.site();
                    {
                        let mut state = site.state.borrow_mut();
                        if !state.checking_url || checked_url != state.pending_url {
                            return;
                        }
                        state.checking_url = false;
                        state.pending_url.clear();
                    }
                    if is_download {
                        if let Some(dm) = bungalow_app_handle::current().and_then(|a| a.get_download_manager()) {
                            dm.start_download(checked_url, filename, content_type);
                        }
                        site.window_navigate("bungalow:downloads");
                    } else {
                        site.load_url(checked_url);
                    }
                }),
            );
            return;
        }
        self.load_url(&url);
    }

    fn load_url(&self, url: &str) {
        if !self.is_domain_allowed(url) && !self.show_third_party_warning(&extract_hostname(url)) {
            let previous = {
                let mut state = self.state.borrow_mut();
                state.current_uri = state.previous_uri.clone();
                state.previous_uri.clone()
            };
            if !previous.is_empty() {
                self.update_url_bar(&self.uri_to_url(&previous));
                self.update_bookmark_button(&previous);
            }
            if let Some(parent) = self.element().get_parent_element() {
                if let Some(stack) = parent.downcast::<SPViewStackElement>() {
                    if stack.can_go_back() {
                        stack.go_back();
                    }
                }
            }
            return;
        }

        if normalize_url(&self.state.borrow().loaded_url) == normalize_url(url) {
            if self.browser.borrow().is_some() {
                self.state.borrow_mut().show_browser = true;
                self.element().invalidate();
            }
            return;
        }
        self.show_url(url);
    }

    /// Shows `url` without the consent check or a new history entry.
    fn show_url(&self, url: &str) {
        self.state.borrow_mut().loaded_url = url.to_string();
        if let Some(browser) = self.ensure_browser() {
            {
                let mut state = self.state.borrow_mut();
                state.loading_url = true;
                state.show_browser = true;
                state.page_title.clear();
            }
            browser.load_url(url);
            self.element().invalidate(); // the browser is positioned on the next paint
            return;
        }
        open_in_system_browser(url);
    }

    /// History entries were consented to when first visited, so they are
    /// shown without asking again or pushing a new entry. The system browser
    /// fallback only updates the URL bar.
    pub fn navigate_back_site(&self, uri: &str) {
        let url = self.uri_to_url(uri);
        if url.is_empty() {
            return;
        }
        self.state.borrow_mut().current_uri = uri.to_string();
        self.update_url_bar(&url);
        if self.ensure_browser().is_none() {
            return;
        }
        if normalize_url(&url) != normalize_url(&self.state.borrow().loaded_url) {
            self.show_url(&url);
        } else {
            self.state.borrow_mut().show_browser = true;
            self.element().invalidate();
        }
    }

    pub fn set_visible_site(&self, value: bool) {
        self.element().set_visible_default(value);
        if value {
            if let Some(outer) = self.outer.borrow().clone() {
                ACTIVE_INSTANCE.with(|a| *a.borrow_mut() = Some(outer));
            }
        } else {
            ACTIVE_INSTANCE.with(|a| {
                let mut active = a.borrow_mut();
                let is_this = active
                    .as_ref()
                    .and_then(Weak::upgrade)
                    .is_some_and(|v| std::ptr::addr_eq(v.site() as *const SiteView, self as *const SiteView));
                if is_this {
                    *active = None;
                }
            });
        }

        let Some(browser) = self.browser.borrow().clone() else { return };
        if value {
            // Pick up theme changes made while hidden (e.g. in the settings view).
            browser.set_dark_color_scheme(self.element().get_color_scheme() == SP_COLOR_SCHEME_DARK);
            browser.execute_script(&self.theme_script());
            self.element().invalidate();
        } else {
            browser.set_visible(false); // hidden views are not painted
        }
    }

    /// Hides the site browser until the next site navigation, for
    /// subclasses that show other content in its place.
    pub fn hide_site_browser(&self) {
        self.state.borrow_mut().show_browser = false;
        if let Some(browser) = self.browser.borrow().clone() {
            browser.set_visible(false);
        }
    }

    // ── Embedded browser ──────────────────────────────────────────────────

    fn ensure_browser(&self) -> Option<Rc<dyn SPEmbeddedSiteBrowser>> {
        if let Some(browser) = self.browser.borrow().clone() {
            return Some(browser);
        }
        if self.state.borrow().browser_failed {
            return None;
        }
        let options = SPEmbeddedSiteBrowserOptions {
            developer_mode: bungalow_app_handle::current().is_some_and(|a| a.is_developer_mode()),
            dark_color_scheme: self.element().get_color_scheme() == SP_COLOR_SCHEME_DARK,
            startup_script: self.startup_theme_script(),
        };

        let outer = self.outer.borrow().clone().unwrap_or_else(|| Weak::<SiteView>::new());
        let site_callback = |f: fn(&SiteView, &str)| -> Option<Rc<dyn Fn(&str)>> {
            let outer = outer.clone();
            Some(Rc::new(move |text: &str| {
                if let Some(outer) = outer.upgrade() {
                    f(outer.site(), text);
                }
            }))
        };
        let callbacks = SPEmbeddedSiteBrowserCallbacks {
            address_changed: site_callback(SiteView::on_address_changed),
            navigate_requested: site_callback(|site, uri| {
                if site.is_visible() {
                    site.window_navigate(uri);
                }
            }),
            navigation_requested: site_callback(SiteView::on_navigation_requested),
            download_requested: site_callback(SiteView::on_download_requested),
            title_changed: site_callback(|site, title| site.state.borrow_mut().page_title = title.to_string()),
        };

        let browser: Option<Rc<dyn SPEmbeddedSiteBrowser>> =
            create_embedded_site_browser(&self.element().this(), options, callbacks).map(Rc::from);
        self.state.borrow_mut().browser_failed = browser.is_none();
        *self.browser.borrow_mut() = browser.clone();
        browser
    }

    /// The page committed a new address: a load we started, a link the page
    /// followed on an allowed host, or a pushState/hash change.
    fn on_address_changed(&self, url: &str) {
        let started_here = {
            let mut state = self.state.borrow_mut();
            let started_here = state.loading_url;
            state.loading_url = false;
            state.loaded_url = url.to_string();
            started_here
        };
        self.update_url_bar(url);

        let uri = self.url_to_uri(url);
        if uri.is_empty() || uri == self.state.borrow().current_uri {
            return;
        }
        if started_here || !self.is_visible() {
            // A redirect of our own load: no extra history entry.
            self.state.borrow_mut().current_uri = uri;
            return;
        }
        // navigate() finds loaded_url already showing and only records it.
        self.window_navigate(&uri);
    }

    /// The page tried to open a page on a host that has not been allowed yet
    /// (or in a new window): go through navigate() for consent and download
    /// detection.
    fn on_navigation_requested(&self, url: &str) {
        if !self.is_visible() {
            return;
        }
        let uri = self.url_to_uri(url);
        if !uri.is_empty() {
            self.window_navigate(&uri);
            return;
        }
        // Plain http has no bungalow: URI; show it in place without history.
        if self.is_domain_allowed(url) || self.show_third_party_warning(&extract_hostname(url)) {
            self.show_url(url);
        }
    }

    fn on_download_requested(&self, url: &str) {
        let Some(dm) = bungalow_app_handle::current().and_then(|a| a.get_download_manager()) else { return };
        let weak = self.outer.borrow().clone();
        dm.check_url(
            url,
            Box::new(move |checked_url, _is_download, filename, content_type| {
                if let Some(dm) = bungalow_app_handle::current().and_then(|a| a.get_download_manager()) {
                    dm.start_download(checked_url, filename, content_type);
                }
                if let Some(outer) = weak.as_ref().and_then(Weak::upgrade) {
                    outer.site().window_navigate("bungalow:downloads");
                }
            }),
        );
    }

    /// Absolute bounds are only known once painted, so the browser is
    /// positioned and shown from here; layout changes and window resizes
    /// both end in a repaint.
    pub fn paint_site(&self, x: i32, y: i32, gc: &dyn SPGraphicsContext) {
        self.element().paint_default(x, y, gc);
        let Some(browser) = self.browser.borrow().clone() else { return };
        if !self.state.borrow().show_browser || !self.is_visible() {
            return;
        }
        let bounds = self.element().get_absolute_bounds();
        let nav_offset = self.get_nav_offset();
        browser.set_bounds(bounds.x, bounds.y + nav_offset, bounds.width, bounds.height - nav_offset);
        browser.set_visible(true);
    }

    pub fn reload_default(&self) {
        if let Some(browser) = self.browser.borrow().clone() {
            if self.state.borrow().show_browser {
                browser.reload();
            }
            return;
        }
        let loaded = self.state.borrow().loaded_url.clone();
        if !loaded.is_empty() {
            open_in_system_browser(&loaded);
        }
    }

    pub fn show_inspector_default(&self) {
        if let Some(browser) = self.browser.borrow().clone() {
            if self.state.borrow().show_browser {
                browser.show_dev_tools();
            }
        }
    }
}

impl SiteViewClass for SiteView {
    fn site(&self) -> &SiteView {
        self
    }
}

impl SPElementClass for SiteView {
    fn get_type(&self) -> String {
        "view".into()
    }
    view_element_overrides!();
    fn set_visible(&self, value: bool) {
        self.set_visible_site(value);
    }
    fn paint(&self, x: i32, y: i32, gc: &dyn SPGraphicsContext) {
        self.paint_site(x, y, gc);
    }
}

impl SPViewClass for SiteView {
    fn view(&self) -> &SPViewElement {
        &self.view
    }
    fn accepts_uri(&self, uri: &str) -> bool {
        self.accepts_site_uri(uri)
    }
    fn navigate(&self, uri: &str) {
        self.navigate_site(uri);
    }
    fn navigate_back(&self, uri: &str) {
        self.navigate_back_site(uri);
    }
    fn navigate_forward(&self, uri: &str) {
        self.navigate_back_site(uri);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosted_uris_round_trip() {
        let url = hosted_uri_to_url("bungalow:curator:mine:page", "bungalow:curator:", ".curator.bungalow.se");
        assert_eq!(url, "https://mine.curator.bungalow.se/page");
        assert_eq!(hosted_url_to_uri(&url, "bungalow:curator:", ".curator.bungalow.se"), "bungalow:curator:mine:page");
        assert_eq!(
            hosted_uri_to_url("bungalow:store:example.com", "bungalow:store:", ".store.bungalow.se"),
            "https://example.com"
        );
        assert_eq!(
            hosted_url_to_uri("https://example.com/a/", "bungalow:store:", ".store.bungalow.se"),
            "bungalow:store:example.com:a"
        );
    }

    #[test]
    fn hostnames_drop_port_and_path() {
        assert_eq!(extract_hostname("https://example.com:8080/a?b#c"), "example.com");
        assert_eq!(normalize_url("https://a.se/"), "https://a.se");
    }
}
