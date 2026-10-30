//! Port of `SPXmlSkin`: a skin described by a skin.xml stylesheet inside an
//! app bundle (`<app>/skins/<name>/skin.xml`).
//!
//! `<resources>` declares sprite images (`<image src="sp://app/..." id="skin"/>`,
//! optionally `brightness="0.4"` for a darker variant of the same sheet) and
//! named colors with per-mode states. Each `<selector match="sp-button"
//! state="active" mode="dark">` holds `<property>` entries:
//!
//! * `name="background" value="$color" | "#rrggbb"` → solid fill
//! * `name="skin"/"background" value="url(#imageId)" slice="x1,y1;x2,y2" nine-patch="l,t,r,b"` → nine-patch sprite
//! * `name="padding" value="l,t,r,b"` → content inset (`get_content_bounds`)
//! * `name="color" value="$color.foreground" | "#hex"` → label colour (buttons, checkboxes)
//! * `name="radius" value="6"` → rounds solid backgrounds
//!
//! A selector with mode="dark"/"light" wins over one without a mode in that
//! colour scheme and is ignored in the other.
//!
//! Sprites and colors are colorized by the skin's hue/saturation/lightness so
//! the HSL sliders tint the whole skin. Anything the stylesheet does not
//! style falls back to [`SPModernSkin`]'s procedural drawing.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;

use crate::libspace::SPRectangle;
use crate::sp_app_manager::SPAppManager;
use crate::sp_color::SPColor;
use crate::sp_element::{ElementRef, SPElement, SP_COLOR_SCHEME_DARK};
use crate::sp_graphics_context::SPGraphicsContext;
use crate::sp_image::{Pixel, SPImage};
use crate::sp_modern_skin::SPModernSkin;
use crate::sp_skin::{SPButtonStyle, SPSkin, SPSkinBase};
use crate::sp_tab_bar_element::{SPTab, SPTabBarElement};
use crate::sp_table_element::SPTableElement;
use crate::sp_tree_item::SPTreeItem;
use crate::sp_tree_view_element::SPTreeViewElement;
use crate::sp_xml_parser::{parse_sp_xml, SPXmlNode};

/// How far the lightness slider (0..1, neutral at 0.5) shifts skin colours.
const LIGHTNESS_RANGE: f32 = 0.3;

fn attr(node: &SPXmlNode, key: &str) -> String {
    node.attributes.get(key).cloned().unwrap_or_default()
}

fn float_attr(node: &SPXmlNode, key: &str, def: f32) -> f32 {
    node.attributes.get(key).and_then(|v| v.trim().parse().ok()).unwrap_or(def)
}

fn bool_attr(node: &SPXmlNode, key: &str, def: bool) -> bool {
    node.attributes.get(key).map(|v| v != "false" && v != "none" && v != "0").unwrap_or(def)
}

fn parse_ints(text: &str) -> Vec<i32> {
    text.split([',', ';', ' ']).filter(|t| !t.is_empty()).filter_map(|t| t.parse().ok()).collect()
}

/// #rgb, #rrggbb or #rrggbbaa.
pub fn parse_hex(text: &str) -> Option<SPColor> {
    let hex = text.strip_prefix('#')?;
    let hex: String = if hex.len() == 3 { hex.chars().flat_map(|c| [c, c]).collect() } else { hex.to_string() };
    if (hex.len() != 6 && hex.len() != 8) || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let mut v = [0i16, 0, 0, 255];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        v[i] = i16::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
    }
    Some(SPColor::new(v[0], v[1], v[2], v[3]))
}

/// "url(#skin)" → "skin"
fn url_reference(value: &str) -> String {
    match value.strip_prefix("url(").and_then(|v| v.strip_suffix(')')) {
        Some(reference) => reference.trim_start_matches('#').to_string(),
        None => String::new(),
    }
}

/// Selector aliases for element types whose stylesheet name differs.
fn type_alias(type_: &str) -> Option<&'static str> {
    match type_ {
        "treeview" => Some("sp-sidebar"),
        _ => None,
    }
}

