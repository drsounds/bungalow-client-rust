//! Port of `CefBrowserWindow`: plumbing shared by the app and site
//! browsers.
//!
//! The C++ browsers were native child windows on CEF's own UI thread. Wef
//! renders offscreen on the main thread, so a [`BrowserWindow`] instead
//! keeps the latest frame as an egui texture, the host paints it at the
//! element's bounds, and input over those bounds is forwarded to the
//! browser (see [`super`]). Wef callbacks only queue [`BrowserEvent`]s; the
//! frame loop delivers them to the window's [`BrowserClient`], like the C++
//! clients posted to the UI thread.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use wef::{
    Browser, BrowserHandler, ContextMenuEditStateFlags, ContextMenuParams, CursorInfo, CursorType, DirtyRects,
    FuncRegistry, ImageBuffer, LogicalUnit, PaintElementType, PhysicalUnit, Point, Rect, Size,
};

/// What a browser reported, delivered on the UI thread.
#[derive(Debug, Clone)]
pub enum BrowserEvent {
    AddressChanged {
        url: String,
        main_frame: bool,
    },
    TitleChanged(String),
    /// A new window was requested; Wef already blocked it.
    PopupRequested(String),
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
    pub edit_flags: ContextMenuEditStateFlags,
}

impl ContextMenuInfo {
    fn from_params(params: &ContextMenuParams) -> Self {
        Self {
            x: params.crood.x.0,
            y: params.crood.y.0,
            is_editable: params.is_editable,
            has_selection: params.selection_text.is_some_and(|s| !s.is_empty()),
            edit_flags: params.edit_state_flags,
        }
    }
}

/// The app- or site-specific side of a browser (the C++ `AppClient` and
/// `SiteClient`).
pub trait BrowserClient {
    fn on_event(&self, window: &BrowserWindow, event: BrowserEvent);
}

pub type EventQueue = Arc<Mutex<VecDeque<BrowserEvent>>>;

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

/// Wef's view of a browser: turns callbacks into textures and queued events.
struct Handler {
    ctx: egui::Context,
    surface: Rc<RefCell<Surface>>,
    events: EventQueue,
    name: String,
}

impl Handler {
    fn push(&self, event: BrowserEvent) {
        self.events.lock().unwrap().push_back(event);
        self.ctx.request_repaint();
    }
}

/// CEF paints premultiplied BGRA.
fn color_image(buffer: &ImageBuffer) -> egui::ColorImage {
    let (width, height) = buffer.dimensions();
    let pixels = buffer
        .as_raw()
        .chunks_exact(4)
        .map(|p| egui::Color32::from_rgba_premultiplied(p[2], p[1], p[0], p[3]))
        .collect();
    egui::ColorImage { size: [width as usize, height as usize], pixels }
}

impl BrowserHandler for Handler {
    fn on_paint(&mut self, type_: PaintElementType, _dirty_rects: &DirtyRects, image_buffer: ImageBuffer) {
        let image = color_image(&image_buffer);
        let mut surface = self.surface.borrow_mut();
        let slot = match type_ {
            PaintElementType::View => &mut surface.view,
            PaintElementType::Popup => &mut surface.popup,
        };
        match slot {
            Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
            None => {
                *slot =
                    Some(self.ctx.load_texture(format!("{}-{type_:?}", self.name), image, egui::TextureOptions::LINEAR))
            }
        }
        self.ctx.request_repaint();
    }

    fn on_popup_show(&mut self, show: bool) {
        let mut surface = self.surface.borrow_mut();
        surface.popup_visible = show;
        if !show {
            surface.popup = None;
        }
        self.ctx.request_repaint();
    }

    fn on_popup_position(&mut self, rect: Rect<LogicalUnit<i32>>) {
        self.surface.borrow_mut().popup_rect = Some(egui::Rect::from_min_size(
            egui::pos2(rect.x.0 as f32, rect.y.0 as f32),
            egui::vec2(rect.width.0 as f32, rect.height.0 as f32),
        ));
    }

    fn on_address_changed(&mut self, frame: wef::Frame, url: &str) {
        self.push(BrowserEvent::AddressChanged { url: url.to_string(), main_frame: frame.is_main() });
    }

    fn on_title_changed(&mut self, title: &str) {
        self.push(BrowserEvent::TitleChanged(title.to_string()));
    }

    fn on_cursor_changed(&mut self, cursor_type: CursorType, _cursor_info: Option<CursorInfo>) -> bool {
        self.surface.borrow_mut().cursor = cursor_icon(cursor_type);
        true
    }

    fn on_before_popup(&mut self, url: &str) {
        self.push(BrowserEvent::PopupRequested(url.to_string()));
    }

    fn on_load_start(&mut self, frame: wef::Frame) {
        self.push(BrowserEvent::LoadStart { url: frame.url(), main_frame: frame.is_main() });
    }

