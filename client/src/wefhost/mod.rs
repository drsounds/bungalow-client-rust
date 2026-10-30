//! Port of `windows/src/cefhost`: embedded Chromium browsers for app bundles
//! and web sites, through Wef (offscreen CEF) instead of CEF's native
//! child windows.
//!
//! | C++                     | Rust                                   |
//! |-------------------------|----------------------------------------|
//! | `CefRuntime`            | [`wef_runtime`]                        |
//! | `CefBrowserWindow`      | `browser_window::BrowserWindow`        |
//! | `CefAppBrowser`         | `app_browser::WefAppBrowser`           |
//! | `CefSiteBrowser`        | `site_browser::WefSiteBrowser`         |
//! | `SpSchemeHandler`       | [`sp_scheme_server`] (loopback HTTP)   |
//!
//! The browsers draw into textures that [`paint`] puts at their host
//! elements' bounds, and the main window routes input over those bounds
//! here ([`pointer_moved`], [`pointer_button`], [`key`], ...).

pub mod sp_scheme_server;

#[cfg(feature = "wef")]
mod app_browser;
#[cfg(feature = "wef")]
mod browser_window;
#[cfg(feature = "wef")]
mod site_browser;
#[cfg(feature = "wef")]
mod wef_runtime;

#[cfg(feature = "wef")]
pub use host::*;

pub fn is_bungalow_uri(uri: &str) -> bool {
    ["bungalow:", "space:", "musik:"].iter().any(|prefix| uri.starts_with(prefix))
}

pub fn is_web_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// "https://user@Example.com:8080/a" -> "example.com".
pub fn host_of(url: &str) -> String {
    let Some((_, rest)) = url.split_once("://") else { return String::new() };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let authority = authority.rsplit_once('@').map_or(authority, |(_, a)| a);
    let host = if authority.starts_with('[') {
        authority.split_once(']').map_or(authority, |(h, _)| h).trim_start_matches('[')
    } else {
        authority.split(':').next().unwrap_or_default()
    };
    host.to_ascii_lowercase()
}

#[cfg(feature = "wef")]
mod host {
    use std::cell::{Cell, RefCell};
    use std::rc::{Rc, Weak};
    use std::time::Duration;

    use space::sp_app_bundle::SPAppBundle;
    use space::sp_element::ElementRef;
    use space::sp_embedded_browser::{
        self, SPEmbeddedBrowser, SPEmbeddedBrowserBackend, SPEmbeddedBrowserCallbacks, SPEmbeddedBrowserOptions,
        SPEmbeddedSiteBrowser, SPEmbeddedSiteBrowserCallbacks, SPEmbeddedSiteBrowserOptions,
    };
    use wef::{KeyCode, KeyModifier, LogicalUnit, MouseButton, Point};

    use super::app_browser::WefAppBrowser;
    use super::browser_window::BrowserWindow;
    use super::site_browser::WefSiteBrowser;
    use super::sp_scheme_server::SpSchemeServer;
    use super::wef_runtime;

    pub use super::wef_runtime::{execute_subprocess, shutdown};

    /// Starts CEF. On failure app bundles and sites fall back to the
    /// remote and system-browser paths; the rest of the UI still works.
    pub fn initialize() {
        wef_runtime::initialize();
    }

    const DOUBLE_CLICK_TIME: f64 = 0.5;
    const DOUBLE_CLICK_DISTANCE: f32 = 4.0;

    struct LastClick {
        window: u64,
        button: i32,
        time: f64,
        pos: egui::Pos2,
        count: usize,
    }

    /// Every live browser window and the input state shared between them.
    pub struct Host {
        ctx: egui::Context,
        pub(super) server: SpSchemeServer,
        windows: RefCell<Vec<Weak<BrowserWindow>>>,
        next_id: Cell<u64>,
        focused: Cell<Option<u64>>,
        /// The window that got the last mouse-down gets the mouse-up too.
        captured: Cell<Option<u64>>,
        hovered: Cell<Option<u64>>,
        last_click: RefCell<Option<LastClick>>,
    }

    impl Host {
        pub(super) fn new_window(&self, name: &str, dark_color_scheme: bool) -> Rc<BrowserWindow> {
            let id = self.next_id.get() + 1;
            self.next_id.set(id);
            let window = Rc::new(BrowserWindow::new(id, name, &self.ctx, dark_color_scheme));
            self.windows.borrow_mut().push(Rc::downgrade(&window));
            window
        }

        /// Whether another live window shows the bundle `name`.
        pub(super) fn has_window_named(&self, name: &str, except: u64) -> bool {
            self.live_windows().iter().any(|w| w.id != except && w.name() == name && w.is_running())
        }

