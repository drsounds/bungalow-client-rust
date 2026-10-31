//! Port of `Win32GraphicsContext`: draws the Space element tree with an
//! egui [`Painter`] instead of GDI/GDI+.
//!
//! Space coordinates are egui points, so the UI scales with the display's
//! pixels-per-point. Text inputs are egui `TextEdit`s laid over the painted
//! tree after each frame, the counterpart of the Win32 EDIT child windows.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, TextureHandle, Vec2};

use space::libspace::SPRectangle;
use space::sp_color::{SPColor, SPGradient};
use space::sp_element::{ElementRef, ElementRefExt, WeakElementRef};
use space::sp_font_style::SPFontStyle;
use space::sp_graphics_context::{RequestCallback, SPGraphicsContext};
use space::sp_image::SPImage;
use space::sp_input_element::SPInputElement;
use space::sp_input_event_args::SPInputEventArgs;
use space::sp_object::SPObject;

/// An input element painted during the current frame, and where.
struct PaintedInput {
    element: WeakElementRef,
    rect: Rect,
}

/// The text an input element is being edited with, keyed by element.
struct InputState {
    element: WeakElementRef,
    text: String,
}

pub struct EguiGraphicsContext {
    ctx: egui::Context,
    painter: RefCell<Option<Painter>>,
    size: Cell<(i32, i32)>,
    origin: Cell<(i32, i32)>,
    clip_stack: RefCell<Vec<Rect>>,
    frozen: Cell<bool>,
    textures: RefCell<HashMap<u64, TextureHandle>>,
    painted_inputs: RefCell<Vec<PaintedInput>>,
    inputs: RefCell<Vec<InputState>>,
}

fn color32(color: SPColor) -> Color32 {
    let c = |v: i16| v.clamp(0, 255) as u8;
    Color32::from_rgba_unmultiplied(c(color.get_r()), c(color.get_g()), c(color.get_b()), c(color.get_a()))
}

fn font_id(font: &SPFontStyle) -> FontId {
    let size = font.height.max(1) as f32;
    if font.family.to_ascii_lowercase().contains("mono") || font.family.eq_ignore_ascii_case("courier") {
        FontId::monospace(size)
    } else {
        FontId::proportional(size)
    }
}

fn corner(radius: i32) -> CornerRadius {
    CornerRadius::same(radius.clamp(0, 255) as u8)
}

impl EguiGraphicsContext {
    pub fn new(ctx: egui::Context) -> Self {
        Self {
            ctx,
            painter: RefCell::new(None),
            size: Cell::new((0, 0)),
            origin: Cell::new((0, 0)),
            clip_stack: RefCell::new(Vec::new()),
            frozen: Cell::new(false),
            textures: RefCell::new(HashMap::new()),
            painted_inputs: RefCell::new(Vec::new()),
            inputs: RefCell::new(Vec::new()),
        }
    }

    pub fn set_size(&self, width: i32, height: i32) {
        self.size.set((width, height));
    }

    /// `setDeviceContext` + `beginPaint`: the painter used until `end_paint`.
    pub fn begin_paint(&self, painter: Painter) {
        *self.painter.borrow_mut() = Some(painter);
        self.origin.set((0, 0));
        self.clip_stack.borrow_mut().clear();
        self.painted_inputs.borrow_mut().clear();
    }

    /// `endPaint`: lays the text inputs painted this frame over the tree.
    /// Inputs of hidden elements were not painted, so they are not shown.
    pub fn end_paint(&self, ui: &mut egui::Ui) {
        *self.painter.borrow_mut() = None;
        let painted: Vec<(ElementRef, Rect)> = self
            .painted_inputs
            .borrow_mut()
            .drain(..)
            .filter_map(|p| p.element.upgrade().map(|e| (e, p.rect)))
            .collect();
        self.inputs.borrow_mut().retain(|s| s.element.strong_count() > 0);
        for (element, rect) in painted {
            self.show_input(ui, &element, rect);
        }
    }

