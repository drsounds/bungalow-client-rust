//! Port of `PlayerView` (bungalow/): Now Playing. A canvas slot on top
//! (the music visualizer by default), a parameter panel for the active
//! canvas implementation, and the hue/saturation/lightness bar that tints
//! every skin.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use serde_json::{json, Map, Value};

use bungalow::views::config_view::ConfigView;

use space::impl_element_base;
use space::libspace::{SPMargin, SPRectangle};
use space::sp_aero_color_slider::{SPAeroColorSlider, SPAeroSliderType};
use space::sp_color::SPColor;
use space::sp_element::{construct, ElementRef, ElementRefExt, SPElementBase, SPElementClass};
use space::sp_player_canvas_implementation_element::SPPlayerCanvasImplementationElement;
use space::sp_text_element::SPTextElement;
use space::sp_vbox_element::SPVBoxElement;
use space::sp_view_element::{SPViewClass, SPViewElement};
use space::sp_visualization::SPVisualizationParamData;
use space::sp_window_element::SPWindowElement;

use super::sp_music_visualizer_canvas_element::SPMusicVisualizerCanvasElement;
use super::sp_player_canvas_element::SPPlayerCanvasElement;

#[derive(Default)]
struct Parts {
    canvas: Option<Rc<SPPlayerCanvasElement>>,
    /// Default implementation, owned here.
    viz_impl: Option<Rc<SPMusicVisualizerCanvasElement>>,
    param_panel: Option<Rc<SPVBoxElement>>,
    hue_slider: Option<Rc<SPAeroColorSlider>>,
    saturation_slider: Option<Rc<SPAeroColorSlider>>,
    lightness_slider: Option<Rc<SPAeroColorSlider>>,
}

pub struct PlayerView {
    pub view: SPViewElement,
    parts: RefCell<Parts>,
    hue: Cell<f32>,
    saturation: Cell<f32>,
    lightness: Cell<f32>,
}

impl Default for PlayerView {
    fn default() -> Self {
        Self {
            view: SPViewElement::default(),
            parts: RefCell::new(Parts::default()),
            hue: Cell::new(200.0),
            saturation: Cell::new(0.0),
            lightness: Cell::new(0.50),
        }
    }
}

impl_element_base!(PlayerView, view);

fn label(parent: &ElementRef, text: &str, height: i32) -> Rc<SPTextElement> {
    let label = SPTextElement::new(Some(parent));
    label.element().set_inner_text(text);
    label.element().set_height(height);
    parent.append_child(&(label.clone() as ElementRef));
    label
}

impl PlayerView {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        SPViewElement::init_view(&rc.view);
        let el = rc.element();
        el.set_clip_view(false);
        let this = el.this();
        this.set_uri("bungalow:player");

        // ── Top: canvas slot (flex, fills available space) ────────────────
        let canvas = SPPlayerCanvasElement::new(Some(&this));
        canvas.element().set_flex(1);
        this.append_child(&(canvas.clone() as ElementRef));

        // Rebuild the parameter panel whenever the active implementation changes.
        let weak = Rc::downgrade(&rc);
        canvas.element().add_event_listener(
            "implementationChanged",
            Rc::new(move |_, _| {
                if let Some(view) = weak.upgrade() {
                    view.rebuild_param_panel();
                }
                0
            }),
        );

        // ── Visualizer parameter panel (rebuilt dynamically) ──────────────
        let param_panel = SPVBoxElement::new(Some(&this));
        param_panel.element().set_background_color(SPColor::new(25, 25, 25, 255));
        param_panel.element().set_padding(SPMargin::new(8, 12, 8, 12));
        param_panel.element().set_gap(4);
        param_panel.element().set_height(0); // hidden until the active impl provides params
        this.append_child(&(param_panel.clone() as ElementRef));

        // ── Bottom: colour control bar ────────────────────────────────────
        let color_bar = SPVBoxElement::new(Some(&this));
        color_bar.element().set_padding(SPMargin::new(12, 16, 12, 16));
        color_bar.element().set_gap(4);
        color_bar.element().set_height(252);
        color_bar.element().set_background_color(SPColor::new(30, 30, 30, 255));
        let bar: ElementRef = color_bar.clone();
        this.append_child(&bar);

