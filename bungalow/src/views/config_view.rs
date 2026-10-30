//! Port of `ConfigView`: the bungalow:config settings page. Settings are kept
//! in `<user data dir>/settings.json` and applied to the window on save.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use serde_json::{json, Map, Value};
use space::impl_element_base;
use space::libspace::{SPMargin, SPRectangle};
use space::sp_aero_color_slider::{SPAeroColorSlider, SPAeroSliderType};
use space::sp_app_manager::SPAppManager;
use space::sp_button_element::SPButtonElement;
use space::sp_classic_skin::SPClassicSkin;
use space::sp_color::SPColor;
use space::sp_color_chooser::SPColorChooser;
use space::sp_element::{
    construct, ElementRef, SPElementBase, SPElementClass, SP_COLOR_SCHEME_DARK, SP_COLOR_SCHEME_LIGHT,
};
use space::sp_frutiger_aero_skin::{AeroHeaderStyle, SPFrutigerAeroSkin};
use space::sp_hbox_element::SPHBoxElement;
use space::sp_input_element::SPInputElement;
use space::sp_modern_skin::SPModernSkin;
use space::sp_object::SPObject;
use space::sp_skin::{SPButtonStyle, SPSkin};
use space::sp_text_element::SPTextElement;
use space::sp_vbox_element::SPVBoxElement;
use space::sp_view_element::{SPViewClass, SPViewElement};
use space::sp_window_element::SPWindowElement;

use crate::bungalow_app_handle;
use crate::download_manager::DownloadManager;
use crate::platform_ui;

const SELECTED_COLOR: SPColor = SPColor::new(60, 100, 220, 220);
const DESELECTED_COLOR: SPColor = SPColor::new(160, 160, 160, 255);

fn apply_toggle_button(btn: &Option<Rc<SPButtonElement>>, active: bool, on_label: &str, off_label: &str) {
    if let Some(btn) = btn {
        btn.el.set_inner_text(if active { on_label } else { off_label });
        btn.el.set_background_color(if active { SELECTED_COLOR } else { DESELECTED_COLOR });
    }
}

fn set_selected(btn: &Option<Rc<SPButtonElement>>, selected: bool) {
    if let Some(btn) = btn {
        btn.el.set_background_color(if selected { SELECTED_COLOR } else { DESELECTED_COLOR });
    }
}

fn make_label(parent: &ElementRef, text: &str, height: i32) -> Rc<SPTextElement> {
    let lbl = SPTextElement::new(Some(parent));
    lbl.el.set_inner_text(text);
    lbl.el.set_height(height);
    parent.append_child(&(lbl.clone() as ElementRef));
    lbl
}

fn make_button(parent: &ElementRef, label: &str, width: i32, height: i32) -> Rc<SPButtonElement> {
    let btn = SPButtonElement::new(Some(parent));
    btn.el.set_inner_text(label);
    btn.el.set_width(width);
    btn.el.set_height(height);
    parent.append_child(&(btn.clone() as ElementRef));
    btn
}

fn make_row(parent: &ElementRef, gap: i32) -> ElementRef {
    let row: ElementRef = SPHBoxElement::new(Some(parent));
    row.element().set_height(28);
    row.element().set_gap(gap);
    row
}

#[derive(Default)]
struct Widgets {
    color_chooser: Option<Rc<SPColorChooser>>,
    save_button: Option<Rc<SPButtonElement>>,
    light_button: Option<Rc<SPButtonElement>>,
    dark_button: Option<Rc<SPButtonElement>>,
    glossy_button: Option<Rc<SPButtonElement>>,
    downloads_folder_input: Option<Rc<SPInputElement>>,
    music_folder_input: Option<Rc<SPInputElement>>,
    browse_downloads_btn: Option<Rc<SPButtonElement>>,
    browse_music_btn: Option<Rc<SPButtonElement>>,
    developer_mode_button: Option<Rc<SPButtonElement>>,
    classic_skin_button: Option<Rc<SPButtonElement>>,
    modern_skin_button: Option<Rc<SPButtonElement>>,
    frutiger_aero_skin_button: Option<Rc<SPButtonElement>>,
    /// Skins from app bundles other than the default modern skin, with their skin type.
    bundle_skin_buttons: Vec<(Rc<SPButtonElement>, String)>,
    full_width_address_bar_button: Option<Rc<SPButtonElement>>,