    fn on_load_end(&mut self, frame: wef::Frame) {
        self.push(BrowserEvent::LoadEnd { url: frame.url(), main_frame: frame.is_main() });
    }

    fn on_context_menu(&mut self, _frame: wef::Frame, params: ContextMenuParams) {
        self.push(BrowserEvent::ContextMenu(ContextMenuInfo::from_params(&params)));
    }
}

fn cursor_icon(cursor: CursorType) -> egui::CursorIcon {
    use egui::CursorIcon as C;
    match cursor {
        CursorType::Cross => C::Crosshair,
        CursorType::Hand => C::PointingHand,
        CursorType::IBeam => C::Text,
        CursorType::VerticalText => C::VerticalText,
        CursorType::Wait => C::Wait,
        CursorType::Progress => C::Progress,
        CursorType::Help => C::Help,
        CursorType::EastResize | CursorType::WestResize | CursorType::EastWestResize | CursorType::ColumnResize => {
            C::ResizeHorizontal
        }
        CursorType::NorthResize | CursorType::SouthResize | CursorType::NorthSouthResize | CursorType::RowResize => {
            C::ResizeVertical
        }
        CursorType::NorthEastResize | CursorType::SouthWestResize | CursorType::NorthEastSouthWestResize => {
            C::ResizeNeSw
        }
        CursorType::NorthWestResize | CursorType::SouthEastResize | CursorType::NorthWestSouthEastResize => {
            C::ResizeNwSe
        }
        CursorType::Move => C::Move,
        CursorType::Cell => C::Cell,
        CursorType::ContextMenu => C::ContextMenu,
        CursorType::Alias | CursorType::DndLink => C::Alias,
        CursorType::Copy | CursorType::DndCopy => C::Copy,
        CursorType::NoDrop | CursorType::NotAllowed | CursorType::DndNone => C::NotAllowed,
        CursorType::None => C::None,
        CursorType::ZoomIn => C::ZoomIn,
        CursorType::ZoomOut => C::ZoomOut,
        CursorType::Grab => C::Grab,
        CursorType::Grabbing | CursorType::DndMove => C::Grabbing,
        _ => C::Default,
    }
}

/// Registers `jsBridge.navigate(uri)`: the renderer side of
/// `window.bungalow.navigate` (the C++ `NavigateHandler`).
pub fn bridge_functions(events: &EventQueue, ctx: &egui::Context) -> FuncRegistry {
    let events = events.clone();
    let ctx = ctx.clone();
    FuncRegistry::builder()
        .register("navigate", move |uri: String| {
            events.lock().unwrap().push_back(BrowserEvent::NavigateRequested(uri));
            ctx.request_repaint();
        })
        .build()
}

/// One offscreen browser and where the host element shows it. Creates the
/// browser lazily and closes it when dropped.
pub struct BrowserWindow {
    pub id: u64,
    name: String,
    ctx: egui::Context,
    browser: RefCell<Option<Browser>>,
    surface: Rc<RefCell<Surface>>,
    events: EventQueue,
    client: RefCell<Option<Rc<dyn BrowserClient>>>,
    /// Window-relative bounds in points.
    bounds: Cell<egui::Rect>,
    /// Physical size last sent to the browser.
    size: Cell<(i32, i32)>,
    visible: Cell<bool>,
    dark_color_scheme: Cell<bool>,
    closed: Cell<bool>,
}