#[derive(Debug, Clone, Default)]
struct ColorState {
    background: Option<SPColor>,
    foreground: Option<SPColor>,
}

#[derive(Debug, Clone)]
struct NamedColor {
    /// "dark" / "light"
    modes: BTreeMap<String, ColorState>,
    tint: bool,
    tint_saturation: f32,
}

#[derive(Debug, Clone)]
struct ImageResource {
    /// shared by resources with the same src
    image: Rc<SPImage>,
    tint: bool,
    tint_saturation: f32,
    /// multiplies sprite lightness
    brightness: f32,
}

#[derive(Default)]
struct Property {
    name: String,
    value: String,
    /// set for url(#id) values
    image_id: String,
    /// sprite rect within the image
    sw: i32,
    sh: i32,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    /// untinted crop of the sprite
    source: Option<Rc<SPImage>>,
    /// source colorized by the current HSL
    tinted: RefCell<Option<Rc<SPImage>>>,
}

struct Rule {
    match_: String,
    state: String,
    /// "dark", "light" or "" for both
    mode: String,
    properties: Vec<Property>,
}

pub struct SPXmlSkin {
    modern: SPModernSkin,
    name: RefCell<String>,
    colors: RefCell<BTreeMap<String, NamedColor>>,
    images: RefCell<BTreeMap<String, ImageResource>>,
    rules: RefCell<Vec<Rule>>,
    /// HSL the cached tinted sprites were generated for.
    tinted_hsl: Cell<(f32, f32, f32)>,
}

impl Default for SPXmlSkin {
    fn default() -> Self {
        Self::new()
    }
}

impl SPXmlSkin {
    pub fn new() -> Self {
        Self {
            modern: SPModernSkin::new(),
            name: RefCell::default(),
            colors: RefCell::default(),
            images: RefCell::default(),
            rules: RefCell::default(),
            tinted_hsl: Cell::new((-1.0, -1.0, -1.0)),
        }
    }

    pub fn get_name(&self) -> String {
        self.name.borrow().clone()
    }

    /// Loads the stylesheet. Relative image paths resolve against `base_dir`.
    pub fn load(&self, skin_xml_path: &str, base_dir: &str) -> bool {
        let Ok(source) = std::fs::read_to_string(skin_xml_path) else {
            log::error!("Skin not found: {skin_xml_path}");
            return false;
        };
        self.load_from_str(&source, base_dir)
            .map_err(|error| log::error!("Invalid skin {skin_xml_path}: {error}"))
            .is_ok()
    }

    pub fn load_from_str(&self, source: &str, base_dir: &str) -> Result<(), String> {
        let root = parse_sp_xml(source)?;
        if root.name != "skin" {
            return Err("root element must be <skin>".into());
        }
        for child in root.children.iter().filter(|c| !c.text_node) {
            match child.name.as_str() {
                "name" => {
                    let mut name = self.name.borrow_mut();
                    for t in child.children.iter().filter(|t| t.text_node) {
                        name.push_str(&t.text);
                    }
                }
                "resources" => self.parse_resources(child, base_dir),
                "selector" => self.parse_selector(child),
                _ => {}
            }
        }
        Ok(())
    }

