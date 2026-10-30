//! Port of `wtl_main.cpp`: the Bungalow main window.
//!
//! Like the WTL frame, the window has no system frame: the app paints the
//! whole window, and a caption strip overlays the top of the app header with
//! the File and Develop menus, the title and the minimize/maximize/close
//! buttons. The caption and edges drag and resize the window.
//!
//! With the `wef` feature, CEF relaunches this executable for its renderer
//! and GPU processes, so `main` first hands those over to Wef.

// The ported classes keep their whole C++ API, including members only the
// C++ callers (or the not-yet-ported linux/ shell) used.
#![allow(dead_code)]

mod bungalow_app;
mod egui_graphics_context;
mod favicon_cache;
mod main_element;
mod native_application;
mod platform;
#[macro_use]
mod views;
mod wefhost;

use std::rc::Rc;

use egui::{Color32, CursorIcon, FontId, Pos2, Rect, ResizeDirection, Vec2, ViewportCommand};

use bungalow::platform_ui;

use native_application::{vk, NativeApplication};
use platform::{ClientPlatformHooks, PopupMenus};

const CAPTION_BUTTON_WIDTH: f32 = 46.0;
/// Width of the invisible resize border around the window.
const RESIZE_BORDER: f32 = 5.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CaptionItem {
    File,
    Develop,
    Minimize,
    Maximize,
    Close,
}

const CAPTION_ITEMS: [CaptionItem; 5] =
    [CaptionItem::File, CaptionItem::Develop, CaptionItem::Minimize, CaptionItem::Maximize, CaptionItem::Close];

#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuCommand {
    FileAbout,
    FileExit,
    DevelopShowInspector,
    DevelopReloadPage,
}

impl CaptionItem {
    fn is_menu(self) -> bool {
        matches!(self, CaptionItem::File | CaptionItem::Develop)
    }

    fn label(self) -> &'static str {
        if self == CaptionItem::Develop {
            "Develop"
        } else {
            "File"
        }
    }

    fn menu(self) -> &'static [(MenuCommand, &'static str)] {
        match self {
            CaptionItem::File => &[(MenuCommand::FileAbout, "About Bungalow"), (MenuCommand::FileExit, "Exit")],
            CaptionItem::Develop => &[
                (MenuCommand::DevelopShowInspector, "Show Inspector"),
                (MenuCommand::DevelopReloadPage, "Reload Page"),
            ],
            _ => &[],
        }
    }
}

struct MainFrame {
    app: NativeApplication,
    menus: Rc<PopupMenus>,
    initialization_error: Option<String>,
    size: (i32, i32),
    initialized: bool,
    placed: bool,
    hovered_item: Option<CaptionItem>,
    pressed_item: Option<CaptionItem>,
    open_menu: Option<CaptionItem>,
}

/// The Windows virtual-key code of an egui key, as the C++ app received in
/// WM_KEYDOWN; arrow up/down use the Space key codes.
fn key_code(key: egui::Key) -> Option<i32> {
    use egui::Key;
    let code = match key {
        Key::ArrowUp => space::sp_event_args::SPKeyEventArgs::KEY_UP,
        Key::ArrowDown => space::sp_event_args::SPKeyEventArgs::KEY_DOWN,
        Key::ArrowLeft => vk::LEFT,
        Key::ArrowRight => vk::RIGHT,
        Key::Backspace => vk::BACK,
        Key::Tab => vk::TAB,
        Key::Enter => vk::RETURN,
        Key::Escape => vk::ESCAPE,
        Key::Space => vk::SPACE,
        Key::PageUp => vk::PRIOR,
        Key::PageDown => vk::NEXT,
        Key::End => vk::END,
        Key::Home => vk::HOME,
        Key::Delete => vk::DELETE,
        _ => {
            let name = key.name();
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_alphanumeric() => c.to_ascii_uppercase() as i32,
                _ => return None,
            }
        }
    };
    Some(code)
}

impl MainFrame {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let menus = Rc::new(PopupMenus::default());
        platform_ui::set_hooks(Some(Rc::new(ClientPlatformHooks { menus: menus.clone() })));