    aero_header_solid_btn: Option<Rc<SPButtonElement>>,
    aero_header_gradient_btn: Option<Rc<SPButtonElement>>,
    aero_header_glossy_btn: Option<Rc<SPButtonElement>>,
    aero_header_accent_glossy_btn: Option<Rc<SPButtonElement>>,
    aero_header_dark_glossy_btn: Option<Rc<SPButtonElement>>,

    btn_style_flat_btn: Option<Rc<SPButtonElement>>,
    btn_style_gradient_btn: Option<Rc<SPButtonElement>>,
    btn_style_modern_grad_btn: Option<Rc<SPButtonElement>>,
    btn_style_aqua_btn: Option<Rc<SPButtonElement>>,

    hue_slider: Option<Rc<SPAeroColorSlider>>,
    saturation_slider: Option<Rc<SPAeroColorSlider>>,
    lightness_slider: Option<Rc<SPAeroColorSlider>>,

    /// Hidden unless the frutiger-aero skin is selected.
    aero_section: Option<ElementRef>,
}

struct Pending {
    scheme: Cell<i16>,
    glossy: Cell<bool>,
    developer_mode: Cell<bool>,
    skin_type: RefCell<String>,
    full_width_address_bar: Cell<bool>,
    aero_hue: Cell<f32>,
    aero_saturation: Cell<f32>,
    aero_lightness: Cell<f32>,
    aero_header_style: Cell<AeroHeaderStyle>,
    button_style: Cell<SPButtonStyle>,
}

impl Default for Pending {
    fn default() -> Self {
        Self {
            scheme: Cell::new(SP_COLOR_SCHEME_LIGHT),
            glossy: Cell::new(false),
            developer_mode: Cell::new(false),
            skin_type: RefCell::new("modern".into()),
            full_width_address_bar: Cell::new(true),
            aero_hue: Cell::new(200.0),
            aero_saturation: Cell::new(0.0),
            aero_lightness: Cell::new(0.50),
            aero_header_style: Cell::new(AeroHeaderStyle::Glossy),
            button_style: Cell::new(SPButtonStyle::Aqua),
        }
    }
}

#[derive(Default)]
pub struct ConfigView {
    pub view: SPViewElement,
    w: RefCell<Widgets>,
    pending: Pending,
}

impl_element_base!(ConfigView, view);

/// Adds a click listener that calls `f` on the view.
fn on_click(btn: &Rc<SPButtonElement>, view: &Weak<ConfigView>, f: fn(&ConfigView, Option<&SPObject>)) {
    let view = view.clone();
    btn.el.add_event_listener(
        "click",
        Rc::new(move |sender, _e| {
            if let Some(view) = view.upgrade() {
                f(&view, sender);
            }
            0
        }),
    );
}

impl ConfigView {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        SPViewElement::init_view(&rc.view);
        let weak = Rc::downgrade(&rc);
        let el = rc.element();
        let this = el.this();
        el.set_clip_view(false);
        this.set_uri("bungalow:config");
        el.set_padding(SPMargin::new(16, 16, 16, 16));
        el.set_gap(8);

        let mut w = Widgets::default();
        make_label(&this, "Settings", 36);

        // ── Appearance section ─────────────────────────────────────────────
        make_label(&this, "Appearance", 20);
        let mode_row = make_row(&this, 6);
        this.append_child(&mode_row);

        let light = make_button(&mode_row, "Light Mode", 90, 28);
        on_click(&light, &weak, |v, _| v.on_scheme_clicked(SP_COLOR_SCHEME_LIGHT));
        w.light_button = Some(light);

        let dark = make_button(&mode_row, "Dark Mode", 90, 28);
        on_click(&dark, &weak, |v, _| v.on_scheme_clicked(SP_COLOR_SCHEME_DARK));
        w.dark_button = Some(dark);