    fn show_input(&self, ui: &mut egui::Ui, element: &ElementRef, rect: Rect) {
        let Some(input) = element.downcast::<SPInputElement>() else { return };
        let id = egui::Id::new(("sp-input", Rc::as_ptr(element) as *const () as usize));
        let has_focus = ui.memory(|m| m.has_focus(id));
        let mut text = {
            let mut inputs = self.inputs.borrow_mut();
            let index = match inputs.iter().position(|s| s.element.ptr_eq(&Rc::downgrade(element))) {
                Some(index) => index,
                None => {
                    inputs.push(InputState { element: Rc::downgrade(element), text: input.get_value() });
                    inputs.len() - 1
                }
            };
            // Like the EDIT control, show the element's value unless the user is typing.
            if !has_focus {
                inputs[index].text = input.get_value();
            }
            inputs[index].text.clone()
        };
        let edit = egui::TextEdit::singleline(&mut text)
            .id(id)
            .font(FontId::proportional(13.0))
            .margin(Vec2::new(4.0, 2.0))
            .vertical_align(egui::Align::Center);
        let response = ui.put(rect, edit);
        if let Some(state) = self.inputs.borrow_mut().iter_mut().find(|s| s.element.ptr_eq(&Rc::downgrade(element))) {
            state.text = text.clone();
        }
        if response.changed() {
            // EN_CHANGE -> setValue
            input.set_value(&text);
        }
        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let args = SPInputEventArgs::new(&text);
            element.notify("enter", Some(&SPObject::Element(element.clone())), &args);
        }
    }

    fn offset(&self, x: i32, y: i32) -> Pos2 {
        let (ox, oy) = self.origin.get();
        Pos2::new((x + ox) as f32, (y + oy) as f32)
    }

    fn rect(&self, x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect::from_min_size(self.offset(x, y), Vec2::new(width as f32, height as f32))
    }

    fn with_painter(&self, f: impl FnOnce(&Painter)) {
        if let Some(painter) = self.painter.borrow().as_ref() {
            f(painter);
        }
    }

    fn texture_for(&self, image: &SPImage) -> Option<TextureHandle> {
        if image.width == 0 || image.height == 0 {
            return None;
        }
        let mut textures = self.textures.borrow_mut();
        if let Some(handle) = textures.get(&image.id) {
            return Some(handle.clone());
        }
        let mut rgba = Vec::with_capacity(image.pixels.len() * 4);
        for p in &image.pixels {
            rgba.extend_from_slice(&[p.r, p.g, p.b, p.a]);
        }
        let color_image =
            egui::ColorImage::from_rgba_unmultiplied([image.width as usize, image.height as usize], &rgba);
        let handle = self.ctx.load_texture(format!("sp-image-{}", image.id), color_image, egui::TextureOptions::LINEAR);
        textures.insert(image.id, handle.clone());
        Some(handle)
    }

    /// A rounded rectangle outline, clockwise from the top-left arc.
    fn rounded_outline(rect: Rect, radius: f32) -> Vec<Pos2> {
        let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0).max(0.0);
        if r <= 0.5 {
            return vec![rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()];
        }
        let mut points = Vec::new();
        let corners = [
            (Pos2::new(rect.left() + r, rect.top() + r), 180.0f32),
            (Pos2::new(rect.right() - r, rect.top() + r), 270.0),
            (Pos2::new(rect.right() - r, rect.bottom() - r), 0.0),
            (Pos2::new(rect.left() + r, rect.bottom() - r), 90.0),
        ];
        for (center, start) in corners {
            for step in 0..=6 {
                let angle = (start + 15.0 * step as f32).to_radians();
                points.push(Pos2::new(center.x + r * angle.cos(), center.y + r * angle.sin()));
            }
        }
        points
    }

    /// Fills `rect` (optionally rounded) with a linear gradient, as a mesh
    /// whose vertex colours interpolate along the gradient direction.
    fn fill_gradient(&self, rect: Rect, radius: f32, gradient: &SPGradient) {
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }
        let horizontal = gradient.get_direction() != 0;
        let (start, end) = (color32(gradient.get_start()), color32(gradient.get_end()));
        let color_at = |p: Pos2| {
            let t = if horizontal { (p.x - rect.left()) / rect.width() } else { (p.y - rect.top()) / rect.height() };
            start.lerp_to_gamma(end, t.clamp(0.0, 1.0))
        };
        let outline = Self::rounded_outline(rect, radius);
        let mut mesh = Mesh::default();
        let center = rect.center();
        mesh.vertices.push(Vertex { pos: center, uv: WHITE_UV, color: color_at(center) });
        for p in &outline {
            mesh.vertices.push(Vertex { pos: *p, uv: WHITE_UV, color: color_at(*p) });
        }
        let n = outline.len() as u32;
        for i in 0..n {
            mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
        }
        self.with_painter(|p| {
            p.add(egui::Shape::mesh(mesh));
        });
    }
}