        fn live_windows(&self) -> Vec<Rc<BrowserWindow>> {
            let mut windows = self.windows.borrow_mut();
            windows.retain(|w| w.strong_count() > 0);
            windows.iter().filter_map(Weak::upgrade).collect()
        }

        fn window(&self, id: Option<u64>) -> Option<Rc<BrowserWindow>> {
            let id = id?;
            self.live_windows().into_iter().find(|w| w.id == id)
        }

        /// The topmost visible window under pos (the last one created wins).
        fn window_at(&self, pos: egui::Pos2) -> Option<(Rc<BrowserWindow>, Point<LogicalUnit<i32>>)> {
            self.live_windows().into_iter().rev().find_map(|w| w.to_view(pos).map(|p| (w, p)))
        }

        fn focus(&self, window: Option<&Rc<BrowserWindow>>) {
            let id = window.map(|w| w.id);
            if self.focused.get() == id {
                return;
            }
            if let Some(previous) = self.window(self.focused.get()) {
                previous.with_browser(|b| b.set_focus(false));
            }
            if let Some(window) = window {
                window.with_browser(|b| b.set_focus(true));
            }
            self.focused.set(id);
        }

        fn focused_window(&self) -> Option<Rc<BrowserWindow>> {
            self.window(self.focused.get()).filter(|w| w.is_visible())
        }

        fn click_count(&self, window: u64, button: i32, pos: egui::Pos2) -> usize {
            let time = self.ctx.input(|i| i.time);
            let mut last = self.last_click.borrow_mut();
            let count = match last.as_ref() {
                Some(l)
                    if l.window == window
                        && l.button == button
                        && time - l.time < DOUBLE_CLICK_TIME
                        && l.pos.distance(pos) < DOUBLE_CLICK_DISTANCE =>
                {
                    l.count % 3 + 1
                }
                _ => 1,
            };
            *last = Some(LastClick { window, button, time, pos, count });
            count
        }
    }

    thread_local! {
        static HOST: RefCell<Option<Rc<Host>>> = const { RefCell::new(None) };
    }

    fn host() -> Option<Rc<Host>> {
        HOST.with(|h| h.borrow().clone())
    }

    pub(super) fn find_window(id: u64) -> Option<Rc<BrowserWindow>> {
        host()?.window(Some(id))
    }

    /// `createEmbeddedAppBrowser` / `createEmbeddedSiteBrowser` for Wef.
    struct WefBackend {
        host: Rc<Host>,
    }

    impl SPEmbeddedBrowserBackend for WefBackend {
        fn create_app_browser(
            &self,
            bundle: &SPAppBundle,
            _host: &ElementRef,
            options: SPEmbeddedBrowserOptions,
            callbacks: SPEmbeddedBrowserCallbacks,
        ) -> Option<Box<dyn SPEmbeddedBrowser>> {
            if !wef_runtime::is_initialized() {
                return None;
            }
            Some(Box::new(WefAppBrowser::new(&self.host, bundle, options, callbacks)))
        }

        fn create_site_browser(
            &self,
            _host: &ElementRef,
            options: SPEmbeddedSiteBrowserOptions,
            callbacks: SPEmbeddedSiteBrowserCallbacks,
        ) -> Option<Box<dyn SPEmbeddedSiteBrowser>> {
            if !wef_runtime::is_initialized() {
                return None;
            }
            Some(Box::new(WefSiteBrowser::new(&self.host, options, callbacks)))
        }
    }

    /// Makes the embedded browsers available to the views, if CEF started.
    pub fn register_backend(ctx: &egui::Context) {
        if !wef_runtime::is_initialized() {
            return;
        }
        let server = match SpSchemeServer::start() {
            Ok(server) => server,
            Err(e) => {
                log::error!("Cannot serve app bundles: {e}");
                return;
            }
        };
        let host = Rc::new(Host {
            ctx: ctx.clone(),
            server,
            windows: RefCell::default(),
            next_id: Cell::new(0),
            focused: Cell::new(None),
            captured: Cell::new(None),
            hovered: Cell::new(None),
            last_click: RefCell::new(None),
        });
        HOST.with(|h| *h.borrow_mut() = Some(host.clone()));
        sp_embedded_browser::set_backend(Some(Rc::new(WefBackend { host })));
    }