        let glossy = make_button(&this, "Glossy: On", 110, 28);
        on_click(&glossy, &weak, |v, _| v.on_glossy_clicked());
        w.glossy_button = Some(glossy);

        // ── Button Style section (built but not shown, like the C++ view) ──
        let btn_style_row = make_row(&this, 5);
        let styles = [
            ("Flat", 56, SPButtonStyle::Flat),
            ("Gradient", 80, SPButtonStyle::Gradient),
            ("Modern Grad", 100, SPButtonStyle::ModernGradient),
            ("Aqua", 60, SPButtonStyle::Aqua),
        ];
        let mut style_buttons = Vec::new();
        for (label, width, style) in styles {
            let btn = make_button(&btn_style_row, label, width, 28);
            let view = weak.clone();
            btn.el.add_event_listener(
                "click",
                Rc::new(move |_, _| {
                    if let Some(view) = view.upgrade() {
                        view.on_button_style_clicked(style);
                    }
                    0
                }),
            );
            style_buttons.push(btn);
        }
        let mut style_buttons = style_buttons.into_iter();
        w.btn_style_flat_btn = style_buttons.next();
        w.btn_style_gradient_btn = style_buttons.next();
        w.btn_style_modern_grad_btn = style_buttons.next();
        w.btn_style_aqua_btn = style_buttons.next();

        // ── Tint Color (built but not shown) ──────────────────────────────
        let chooser = SPColorChooser::new(Some(&this));
        {
            // When a tint swatch is picked, sync H+S into the Aero sliders.
            // Lightness is kept so the user's brightness setting is preserved.
            let view = weak.clone();
            *chooser.on_change.borrow_mut() = Some(Box::new(move |c: &SPColor| {
                if let Some(view) = view.upgrade() {
                    let (h, s, _l) = c.to_hsl();
                    view.pending.aero_hue.set(h);
                    view.pending.aero_saturation.set(s);
                    let w = view.w.borrow();
                    if let Some(slider) = &w.hue_slider {
                        slider.set_value(h);
                    }
                    if let Some(slider) = &w.saturation_slider {
                        slider.set_value(s);
                    }
                    drop(w);
                    view.sync_slider_context();
                    view.element().invalidate();
                }
            }));
        }
        w.color_chooser = Some(chooser);

        // ── Skin section ───────────────────────────────────────────────────
        make_label(&this, "Skin", 20);
        let skin_row = make_row(&this, 6);
        this.append_child(&skin_row);

        let classic = make_button(&skin_row, "Classic", 76, 28);
        on_click(&classic, &weak, |v, _| v.on_skin_clicked("classic"));
        w.classic_skin_button = Some(classic);

        let modern = make_button(&skin_row, "Modern", 76, 28);
        on_click(&modern, &weak, |v, _| v.on_skin_clicked("modern"));
        w.modern_skin_button = Some(modern);

        let aero = make_button(&skin_row, "Frutiger Aero", 120, 28);
        on_click(&aero, &weak, |v, _| v.on_skin_clicked("frutiger-aero"));
        w.frutiger_aero_skin_button = Some(aero);

        // Additional skins installed as app bundles. modern.skin is the "Modern" button.
        let extensions: Vec<(String, String)> = SPAppManager::with(|m| {
            m.get_extensions("Skin")
                .into_iter()
                .filter(|ext| ext.identifier != "modern.skin")
                .map(|ext| (ext.identifier.clone(), ext.get_name("en")))
                .collect()
        });
        for (identifier, name) in extensions {
            let btn = make_button(&skin_row, &name, name.len() as i32 * 7 + 24, 28);
            on_click(&btn, &weak, |v, sender| v.on_bundle_skin_clicked(sender));
            w.bundle_skin_buttons.push((btn, identifier));
        }

        // ── Frutiger Aero options (hidden unless aero skin active) ─────────
        let aero_section: ElementRef = SPVBoxElement::new(Some(&this));
        aero_section.element().set_gap(8);
        this.append_child(&aero_section);

        make_label(&aero_section, "Aero Header / Footer Style", 18);
        // The C++ view parents this row to the aero section but appends it to the view.
        let header_style_row = make_row(&aero_section, 4);
        this.append_child(&header_style_row);

