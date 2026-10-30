//! Port of `CefSiteBrowser`: ordinary web sites in an offscreen browser.
//!
//! All site browsers share Wef's single profile (the C++ "sites" profile).
//! Top-level pages stay in the browser for allowed hosts; bungalow: links,
//! new windows and pages on other hosts are handed to the host element.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use bungalow::platform_ui;
use space::sp_embedded_browser::{SPEmbeddedSiteBrowser, SPEmbeddedSiteBrowserCallbacks, SPEmbeddedSiteBrowserOptions};

use super::app_browser::BRIDGE_SCRIPT;
use super::browser_window::{BrowserClient, BrowserEvent, BrowserWindow, ContextMenuInfo};
use super::{host_of, is_bungalow_uri, is_web_url, Host};

/// Hosts whose top-level pages may load without asking the host element.
#[derive(Default)]
struct AllowedHosts {
    hosts: RefCell<BTreeSet<String>>,
}

impl AllowedHosts {
    fn add(&self, host: &str) {
        if !host.is_empty() {
            self.hosts.borrow_mut().insert(host.to_string());
        }
    }

    fn contains(&self, host: &str) -> bool {
        self.hosts.borrow().contains(host)
    }
}

/// The C++ `SiteClient`.
struct SiteClient {
    developer_mode: bool,
    startup_script: String,
    allowed_hosts: Rc<AllowedHosts>,
    /// Set by load_url() until that load ends: address changes meanwhile are
    /// its redirects, which CEF never filtered.
    host_load_pending: Rc<Cell<bool>>,
    callbacks: SPEmbeddedSiteBrowserCallbacks,
}

impl SiteClient {
    fn hand_over(&self, url: &str) {
        if is_bungalow_uri(url) {
            if let Some(cb) = &self.callbacks.navigate_requested {
                cb(url);
            }
        } else if is_web_url(url) {
            if let Some(cb) = &self.callbacks.navigation_requested {
                cb(url);
            }
        }
    }

    /// Keeps CEF's default menu (minus view-source); developer mode adds the
    /// Develop menu command, like the caption bar's.
    fn show_context_menu(&self, window: &BrowserWindow, info: &ContextMenuInfo) {
        use wef::ContextMenuEditStateFlags as E;
        const BACK: i32 = 1;
        const FORWARD: i32 = 2;
        const RELOAD: i32 = 3;
        const CUT: i32 = 4;
        const COPY: i32 = 5;
        const PASTE: i32 = 6;
        const SELECT_ALL: i32 = 7;
        const INSPECT: i32 = 8;

        let mut items = Vec::new();
        if info.is_editable {
            if info.edit_flags.contains(E::CAN_CUT) {
                items.push((CUT, "Cut".to_string()));
            }
            if info.edit_flags.contains(E::CAN_COPY) {
                items.push((COPY, "Copy".to_string()));
            }
            if info.edit_flags.contains(E::CAN_PASTE) {
                items.push((PASTE, "Paste".to_string()));
            }
            if info.edit_flags.contains(E::CAN_SELECT_ALL) {
                items.push((SELECT_ALL, "Select All".to_string()));
            }
        } else if info.has_selection {
            items.push((COPY, "Copy".to_string()));
        } else {
            if window.with_browser(|b| b.can_back()).unwrap_or(false) {
                items.push((BACK, "Back".to_string()));
            }
            if window.with_browser(|b| b.can_forward()).unwrap_or(false) {
                items.push((FORWARD, "Forward".to_string()));
            }
            items.push((RELOAD, "Reload".to_string()));
        }
        if self.developer_mode {
            items.push((INSPECT, "Show Inspector".to_string()));
        }
        let id = window.id;
        platform_ui::show_popup_menu(
            &items,
            Box::new(move |choice| {
                let Some(window) = super::find_window(id) else { return };
                let frame = || window.with_browser(|b| b.focused_frame().or_else(|| b.main_frame())).flatten();
                match choice {
                    BACK => _ = window.with_browser(|b| b.back()),
                    FORWARD => _ = window.with_browser(|b| b.forward()),
                    RELOAD => window.reload(),
                    CUT => _ = frame().map(|f| f.cut()),
                    COPY => _ = frame().map(|f| f.copy()),
                    PASTE => _ = frame().map(|f| f.paste()),
                    SELECT_ALL => _ = frame().map(|f| f.select_all()),
                    INSPECT => window.show_dev_tools("Site"),
                    _ => {}
                }
            }),
        );
    }
}