impl SPGraphicsContext for EguiGraphicsContext {
    fn request(&self, method: &str, uri: &str, data: &str, callback: RequestCallback) {
        let request = ureq::request(method, uri).set("Content-Type", "application/json");
        let response = if data.is_empty() { request.call() } else { request.send_string(data) };
        match response {
            Ok(response) if (200..300).contains(&response.status()) => match response.into_string() {
                Ok(body) => callback(true, body),
                Err(_) => callback(false, String::new()),
            },
            _ => callback(false, String::new()),
        }
    }

    fn set_control_attribute(&self, element: &ElementRef, name: &str, value: &str) {
        if name != "value" && name != "text" {
            return;
        }
        let weak = Rc::downgrade(element);
        if let Some(state) = self.inputs.borrow_mut().iter_mut().find(|s| s.element.ptr_eq(&weak)) {
            if state.text != value {
                state.text = value.to_string();
            }
        }
    }

    fn draw_image_part(
        &self,
        image: &SPImage,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
    ) {
        // Like AlphaBlend: (x1, y1) is the source origin, (x2, y2) the source size.
        let Some(texture) = self.texture_for(image) else { return };
        let (w, h) = (image.width as f32, image.height as f32);
        let uv = Rect::from_min_max(
            Pos2::new(x1 as f32 / w, y1 as f32 / h),
            Pos2::new((x1 + x2) as f32 / w, (y1 + y2) as f32 / h),
        );
        let rect = self.rect(x, y, width, height);
        self.with_painter(|p| {
            p.image(texture.id(), rect, uv, Color32::WHITE);
        });
    }

    fn restore_origo(&self) {
        self.origin.set((0, 0));
    }

    fn set_origo(&self, x: i32, y: i32) {
        self.origin.set((x, y));
    }

    fn set_clip(&self, rect: &SPRectangle) {
        let clip = self.rect(rect.x, rect.y, rect.width, rect.height);
        let mut painter = self.painter.borrow_mut();
        let Some(painter) = painter.as_mut() else { return };
        let current = painter.clip_rect();
        self.clip_stack.borrow_mut().push(current);
        painter.set_clip_rect(current.intersect(clip));
    }

    fn declip_rect(&self) {
        let Some(previous) = self.clip_stack.borrow_mut().pop() else { return };
        if let Some(painter) = self.painter.borrow_mut().as_mut() {
            painter.set_clip_rect(previous);
        }
    }

    fn draw_line(&self, x: i32, y: i32, x2: i32, y2: i32, color: SPColor) {
        if color.get_a() <= 0 {
            return;
        }
        // Axis-aligned lines are 1px solid fills, as in the Win32 backend.
        if x == x2 || y == y2 {
            self.fill_rectangle(x.min(x2), y.min(y2), (x2 - x).abs() + 1, (y2 - y).abs() + 1, color);
            return;
        }
        let (a, b) = (self.offset(x, y) + Vec2::splat(0.5_f32), self.offset(x2, y2) + Vec2::splat(0.5_f32));
        self.with_painter(|p| {
            p.line_segment([a, b], Stroke::new(1.0_f32, color32(color)));
        });
    }