        let header_styles = [
            ("Solid", 56, AeroHeaderStyle::Solid),
            ("WMP9", 56, AeroHeaderStyle::Gradient),
            ("WMP10", 60, AeroHeaderStyle::Glossy),
            ("WMP11b", 68, AeroHeaderStyle::AccentGlossy),
            ("WMP11", 60, AeroHeaderStyle::DarkGlossy),
        ];
        let mut header_buttons = Vec::new();
        for (label, width, style) in header_styles {
            let btn = make_button(&header_style_row, label, width, 28);
            let view = weak.clone();
            btn.el.add_event_listener(
                "click",
                Rc::new(move |_, _| {
                    if let Some(view) = view.upgrade() {
                        view.on_aero_header_style_clicked(style);
                    }
                    0
                }),
            );
            header_buttons.push(btn);
        }
        let mut header_buttons = header_buttons.into_iter();
        w.aero_header_solid_btn = header_buttons.next();
        w.aero_header_gradient_btn = header_buttons.next();
        w.aero_header_glossy_btn = header_buttons.next();
        w.aero_header_accent_glossy_btn = header_buttons.next();
        w.aero_header_dark_glossy_btn = header_buttons.next();

        // The sliders are created but, as in the C++ view, not appended.
        make_label(&aero_section, "Hue", 18);
        let hue = SPAeroColorSlider::new(Some(&aero_section), SPAeroSliderType::Hue);
        hue.el.set_height(20);
        {
            let view = weak.clone();
            *hue.on_change.borrow_mut() = Some(Box::new(move |v| {
                if let Some(view) = view.upgrade() {
                    view.pending.aero_hue.set(v);
                    view.on_hue_or_saturation_changed();
                }
            }));
        }
        w.hue_slider = Some(hue);

        make_label(&aero_section, "Saturation", 18);
        let saturation = SPAeroColorSlider::new(Some(&aero_section), SPAeroSliderType::Saturation);
        saturation.el.set_height(20);
        {
            let view = weak.clone();
            *saturation.on_change.borrow_mut() = Some(Box::new(move |v| {
                if let Some(view) = view.upgrade() {
                    view.pending.aero_saturation.set(v);
                    view.on_hue_or_saturation_changed();
                }
            }));
        }
        w.saturation_slider = Some(saturation);

        make_label(&aero_section, "Lightness", 18);
        let lightness = SPAeroColorSlider::new(Some(&aero_section), SPAeroSliderType::Lightness);
        lightness.el.set_height(20);
        {
            let view = weak.clone();
            *lightness.on_change.borrow_mut() = Some(Box::new(move |v| {
                if let Some(view) = view.upgrade() {
                    view.pending.aero_lightness.set(v);
                    view.element().invalidate();
                }
            }));
        }
        w.lightness_slider = Some(lightness);
        w.aero_section = Some(aero_section);

        // ── Address bar section ────────────────────────────────────────────
        make_label(&this, "Address Bar", 20);
        let full_width = make_button(&this, "Full Width: On", 140, 28);
        on_click(&full_width, &weak, |v, _| v.on_full_width_address_bar_clicked());
        w.full_width_address_bar_button = Some(full_width);

        // ── Developer section ──────────────────────────────────────────────
        make_label(&this, "Developer", 20);
        let developer = make_button(&this, "Developer Mode: Off", 160, 28);
        on_click(&developer, &weak, |v, _| v.on_developer_mode_clicked());
        w.developer_mode_button = Some(developer);

        // ── Save button ────────────────────────────────────────────────────
        let save = make_button(&this, "Save Changes", 120, 28);
        on_click(&save, &weak, |v, _| v.save_settings());
        w.save_button = Some(save);

        // ── Downloads Folder section ───────────────────────────────────────
        make_label(&this, "Downloads Folder", 20);
        let dl_row = make_row(&this, 6);
        this.append_child(&dl_row);
        let dl_input = SPInputElement::new(Some(&dl_row));
        dl_input.element().set_flex(1);
        dl_input.element().set_height(28);
        dl_row.append_child(&(dl_input.clone() as ElementRef));
        w.downloads_folder_input = Some(dl_input);
        let browse_dl = make_button(&dl_row, "Browse…", 72, 28);
        on_click(&browse_dl, &weak, |v, _| v.on_browse_downloads_clicked());
        w.browse_downloads_btn = Some(browse_dl);