impl BrowserClient for SiteClient {
    fn on_event(&self, window: &BrowserWindow, event: BrowserEvent) {
        match event {
            BrowserEvent::AddressChanged { url, main_frame: true } => {
                if !is_web_url(&url) {
                    return;
                }
                let host = host_of(&url);
                if self.host_load_pending.get() {
                    self.allowed_hosts.add(&host);
                }
                if self.allowed_hosts.contains(&host) {
                    if let Some(cb) = &self.callbacks.address_changed {
                        cb(&url);
                    }
                } else {
                    // CEF cancelled these in OnBeforeBrowse; Wef can only
                    // notice them, so the page goes back while the host
                    // element asks for consent.
                    window.go_back();
                    self.hand_over(&url);
                }
            }
            BrowserEvent::LoadStart { main_frame: true, .. } => {
                if !self.startup_script.is_empty() {
                    window.execute_script(&self.startup_script);
                }
                window.execute_script(BRIDGE_SCRIPT);
            }
            BrowserEvent::LoadEnd { main_frame: true, .. } => self.host_load_pending.set(false),
            BrowserEvent::TitleChanged(title) => {
                if let Some(cb) = &self.callbacks.title_changed {
                    cb(&title);
                }
            }
            // There are no extra windows: target=_blank links and window.open
            // open in the view instead.
            BrowserEvent::PopupRequested(url) => self.hand_over(&url),
            BrowserEvent::NavigateRequested(uri) => {
                if is_bungalow_uri(&uri) {
                    self.hand_over(&uri);
                }
            }
            BrowserEvent::ContextMenu(info) => self.show_context_menu(window, &info),
            _ => {}
        }
    }
}

pub struct WefSiteBrowser {
    allowed_hosts: Rc<AllowedHosts>,
    host_load_pending: Rc<Cell<bool>>,
    window: Rc<BrowserWindow>,
}

impl WefSiteBrowser {
    pub fn new(
        host: &Rc<Host>,
        options: SPEmbeddedSiteBrowserOptions,
        callbacks: SPEmbeddedSiteBrowserCallbacks,
    ) -> Self {
        let allowed_hosts = Rc::new(AllowedHosts::default());
        let host_load_pending = Rc::new(Cell::new(false));
        let window = host.new_window("site", options.dark_color_scheme);
        window.set_client(Rc::new(SiteClient {
            developer_mode: options.developer_mode,
            startup_script: options.startup_script,
            allowed_hosts: allowed_hosts.clone(),
            host_load_pending: host_load_pending.clone(),
            callbacks,
        }));
        Self { allowed_hosts, host_load_pending, window }
    }
}

impl Drop for WefSiteBrowser {
    fn drop(&mut self) {
        self.window.close();
    }
}

impl SPEmbeddedSiteBrowser for WefSiteBrowser {
    fn set_bounds(&self, x: i32, y: i32, width: i32, height: i32) {
        self.window.set_bounds(x, y, width, height);
    }

    fn set_visible(&self, visible: bool) {
        self.window.set_visible(visible);
    }

    fn allow_host(&self, host: &str) {
        self.allowed_hosts.add(host);
    }

    fn load_url(&self, url: &str) {
        self.allowed_hosts.add(&host_of(url));
        self.host_load_pending.set(true);
        if !self.window.load(url) {
            self.window.create(url);
        }
    }

    fn execute_script(&self, js: &str) {
        self.window.execute_script(js);
    }

    fn reload(&self) {
        self.window.reload();
    }

    fn show_dev_tools(&self) {
        self.window.show_dev_tools("Site");
    }

    fn set_dark_color_scheme(&self, dark: bool) {
        self.window.set_dark_color_scheme(dark);
    }
}
