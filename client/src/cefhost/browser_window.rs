//! Port of `CefBrowserWindow`: plumbing shared by the app and site
//! browsers.
//!
//! The C++ browsers were native child windows on CEF's own UI thread. Here
//! CEF renders offscreen on the main thread, so a [`BrowserWindow`] instead
//! keeps the latest frame as an egui texture, the host paints it at the
//! element's bounds, and input over those bounds is forwarded to the
//! browser (see [`super`]). CEF handlers only queue [`BrowserEvent`]s; the
//! frame loop delivers them to the window's [`BrowserClient`], like the C++
//! clients posted to the UI thread.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::os::raw::c_int;
use std::rc::Rc;

use cef::*;

use super::cef_runtime::NAVIGATE_MESSAGE;

/// Event flags for mouse and key events.
pub const SHIFT_DOWN: u32 = sys::cef_event_flags_t::EVENTFLAG_SHIFT_DOWN.0 as u32;
pub const CONTROL_DOWN: u32 = sys::cef_event_flags_t::EVENTFLAG_CONTROL_DOWN.0 as u32;
pub const ALT_DOWN: u32 = sys::cef_event_flags_t::EVENTFLAG_ALT_DOWN.0 as u32;
pub const LEFT_MOUSE_BUTTON: u32 = sys::cef_event_flags_t::EVENTFLAG_LEFT_MOUSE_BUTTON.0 as u32;
pub const MIDDLE_MOUSE_BUTTON: u32 = sys::cef_event_flags_t::EVENTFLAG_MIDDLE_MOUSE_BUTTON.0 as u32;
pub const RIGHT_MOUSE_BUTTON: u32 = sys::cef_event_flags_t::EVENTFLAG_RIGHT_MOUSE_BUTTON.0 as u32;

/// The VK_RETURN key code.
const ENTER: c_int = 0x0D;

#[cfg(not(target_os = "macos"))]
type NativeCursor = CursorHandle;
#[cfg(target_os = "macos")]
type NativeCursor = *mut u8;

/// What a browser reported, delivered on the UI thread.
#[derive(Debug, Clone)]
pub enum BrowserEvent {
    AddressChanged {
        url: String,
        main_frame: bool,
    },
    TitleChanged(String),
    /// A new window was requested; it was already blocked.
    PopupRequested(String),
    /// [`BrowserClient::allow_navigation`] refused a top-level navigation.
    NavigationBlocked(String),
    LoadStart {
        url: String,
        main_frame: bool,
    },
    LoadEnd {
        url: String,
        main_frame: bool,
    },
    ContextMenu(ContextMenuInfo),
    /// `window.bungalow.navigate(uri)` or a bungalow: link in the page.
    NavigateRequested(String),
}

#[derive(Debug, Clone)]
pub struct ContextMenuInfo {
    pub x: i32,
    pub y: i32,
    pub is_editable: bool,
    pub has_selection: bool,
    pub can_cut: bool,
    pub can_copy: bool,
    pub can_paste: bool,
    pub can_select_all: bool,
}

impl ContextMenuInfo {
    fn from_params(params: &ContextMenuParams) -> Self {
        use sys::cef_context_menu_edit_state_flags_t as E;
        let flags = params.edit_state_flags().as_ref().0;
        let can = |flag: E| flags & flag.0 != 0;
        Self {
            x: params.xcoord(),
            y: params.ycoord(),
            is_editable: params.is_editable() != 0,
            has_selection: !CefString::from(&params.selection_text()).to_string().is_empty(),
            can_cut: can(E::CM_EDITFLAG_CAN_CUT),
            can_copy: can(E::CM_EDITFLAG_CAN_COPY),
            can_paste: can(E::CM_EDITFLAG_CAN_PASTE),
            can_select_all: can(E::CM_EDITFLAG_CAN_SELECT_ALL),
        }
    }
}

/// Editing commands for the focused frame.
pub enum EditCommand {
    Cut,
    Copy,
    Paste,
    SelectAll,
    Undo,
    Redo,
}