    fn parse_resources(&self, node: &SPXmlNode, base_dir: &str) {
        // Variants of one sheet (e.g. a darker copy) decode the file once.
        let mut loaded: BTreeMap<String, Rc<SPImage>> = BTreeMap::new();
        for child in node.children.iter().filter(|c| !c.text_node) {
            match child.name.as_str() {
                "image" => {
                    let id = attr(child, "id");
                    let src = attr(child, "src");
                    if id.is_empty() || src.is_empty() {
                        continue;
                    }
                    let path = SPAppManager::with(|m| m.resolve(&src))
                        .unwrap_or_else(|| Path::new(base_dir).join(&src).to_string_lossy().into_owned());
                    let image = match loaded.get(&path) {
                        Some(image) => image.clone(),
                        None => match SPImage::from_file(&path) {
                            Ok(image) => {
                                let image = Rc::new(image);
                                loaded.insert(path, image.clone());
                                image
                            }
                            Err(e) => {
                                log::error!("Skin image {src} failed to load: {e}");
                                continue;
                            }
                        },
                    };
                    self.images.borrow_mut().insert(
                        id,
                        ImageResource {
                            image,
                            tint: bool_attr(child, "tint", true),
                            tint_saturation: float_attr(child, "tint-saturation", 0.35),
                            brightness: float_attr(child, "brightness", 1.0),
                        },
                    );
                }
                "color" => {
                    let name = attr(child, "name");
                    if name.is_empty() {
                        continue;
                    }
                    let mut color = NamedColor {
                        modes: BTreeMap::new(),
                        tint: bool_attr(child, "tint", true),
                        tint_saturation: float_attr(child, "tint-saturation", 0.12),
                    };
                    for state in child.children.iter().filter(|s| !s.text_node && s.name == "state") {
                        let mode = state.attributes.get("mode").cloned().unwrap_or_else(|| "light".into());
                        color.modes.insert(
                            mode,
                            ColorState {
                                background: parse_hex(&attr(state, "background")),
                                foreground: parse_hex(&attr(state, "foreground")),
                            },
                        );
                    }
                    self.colors.borrow_mut().insert(name, color);
                }
                _ => {}
            }
        }
    }

    fn parse_selector(&self, node: &SPXmlNode) {
        let mut rule = Rule {
            match_: attr(node, "match"),
            state: attr(node, "state"),
            mode: attr(node, "mode"),
            properties: Vec::new(),
        };
        if rule.match_.is_empty() {
            return;
        }
        for child in node.children.iter().filter(|c| !c.text_node && c.name == "property") {
            let mut prop = Property { name: attr(child, "name"), value: attr(child, "value"), ..Default::default() };
            prop.image_id = url_reference(&prop.value);

            if !prop.image_id.is_empty() {
                let images = self.images.borrow();
                let Some(resource) = images.get(&prop.image_id) else {
                    log::warn!("Skin selector {} references unknown image {}", rule.match_, prop.image_id);
                    continue;
                };
                let image = &resource.image;
                let (w, h) = (image.width as i32, image.height as i32);
                // slice="x1,y1;x2,y2" — the two corners of the sprite, end exclusive.
                let s = parse_ints(&attr(child, "slice"));
                let (mut x1, mut y1, mut x2, mut y2) = (0, 0, w, h);
                if s.len() == 4 {
                    x1 = s[0].min(s[2]);
                    x2 = s[0].max(s[2]);
                    y1 = s[1].min(s[3]);
                    y2 = s[1].max(s[3]);
                }
                let (x1, x2, y1, y2) = (x1.clamp(0, w), x2.clamp(0, w), y1.clamp(0, h), y2.clamp(0, h));
                if x2 <= x1 || y2 <= y1 {
                    log::warn!("Skin selector {} has an empty slice", rule.match_);
                    continue;
                }
                prop.sw = x2 - x1;
                prop.sh = y2 - y1;
                prop.source =
                    Some(Rc::new(image.slice_image(x1 as u32, y1 as u32, prop.sw as u32, prop.sh as u32, 0, 0, 0, 0)));
                let n = parse_ints(&attr(child, "nine-patch"));
                if n.len() == 4 {
                    (prop.left, prop.top, prop.right, prop.bottom) = (n[0], n[1], n[2], n[3]);
                }
            }
            rule.properties.push(prop);
        }
        self.rules.borrow_mut().push(rule);
    }

    fn tint_color(&self, color: &SPColor, saturation_floor: f32, brightness: f32) -> SPColor {
        let base = self.base();
        let (_h, s, l) = color.to_hsl();
        let sat = s.max(saturation_floor) * base.get_saturation();
        let light = (l * brightness + (base.get_lightness() - 0.5) * LIGHTNESS_RANGE).clamp(0.0, 1.0);
        SPColor::from_hsl(base.get_hue(), sat, light, color.get_a())
    }