        Self {
            app: NativeApplication::new(),
            menus,
            initialization_error: None,
            size: (0, 0),
            initialized: false,
            placed: false,
            hovered_item: None,
            pressed_item: None,
            open_menu: None,
        }
    }

    // ── Caption geometry ──────────────────────────────────────────────────

    fn caption_height() -> f32 {
        NativeApplication::CAPTION_HEIGHT as f32
    }

    fn menu_label_width(ctx: &egui::Context, item: CaptionItem) -> f32 {
        ctx.fonts(|f| f.layout_no_wrap(item.label().into(), FontId::proportional(12.0), Color32::WHITE).size().x)
    }

    fn caption_item_rect(&self, ctx: &egui::Context, item: CaptionItem) -> Rect {
        let width = self.size.0 as f32;
        let height = Self::caption_height();
        let span = |left: f32, right: f32| Rect::from_min_max(Pos2::new(left, 0.0), Pos2::new(right, height));
        match item {
            CaptionItem::File => span(4.0, 4.0 + Self::menu_label_width(ctx, item) + 16.0),
            CaptionItem::Develop => {
                let left = self.caption_item_rect(ctx, CaptionItem::File).right();
                span(left, left + Self::menu_label_width(ctx, item) + 16.0)
            }
            CaptionItem::Close => span(width - CAPTION_BUTTON_WIDTH, width),
            CaptionItem::Maximize => span(width - CAPTION_BUTTON_WIDTH * 2.0, width - CAPTION_BUTTON_WIDTH),
            CaptionItem::Minimize => span(width - CAPTION_BUTTON_WIDTH * 3.0, width - CAPTION_BUTTON_WIDTH * 2.0),
        }
    }

    fn caption_item_at(&self, ctx: &egui::Context, point: Pos2) -> Option<CaptionItem> {
        if point.y < 0.0 || point.y >= Self::caption_height() {
            return None;
        }
        CAPTION_ITEMS.into_iter().find(|item| self.caption_item_rect(ctx, *item).contains(point))
    }

    /// `OnNcHitTest`'s border part: the resize direction at `point`, if any.
    fn resize_direction_at(&self, ctx: &egui::Context, point: Pos2) -> Option<ResizeDirection> {
        if ctx.input(|i| i.viewport().maximized).unwrap_or(false) {
            return None;
        }
        let (w, h) = (self.size.0 as f32, self.size.1 as f32);
        let left = point.x < RESIZE_BORDER;
        let right = point.x >= w - RESIZE_BORDER;
        let top = point.y < RESIZE_BORDER;
        let bottom = point.y >= h - RESIZE_BORDER;
        // The close button reaches the top-right corner; keep it clickable there.
        if top && right && self.caption_item_at(ctx, point) == Some(CaptionItem::Close) {
            return None;
        }
        Some(match (top, bottom, left, right) {
            (true, _, true, _) => ResizeDirection::NorthWest,
            (true, _, _, true) => ResizeDirection::NorthEast,
            (_, true, true, _) => ResizeDirection::SouthWest,
            (_, true, _, true) => ResizeDirection::SouthEast,
            (_, _, true, _) => ResizeDirection::West,
            (_, _, _, true) => ResizeDirection::East,
            (true, _, _, _) => ResizeDirection::North,
            (_, true, _, _) => ResizeDirection::South,
            _ => return None,
        })
    }

    // ── Commands ──────────────────────────────────────────────────────────

    fn on_command(&self, ctx: &egui::Context, command: MenuCommand) {
        match command {
            MenuCommand::FileAbout => platform_ui::show_message("About Bungalow", "Bungalow Client"),
            MenuCommand::FileExit => ctx.send_viewport_cmd(ViewportCommand::Close),
            MenuCommand::DevelopShowInspector => self.app.show_web_inspector(),
            MenuCommand::DevelopReloadPage => self.app.reload_web_page(),
        }
    }

    fn activate_caption_item(&mut self, ctx: &egui::Context, item: CaptionItem) {
        match item {
            CaptionItem::File | CaptionItem::Develop => self.open_menu = Some(item),
            CaptionItem::Minimize => ctx.send_viewport_cmd(ViewportCommand::Minimized(true)),
            CaptionItem::Maximize => {
                let maximized = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
                ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
            }
            CaptionItem::Close => ctx.send_viewport_cmd(ViewportCommand::Close),
        }
    }

    // ── Input ─────────────────────────────────────────────────────────────

    /// Routes this frame's pointer and key events to the caption, the
    /// embedded browsers or the Space, like the WTL message handlers.
    fn handle_input(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|i| i.events.clone());
        let egui_has_keyboard = ctx.wants_keyboard_input();
        let menu_open = self.menus.is_open() || self.open_menu.is_some();
        for event in events {
            match event {
                egui::Event::PointerMoved(pos) => {
                    let item = self.caption_item_at(ctx, pos);
                    self.hovered_item = item;
                    if let Some(direction) = self.resize_direction_at(ctx, pos) {
                        ctx.set_cursor_icon(resize_cursor(direction));
                    }
                    if item.is_some() || self.pressed_item.is_some() || menu_open {
                        continue;
                    }
                    #[cfg(feature = "wef")]
                    if wefhost::pointer_moved(pos) {
                        continue;
                    }
                    let cmd = ctx.input(|i| i.modifiers.command);
                    self.app.mouse_event("mousemove", pos.x as i32, pos.y as i32, 0, cmd);
                }
                egui::Event::PointerGone => self.hovered_item = None,
                egui::Event::PointerButton { pos, button, pressed, modifiers } => {
                    let button_index = match button {
                        egui::PointerButton::Primary => 0,
                        egui::PointerButton::Secondary => 1,
                        _ => continue,
                    };
                    if menu_open {
                        continue;
                    }
                    if pressed && button_index == 0 {
                        if let Some(direction) = self.resize_direction_at(ctx, pos) {
                            ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
                            continue;
                        }
                    }
                    let item = self.caption_item_at(ctx, pos);
                    if button_index == 0 && pressed {
                        if let Some(item) = item {
                            if item.is_menu() {
                                self.open_menu = Some(item);
                            } else {
                                self.pressed_item = Some(item);
                            }
                            continue;
                        }
                        if pos.y < Self::caption_height() {
                            // HTCAPTION: drag the window; double-click maximizes.
                            if ctx.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary)) {
                                self.activate_caption_item(ctx, CaptionItem::Maximize);
                            } else {
                                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                            }
                            continue;
                        }
                    }
                    if button_index == 0 && !pressed {
                        if let Some(pressed_item) = self.pressed_item.take() {
                            if item == Some(pressed_item) {
                                self.activate_caption_item(ctx, pressed_item);
                            }
                            continue;
                        }
                    }
                    #[cfg(feature = "wef")]
                    if wefhost::pointer_button(pos, button_index, pressed, modifiers) {
                        continue;
                    }
                    let name = if pressed { "mousedown" } else { "mouseup" };
                    self.app.mouse_event(name, pos.x as i32, pos.y as i32, button_index, modifiers.command);
                }
                egui::Event::MouseWheel { delta, unit, .. } => {
                    let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) else { continue };
                    let scale = match unit {
                        egui::MouseWheelUnit::Line => 40.0,
                        egui::MouseWheelUnit::Page => self.size.1 as f32,
                        egui::MouseWheelUnit::Point => 1.0,
                    };
                    #[cfg(feature = "wef")]
                    if wefhost::mouse_wheel(pos, delta * scale) {
                        continue;
                    }
                    if menu_open {
                        continue;
                    }
                    let d = -delta * scale;
                    self.app.scroll_event(d.x as i32, d.y as i32, pos.x as i32, pos.y as i32);
                }
                egui::Event::Key { key, pressed: true, modifiers, .. } => {
                    // Alt+F / Alt+D open the File / Develop menus, like a native menu bar.
                    if modifiers.alt && key == egui::Key::F {
                        self.open_menu = Some(CaptionItem::File);
                        continue;
                    }
                    if modifiers.alt && key == egui::Key::D {
                        self.open_menu = Some(CaptionItem::Develop);
                        continue;
                    }
                    #[cfg(feature = "wef")]
                    if wefhost::key(key, true, modifiers) {
                        continue;
                    }
                    if egui_has_keyboard || menu_open {
                        continue;
                    }
                    if let Some(code) = key_code(key) {
                        self.app.key_event(code, modifiers.command);
                    }
                }
                #[cfg(feature = "wef")]
                egui::Event::Key { key, pressed: false, modifiers, .. } => {
                    wefhost::key(key, false, modifiers);
                }
                #[cfg(feature = "wef")]
                egui::Event::Cut => _ = wefhost::edit_command(wefhost::EditCommand::Cut),
                #[cfg(feature = "wef")]
                egui::Event::Copy => _ = wefhost::edit_command(wefhost::EditCommand::Copy),
                #[cfg(feature = "wef")]
                egui::Event::Paste(_) => _ = wefhost::edit_command(wefhost::EditCommand::Paste),
                #[cfg(feature = "wef")]
                egui::Event::Text(text) => {
                    if !egui_has_keyboard {
                        wefhost::text(&text);
                    }
                }
                _ => {}
            }
        }
    }

    // ── Painting ──────────────────────────────────────────────────────────

    /// Light caption text over a dark app, dark text over a light one.
    fn caption_text_color(&self, ui: &egui::Ui) -> Color32 {
        // The WTL frame sampled the pixels under the caption; egui cannot read
        // back the frame, so the header's own background colour decides.
        let light_background = self.app.header_luminance().is_some_and(|l| l > 150);
        let _ = ui;
        if light_background {
            Color32::from_rgb(24, 24, 24)
        } else {
            Color32::from_rgb(242, 242, 242)
        }
    }

    fn paint_caption(&self, ctx: &egui::Context, ui: &egui::Ui) {
        let painter = ui.painter();
        let height = Self::caption_height();
        let text_color = self.caption_text_color(ui);
        let light_text = text_color.r() > 128;

        // Transparent items: only hover/pressed states draw a translucent wash.
        for item in CAPTION_ITEMS {
            let active = Some(item) == self.pressed_item || Some(item) == self.open_menu;
            if Some(item) != self.hovered_item && !active {
                continue;
            }
            let rect = self.caption_item_rect(ctx, item);
            let wash = if item == CaptionItem::Close {
                Color32::from_rgba_unmultiplied(196, 43, 28, if active { 200 } else { 255 })
            } else if light_text {
                Color32::from_rgba_unmultiplied(255, 255, 255, if active { 48 } else { 28 })
            } else {
                Color32::from_rgba_unmultiplied(0, 0, 0, if active { 48 } else { 28 })
            };
            painter.rect_filled(rect, 0.0, wash);
        }

        let width = self.size.0 as f32;
        painter.text(
            Pos2::new(width / 2.0, height / 2.0),
            egui::Align2::CENTER_CENTER,
            "Bungalow",
            FontId::proportional(13.0),
            text_color,
        );
        for item in [CaptionItem::File, CaptionItem::Develop] {
            let rect = self.caption_item_rect(ctx, item);
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                item.label(),
                FontId::proportional(12.0),
                text_color,
            );
        }

        let maximized = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
        // The Segoe MDL2 caption glyphs, drawn as strokes (egui's fonts lack them).
        for item in [CaptionItem::Minimize, CaptionItem::Maximize, CaptionItem::Close] {
            let rect = self.caption_item_rect(ctx, item);
            let color =
                if item == CaptionItem::Close && Some(item) == self.hovered_item { Color32::WHITE } else { text_color };
            let stroke = egui::Stroke::new(1.0_f32, color);
            let c = rect.center();
            let half = 5.0;
            match item {
                CaptionItem::Minimize => {
                    painter.line_segment([c + Vec2::new(-half, 0.0), c + Vec2::new(half, 0.0)], stroke);
                }
                CaptionItem::Maximize if maximized => {
                    let back = egui::Rect::from_center_size(c + Vec2::new(2.0, -2.0), Vec2::splat(8.0));
                    let front = egui::Rect::from_center_size(c + Vec2::new(-1.0, 1.0), Vec2::splat(8.0));
                    painter.line_segment([back.left_top(), back.right_top()], stroke);
                    painter.line_segment([back.right_top(), back.right_bottom()], stroke);
                    painter.rect_stroke(front, 0.0, stroke, egui::StrokeKind::Inside);
                }
                CaptionItem::Maximize => {
                    let rect = egui::Rect::from_center_size(c, Vec2::splat(10.0));
                    painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);
                }
                _ => {
                    painter.line_segment([c + Vec2::new(-half, -half), c + Vec2::new(half, half)], stroke);
                    painter.line_segment([c + Vec2::new(-half, half), c + Vec2::new(half, -half)], stroke);
                }
            }
        }
    }

    fn caption_menu_ui(&mut self, ctx: &egui::Context) {
        let Some(item) = self.open_menu else { return };
        let anchor = self.caption_item_rect(ctx, item).left_bottom();
        let mut chosen = None;
        let area = egui::Area::new(egui::Id::new(("caption-menu", item.label())))
            .order(egui::Order::Foreground)
            .fixed_pos(anchor)
            .show(ctx, |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_min_width(160.0);
                    for (command, label) in item.menu() {
                        if ui.add(egui::Button::new(*label).frame(false)).clicked() {
                            chosen = Some(*command);
                        }
                    }
                });
            });
        if let Some(command) = chosen {
            self.open_menu = None;
            self.hovered_item = None;
            self.on_command(ctx, command);
            return;
        }
        let pressed_outside = ctx.input(|i| i.pointer.any_pressed())
            && ctx
                .input(|i| i.pointer.interact_pos())
                .is_some_and(|p| !area.response.rect.contains(p) && !self.caption_item_rect(ctx, item).contains(p));
        if pressed_outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.open_menu = None;
        }
    }

    /// 80% of the monitor, centered, once the monitor size is known.
    fn place_window(&mut self, ctx: &egui::Context) {
        if self.placed {
            return;
        }
        let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) else { return };
        self.placed = true;
        let size = monitor * 0.8;
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
        ctx.send_viewport_cmd(ViewportCommand::OuterPosition(((monitor - size) / 2.0).to_pos2()));
    }
}