/// The app- or site-specific side of a browser (the C++ `AppClient` and
/// `SiteClient`).
pub trait BrowserClient {
    /// Decides whether a top-level navigation may go ahead (the C++
    /// `OnBeforeBrowse`). CEF waits for the answer, so this must not call
    /// back into the host: a refused navigation arrives afterwards as
    /// [`BrowserEvent::NavigationBlocked`].
    fn allow_navigation(&self, url: &str, is_redirect: bool) -> bool;

    fn on_event(&self, window: &BrowserWindow, event: BrowserEvent);
}

/// What the browser last painted.
#[derive(Default)]
pub struct Surface {
    pub view: Option<egui::TextureHandle>,
    pub popup: Option<egui::TextureHandle>,
    /// Popup bounds in points, relative to the view.
    pub popup_rect: Option<egui::Rect>,
    pub popup_visible: bool,
    pub cursor: egui::CursorIcon,
}

/// What the window and its CEF handlers share. CEF calls the handlers on
/// the main thread, between frames.
struct Shared {
    ctx: egui::Context,
    name: String,
    client: RefCell<Option<Rc<dyn BrowserClient>>>,
    surface: RefCell<Surface>,
    events: RefCell<VecDeque<BrowserEvent>>,
    /// Physical view size and device scale last given to the browser.
    size: Cell<(i32, i32)>,
    scale: Cell<f32>,
}

impl Shared {
    fn push(&self, event: BrowserEvent) {
        self.events.borrow_mut().push_back(event);
        self.ctx.request_repaint();
    }

    /// A closed window (no client) navigates nowhere.
    fn allows_navigation(&self, url: &str, is_redirect: bool) -> bool {
        self.client.borrow().as_ref().is_some_and(|c| c.allow_navigation(url, is_redirect))
    }
}

fn frame_event(frame: Option<&mut Frame>) -> (String, bool) {
    match frame {
        Some(frame) => (CefString::from(&frame.url()).to_string(), frame.is_main() != 0),
        None => (String::new(), false),
    }
}

/// CEF paints premultiplied BGRA.
fn color_image(buffer: &[u8], width: usize, height: usize) -> egui::ColorImage {
    let pixels = buffer
        .chunks_exact(4)
        .map(|p| egui::Color32::from_rgba_premultiplied(p[2], p[1], p[0], p[3]))
        .collect();
    egui::ColorImage { size: [width, height], pixels }
}

wrap_render_handler! {
    struct ViewRenderer {
        shared: Rc<Shared>,
    }

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            let Some(rect) = rect else { return };
            let (width, height) = self.shared.size.get();
            let scale = self.shared.scale.get();
            *rect = Rect {
                x: 0,
                y: 0,
                width: ((width as f32 / scale) as i32).max(1),
                height: ((height as f32 / scale) as i32).max(1),
            };
        }

        fn screen_info(&self, _browser: Option<&mut Browser>, screen_info: Option<&mut ScreenInfo>) -> c_int {
            let Some(screen_info) = screen_info else { return 0 };
            screen_info.device_scale_factor = self.shared.scale.get();
            1
        }

        fn on_popup_show(&self, _browser: Option<&mut Browser>, show: c_int) {
            let mut surface = self.shared.surface.borrow_mut();
            surface.popup_visible = show != 0;
            if show == 0 {
                surface.popup = None;
            }
            self.shared.ctx.request_repaint();
        }

        fn on_popup_size(&self, _browser: Option<&mut Browser>, rect: Option<&Rect>) {
            let Some(rect) = rect else { return };
            self.shared.surface.borrow_mut().popup_rect = Some(egui::Rect::from_min_size(
                egui::pos2(rect.x as f32, rect.y as f32),
                egui::vec2(rect.width as f32, rect.height as f32),
            ));
        }

        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            type_: PaintElementType,
            _dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: c_int,
            height: c_int,
        ) {
            if buffer.is_null() || width <= 0 || height <= 0 {
                return;
            }
            let (width, height) = (width as usize, height as usize);
            // SAFETY: CEF passes a width x height BGRA buffer, valid for the call.
            let buffer = unsafe { std::slice::from_raw_parts(buffer, width * height * 4) };
            let image = color_image(buffer, width, height);
            let popup = type_ == PaintElementType::POPUP;
            let mut surface = self.shared.surface.borrow_mut();
            let slot = if popup { &mut surface.popup } else { &mut surface.view };
            match slot {
                Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
                None => {
                    let name = format!("{}-{}", self.shared.name, if popup { "popup" } else { "view" });
                    *slot = Some(self.shared.ctx.load_texture(name, image, egui::TextureOptions::LINEAR));
                }
            }
            self.shared.ctx.request_repaint();
        }
    }
}