    fn refresh_tint(&self) {
        let base = self.base();
        let hsl = (base.get_hue(), base.get_saturation(), base.get_lightness());
        if self.tinted_hsl.get() == hsl {
            return;
        }
        self.tinted_hsl.set(hsl);

        let images = self.images.borrow();
        for rule in self.rules.borrow().iter() {
            for prop in &rule.properties {
                let (Some(src), Some(res)) = (&prop.source, images.get(&prop.image_id)) else {
                    continue;
                };
                let mut out = SPImage::new(src.width, src.height);
                for (dst, p) in out.pixels.iter_mut().zip(src.pixels.iter()) {
                    if p.a == 0 || (!res.tint && res.brightness == 1.0) {
                        *dst = *p;
                        continue;
                    }
                    let c = SPColor::new(p.r as i16, p.g as i16, p.b as i16, p.a as i16);
                    let c = if res.tint {
                        self.tint_color(&c, res.tint_saturation, res.brightness)
                    } else {
                        let (h, s, l) = c.to_hsl();
                        SPColor::from_hsl(h, s, (l * res.brightness).clamp(0.0, 1.0), p.a as i16)
                    };
                    *dst = Pixel { r: c.get_r() as u8, g: c.get_g() as u8, b: c.get_b() as u8, a: p.a };
                }
                *prop.tinted.borrow_mut() = Some(Rc::new(out));
            }
        }
    }

    /// Resolves "$name", "$name.foreground" or "#hex" for the given colour
    /// scheme, tinted by the current HSL.
    pub fn resolve_color(&self, value: &str, scheme: i16, mut foreground: bool) -> Option<SPColor> {
        if value.starts_with('#') {
            return parse_hex(value);
        }
        let name = value.strip_prefix('$')?;
        let name = match name.split_once('.') {
            Some((name, part)) => {
                foreground = part == "foreground";
                name
            }
            None => name,
        };
        let colors = self.colors.borrow();
        let color = colors.get(name)?;
        let mode = color
            .modes
            .get(if scheme == SP_COLOR_SCHEME_DARK { "dark" } else { "light" })
            .or_else(|| color.modes.values().next())?;
        let c = if foreground { mode.foreground } else { mode.background }?;
        Some(if color.tint { self.tint_color(&c, color.tint_saturation, 1.0) } else { c })
    }

    fn selectors_for(element: &ElementRef) -> Vec<String> {
        let type_ = element.get_type();
        let mut selectors = vec![format!("sp-{type_}")];
        if let Some(alias) = type_alias(&type_) {
            selectors.push(alias.to_string());
        }
        let id = element.element().get_id();
        if !id.is_empty() {
            selectors.push(format!("#{id}"));
        }
        selectors
    }

    /// Index of the best rule, and whether any rule matched the selectors in
    /// this colour scheme at all.
    fn find_rule(&self, selectors: &[String], states: &[&str], scheme: i16) -> (Option<usize>, bool) {
        let mode = if scheme == SP_COLOR_SCHEME_DARK { "dark" } else { "light" };
        let rules = self.rules.borrow();
        let mut matched = false;
        // Later selectors in the list are more specific (#id beats sp-type).
        for selector in selectors.iter().rev() {
            for state in states {
                let mut any_mode = None;
                for (i, rule) in rules.iter().enumerate() {
                    if rule.match_ != *selector || (!rule.mode.is_empty() && rule.mode != mode) {
                        continue;
                    }
                    matched = true;
                    if rule.state != *state {
                        continue;
                    }
                    // A rule for this colour scheme beats one for both.
                    if rule.mode == mode {
                        return (Some(i), true);
                    }
                    any_mode.get_or_insert(i);
                }
                if any_mode.is_some() {
                    return (any_mode, true);
                }
            }
            if matched {
                return (None, true);
            }
        }
        (None, false)
    }