fn resize_cursor(direction: ResizeDirection) -> CursorIcon {
    match direction {
        ResizeDirection::North | ResizeDirection::South => CursorIcon::ResizeVertical,
        ResizeDirection::East | ResizeDirection::West => CursorIcon::ResizeHorizontal,
        ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::ResizeNwSe,
        ResizeDirection::NorthEast | ResizeDirection::SouthWest => CursorIcon::ResizeNeSw,
    }
}

impl eframe::App for MainFrame {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.place_window(ctx);

        // The Space lays text out while it starts, and egui has no fonts
        // until its first frame, so the app starts inside that frame.
        if !self.initialized {
            self.initialized = true;
            let screen = ctx.screen_rect();
            self.size = (screen.width() as i32, screen.height() as i32);
            if let Err(error) = self.app.initialize(ctx, self.size.0, self.size.1) {
                log::error!("Bungalow startup error: {error}");
                self.initialization_error = Some(error);
            }
        }

        if let Some(error) = &self.initialization_error {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Unable to initialize the Bungalow user interface.");
                ui.label(error.as_str());
            });
            return;
        }

        let screen = ctx.screen_rect();
        let size = (screen.width() as i32, screen.height() as i32);
        if size != self.size {
            self.size = size;
            self.app.resize(size.0, size.1);
        }

        self.handle_input(ctx);
        let next_timer = self.app.timer();

        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(Color32::from_rgb(38, 43, 54))).show(ctx, |ui| {
            ui.set_clip_rect(screen);
            self.app.paint(ui);
            #[cfg(feature = "wef")]
            wefhost::paint(ui);
            self.paint_caption(ctx, ui);
        });

        self.caption_menu_ui(ctx);
        self.menus.ui(ctx);

        #[cfg(feature = "wef")]
        wefhost::pump();

        if let Some(delay) = next_timer {
            ctx.request_repaint_after(delay);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.app.shutdown();
        platform_ui::set_hooks(None);
        #[cfg(feature = "wef")]
        wefhost::close_all_browsers();
    }
}

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    #[cfg(feature = "wef")]
    {
        // CEF relaunches this executable for its renderer and GPU processes.
        if wefhost::execute_subprocess() {
            return Ok(());
        }
        // On failure app bundles show an error view; the rest of the UI still works.
        wefhost::initialize();
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Bungalow")
            .with_decorations(false)
            .with_inner_size(Vec2::new(1280.0, 800.0))
            .with_min_inner_size(Vec2::new(640.0, 420.0)),
        ..Default::default()
    };
    let result = eframe::run_native("Bungalow", options, Box::new(|cc| Ok(Box::new(MainFrame::new(cc)))));

    #[cfg(feature = "wef")]
    wefhost::shutdown();
    result
}