wrap_display_handler! {
    struct ViewDisplay {
        shared: Rc<Shared>,
    }

    impl DisplayHandler {
        fn on_address_change(&self, _browser: Option<&mut Browser>, frame: Option<&mut Frame>, url: Option<&CefString>) {
            let main_frame = frame.is_some_and(|f| f.is_main() != 0);
            let url = url.map(CefString::to_string).unwrap_or_default();
            self.shared.push(BrowserEvent::AddressChanged { url, main_frame });
        }

        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            self.shared.push(BrowserEvent::TitleChanged(title.map(CefString::to_string).unwrap_or_default()));
        }

        fn on_cursor_change(
            &self,
            _browser: Option<&mut Browser>,
            _cursor: NativeCursor,
            type_: CursorType,
            _custom_cursor_info: Option<&CursorInfo>,
        ) -> c_int {
            self.shared.surface.borrow_mut().cursor = cursor_icon(type_);
            1
        }
    }
}

wrap_life_span_handler! {
    struct ViewLifeSpan {
        shared: Rc<Shared>,
    }

    impl LifeSpanHandler {
        /// There are no extra windows: the request goes to the client instead.
        fn on_before_popup(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: c_int,
            target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: c_int,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut c_int>,
        ) -> c_int {
            self.shared.push(BrowserEvent::PopupRequested(target_url.map(CefString::to_string).unwrap_or_default()));
            1
        }
    }
}

wrap_request_handler! {
    struct ViewRequests {
        shared: Rc<Shared>,
    }

    impl RequestHandler {
        /// Top-level navigations the client refuses never start; frames
        /// inside the page are left to its Content-Security-Policy.
        fn on_before_browse(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _user_gesture: c_int,
            is_redirect: c_int,
        ) -> c_int {
            if !frame.is_some_and(|f| f.is_main() != 0) {
                return 0;
            }
            let url = request.map(|r| CefString::from(&r.url()).to_string()).unwrap_or_default();
            if self.shared.allows_navigation(&url, is_redirect != 0) {
                return 0;
            }
            self.shared.push(BrowserEvent::NavigationBlocked(url));
            1
        }

        /// Middle clicks and Ctrl+clicks ask for a new tab: there are none,
        /// so they go to the client like other new windows.
        fn on_open_urlfrom_tab(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            target_url: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: c_int,
        ) -> c_int {
            self.shared.push(BrowserEvent::PopupRequested(target_url.map(CefString::to_string).unwrap_or_default()));
            1
        }
    }
}

wrap_load_handler! {
    struct ViewLoad {
        shared: Rc<Shared>,
    }

    impl LoadHandler {
        fn on_load_start(&self, _browser: Option<&mut Browser>, frame: Option<&mut Frame>, _transition_type: TransitionType) {
            let (url, main_frame) = frame_event(frame);
            self.shared.push(BrowserEvent::LoadStart { url, main_frame });
        }

        fn on_load_end(&self, _browser: Option<&mut Browser>, frame: Option<&mut Frame>, _http_status_code: c_int) {
            let (url, main_frame) = frame_event(frame);
            self.shared.push(BrowserEvent::LoadEnd { url, main_frame });
        }
    }
}