impl BrowserWindow {
    pub fn new(id: u64, name: &str, ctx: &egui::Context, dark_color_scheme: bool) -> Self {
        Self {
            id,
            name: name.to_string(),
            ctx: ctx.clone(),
            browser: RefCell::new(None),
            surface: Rc::default(),
            events: EventQueue::default(),
            client: RefCell::new(None),
            bounds: Cell::new(egui::Rect::NOTHING),
            size: Cell::new((0, 0)),
            visible: Cell::new(false),
            dark_color_scheme: Cell::new(dark_color_scheme),
            closed: Cell::new(false),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn events(&self) -> &EventQueue {
        &self.events
    }

    pub fn set_client(&self, client: Rc<dyn BrowserClient>) {
        *self.client.borrow_mut() = Some(client);
    }

    pub fn is_running(&self) -> bool {
        self.browser.borrow().is_some()
    }

    fn scale(&self) -> f32 {
        self.ctx.pixels_per_point()
    }

    fn physical_size(&self) -> (i32, i32) {
        let bounds = self.bounds.get();
        let scale = self.scale();
        if !bounds.is_positive() {
            return (1, 1);
        }
        (((bounds.width() * scale).round() as i32).max(1), ((bounds.height() * scale).round() as i32).max(1))
    }

    /// Creates the browser with url as its first page.
    pub fn create(&self, url: &str) {
        if self.closed.get() || self.is_running() {
            return;
        }
        let (width, height) = self.physical_size();
        self.size.set((width, height));
        let handler = Handler {
            ctx: self.ctx.clone(),
            surface: self.surface.clone(),
            events: self.events.clone(),
            name: self.name.clone(),
        };
        let browser = Browser::builder()
            .size(width as u32, height as u32)
            .device_scale_factor(self.scale())
            .url(url)
            .handler(handler)
            .func_registry(bridge_functions(&self.events, &self.ctx))
            .build();
        *self.browser.borrow_mut() = Some(browser);
    }

    /// Loads url into the browser. Returns false if create() has not been
    /// called yet.
    pub fn load(&self, url: &str) -> bool {
        match self.browser.borrow().as_ref() {
            Some(browser) => {
                browser.load_url(url);
                true
            }
            None => false,
        }
    }

    pub fn set_bounds(&self, x: i32, y: i32, width: i32, height: i32) {
        self.bounds.set(egui::Rect::from_min_size(
            egui::pos2(x as f32, y as f32),
            egui::vec2(width.max(0) as f32, height.max(0) as f32),
        ));
        let size = self.physical_size();
        if size != self.size.get() {
            self.size.set(size);
            if let Some(browser) = self.browser.borrow().as_ref() {
                browser.resize(Size::new(PhysicalUnit(size.0), PhysicalUnit(size.1)));
            }
        }
        self.ctx.request_repaint();
    }

    pub fn bounds(&self) -> egui::Rect {
        self.bounds.get()
    }

    pub fn set_visible(&self, visible: bool) {
        if self.visible.replace(visible) != visible {
            if !visible {
                if let Some(browser) = self.browser.borrow().as_ref() {
                    browser.set_focus(false);
                }
            }
            self.ctx.request_repaint();
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible.get() && !self.closed.get() && self.bounds.get().is_positive()
    }

    pub fn execute_script(&self, js: &str) {
        if let Some(frame) = self.browser.borrow().as_ref().and_then(Browser::main_frame) {
            frame.execute_javascript(js);
        }
    }

    pub fn reload(&self) {
        if let Some(browser) = self.browser.borrow().as_ref() {
            browser.reload();
        }
    }

    pub fn go_back(&self) -> bool {
        match self.browser.borrow().as_ref() {
            Some(browser) if browser.can_back() => {
                browser.back();
                true
            }
            _ => false,
        }
    }

    /// Wef has no DevTools window.
    pub fn show_dev_tools(&self, title: &str) {
        log::warn!("Web Inspector for {title} is not available with the Wef browser backend");
        bungalow::platform_ui::show_message(
            "Web Inspector",
            "The Web Inspector is not available in this build: the Wef browser backend has no DevTools.",
        );
    }

    /// CEF set the request context's color scheme; Wef cannot, so the page's
    /// `color-scheme` is set instead (form controls and scrollbars follow it,
    /// `prefers-color-scheme` queries do not).
    pub fn set_dark_color_scheme(&self, dark: bool) {
        self.dark_color_scheme.set(dark);
        self.apply_color_scheme();
    }

    pub fn apply_color_scheme(&self) {
        let scheme = if self.dark_color_scheme.get() { "dark" } else { "light" };
        self.execute_script(&format!("document.documentElement.style.colorScheme = '{scheme}';"));
    }

    pub fn with_browser<R>(&self, f: impl FnOnce(&Browser) -> R) -> Option<R> {
        self.browser.borrow().as_ref().map(f)
    }

    /// Delivers queued events to the client.
    pub fn dispatch_events(&self) {
        let client = self.client.borrow().clone();
        loop {
            let event = self.events.lock().unwrap().pop_front();
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
        self.surface.borrow()
    }

    /// Converts a window position to a browser one, if it is over the view.
    pub fn to_view(&self, pos: egui::Pos2) -> Option<Point<LogicalUnit<i32>>> {
        let bounds = self.bounds.get();
        let popup_hit = {
            let surface = self.surface.borrow();
            surface.popup_visible && surface.popup_rect.is_some_and(|r| r.translate(bounds.min.to_vec2()).contains(pos))
        };
        if !self.is_visible() || !(bounds.contains(pos) || popup_hit) {
            return None;
        }
        Some(self.to_view_unchecked(pos))
    }

    pub fn to_view_unchecked(&self, pos: egui::Pos2) -> Point<LogicalUnit<i32>> {
        let local = pos - self.bounds.get().min;
        Point::new(LogicalUnit(local.x.round() as i32), LogicalUnit(local.y.round() as i32))
    }

    /// Force-closes the browser.
    pub fn close(&self) {
        self.closed.set(true);
        self.client.borrow_mut().take();
        self.browser.borrow_mut().take();
        let mut surface = self.surface.borrow_mut();
        surface.view = None;
        surface.popup = None;
    }
}

impl Drop for BrowserWindow {
    fn drop(&mut self) {
        self.close();
    }
}