        let make_slider = |text: &str, type_: SPAeroSliderType, set: fn(&PlayerView, f32)| {
            label(&bar, text, 16);
            let slider = SPAeroColorSlider::new(Some(&bar), type_);
            slider.element().set_height(20);
            bar.append_child(&(slider.clone() as ElementRef));
            let weak: Weak<PlayerView> = Rc::downgrade(&rc);
            *slider.on_change.borrow_mut() = Some(Box::new(move |v| {
                if let Some(view) = weak.upgrade() {
                    set(&view, v);
                    view.sync_slider_context();
                    view.apply_live();
                    view.save_settings();
                }
            }));
            slider
        };
        let hue = make_slider("Hue", SPAeroSliderType::Hue, |v, x| v.hue.set(x));
        let saturation = make_slider("Saturation", SPAeroSliderType::Saturation, |v, x| v.saturation.set(x));
        let lightness = make_slider("Lightness", SPAeroSliderType::Lightness, |v, x| v.lightness.set(x));
        lightness.element().set_flex(1);

        {
            let mut parts = rc.parts.borrow_mut();
            parts.canvas = Some(canvas.clone());
            parts.param_panel = Some(param_panel);
            parts.hue_slider = Some(hue);
            parts.saturation_slider = Some(saturation);
            parts.lightness_slider = Some(lightness);
        }

        // Default implementation: music visualizer.
        let canvas_ref: ElementRef = canvas.clone();
        let viz = SPMusicVisualizerCanvasElement::new(Some(&canvas_ref));
        rc.parts.borrow_mut().viz_impl = Some(viz.clone());
        canvas.set_implementation(Some(viz));