wrap_context_menu_handler! {
    struct ViewContextMenu {
        shared: Rc<Shared>,
    }

    impl ContextMenuHandler {
        /// The client shows its own menu instead of CEF's.
        fn run_context_menu(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            params: Option<&mut ContextMenuParams>,
            _model: Option<&mut MenuModel>,
            callback: Option<&mut RunContextMenuCallback>,
        ) -> c_int {
            if let Some(params) = params {
                self.shared.push(BrowserEvent::ContextMenu(ContextMenuInfo::from_params(params)));
            }
            if let Some(callback) = callback {
                callback.cancel();
            }
            1
        }
    }
}

wrap_jsdialog_handler! {
    struct ViewDialogs;

    impl JsdialogHandler {
        /// Pages always may leave.
        fn on_before_unload_dialog(
            &self,
            _browser: Option<&mut Browser>,
            _message_text: Option<&CefString>,
            _is_reload: c_int,
            callback: Option<&mut JsdialogCallback>,
        ) -> c_int {
            if let Some(callback) = callback {
                callback.cont(1, None);
            }
            1
        }
    }
}

wrap_client! {
    struct ViewClient {
        shared: Rc<Shared>,
    }

    impl Client {
        fn render_handler(&self) -> Option<RenderHandler> {
            Some(ViewRenderer::new(self.shared.clone()))
        }

        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(ViewDisplay::new(self.shared.clone()))
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(ViewLifeSpan::new(self.shared.clone()))
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(ViewLoad::new(self.shared.clone()))
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(ViewRequests::new(self.shared.clone()))
        }

        fn context_menu_handler(&self) -> Option<ContextMenuHandler> {
            Some(ViewContextMenu::new(self.shared.clone()))
        }

        fn jsdialog_handler(&self) -> Option<JsdialogHandler> {
            Some(ViewDialogs::new())
        }

        /// `jsBridge.navigate(uri)` from the renderer (the C++ `NavigateHandler`).
        fn on_process_message_received(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _source_process: ProcessId,
            message: Option<&mut ProcessMessage>,
        ) -> c_int {
            let Some(message) = message else { return 0 };
            if CefString::from(&message.name()).to_string() != NAVIGATE_MESSAGE {
                return 0;
            }
            let Some(args) = message.argument_list() else { return 1 };
            let uri = CefString::from(&args.string(0)).to_string();
            self.shared.push(BrowserEvent::NavigateRequested(uri));
            1
        }
    }
}

// The DevTools window draws itself, so its client handles nothing.
wrap_client! {
    struct DevToolsClient;

    impl Client {}
}

fn cursor_icon(cursor: CursorType) -> egui::CursorIcon {
    use egui::CursorIcon as C;
    match cursor {
        CursorType::CROSS => C::Crosshair,
        CursorType::HAND => C::PointingHand,
        CursorType::IBEAM => C::Text,
        CursorType::VERTICALTEXT => C::VerticalText,
        CursorType::WAIT => C::Wait,
        CursorType::PROGRESS => C::Progress,
        CursorType::HELP => C::Help,
        CursorType::EASTRESIZE | CursorType::WESTRESIZE | CursorType::EASTWESTRESIZE | CursorType::COLUMNRESIZE => {
            C::ResizeHorizontal
        }
        CursorType::NORTHRESIZE | CursorType::SOUTHRESIZE | CursorType::NORTHSOUTHRESIZE | CursorType::ROWRESIZE => {
            C::ResizeVertical
        }
        CursorType::NORTHEASTRESIZE | CursorType::SOUTHWESTRESIZE | CursorType::NORTHEASTSOUTHWESTRESIZE => {
            C::ResizeNeSw
        }
        CursorType::NORTHWESTRESIZE | CursorType::SOUTHEASTRESIZE | CursorType::NORTHWESTSOUTHEASTRESIZE => {
            C::ResizeNwSe
        }
        CursorType::MOVE => C::Move,
        CursorType::CELL => C::Cell,
        CursorType::CONTEXTMENU => C::ContextMenu,
        CursorType::ALIAS | CursorType::DND_LINK => C::Alias,
        CursorType::COPY | CursorType::DND_COPY => C::Copy,
        CursorType::NODROP | CursorType::NOTALLOWED | CursorType::DND_NONE => C::NotAllowed,
        CursorType::NONE => C::None,
        CursorType::ZOOMIN => C::ZoomIn,
        CursorType::ZOOMOUT => C::ZoomOut,
        CursorType::GRAB => C::Grab,
        CursorType::GRABBING | CursorType::DND_MOVE => C::Grabbing,
        _ => C::Default,
    }
}