    fn find_element_rule(&self, element: &ElementRef, state: &str) -> (Option<usize>, bool) {
        let states: Vec<&str> = if state.is_empty() { vec![""] } else { vec![state, ""] };
        self.find_rule(&Self::selectors_for(element), &states, element.element().get_color_scheme())
    }

    fn draw_nine_patch(&self, g: &dyn SPGraphicsContext, p: &Property, b: &SPRectangle) {
        let tinted = p.tinted.borrow().clone();
        let Some(image) = tinted.or_else(|| p.source.clone()) else {
            return;
        };
        if b.width <= 0 || b.height <= 0 {
            return;
        }
        let (mut l, mut r, mut t, mut bt) = (p.left, p.right, p.top, p.bottom);
        // Shrink the fixed borders proportionally when the target is too small.
        if l + r > b.width && l + r > 0 {
            l = b.width * l / (l + r);
            r = b.width - l;
        }
        if t + bt > b.height && t + bt > 0 {
            t = b.height * t / (t + bt);
            bt = b.height - t;
        }
        let src_x = [0, p.left.min(p.sw), (p.sw - p.right).max(0), p.sw];
        let src_y = [0, p.top.min(p.sh), (p.sh - p.bottom).max(0), p.sh];
        let dst_x = [b.x, b.x + l, b.x + b.width - r, b.x + b.width];
        let dst_y = [b.y, b.y + t, b.y + b.height - bt, b.y + b.height];
        for row in 0..3 {
            for col in 0..3 {
                let (sw, sh) = (src_x[col + 1] - src_x[col], src_y[row + 1] - src_y[row]);
                let (dw, dh) = (dst_x[col + 1] - dst_x[col], dst_y[row + 1] - dst_y[row]);
                if sw <= 0 || sh <= 0 || dw <= 0 || dh <= 0 {
                    continue;
                }
                g.draw_image_part(&image, dst_x[col], dst_y[row], dw, dh, src_x[col], src_y[row], sw, sh);
            }
        }
    }

    fn draw_rule(&self, g: &dyn SPGraphicsContext, rule: usize, scheme: i16, bounds: &SPRectangle) {
        self.refresh_tint();
        let rules = self.rules.borrow();
        let rule = &rules[rule];
        let radius = rule
            .properties
            .iter()
            .filter(|p| p.name == "radius")
            .filter_map(|p| parse_ints(&p.value).first().copied())
            .last()
            .unwrap_or(0);
        // Solid backgrounds first so sprites can layer on top of them.
        for prop in rule.properties.iter().filter(|p| p.name == "background" && p.image_id.is_empty()) {
            let Some(color) = self.resolve_color(&prop.value, scheme, false) else {
                continue;
            };
            if radius > 0 {
                g.fill_rounded_rectangle(bounds.x, bounds.y, bounds.width, bounds.height, radius, color);
            } else {
                g.fill_rectangle(bounds.x, bounds.y, bounds.width, bounds.height, color);
            }
        }
        for prop in &rule.properties {
            if (prop.name == "skin" || prop.name == "background") && !prop.image_id.is_empty() {
                self.draw_nine_patch(g, prop, bounds);
            }
        }
    }

    fn rule_text_color(&self, rule: usize, scheme: i16) -> Option<SPColor> {
        let rules = self.rules.borrow();
        rules[rule]
            .properties
            .iter()
            .filter(|p| p.name == "color")
            .map(|p| self.resolve_color(&p.value, scheme, true))
            .last()
            .flatten()
    }

    fn draw_label(&self, g: &dyn SPGraphicsContext, elm: &SPElement, rule: usize, bounds: &SPRectangle) {
        let text = elm.get_inner_text();
        if text.is_empty() {
            return;
        }
        let color = self.rule_text_color(rule, elm.get_color_scheme());
        let font = elm.get_font_style();
        let size = g.measure_string(&text, &font);
        g.draw_string(
            &text,
            &font,
            color.unwrap_or_else(|| elm.get_foreground_color()),
            bounds.x + (bounds.width - size.width) / 2,
            bounds.y + (bounds.height - size.height) / 2,
            size.width,
            size.height,
        );
    }
}