    fn draw_rectangle(&self, x: i32, y: i32, w: i32, h: i32, color: SPColor) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.fill_rectangle(x, y, w, 1, color);
        if h > 1 {
            self.fill_rectangle(x, y + h - 1, w, 1, color);
        }
        if h > 2 {
            self.fill_rectangle(x, y + 1, 1, h - 2, color);
            if w > 1 {
                self.fill_rectangle(x + w - 1, y + 1, 1, h - 2, color);
            }
        }
    }

    fn fill_rectangle(&self, x: i32, y: i32, w: i32, h: i32, color: SPColor) {
        if w <= 0 || h <= 0 || color.get_a() <= 0 {
            return;
        }
        let rect = self.rect(x, y, w, h);
        self.with_painter(|p| {
            p.rect_filled(rect, CornerRadius::ZERO, color32(color));
        });
    }

    fn fill_rounded_rectangle(&self, x: i32, y: i32, w: i32, h: i32, r: i32, color: SPColor) {
        if w <= 0 || h <= 0 || color.get_a() <= 0 {
            return;
        }
        let rect = self.rect(x, y, w, h);
        let r = r.min(w / 2).min(h / 2);
        self.with_painter(|p| {
            p.rect_filled(rect, corner(r), color32(color));
        });
    }

    fn draw_rounded_rectangle(&self, x: i32, y: i32, w: i32, h: i32, r: i32, color: SPColor) {
        if w <= 1 || h <= 1 {
            return;
        }
        if r <= 0 {
            self.draw_rectangle(x, y, w, h, color);
            return;
        }
        let rect = self.rect(x, y, w, h);
        let r = r.min(w / 2).min(h / 2);
        self.with_painter(|p| {
            p.rect_stroke(rect, corner(r), Stroke::new(1.0_f32, color32(color)), StrokeKind::Inside);
        });
    }

    fn fill_rounded_linear_gradient_rectangle(
        &self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        r: i32,
        _dir: i32,
        gradient: &SPGradient,
    ) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.fill_gradient(self.rect(x, y, w, h), r.max(0) as f32, gradient);
    }

    fn draw_image(&self, image: &SPImage, x: i32, y: i32, w: i32, h: i32) {
        self.draw_image_part(image, x, y, w, h, 0, 0, image.width as i32, image.height as i32);
    }

    fn release_image(&self, image: &SPImage) {
        self.textures.borrow_mut().remove(&image.id);
    }

    fn draw_control(&self, name: &str, element: &ElementRef, x: i32, y: i32, w: i32, h: i32) {
        if name == "input" {
            self.draw_input_element(element, x, y, w, h);
        } else {
            let el = element.el();
            self.draw_string(&el.get_inner_text(), &el.get_font_style(), el.get_foreground_color(), x, y, w, h);
        }
    }

    fn draw_string(&self, text: &str, font: &SPFontStyle, color: SPColor, x: i32, y: i32, _w: i32, _h: i32) {
        if text.is_empty() {
            return;
        }
        // DT_LEFT | DT_TOP | DT_NOCLIP: no wrapping, no clipping to the box.
        let pos = self.offset(x, y);
        let font_id = font_id(font);
        let color = color32(color);
        let underline = font.underline;
        self.with_painter(|p| {
            let rect = p.text(pos, egui::Align2::LEFT_TOP, text, font_id, color);
            if underline {
                p.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0_f32, color));
            }
        });
    }

    fn draw_html_text(&self, html: &str, font: &SPFontStyle, color: SPColor, x: i32, y: i32, w: i32, h: i32) {
        self.draw_string(html, font, color, x, y, w, h);
    }

    fn invalidate_region(&self, _region: SPRectangle) {
        if !self.frozen.get() {
            self.ctx.request_repaint();
        }
    }

    fn draw_linear_gradient(&self, rect: &SPRectangle, gradient: &SPGradient) {
        if rect.width <= 0 || rect.height <= 0 {
            return;
        }
        self.fill_gradient(self.rect(rect.x, rect.y, rect.width, rect.height), 0.0, gradient);
    }

    fn load_image(&self, path: &str) -> Option<Rc<SPImage>> {
        SPImage::from_file(path).ok().map(Rc::new)
    }

    fn freeze(&self) {
        self.frozen.set(true);
    }

    fn thaw(&self) {
        self.frozen.set(false);
        self.ctx.request_repaint();
    }

    fn measure_string(&self, text: &str, font: &SPFontStyle) -> SPRectangle {
        let font_id = font_id(font);
        let size = self.ctx.fonts(|f| f.layout_no_wrap(text.to_string(), font_id, Color32::WHITE).size());
        SPRectangle::new(0, 0, size.x.ceil() as i32, size.y.ceil() as i32)
    }

    fn get_width(&self) -> i32 {
        self.size.get().0
    }

    fn get_height(&self) -> i32 {
        self.size.get().1
    }

    fn draw_input_element(&self, element: &ElementRef, x: i32, y: i32, w: i32, h: i32) {
        let rect = self.rect(x, y, w, h);
        let weak = Rc::downgrade(element);
        let mut painted = self.painted_inputs.borrow_mut();
        if let Some(existing) = painted.iter_mut().find(|p| p.element.ptr_eq(&weak)) {
            existing.rect = rect;
        } else {
            painted.push(PaintedInput { element: weak, rect });
        }
    }
}