/// One offscreen browser and where the host element shows it. Creates the
/// browser lazily and closes it when dropped.
pub struct BrowserWindow {
    pub id: u64,
    shared: Rc<Shared>,
    browser: RefCell<Option<Browser>>,
    /// Window-relative bounds in points.
    bounds: Cell<egui::Rect>,
    visible: Cell<bool>,
    dark_color_scheme: Cell<bool>,
    closed: Cell<bool>,
}

impl BrowserWindow {
    pub fn new(id: u64, name: &str, ctx: &egui::Context, dark_color_scheme: bool) -> Self {
        Self {
            id,
            shared: Rc::new(Shared {
                ctx: ctx.clone(),
                name: name.to_string(),
                client: RefCell::new(None),
                surface: RefCell::default(),
                events: RefCell::default(),
                size: Cell::new((0, 0)),
                scale: Cell::new(ctx.pixels_per_point()),
            }),
            browser: RefCell::new(None),
            bounds: Cell::new(egui::Rect::NOTHING),
            visible: Cell::new(false),
            dark_color_scheme: Cell::new(dark_color_scheme),
            closed: Cell::new(false),
        }
    }

    pub fn name(&self) -> &str {
        &self.shared.name
    }

    pub fn set_client(&self, client: Rc<dyn BrowserClient>) {
        *self.shared.client.borrow_mut() = Some(client);
    }

    pub fn is_running(&self) -> bool {
        self.browser.borrow().is_some()
    }

    fn scale(&self) -> f32 {
        self.shared.ctx.pixels_per_point()
    }

    fn physical_size(&self) -> (i32, i32) {
        let bounds = self.bounds.get();
        let scale = self.scale();
        if !bounds.is_positive() {
            return (1, 1);
        }
        (((bounds.width() * scale).round() as i32).max(1), ((bounds.height() * scale).round() as i32).max(1))
    }

    fn host(&self) -> Option<BrowserHost> {
        self.browser.borrow().as_ref().and_then(|b| b.host())
    }

    fn with_frame(&self, f: impl FnOnce(&Frame)) {
        let frame = self.browser.borrow().as_ref().and_then(|b| b.focused_frame().or_else(|| b.main_frame()));
        if let Some(frame) = frame {
            f(&frame);
        }
    }

    /// Creates the browser with url as its first page.
    pub fn create(&self, url: &str) {
        if self.closed.get() || self.is_running() {
            return;
        }
        self.shared.size.set(self.physical_size());
        self.shared.scale.set(self.scale());
        let window_info =
            WindowInfo { windowless_rendering_enabled: 1, runtime_style: RuntimeStyle::ALLOY, ..Default::default() };
        let settings =
            BrowserSettings { windowless_frame_rate: 60, background_color: 0xFFFF_FFFF, ..Default::default() };
        let mut client = ViewClient::new(self.shared.clone());
        let browser = browser_host_create_browser_sync(
            Some(&window_info),
            Some(&mut client),
            Some(&CefString::from(url)),
            Some(&settings),
            None,
            None,
        );
        if browser.is_none() {
            log::error!("Cannot create a browser for {url}");
        }
        *self.browser.borrow_mut() = browser;
    }

    /// Loads url into the browser. Returns false if create() has not been
    /// called yet.
    pub fn load(&self, url: &str) -> bool {
        let frame = match self.browser.borrow().as_ref() {
            Some(browser) => browser.main_frame(),
            None => return false,
        };
        if let Some(frame) = frame {
            frame.load_url(Some(&CefString::from(url)));
        }
        true
    }