impl SPSkin for SPXmlSkin {
    fn base(&self) -> &SPSkinBase {
        self.modern.base()
    }

    fn set_glossy(&self, value: bool) {
        self.modern.set_glossy(value);
    }
    fn is_glossy(&self) -> bool {
        self.modern.is_glossy()
    }
    fn set_button_style(&self, style: SPButtonStyle) {
        self.modern.set_button_style(style);
    }
    fn get_button_style(&self) -> SPButtonStyle {
        self.modern.get_button_style()
    }

    fn draw_element(&self, g: &dyn SPGraphicsContext, element: &ElementRef, state: &str, bounds: &SPRectangle) -> bool {
        let (rule, matched) = self.find_element_rule(element, state);
        if !matched {
            return self.modern.draw_element(g, element, state, bounds);
        }
        if let Some(rule) = rule {
            self.draw_rule(g, rule, element.element().get_color_scheme(), bounds);
            // Buttons leave their label to the skin.
            if element.get_type() == "button" {
                self.draw_label(g, element.element(), rule, bounds);
            }
        }
        true
    }

    /// Insets bounds by the matched selector's padding="l,t,r,b" property.
    fn get_content_bounds(&self, element: &ElementRef, bounds: &SPRectangle) -> SPRectangle {
        let (Some(rule), _) = self.find_element_rule(element, "") else {
            return self.modern.get_content_bounds(element, bounds);
        };
        let rules = self.rules.borrow();
        let Some(padding) = rules[rule].properties.iter().find(|p| p.name == "padding") else {
            return *bounds;
        };
        let p = parse_ints(&padding.value);
        if p.len() != 4 {
            return *bounds;
        }
        SPRectangle::new(
            bounds.x + p[0],
            bounds.y + p[1],
            (bounds.width - p[0] - p[2]).max(0),
            (bounds.height - p[1] - p[3]).max(0),
        )
    }

    /// The matched selector's color property.
    fn get_text_color(&self, element: &ElementRef, state: &str) -> Option<SPColor> {
        let (rule, _) = self.find_element_rule(element, state);
        let rules = self.rules.borrow();
        let prop = rules[rule?].properties.iter().find(|p| p.name == "color")?;
        self.resolve_color(&prop.value, element.element().get_color_scheme(), true)
    }

    fn draw_tab_bar(
        &self,
        g: &dyn SPGraphicsContext,
        tab_bar: &SPTabBarElement,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) {
        let (rule, matched) = self.find_element_rule(&tab_bar.el.this(), "");
        if !matched {
            return self.modern.draw_tab_bar(g, tab_bar, x, y, width, height);
        }
        if let Some(rule) = rule {
            self.draw_rule(g, rule, tab_bar.el.get_color_scheme(), &SPRectangle::new(x, y, width, height));
        }
    }

    /// sp-table-row with state "selected", "odd" or "even".
    fn draw_table_row(
        &self,
        g: &dyn SPGraphicsContext,
        table: &SPTableElement,
        i: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        row: i32,
    ) {
        let mut states = Vec::new();
        if table.is_row_selected(row) {
            states.push("selected");
        }
        states.push(if row % 2 == 0 { "even" } else { "odd" });
        states.push("");
        let scheme = table.el.get_color_scheme();
        let (rule, matched) = self.find_rule(&["sp-table-row".to_string()], &states, scheme);
        if !matched {
            return self.modern.draw_table_row(g, table, i, x, y, width, height, row);
        }
        if let Some(rule) = rule {
            self.draw_rule(g, rule, scheme, &SPRectangle::new(x, y, width, height));
        }
    }

    /// sp-table-header.
    fn draw_table_header(
        &self,
        g: &dyn SPGraphicsContext,
        table: &SPTableElement,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) {
        let scheme = table.el.get_color_scheme();
        let (rule, matched) = self.find_rule(&["sp-table-header".to_string()], &[""], scheme);
        if !matched {
            return self.modern.draw_table_header(g, table, x, y, width, height);
        }
        if let Some(rule) = rule {
            self.draw_rule(g, rule, scheme, &SPRectangle::new(x, y, width, height));
        }
    }