        // ── Music Folder section ───────────────────────────────────────────
        make_label(&this, "Music Folder", 20);
        let music_row = make_row(&this, 6);
        this.append_child(&music_row);
        let music_input = SPInputElement::new(Some(&music_row));
        music_input.element().set_flex(1);
        music_input.element().set_height(28);
        music_row.append_child(&(music_input.clone() as ElementRef));
        w.music_folder_input = Some(music_input);
        let browse_music = make_button(&music_row, "Browse…", 72, 28);
        on_click(&browse_music, &weak, |v, _| v.on_browse_music_clicked());
        w.browse_music_btn = Some(browse_music);

        *rc.w.borrow_mut() = w;
        rc.load_settings();
        rc
    }

    pub fn get_settings_path() -> String {
        format!("{}/settings.json", platform_ui::user_data_dir())
    }

    // ── Button state helpers ──────────────────────────────────────────────

    fn update_mode_buttons(&self) {
        let w = self.w.borrow();
        let scheme = self.pending.scheme.get();
        set_selected(&w.light_button, scheme == SP_COLOR_SCHEME_LIGHT);
        set_selected(&w.dark_button, scheme == SP_COLOR_SCHEME_DARK);
    }

    fn update_glossy_button(&self) {
        apply_toggle_button(&self.w.borrow().glossy_button, self.pending.glossy.get(), "Glossy: On", "Glossy: Off");
    }

    fn update_button_style_buttons(&self) {
        let w = self.w.borrow();
        let style = self.pending.button_style.get();
        set_selected(&w.btn_style_flat_btn, style == SPButtonStyle::Flat);
        set_selected(&w.btn_style_gradient_btn, style == SPButtonStyle::Gradient);
        set_selected(&w.btn_style_modern_grad_btn, style == SPButtonStyle::ModernGradient);
        set_selected(&w.btn_style_aqua_btn, style == SPButtonStyle::Aqua);
    }

    fn update_aero_header_style_buttons(&self) {
        let w = self.w.borrow();
        let style = self.pending.aero_header_style.get();
        set_selected(&w.aero_header_solid_btn, style == AeroHeaderStyle::Solid);
        set_selected(&w.aero_header_gradient_btn, style == AeroHeaderStyle::Gradient);
        set_selected(&w.aero_header_glossy_btn, style == AeroHeaderStyle::Glossy);
        set_selected(&w.aero_header_accent_glossy_btn, style == AeroHeaderStyle::AccentGlossy);
        set_selected(&w.aero_header_dark_glossy_btn, style == AeroHeaderStyle::DarkGlossy);
    }

    fn update_aero_section_visibility(&self) {
        if let Some(section) = &self.w.borrow().aero_section {
            section.set_visible(*self.pending.skin_type.borrow() == "frutiger-aero");
        }
    }

    fn update_developer_mode_button(&self) {
        apply_toggle_button(
            &self.w.borrow().developer_mode_button,
            self.pending.developer_mode.get(),
            "Developer Mode: On",
            "Developer Mode: Off",
        );
    }

    fn update_skin_buttons(&self) {
        let w = self.w.borrow();
        let skin_type = self.pending.skin_type.borrow();
        set_selected(&w.classic_skin_button, *skin_type == "classic");
        set_selected(&w.modern_skin_button, *skin_type == "modern");
        set_selected(&w.frutiger_aero_skin_button, *skin_type == "frutiger-aero");
        for (btn, type_) in &w.bundle_skin_buttons {
            set_selected(&Some(btn.clone()), *skin_type == *type_);
        }
    }

    fn update_full_width_address_bar_button(&self) {
        apply_toggle_button(
            &self.w.borrow().full_width_address_bar_button,
            self.pending.full_width_address_bar.get(),
            "Full Width: On",
            "Full Width: Off",
        );
    }

    fn sync_slider_context(&self) {
        let w = self.w.borrow();
        if let Some(slider) = &w.saturation_slider {
            slider.set_linked_hue(self.pending.aero_hue.get());
        }
        if let Some(slider) = &w.lightness_slider {
            slider.set_linked_hue(self.pending.aero_hue.get());
            slider.set_linked_sat(self.pending.aero_saturation.get());
        }
    }

    /// Mirrors the current hue + saturation into the colour chooser so the
    /// selected swatch tracks the sliders.
    fn on_hue_or_saturation_changed(&self) {
        self.sync_slider_context();
        if let Some(chooser) = &self.w.borrow().color_chooser {
            chooser.set_selected_color(SPColor::from_hsl(
                self.pending.aero_hue.get(),
                self.pending.aero_saturation.get(),
                0.55,
                255,
            ));
        }
        self.element().invalidate();
    }

    // ── Load / Save ───────────────────────────────────────────────────────

    pub fn load_settings(&self) {
        self.pending.scheme.set(SP_COLOR_SCHEME_LIGHT);
        self.pending.glossy.set(true);

        let settings = std::fs::read_to_string(Self::get_settings_path())
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok());
        if let Some(Value::Object(obj)) = settings {
            let w = self.w.borrow();
            let p = &self.pending;
            if let (Some(hex), Some(chooser)) = (obj.get("tintColor").and_then(Value::as_str), &w.color_chooser) {
                chooser.set_selected_color(SPColor::from_html(hex));
            }
            if let Some(v) = obj.get("colorScheme").and_then(Value::as_i64) {
                p.scheme.set(v as i16);
            }
            if let Some(v) = obj.get("glossy").and_then(Value::as_bool) {
                p.glossy.set(v);
            }
            if let Some(v) = obj.get("developerMode").and_then(Value::as_bool) {
                p.developer_mode.set(v);
            }
            if let Some(v) = obj.get("skinType").and_then(Value::as_str) {
                *p.skin_type.borrow_mut() = v.to_string();
            }
            if let Some(v) = obj.get("fullWidthAddressBar").and_then(Value::as_bool) {
                p.full_width_address_bar.set(v);
            }
            if let (Some(v), Some(input)) = (obj.get("downloadsFolder").and_then(Value::as_str), &w.downloads_folder_input) {
                input.set_value(v);
            }
            if let (Some(v), Some(input)) = (obj.get("musicFolder").and_then(Value::as_str), &w.music_folder_input) {
                input.set_value(v);
            }
            if let Some(v) = obj.get("aeroHue").and_then(Value::as_f64) {
                p.aero_hue.set(v as f32);
            }
            if let Some(v) = obj.get("aeroSaturation").and_then(Value::as_f64) {
                p.aero_saturation.set(v as f32);
            }
            if let Some(v) = obj.get("aeroLightness").and_then(Value::as_f64) {
                p.aero_lightness.set(v as f32);
            }
            if let Some(v) = obj.get("aeroHeaderStyle").and_then(Value::as_i64) {
                p.aero_header_style.set(AeroHeaderStyle::from_i32(v as i32));
            }
            if let Some(v) = obj.get("buttonStyle").and_then(Value::as_i64) {
                p.button_style.set(SPButtonStyle::from_i32(v as i32));
            }
        }

        {
            let w = self.w.borrow();
            if let Some(s) = &w.hue_slider {
                s.set_value(self.pending.aero_hue.get());
            }
            if let Some(s) = &w.saturation_slider {
                s.set_value(self.pending.aero_saturation.get());
            }
            if let Some(s) = &w.lightness_slider {
                s.set_value(self.pending.aero_lightness.get());
            }
        }
        self.sync_slider_context();
        if let Some(chooser) = &self.w.borrow().color_chooser {
            chooser.set_selected_color(SPColor::from_hsl(
                self.pending.aero_hue.get(),
                self.pending.aero_saturation.get(),
                0.55,
                255,
            ));
        }

        self.update_mode_buttons();
        self.update_glossy_button();
        self.update_button_style_buttons();
        self.update_developer_mode_button();
        self.update_skin_buttons();
        self.update_full_width_address_bar_button();
        self.update_aero_header_style_buttons();
        self.update_aero_section_visibility();

        let w = self.w.borrow();
        if let Some(input) = w.downloads_folder_input.as_ref().filter(|i| i.get_value().is_empty()) {
            input.set_value(&DownloadManager::get_downloads_folder());
        }
        if let Some(input) = w.music_folder_input.as_ref().filter(|i| i.get_value().is_empty()) {
            input.set_value(&DownloadManager::get_music_folder());
        }
    }

    pub fn save_settings(&self) {
        let w = self.w.borrow();
        let Some(chooser) = &w.color_chooser else { return };
        let p = &self.pending;
        let color = chooser.get_selected_color();

        let mut obj = Map::new();
        obj.insert("tintColor".into(), json!(color.to_html_color()));
        obj.insert("colorScheme".into(), json!(p.scheme.get()));
        obj.insert("glossy".into(), json!(p.glossy.get()));
        obj.insert("developerMode".into(), json!(p.developer_mode.get()));
        obj.insert("skinType".into(), json!(*p.skin_type.borrow()));
        obj.insert("fullWidthAddressBar".into(), json!(p.full_width_address_bar.get()));
        obj.insert("aeroHue".into(), json!(p.aero_hue.get() as f64));
        obj.insert("aeroSaturation".into(), json!(p.aero_saturation.get() as f64));
        obj.insert("aeroLightness".into(), json!(p.aero_lightness.get() as f64));
        obj.insert("aeroHeaderStyle".into(), json!(p.aero_header_style.get() as i32));
        obj.insert("buttonStyle".into(), json!(p.button_style.get() as i32));
        if let Some(v) = w.downloads_folder_input.as_ref().map(|i| i.get_value()).filter(|v| !v.is_empty()) {
            obj.insert("downloadsFolder".into(), json!(v));
        }
        if let Some(v) = w.music_folder_input.as_ref().map(|i| i.get_value()).filter(|v| !v.is_empty()) {
            obj.insert("musicFolder".into(), json!(v));
        }
        drop(w);
        if let Err(e) = std::fs::write(Self::get_settings_path(), Value::Object(obj).to_string()) {
            log::warn!("Cannot save settings: {e}");
        }

        // Apply all settings instantly.
        let Some(win) = self.element().get_window_element() else { return };
        let (hue, sat, light) = (p.aero_hue.get(), p.aero_saturation.get(), p.aero_lightness.get());
        win.element().set_tint_color(Some(SPColor::from_hsl(hue, sat, light, 255)));
        win.element().set_color_scheme(p.scheme.get());

        let skin_type = p.skin_type.borrow().clone();
        let new_skin: Rc<dyn SPSkin> = if skin_type == "frutiger-aero" {
            let aero = SPFrutigerAeroSkin::new();
            aero.set_header_style(p.aero_header_style.get());
            Rc::new(aero)
        } else if skin_type == "classic" {
            Rc::new(SPClassicSkin::new())
        } else {
            // "modern" and installed skin bundles are skin.xml skins; fall
            // back to procedural drawing if the bundle is missing.
            SPAppManager::with(|m| m.create_skin(&skin_type)).unwrap_or_else(|| Rc::new(SPModernSkin::new()))
        };
        new_skin.set_glossy(p.glossy.get());
        new_skin.set_button_style(p.button_style.get());
        // Hue/saturation/lightness apply to every skin (same as at startup).
        new_skin.base().set_hue(hue);
        new_skin.base().set_saturation(sat);
        new_skin.base().set_lightness(light);
        win.element().set_skin(Some(new_skin));

        if let Some(app) = bungalow_app_handle::current() {
            app.set_developer_mode(p.developer_mode.get());
            app.set_full_width_address_bar(p.full_width_address_bar.get());
        }

        if let (Some(gc), Some(window)) = (win.create_graphics(), win.downcast_window()) {
            window.invalidate_region(SPRectangle::new(0, 0, gc.get_width(), gc.get_height()));
        }
    }

    // ── Callbacks ─────────────────────────────────────────────────────────

    fn on_scheme_clicked(&self, scheme: i16) {
        self.pending.scheme.set(scheme);
        self.update_mode_buttons();
        self.element().invalidate();
    }

    fn on_glossy_clicked(&self) {
        self.pending.glossy.set(!self.pending.glossy.get());
        self.update_glossy_button();
        self.element().invalidate();
    }

    fn choose_folder(&self, input: Option<Rc<SPInputElement>>, title: &str, fallback: &str) {
        let current = match &input {
            Some(input) => input.get_value(),
            None => format!("{}/{fallback}", platform_ui::home_dir()),
        };
        let chosen = platform_ui::show_folder_dialog(title, &current);
        if let (false, Some(input)) = (chosen.is_empty(), input) {
            input.set_value(&chosen);
        }
    }

    fn on_browse_downloads_clicked(&self) {
        let input = self.w.borrow().downloads_folder_input.clone();
        self.choose_folder(input, "Choose Downloads Folder", "Downloads");
    }

    fn on_browse_music_clicked(&self) {
        let input = self.w.borrow().music_folder_input.clone();
        self.choose_folder(input, "Choose Music Folder", "Music");
    }

    fn on_developer_mode_clicked(&self) {
        self.pending.developer_mode.set(!self.pending.developer_mode.get());
        self.update_developer_mode_button();
        self.element().invalidate();
    }

    fn on_skin_clicked(&self, skin_type: &str) {
        *self.pending.skin_type.borrow_mut() = skin_type.to_string();
        self.update_skin_buttons();
        self.update_aero_section_visibility();
        self.element().invalidate();
    }

    fn on_bundle_skin_clicked(&self, sender: Option<&SPObject>) {
        let skin_type = self.w.borrow().bundle_skin_buttons.iter().find_map(|(btn, type_)| {
            let btn: ElementRef = btn.clone();
            sender.and_then(SPObject::element).is_some_and(|e| std::ptr::addr_eq(Rc::as_ptr(e), Rc::as_ptr(&btn))).then(|| type_.clone())
        });
        if let Some(skin_type) = skin_type {
            *self.pending.skin_type.borrow_mut() = skin_type;
        }
        self.update_skin_buttons();
        self.update_aero_section_visibility();
        self.element().invalidate();
    }

    fn on_full_width_address_bar_clicked(&self) {
        self.pending.full_width_address_bar.set(!self.pending.full_width_address_bar.get());
        self.update_full_width_address_bar_button();
        self.element().invalidate();
    }

    fn on_aero_header_style_clicked(&self, style: AeroHeaderStyle) {
        self.pending.aero_header_style.set(style);
        self.update_aero_header_style_buttons();
        self.element().invalidate();
    }

    fn on_button_style_clicked(&self, style: SPButtonStyle) {
        self.pending.button_style.set(style);
        self.update_button_style_buttons();
        self.element().invalidate();
    }
}

