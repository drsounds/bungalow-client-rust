//! Port of `windows/src/cefhost`: embedded Chromium browsers for app bundles
//! and web sites, through the `cef` crate with offscreen rendering instead
//! of CEF's native child windows.
//!
//! | C++                     | Rust                                   |
//! |-------------------------|----------------------------------------|
//! | `CefRuntime`            | [`cef_runtime`]                        |
//! | `CefBrowserWindow`      | `browser_window::BrowserWindow`        |
//! | `CefAppBrowser`         | `app_browser::CefAppBrowser`           |
//! | `CefSiteBrowser`        | `site_browser::CefSiteBrowser`         |
//! | `SpSchemeHandler`       | [`sp_scheme_handler`]                  |
//!
//! The browsers draw into textures that [`paint`] puts at their host
//! elements' bounds, and the main window routes input over those bounds
//! here ([`pointer_moved`], [`pointer_button`], [`key`], ...).

pub mod sp_scheme_handler;

#[cfg(feature = "cef")]
mod app_browser;
#[cfg(feature = "cef")]
mod browser_window;
#[cfg(feature = "cef")]
mod cef_runtime;
#[cfg(feature = "cef")]
mod site_browser;

#[cfg(feature = "cef")]
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

#[cfg(feature = "cef")]
mod host {
    use std::cell::{Cell, RefCell};
    use std::rc::{Rc, Weak};
    use std::time::Duration;

    use cef::MouseButtonType;
    use space::sp_app_bundle::SPAppBundle;
    use space::sp_element::ElementRef;
    use space::sp_embedded_browser::{
        self, SPEmbeddedBrowser, SPEmbeddedBrowserBackend, SPEmbeddedBrowserCallbacks, SPEmbeddedBrowserOptions,
        SPEmbeddedSiteBrowser, SPEmbeddedSiteBrowserCallbacks, SPEmbeddedSiteBrowserOptions,
    };

    use super::app_browser::CefAppBrowser;
    use super::browser_window::{self as bw, BrowserWindow};
    use super::cef_runtime;
    use super::site_browser::CefSiteBrowser;
    use super::sp_scheme_handler::SpSchemeHandler;

    pub use super::browser_window::EditCommand;
    pub use super::cef_runtime::{execute_subprocess, shutdown};