    /// sp-treeitem with state "selected" or "hover".
    fn draw_tree_item(
        &self,
        g: &dyn SPGraphicsContext,
        tree_view: &SPTreeViewElement,
        item: &SPTreeItem,
        item_height: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) {
        let mut states = Vec::new();
        if item.is_selected() {
            states.push("selected");
        }
        if item.is_highlighted() {
            states.push("hover");
        }
        states.push("");
        let scheme = tree_view.el.get_color_scheme();
        let (rule, matched) = self.find_rule(&["sp-treeitem".to_string()], &states, scheme);
        if !matched {
            return self.modern.draw_tree_item(g, tree_view, item, item_height, x, y, width, height);
        }
        // Same inset pill footprint as the procedural modern skin.
        if let Some(rule) = rule {
            let bounds = SPRectangle::new(x + 4, y + 2, tree_view.el.get_width() - 8, item_height - 4);
            self.draw_rule(g, rule, scheme, &bounds);
        }
    }

    fn draw_tab_bar_tab(
        &self,
        g: &dyn SPGraphicsContext,
        tab_bar: &SPTabBarElement,
        tab: &SPTab,
        index: i32,
        active: bool,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) {
        let states: &[&str] = if active { &["selected", "active", ""] } else { &[""] };
        let scheme = tab_bar.el.get_color_scheme();
        let (rule, matched) = self.find_rule(&["sp-tabbar-tab".to_string()], states, scheme);
        if !matched {
            return self.modern.draw_tab_bar_tab(g, tab_bar, tab, index, active, x, y, width, height);
        }
        if let Some(rule) = rule {
            self.draw_rule(g, rule, scheme, &SPRectangle::new(x, y, width, height));
        }
    }

    fn draw_section_header(
        &self,
        g: &dyn SPGraphicsContext,
        tree_view: &SPTreeViewElement,
        title: &str,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) {
        self.modern.draw_section_header(g, tree_view, title, x, y, width, height);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SKIN: &str = r##"<skin>
        <name>Test</name>
        <resources>
            <color name="panel" tint="false">
                <state mode="light" background="#eeeeee" foreground="#111111"/>
                <state mode="dark" background="#222222" foreground="#fafafa"/>
            </color>
        </resources>
        <selector match="sp-button"><property name="background" value="$panel"/><property name="padding" value="4,2,4,2"/></selector>
        <selector match="sp-button" state="active" mode="dark"><property name="color" value="$panel.foreground"/></selector>
    </skin>"##;

    #[test]
    fn resolves_named_colors_per_mode() {
        let skin = SPXmlSkin::new();
        skin.load_from_str(SKIN, ".").unwrap();
        assert_eq!(skin.get_name(), "Test");
        assert_eq!(skin.resolve_color("$panel", 0, false), Some(SPColor::new(0xee, 0xee, 0xee, 255)));
        assert_eq!(
            skin.resolve_color("$panel.foreground", SP_COLOR_SCHEME_DARK, false),
            Some(SPColor::new(0xfa, 0xfa, 0xfa, 255))
        );
        assert_eq!(skin.resolve_color("#abc", 0, false), Some(SPColor::new(0xaa, 0xbb, 0xcc, 255)));
    }

    #[test]
    fn mode_specific_rules_win() {
        let skin = SPXmlSkin::new();
        skin.load_from_str(SKIN, ".").unwrap();
        let selectors = vec!["sp-button".to_string()];
        let (light, matched) = skin.find_rule(&selectors, &["active", ""], 0);
        assert!(matched);
        assert_eq!(light, Some(0));
        let (dark, _) = skin.find_rule(&selectors, &["active", ""], SP_COLOR_SCHEME_DARK);
        assert_eq!(dark, Some(1));
        assert_eq!(skin.find_rule(&["sp-nothing".to_string()], &[""], 0), (None, false));
    }
}