    /// Called on every paint, so it only repaints when the bounds change;
    /// repainting unconditionally would redraw the window nonstop.
    pub fn set_bounds(&self, x: i32, y: i32, width: i32, height: i32) {
        let bounds = egui::Rect::from_min_size(
            egui::pos2(x as f32, y as f32),
            egui::vec2(width.max(0) as f32, height.max(0) as f32),
        );
        if self.bounds.replace(bounds) == bounds {
            return;
        }
        let size = self.physical_size();
        let scale = self.scale();
        let scale_changed = self.shared.scale.replace(scale) != scale;
        if self.shared.size.replace(size) != size || scale_changed {
            if let Some(host) = self.host() {
                if scale_changed {
                    host.notify_screen_info_changed();
                }
                host.was_resized();
            }
        }
        self.shared.ctx.request_repaint();
    }

    pub fn bounds(&self) -> egui::Rect {
        self.bounds.get()
    }

    pub fn set_visible(&self, visible: bool) {
        if self.visible.replace(visible) != visible {
            if let Some(host) = self.host() {
                if !visible {
                    host.set_focus(0);
                }
                host.was_hidden((!visible).into());
            }
            self.shared.ctx.request_repaint();
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible.get() && !self.closed.get() && self.bounds.get().is_positive()
    }

    pub fn execute_script(&self, js: &str) {
        let frame = self.browser.borrow().as_ref().and_then(|b| b.main_frame());
        if let Some(frame) = frame {
            frame.execute_java_script(Some(&CefString::from(js)), None, 0);
        }
    }

    pub fn reload(&self) {
        if let Some(browser) = self.browser.borrow().as_ref() {
            browser.reload();
        }
    }

    pub fn can_go_back(&self) -> bool {
        self.browser.borrow().as_ref().is_some_and(|b| b.can_go_back() != 0)
    }

    pub fn can_go_forward(&self) -> bool {
        self.browser.borrow().as_ref().is_some_and(|b| b.can_go_forward() != 0)
    }

    pub fn go_back(&self) -> bool {
        match self.browser.borrow().as_ref() {
            Some(browser) if browser.can_go_back() != 0 => {
                browser.go_back();
                true
            }
            _ => false,
        }
    }

    pub fn go_forward(&self) {
        if let Some(browser) = self.browser.borrow().as_ref() {
            browser.go_forward();
        }
    }

    /// Opens the Web Inspector in a window of its own.
    pub fn show_dev_tools(&self, title: &str) {
        let Some(host) = self.host() else { return };
        #[cfg(target_os = "windows")]
        let window_info = WindowInfo { runtime_style: RuntimeStyle::ALLOY, ..Default::default() }
            .set_as_popup(Default::default(), &format!("Web Inspector - {title}"));
        #[cfg(not(target_os = "windows"))]
        let window_info = {
            let _ = title;
            WindowInfo { runtime_style: RuntimeStyle::ALLOY, ..Default::default() }
        };
        let mut client = DevToolsClient::new();
        host.show_dev_tools(Some(&window_info), Some(&mut client), Some(&BrowserSettings::default()), None);
    }

    /// The page's `color-scheme` follows the theme (form controls and
    /// scrollbars follow it, `prefers-color-scheme` queries do not).
    pub fn set_dark_color_scheme(&self, dark: bool) {
        self.dark_color_scheme.set(dark);
        self.apply_color_scheme();
    }

    pub fn apply_color_scheme(&self) {
        let scheme = if self.dark_color_scheme.get() { "dark" } else { "light" };
        self.execute_script(&format!("document.documentElement.style.colorScheme = '{scheme}';"));
    }

    pub fn set_focus(&self, focus: bool) {
        if let Some(host) = self.host() {
            host.set_focus(focus.into());
        }
    }

    /// point is in view coordinates; modifiers are event flags.
    pub fn send_mouse_move(&self, point: (i32, i32), modifiers: u32, leave: bool) {
        if let Some(host) = self.host() {
            let event = MouseEvent { x: point.0, y: point.1, modifiers };
            host.send_mouse_move_event(Some(&event), leave.into());
        }
    }

    pub fn send_mouse_click(&self, point: (i32, i32), button: MouseButtonType, up: bool, count: usize, modifiers: u32) {
        if let Some(host) = self.host() {
            let event = MouseEvent { x: point.0, y: point.1, modifiers };
            host.send_mouse_click_event(Some(&event), button, up.into(), count.min(3) as c_int);
        }
    }

    pub fn send_mouse_wheel(&self, point: (i32, i32), delta: (i32, i32), modifiers: u32) {
        if let Some(host) = self.host() {
            let event = MouseEvent { x: point.0, y: point.1, modifiers };
            host.send_mouse_wheel_event(Some(&event), delta.0, delta.1);
        }
    }

    /// key_code is a Windows virtual-key code. Enter also types a carriage
    /// return, which is what submits forms and breaks lines.
    pub fn send_key(&self, pressed: bool, key_code: c_int, modifiers: u32) {
        let Some(host) = self.host() else { return };
        let event = KeyEvent {
            type_: if pressed { KeyEventType::RAWKEYDOWN } else { KeyEventType::KEYUP },
            modifiers,
            windows_key_code: key_code,
            native_key_code: key_code,
            ..Default::default()
        };
        host.send_key_event(Some(&event));
        if pressed && key_code == ENTER {
            self.send_char(ENTER as u16, modifiers);
        }
    }

    /// One UTF-16 unit of typed text.
    pub fn send_char(&self, unit: u16, modifiers: u32) {
        let Some(host) = self.host() else { return };
        let event = KeyEvent {
            type_: KeyEventType::CHAR,
            modifiers,
            windows_key_code: unit as c_int,
            native_key_code: unit as c_int,
            character: unit,
            unmodified_character: unit,
            ..Default::default()
        };
        host.send_key_event(Some(&event));
    }

    /// CEF reads and writes the system clipboard itself.
    pub fn edit(&self, command: EditCommand) {
        self.with_frame(|frame| match command {
            EditCommand::Cut => frame.cut(),
            EditCommand::Copy => frame.copy(),
            EditCommand::Paste => frame.paste(),
            EditCommand::SelectAll => frame.select_all(),
            EditCommand::Undo => frame.undo(),
            EditCommand::Redo => frame.redo(),
        });
    }

    /// Delivers queued events to the client.
    pub fn dispatch_events(&self) {
        let client = self.shared.client.borrow().clone();
        loop {
            let event = self.shared.events.borrow_mut().pop_front();
            let Some(event) = event else { break };
            if let BrowserEvent::LoadStart { main_frame: true, .. } = &event {
                self.apply_color_scheme();
            }
            if let Some(client) = &client {
                client.on_event(self, event);
            }
        }
    }

    pub fn surface(&self) -> std::cell::Ref<'_, Surface> {
        self.shared.surface.borrow()
    }

    /// Converts a window position to a browser one, if it is over the view.
    pub fn to_view(&self, pos: egui::Pos2) -> Option<(i32, i32)> {
        let bounds = self.bounds.get();
        let popup_hit = {
            let surface = self.surface();
            surface.popup_visible && surface.popup_rect.is_some_and(|r| r.translate(bounds.min.to_vec2()).contains(pos))
        };
        if !self.is_visible() || !(bounds.contains(pos) || popup_hit) {
            return None;
        }
        Some(self.to_view_unchecked(pos))
    }

    pub fn to_view_unchecked(&self, pos: egui::Pos2) -> (i32, i32) {
        let local = pos - self.bounds.get().min;
        (local.x.round() as i32, local.y.round() as i32)
    }

    /// Force-closes the browser.
    pub fn close(&self) {
        self.closed.set(true);
        self.shared.client.borrow_mut().take();
        if let Some(host) = self.browser.borrow_mut().take().and_then(|b| b.host()) {
            host.close_browser(1);
        }
        let mut surface = self.shared.surface.borrow_mut();
        surface.view = None;
        surface.popup = None;
    }
}

impl Drop for BrowserWindow {
    fn drop(&mut self) {
        self.close();
    }
}