    /// Starts CEF. On failure app bundles and sites fall back to the
    /// remote and system-browser paths; the rest of the UI still works.
    pub fn initialize() {
        cef_runtime::initialize();
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
        pub(super) scheme_handler: SpSchemeHandler,
        windows: RefCell<Vec<Weak<BrowserWindow>>>,
        next_id: Cell<u64>,
        focused: Cell<Option<u64>>,
        /// The window that got the last mouse-down gets the mouse-up too.
        captured: Cell<Option<u64>>,
        /// Event flag of the button held over the captured window.
        held_button: Cell<u32>,
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
        fn window_at(&self, pos: egui::Pos2) -> Option<(Rc<BrowserWindow>, (i32, i32))> {
            self.live_windows().into_iter().rev().find_map(|w| w.to_view(pos).map(|p| (w, p)))
        }

        fn focus(&self, window: Option<&Rc<BrowserWindow>>) {
            let id = window.map(|w| w.id);
            if self.focused.get() == id {
                return;
            }
            if let Some(previous) = self.window(self.focused.get()) {
                previous.set_focus(false);
            }
            if let Some(window) = window {
                window.set_focus(true);
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

    /// `createEmbeddedAppBrowser` / `createEmbeddedSiteBrowser` for CEF.
    struct CefBackend {
        host: Rc<Host>,
    }

    impl SPEmbeddedBrowserBackend for CefBackend {
        fn create_app_browser(
            &self,
            bundle: &SPAppBundle,
            _host: &ElementRef,
            options: SPEmbeddedBrowserOptions,
            callbacks: SPEmbeddedBrowserCallbacks,
        ) -> Option<Box<dyn SPEmbeddedBrowser>> {
            if !cef_runtime::is_initialized() {
                return None;
            }
            Some(Box::new(CefAppBrowser::new(&self.host, bundle, options, callbacks)))
        }

        fn create_site_browser(
            &self,
            _host: &ElementRef,
            options: SPEmbeddedSiteBrowserOptions,
            callbacks: SPEmbeddedSiteBrowserCallbacks,
        ) -> Option<Box<dyn SPEmbeddedSiteBrowser>> {
            if !cef_runtime::is_initialized() {
                return None;
            }
            Some(Box::new(CefSiteBrowser::new(&self.host, options, callbacks)))
        }
    }

    /// Makes the embedded browsers available to the views, if CEF started.
    pub fn register_backend(ctx: &egui::Context) {
        if !cef_runtime::is_initialized() {
            return;
        }
        cef_runtime::set_pump_context(ctx);
        let scheme_handler = SpSchemeHandler::new();
        if !scheme_handler.register() {
            log::error!("Cannot serve app bundles: the sp:// scheme handler was refused");
            return;
        }
        let host = Rc::new(Host {
            ctx: ctx.clone(),
            scheme_handler,
            windows: RefCell::default(),
            next_id: Cell::new(0),
            focused: Cell::new(None),
            captured: Cell::new(None),
            held_button: Cell::new(0),
            hovered: Cell::new(None),
            last_click: RefCell::new(None),
        });
        HOST.with(|h| *h.borrow_mut() = Some(host.clone()));
        sp_embedded_browser::set_backend(Some(Rc::new(CefBackend { host })));
    }

    /// Runs CEF work and delivers browser events to the views. Call once
    /// per frame.
    pub fn pump() {
        if !cef_runtime::is_initialized() {
            return;
        }
        cef_runtime::do_message_loop_work();
        let Some(host) = host() else { return };
        let windows = host.live_windows();
        for window in &windows {
            window.dispatch_events();
        }
        // CEF asks for frames when it schedules work, but keep them coming
        // while browsers are alive in case a request was missed.
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
                // Drawn 1:1 at its own size, clipped to the bounds: while the
                // window resizes, CEF's last frame is still the old size, and
                // stretching it to the new bounds makes the page wobble.
                Some(texture) => {
                    let size = texture.size_vec2() / ui.ctx().pixels_per_point();
                    painter.with_clip_rect(bounds).image(
                        texture.id(),
                        egui::Rect::from_min_size(bounds.min, size),
                        uv,
                        egui::Color32::WHITE,
                    );
                }
                None => {
                    painter.rect_filled(bounds, 0.0, egui::Color32::WHITE);
                }
            }
            if let (true, Some(popup), Some(rect)) = (surface.popup_visible, &surface.popup, surface.popup_rect) {
                let rect = rect.translate(bounds.min.to_vec2());
                ui.ctx()
                    .layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new(("cef-popup", window.id))))
                    .image(popup.id(), rect, uv, egui::Color32::WHITE);
            }
            if pointer.is_some_and(|p| bounds.contains(p)) {
                ui.ctx().set_cursor_icon(surface.cursor);
            }
        }
    }

    /// CEF event flags for the keyboard modifiers.
    fn key_modifier(modifiers: egui::Modifiers) -> u32 {
        let mut flags = 0;
        if modifiers.shift {
            flags |= bw::SHIFT_DOWN;
        }
        if modifiers.ctrl || modifiers.mac_cmd {
            flags |= bw::CONTROL_DOWN;
        }
        if modifiers.alt {
            flags |= bw::ALT_DOWN;
        }
        flags
    }

    fn current_modifiers(host: &Host) -> u32 {
        key_modifier(host.ctx.input(|i| i.modifiers)) | host.held_button.get()
    }

    /// Returns true if a browser took the pointer move.
    pub fn pointer_moved(pos: egui::Pos2) -> bool {
        let Some(host) = host() else { return false };
        let modifiers = current_modifiers(&host);
        if let Some(window) = host.window(host.captured.get()) {
            window.send_mouse_move(window.to_view_unchecked(pos), modifiers, false);
            return true;
        }
        let target = host.window_at(pos);
        let target_id = target.as_ref().map(|(w, _)| w.id);
        if host.hovered.get() != target_id {
            if let Some(previous) = host.window(host.hovered.get()) {
                previous.send_mouse_move(previous.to_view_unchecked(pos), modifiers, true);
            }
            host.hovered.set(target_id);
        }
        match target {
            Some((window, point)) => {
                window.send_mouse_move(point, modifiers, false);
                true
            }
            None => false,
        }
    }

    /// button: 0 = left, 1 = right, like the Space mouse events. Returns
    /// true if a browser took the click.
    pub fn pointer_button(pos: egui::Pos2, button: i32, pressed: bool, modifiers: egui::Modifiers) -> bool {
        let Some(host) = host() else { return false };
        let (button_type, button_flag) = match button {
            0 => (MouseButtonType::LEFT, bw::LEFT_MOUSE_BUTTON),
            1 => (MouseButtonType::RIGHT, bw::RIGHT_MOUSE_BUTTON),
            _ => (MouseButtonType::MIDDLE, bw::MIDDLE_MOUSE_BUTTON),
        };
        let modifiers = key_modifier(modifiers) | button_flag;
        if pressed {
            let Some((window, point)) = host.window_at(pos) else {
                host.focus(None);
                return false;
            };
            host.focus(Some(&window));
            host.captured.set(Some(window.id));
            host.held_button.set(button_flag);
            let count = host.click_count(window.id, button, pos);
            window.send_mouse_move(point, modifiers, false);
            window.send_mouse_click(point, button_type, false, count, modifiers);
            true
        } else {
            host.held_button.set(0);
            let window = host.window(host.captured.take()).or_else(|| host.window_at(pos).map(|(w, _)| w));
            let Some(window) = window else { return false };
            let count = host.last_click.borrow().as_ref().map_or(1, |l| l.count);
            let point = window.to_view_unchecked(pos);
            window.send_mouse_move(point, modifiers, false);
            window.send_mouse_click(point, button_type, true, count, modifiers);
            true
        }
    }

    /// delta in points, positive = up/left like egui. Returns true if a
    /// browser took the wheel.
    pub fn mouse_wheel(pos: egui::Pos2, delta: egui::Vec2) -> bool {
        let Some(host) = host() else { return false };
        let Some((window, point)) = host.window_at(pos) else { return false };
        let modifiers = current_modifiers(&host);
        window.send_mouse_move(point, modifiers, false);
        window.send_mouse_wheel(point, (delta.x.round() as i32, delta.y.round() as i32), modifiers);
        true
    }

    /// The Windows virtual-key code CEF expects for an editing or
    /// navigation key.
    fn key_code(key: egui::Key) -> Option<i32> {
        use egui::Key;
        Some(match key {
            Key::Backspace => 0x08,
            Key::Tab => 0x09,
            Key::Enter => 0x0D,
            Key::Escape => 0x1B,
            Key::PageUp => 0x21,
            Key::PageDown => 0x22,
            Key::End => 0x23,
            Key::Home => 0x24,
            Key::ArrowLeft => 0x25,
            Key::ArrowUp => 0x26,
            Key::ArrowRight => 0x27,
            Key::ArrowDown => 0x28,
            Key::Delete => 0x2E,
            _ => return None,
        })
    }

    /// Returns true if the focused browser took the key. Only editing and
    /// navigation keys are forwarded; text arrives through [`text`], and
    /// the editing shortcuts are mapped to frame commands.
    pub fn key(key: egui::Key, pressed: bool, modifiers: egui::Modifiers) -> bool {
        let Some(host) = host() else { return false };
        let Some(window) = host.focused_window() else { return false };
        if pressed && modifiers.command {
            match (key, modifiers.shift) {
                (egui::Key::A, _) => window.edit(EditCommand::SelectAll),
                (egui::Key::Z, false) => window.edit(EditCommand::Undo),
                (egui::Key::Z, true) | (egui::Key::Y, _) => window.edit(EditCommand::Redo),
                (egui::Key::R, _) => window.reload(),
                _ => {}
            }
            return true;
        }
        if let Some(code) = key_code(key) {
            window.send_key(pressed, code, key_modifier(modifiers));
        }
        true
    }

    /// Typed text for the focused browser.
    pub fn text(text: &str) {
        let Some(host) = host() else { return };
        let Some(window) = host.focused_window() else { return };
        let modifiers = key_modifier(host.ctx.input(|i| i.modifiers));
        for unit in text.encode_utf16() {
            window.send_char(unit, modifiers);
        }
    }

    /// Clipboard shortcuts (egui turns Ctrl+X/C/V into these). CEF reads and
    /// writes the system clipboard itself. Returns true if a browser has focus.
    pub fn edit_command(command: EditCommand) -> bool {
        let Some(host) = host() else { return false };
        let Some(window) = host.focused_window() else { return false };
        window.edit(command);
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
            cef_runtime::do_message_loop_work();
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