    /// Runs CEF work and delivers browser events to the views. Call once
    /// per frame.
    pub fn pump() {
        if !wef_runtime::is_initialized() {
            return;
        }
        wef_runtime::do_message_loop_work();
        let Some(host) = host() else { return };
        let windows = host.live_windows();
        for window in &windows {
            window.dispatch_events();
        }
        // CEF schedules its work on a message pump the frame loop has to
        // drive, so keep frames coming while browsers are alive.
        let delay = if windows.iter().any(|w| w.is_visible()) {
            Duration::from_millis(16)
        } else if !windows.is_empty() {
            Duration::from_millis(50)
        } else {
            Duration::from_millis(250)
        };
        host.ctx.request_repaint_after(delay);
    }

    /// Paints every visible browser at its host element's bounds.
    pub fn paint(ui: &mut egui::Ui) {
        let Some(host) = host() else { return };
        let painter = ui.painter();
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        let pointer = ui.ctx().input(|i| i.pointer.latest_pos());
        for window in host.live_windows() {
            if !window.is_visible() {
                continue;
            }
            let bounds = window.bounds();
            let surface = window.surface();
            match &surface.view {
                Some(texture) => painter.image(texture.id(), bounds, uv, egui::Color32::WHITE),
                None => painter.rect_filled(bounds, 0.0, egui::Color32::WHITE),
            };
            if let (true, Some(popup), Some(rect)) = (surface.popup_visible, &surface.popup, surface.popup_rect) {
                let rect = rect.translate(bounds.min.to_vec2());
                ui.ctx()
                    .layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new(("wef-popup", window.id))))
                    .image(popup.id(), rect, uv, egui::Color32::WHITE);
            }
            if pointer.is_some_and(|p| bounds.contains(p)) {
                ui.ctx().set_cursor_icon(surface.cursor);
            }
        }
    }

    fn key_modifier(modifiers: egui::Modifiers) -> KeyModifier {
        let mut result = KeyModifier::empty();
        result.set(KeyModifier::SHIFT, modifiers.shift);
        result.set(KeyModifier::CONTROL, modifiers.ctrl || modifiers.mac_cmd);
        result.set(KeyModifier::ALT, modifiers.alt);
        result
    }

    fn current_modifiers(host: &Host) -> KeyModifier {
        key_modifier(host.ctx.input(|i| i.modifiers))
    }

    /// Returns true if a browser took the pointer move.
    pub fn pointer_moved(pos: egui::Pos2) -> bool {
        let Some(host) = host() else { return false };
        let modifiers = current_modifiers(&host);
        if let Some(window) = host.window(host.captured.get()) {
            window.with_browser(|b| b.send_mouse_move_event(window.to_view_unchecked(pos), modifiers));
            return true;
        }
        let target = host.window_at(pos);
        let target_id = target.as_ref().map(|(w, _)| w.id);
        if host.hovered.get() != target_id {
            // Wef has no mouse-leave event: a move outside the view clears
            // hover state in the page.
            if let Some(previous) = host.window(host.hovered.get()) {
                previous
                    .with_browser(|b| b.send_mouse_move_event(Point::new(LogicalUnit(-1), LogicalUnit(-1)), modifiers));
            }
            host.hovered.set(target_id);
        }
        match target {
            Some((window, point)) => {
                window.with_browser(|b| b.send_mouse_move_event(point, modifiers));
                true
            }
            None => false,
        }
    }

    /// button: 0 = left, 1 = right, like the Space mouse events. Returns
    /// true if a browser took the click.
    pub fn pointer_button(pos: egui::Pos2, button: i32, pressed: bool, modifiers: egui::Modifiers) -> bool {
        let Some(host) = host() else { return false };
        let mouse_button = match button {
            0 => MouseButton::Left,
            1 => MouseButton::Right,
            _ => MouseButton::Middle,
        };
        let modifiers = key_modifier(modifiers);
        if pressed {
            let Some((window, point)) = host.window_at(pos) else {
                host.focus(None);
                return false;
            };
            host.focus(Some(&window));
            host.captured.set(Some(window.id));
            let count = host.click_count(window.id, button, pos);
            window.with_browser(|b| {
                b.send_mouse_move_event(point, modifiers);
                b.send_mouse_click_event(mouse_button, false, count, modifiers);
            });
            true
        } else {
            let window = host.window(host.captured.take()).or_else(|| host.window_at(pos).map(|(w, _)| w));
            let Some(window) = window else { return false };
            let count = host.last_click.borrow().as_ref().map_or(1, |l| l.count);
            window.with_browser(|b| {
                b.send_mouse_move_event(window.to_view_unchecked(pos), modifiers);
                b.send_mouse_click_event(mouse_button, true, count, modifiers);
            });
            true
        }
    }

    /// delta in points, positive = up/left like egui. Returns true if a
    /// browser took the wheel.
    pub fn mouse_wheel(pos: egui::Pos2, delta: egui::Vec2) -> bool {
        let Some(host) = host() else { return false };
        let Some((window, point)) = host.window_at(pos) else { return false };
        let modifiers = current_modifiers(&host);
        window.with_browser(|b| {
            b.send_mouse_move_event(point, modifiers);
            b.send_mouse_wheel_event(Point::new(
                LogicalUnit(delta.x.round() as i32),
                LogicalUnit(delta.y.round() as i32),
            ));
        });
        true
    }

    fn key_code(key: egui::Key) -> Option<KeyCode> {
        use egui::Key;
        Some(match key {
            Key::Backspace => KeyCode::Backspace,
            Key::Delete => KeyCode::Delete,
            Key::Tab => KeyCode::Tab,
            Key::Enter => KeyCode::Enter,
            Key::PageUp => KeyCode::PageUp,
            Key::PageDown => KeyCode::PageDown,
            Key::End => KeyCode::End,
            Key::Home => KeyCode::Home,
            Key::ArrowLeft => KeyCode::ArrowLeft,
            Key::ArrowUp => KeyCode::ArrowUp,
            Key::ArrowRight => KeyCode::ArrowRight,
            Key::ArrowDown => KeyCode::ArrowDown,
            _ => return None,
        })
    }

    /// Returns true if the focused browser took the key. Wef only forwards
    /// editing and navigation keys; text arrives through [`text`], and the
    /// editing shortcuts are mapped to frame commands.
    pub fn key(key: egui::Key, pressed: bool, modifiers: egui::Modifiers) -> bool {
        let Some(host) = host() else { return false };
        let Some(window) = host.focused_window() else { return false };
        if pressed && modifiers.command {
            let frame = window.with_browser(|b| b.focused_frame().or_else(|| b.main_frame())).flatten();
            match (key, modifiers.shift) {
                (egui::Key::A, _) => _ = frame.map(|f| f.select_all()),
                (egui::Key::Z, false) => _ = frame.map(|f| f.undo()),
                (egui::Key::Z, true) | (egui::Key::Y, _) => _ = frame.map(|f| f.redo()),
                (egui::Key::R, _) => window.reload(),
                _ => {}
            }
            return true;
        }
        if let Some(code) = key_code(key) {
            window.with_browser(|b| b.send_key_event(pressed, code, key_modifier(modifiers)));
        }
        true
    }

    /// Typed text for the focused browser.
    pub fn text(text: &str) {
        let Some(host) = host() else { return };
        let Some(window) = host.focused_window() else { return };
        window.with_browser(|b| {
            for unit in text.encode_utf16() {
                b.send_char_event(unit);
            }
        });
    }

    pub enum EditCommand {
        Cut,
        Copy,
        Paste,
    }

    /// Clipboard shortcuts (egui turns Ctrl+X/C/V into these). CEF reads and
    /// writes the system clipboard itself. Returns true if a browser has focus.
    pub fn edit_command(command: EditCommand) -> bool {
        let Some(host) = host() else { return false };
        let Some(window) = host.focused_window() else { return false };
        let Some(frame) = window.with_browser(|b| b.focused_frame().or_else(|| b.main_frame())).flatten() else {
            return true;
        };
        match command {
            EditCommand::Cut => frame.cut(),
            EditCommand::Copy => frame.copy(),
            EditCommand::Paste => frame.paste(),
        }
        true
    }

    /// Closes every browser before CEF shuts down (the C++
    /// `closeAllBrowsers`): views may outlive the window, but their
    /// browsers must not outlive CEF.
    pub fn close_all_browsers() {
        sp_embedded_browser::set_backend(None);
        let Some(host) = HOST.with(|h| h.borrow_mut().take()) else { return };
        for window in host.live_windows() {
            window.close();
        }
        // Let CEF finish closing them.
        for _ in 0..20 {
            wef_runtime::do_message_loop_work();
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_classified() {
        assert!(is_bungalow_uri("bungalow:app:testapp"));
        assert!(is_bungalow_uri("musik:album:1"));
        assert!(!is_bungalow_uri("https://bungalow.se"));
        assert!(is_web_url("http://a"));
        assert!(!is_web_url("file:///etc"));
    }

    #[test]
    fn hosts_are_extracted() {
        assert_eq!(host_of("https://user:pw@Example.com:8080/a?b#c"), "example.com");
        assert_eq!(host_of("http://[::1]:80/"), "::1");
        assert_eq!(host_of("https://bungalow.se"), "bungalow.se");
        assert_eq!(host_of("not a url"), "");
    }
}