        rc.load_settings();
        rc
    }

    /// Swaps the active canvas implementation.
    pub fn set_canvas_implementation(&self, implementation: Option<Rc<dyn SPPlayerCanvasImplementationElement>>) {
        let canvas = self.parts.borrow().canvas.clone();
        if let Some(canvas) = canvas {
            canvas.set_implementation(implementation);
        }
    }

    // ── Settings ──────────────────────────────────────────────────────────

    fn read_settings() -> Option<Map<String, Value>> {
        let text = std::fs::read_to_string(ConfigView::get_settings_path()).ok()?;
        match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(obj)) => Some(obj),
            _ => None,
        }
    }

    fn load_settings(&self) {
        if let Some(obj) = Self::read_settings() {
            if let Some(v) = obj.get("aeroHue").and_then(Value::as_f64) {
                self.hue.set(v as f32);
            }
            if let Some(v) = obj.get("aeroSaturation").and_then(Value::as_f64) {
                self.saturation.set(v as f32);
            }
            if let Some(v) = obj.get("aeroLightness").and_then(Value::as_f64) {
                self.lightness.set(v as f32);
            }
        }
        {
            let parts = self.parts.borrow();
            if let Some(s) = &parts.hue_slider {
                s.set_value(self.hue.get());
            }
            if let Some(s) = &parts.saturation_slider {
                s.set_value(self.saturation.get());
            }
            if let Some(s) = &parts.lightness_slider {
                s.set_value(self.lightness.get());
            }
        }
        self.sync_slider_context();
        self.apply_live();
    }

    /// Merges into the current file rather than rewriting it from a cached
    /// copy, which may be stale (ConfigView saves to the same file) and does
    /// not cover every key.
    fn save_settings(&self) {
        let mut obj = Self::read_settings().unwrap_or_default();
        obj.insert("aeroHue".into(), json!(self.hue.get() as f64));
        obj.insert("aeroSaturation".into(), json!(self.saturation.get() as f64));
        obj.insert("aeroLightness".into(), json!(self.lightness.get() as f64));
        if let Err(e) = std::fs::write(ConfigView::get_settings_path(), Value::Object(obj).to_string()) {
            log::warn!("Cannot save settings: {e}");
        }
    }

    fn apply_live(&self) {
        let Some(window) = self.element().get_window_element() else { return };
        if let Some(skin) = self.element().get_skin() {
            let (h, s, l) = (self.hue.get(), self.saturation.get(), self.lightness.get());
            skin.base().set_hue(h);
            skin.base().set_saturation(s);
            skin.base().set_lightness(l);
            window.element().set_tint_color(Some(SPColor::from_hsl(h, s, l, 255)));
        }
        if let (Some(gc), Some(window)) = (window.create_graphics(), window.downcast::<SPWindowElement>()) {
            window.invalidate_region(SPRectangle::new(0, 0, gc.get_width(), gc.get_height()));
        }
    }

    fn sync_slider_context(&self) {
        let parts = self.parts.borrow();
        if let Some(s) = &parts.saturation_slider {
            s.set_linked_hue(self.hue.get());
        }
        if let Some(s) = &parts.lightness_slider {
            s.set_linked_hue(self.hue.get());
            s.set_linked_sat(self.saturation.get());
        }
    }

    // ── Visualizer parameter panel ────────────────────────────────────────

    fn rebuild_param_panel(&self) {
        let (panel, implementation) = {
            let parts = self.parts.borrow();
            let Some(panel) = parts.param_panel.clone() else { return };
            (panel, parts.canvas.as_ref().and_then(|c| c.get_implementation()))
        };
        let panel_ref: ElementRef = panel.clone();
        panel_ref.clear();
        let this = self.element().this();

        let params = implementation.as_ref().and_then(|i| i.get_visualization_params()).unwrap_or_default();
        let Some(implementation) = implementation.filter(|_| !params.is_empty()) else {
            panel.element().set_height(0);
            this.pack();
            return;
        };

        // Children: [name] [label + control] × N, so the height is
        // padding(8+8) + name(16) + gap×(2N) + N×(label(16)+control(20)).
        panel.element().set_height(32 + 44 * params.len() as i32);

        let name_label = label(&panel_ref, &implementation.get_name(), 16);
        name_label.element().set_foreground_color(SPColor::new(200, 200, 200, 255));
        name_label.element().set_integer_attribute("size", 11);

        for (index, param) in params.iter().enumerate() {
            let param_label = label(&panel_ref, &param.label, 16);
            param_label.element().set_foreground_color(SPColor::new(160, 160, 160, 255));
            param_label.element().set_integer_attribute("size", 10);

            match &param.data {
                SPVisualizationParamData::Float { value, min, max, .. } => {
                    let range = max - min;
                    let normalised = if range > 0.0 { (value - min) / range } else { 0.0 };
                    let slider = SPAeroColorSlider::new(Some(&panel_ref), SPAeroSliderType::Lightness);
                    slider.element().set_height(20);
                    slider.set_value(normalised);
                    let target = Rc::downgrade(&implementation);
                    let param = param.clone();
                    *slider.on_change.borrow_mut() = Some(Box::new(move |v| {
                        let Some(target) = target.upgrade() else { return };
                        let mut updated = param.clone();
                        if let SPVisualizationParamData::Float { value, min, max, .. } = &mut updated.data {
                            *value = *min + v * (*max - *min);
                        }
                        target.set_visualization_param(index, updated);
                    }));
                    panel_ref.append_child(&(slider as ElementRef));
                }
                SPVisualizationParamData::Bool { value } => {
                    let text = label(&panel_ref, if *value { "Enabled" } else { "Disabled" }, 20);
                    text.element().set_foreground_color(SPColor::new(130, 200, 130, 255));
                    text.element().set_integer_attribute("size", 10);
                }
                SPVisualizationParamData::Enum { value, options } => {
                    let current = if options.is_empty() {
                        String::new()
                    } else {
                        options[(*value as usize) % options.len()].clone()
                    };
                    let text = label(&panel_ref, &current, 20);
                    text.element().set_foreground_color(SPColor::new(130, 180, 220, 255));
                    text.element().set_integer_attribute("size", 10);
                }
                SPVisualizationParamData::Color { value } => {
                    let swatch = label(&panel_ref, "", 20);
                    swatch.element().set_background_color(*value);
                }
            }
        }

        this.pack();
        self.element().invalidate();
    }
}

impl SPElementClass for PlayerView {
    fn get_type(&self) -> String {
        "playerview".into()
    }
    view_element_overrides!();

    fn set_visible(&self, value: bool) {
        self.element().set_visible_default(value);
        let canvas = self.parts.borrow().canvas.clone();
        if let Some(canvas) = canvas {
            canvas.set_visible(value);
        }
        // Settings may have been changed from ConfigView since we last loaded.
        if value {
            self.load_settings();
        }
    }
}

impl SPViewClass for PlayerView {
    fn view(&self) -> &SPViewElement {
        &self.view
    }
    fn accepts_uri(&self, uri: &str) -> bool {
        uri == "bungalow:player" || uri.starts_with("bungalow:player:")
    }
    fn navigate(&self, uri: &str) {
        self.element().this().set_uri(uri);
    }
}