/// Downcast helper for the window element returned by `get_window_element`.
trait DowncastWindow {
    fn downcast_window(&self) -> Option<&SPWindowElement>;
}

impl DowncastWindow for ElementRef {
    fn downcast_window(&self) -> Option<&SPWindowElement> {
        self.as_any().downcast_ref::<SPWindowElement>()
    }
}

impl SPViewClass for ConfigView {
    fn view(&self) -> &SPViewElement {
        &self.view
    }
    fn accepts_uri(&self, uri: &str) -> bool {
        uri.starts_with("bungalow:config")
    }
    fn navigate(&self, uri: &str) {
        self.element().this().set_uri(uri);
    }
}

impl SPElementClass for ConfigView {
    fn get_type(&self) -> String {
        "configview".into()
    }
    fn pack(&self) {
        SPViewElement::pack_view(self.element());
    }
    fn scroll(&self, scroll_x: i32, scroll_y: i32, _x: i32, _y: i32) {
        SPViewElement::scroll_view(self.element(), scroll_x, scroll_y);
    }
    fn set_visible(&self, value: bool) {
        self.element().set_visible_default(value);
        // Refresh pending values from disk so saving here doesn't overwrite
        // colour changes made elsewhere (e.g. the PlayerView sliders).
        if value {
            self.load_settings();
        }
    }
    fn as_view(&self) -> Option<&dyn SPViewClass> {
        Some(self)
    }
}

