//! Port of `SPFrutigerAeroSkin` — Vista-era translucent aqua aesthetic.
//!
//! All colours are derived from the skin's hue (0–360), saturation (0–1)
//! and lightness (0–1) on [`SPSkinBase`].
//!
//! Light mode: pale tinted-gray background, tropical gradient tabs, dark-blue text.
//! Dark mode: same hue but dark navy background, lighter tropical tabs.

use std::cell::Cell;

use crate::libspace::SPRectangle;
use crate::sp_color::{SPColor, SPGradient};
use crate::sp_element::{ElementRef, SPElement, SP_COLOR_SCHEME_DARK};
use crate::sp_font_style::SPFontStyle;
use crate::sp_graphics_context::SPGraphicsContext;
use crate::sp_skin::{SPButtonStyle, SPSkin, SPSkinBase};
use crate::sp_tab_bar_element::{SPTab, SPTabBarElement};
use crate::sp_table_element::SPTableElement;
use crate::sp_tree_item::SPTreeItem;
use crate::sp_tree_view_element::SPTreeViewElement;

/// Header/footer rendering modes — inspired by the Windows Media Player lineage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AeroHeaderStyle {
    /// Flat fill, like SPModernSkin
    Solid = 0,
    /// Gentle top-to-bottom gradient (WMP 9)
    Gradient = 1,
    /// Gradient + glass sheen (WMP 10)
    #[default]
    Glossy = 2,
    /// Strong accent hue + heavy gloss (WMP 11 beta)
    AccentGlossy = 3,
    /// Dark-tinted accent + gloss (WMP 11 final)
    DarkGlossy = 4,
}

impl AeroHeaderStyle {
    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => Self::Solid,
            1 => Self::Gradient,
            3 => Self::AccentGlossy,
            4 => Self::DarkGlossy,
            _ => Self::Glossy,
        }
    }
}

pub struct SPFrutigerAeroSkin {
    base: SPSkinBase,
    header_style: Cell<AeroHeaderStyle>,
    glossy: Cell<bool>,
    button_style: Cell<SPButtonStyle>,
}

impl Default for SPFrutigerAeroSkin {
    fn default() -> Self {
        Self::new()
    }
}

/// Separator colour.
fn sep_color(scheme: i16) -> SPColor {
    if scheme == SP_COLOR_SCHEME_DARK {
        SPColor::new(255, 255, 255, 35)
    } else {
        SPColor::new(0, 0, 0, 22)
    }
}

/// Blend the element's tint into a base colour at a fixed low alpha so the
/// overall Vista-gray look is preserved while the tint shifts the hue.
fn apply_tint_to_base(base: SPColor, tint: Option<SPColor>) -> SPColor {
    match tint {
        Some(t) => base.tint(&SPColor::new(t.get_r(), t.get_g(), t.get_b(), 45)),
        None => base,
    }
}

fn with_alpha(color: SPColor, alpha: i16) -> SPColor {
    SPColor::new(color.get_r(), color.get_g(), color.get_b(), alpha)
}

/// Subtle Aero glass sheen (similar to SPModernSkin but slightly stronger specular).
fn draw_aero_sheen(g: &dyn SPGraphicsContext, x: i32, y: i32, w: i32, h: i32, r: i32) {
    let top_h = h * 2 / 5;
    let spec = SPGradient::new(SPColor::new(255, 255, 255, 110), SPColor::new(255, 255, 255, 0), 0);
    g.fill_rounded_linear_gradient_rectangle(x, y, w, top_h, r, 0, &spec);
    let refr = SPGradient::new(SPColor::new(255, 255, 255, 0), SPColor::new(255, 255, 255, 32), 0);
    g.fill_rounded_linear_gradient_rectangle(x, y + top_h, w, h - top_h, r, 0, &refr);
}

/// Heavy WMP10/11-style gloss — taller specular band.
fn draw_heavy_gloss(g: &dyn SPGraphicsContext, x: i32, y: i32, w: i32, h: i32, r: i32) {
    let top_h = h / 2;
    let spec = SPGradient::new(SPColor::new(255, 255, 255, 160), SPColor::new(255, 255, 255, 10), 0);
    g.fill_rounded_linear_gradient_rectangle(x, y, w, top_h, r, 0, &spec);
    let refr = SPGradient::new(SPColor::new(255, 255, 255, 0), SPColor::new(255, 255, 255, 50), 0);
    g.fill_rounded_linear_gradient_rectangle(x, y + top_h, w, h - top_h, r, 0, &refr);
}

impl SPFrutigerAeroSkin {
    pub fn new() -> Self {
        let base = SPSkinBase::new();
        base.set_hue(200.0);
        base.set_saturation(0.70);
        base.set_lightness(0.50);
        Self {
            base,
            header_style: Cell::new(AeroHeaderStyle::Glossy),
            glossy: Cell::new(true),
            button_style: Cell::new(SPButtonStyle::Aqua),
        }
    }

    pub fn set_header_style(&self, style: AeroHeaderStyle) {
        self.header_style.set(style);
    }

    pub fn get_header_style(&self) -> AeroHeaderStyle {
        self.header_style.get()
    }

    fn draw_header_footer(&self, g: &dyn SPGraphicsContext, elm: &SPElement, x: i32, y: i32, w: i32, h: i32) {
        let scheme = elm.get_color_scheme();
        let base = apply_tint_to_base(self.base.bg(scheme), elm.get_tint_color());
        let acc = self.base.accent();
        let rect = SPRectangle::new(x, y, w, h);
        match self.header_style.get() {
            AeroHeaderStyle::Solid => {
                // Flat fill — just the base background with light translucency
                g.fill_rectangle(x, y, w, h, with_alpha(base, 235));
            }
            AeroHeaderStyle::Gradient => {
                // WMP 9: gentle top→bottom gradient from slightly lighter to slightly darker
                g.draw_linear_gradient(&rect, &SPGradient::new(base.lighten(0.12), base.lighten(-0.08), 0));
            }
            AeroHeaderStyle::Glossy => {
                // WMP 10: gradient base + Aero glass sheen
                g.draw_linear_gradient(&rect, &SPGradient::new(base.lighten(0.10), base.lighten(-0.07), 0));
                draw_aero_sheen(g, x, y, w, h, 0);
            }
            AeroHeaderStyle::AccentGlossy => {
                // WMP 11 beta: strong accent hue dominates, heavy gloss on top
                let mixed = with_alpha(base.mix(&acc, 0.55), 240);
                g.draw_linear_gradient(&rect, &SPGradient::new(mixed.lighten(0.15), mixed.lighten(-0.10), 0));
                draw_heavy_gloss(g, x, y, w, h, 0);
            }
            AeroHeaderStyle::DarkGlossy => {
                // WMP 11 final: dark-tinted accent (deep jewel tone) + heavy gloss
                let channel = |v: i16| (v as f32 * 0.45) as i16;
                let mut dark = SPColor::new(channel(acc.get_r()), channel(acc.get_g()), channel(acc.get_b()), 255);
                if scheme == SP_COLOR_SCHEME_DARK {
                    dark = dark.lighten(0.5);
                }
                g.draw_linear_gradient(&rect, &SPGradient::new(dark.lighten(0.20), dark, 0));
                draw_heavy_gloss(g, x, y, w, h, 0);
            }
        }
    }
}

impl SPSkin for SPFrutigerAeroSkin {
    fn base(&self) -> &SPSkinBase {
        &self.base
    }

    fn set_glossy(&self, value: bool) {
        self.glossy.set(value);
    }
    fn is_glossy(&self) -> bool {
        self.glossy.get()
    }
    fn set_button_style(&self, style: SPButtonStyle) {
        self.button_style.set(style);
    }
    fn get_button_style(&self) -> SPButtonStyle {
        self.button_style.get()
    }

    fn draw_element(&self, g: &dyn SPGraphicsContext, element: &ElementRef, state: &str, bounds: &SPRectangle) -> bool {
        let elm = element.element();
        let scheme = elm.get_color_scheme();
        let b = *bounds;
        match element.get_type().as_str() {
            // Let the platform render the native input; just mark handled.
            "input" => true,
            "splitter" => {
                g.fill_rectangle(b.x, b.y, b.width, b.height, sep_color(scheme));
                true
            }
            "button" => {
                const R: i32 = 10;
                let is_primary = elm.get_string_attribute("variant", "default") == "primary";
                let mut btn_bg = if is_primary { self.base.accent() } else { self.base.bg(scheme).lighten(0.06) };
                if state == "active" {
                    btn_bg = btn_bg.lighten(-0.18);
                } else if state == "hover" {
                    btn_bg = btn_bg.lighten(0.08);
                }
                match self.button_style.get() {
                    SPButtonStyle::Flat => g.fill_rounded_rectangle(b.x, b.y, b.width, b.height, R, with_alpha(btn_bg, 215)),
                    SPButtonStyle::Gradient => {
                        let grad = SPGradient::new(btn_bg.lighten(0.12), btn_bg.lighten(-0.10), 0);
                        g.fill_rounded_linear_gradient_rectangle(b.x, b.y, b.width, b.height, R, 0, &grad);
                    }
                    SPButtonStyle::ModernGradient => {
                        // Chromatic vertical gradient — accent hue shifts toward +60°
                        let (h, s, l) = btn_bg.to_hsl();
                        let sec = SPColor::from_hsl(h + 60.0, s * 0.85, l * 0.85, btn_bg.get_a());
                        g.fill_rounded_linear_gradient_rectangle(b.x, b.y, b.width, b.height, R, 0, &SPGradient::new(btn_bg, sec, 0));
                    }
                    SPButtonStyle::Aqua => {
                        // macOS X Aqua gel: vivid base, wide upper highlight, thin bottom shine
                        g.fill_rounded_rectangle(b.x, b.y, b.width, b.height, R, with_alpha(btn_bg, 230));
                        let top_h = b.height * 2 / 5;
                        let hi = SPGradient::new(SPColor::new(255, 255, 255, 165), SPColor::new(255, 255, 255, 0), 0);
                        g.fill_rounded_linear_gradient_rectangle(b.x + 3, b.y + 2, b.width - 6, top_h, R, 0, &hi);
                        let bot_h = (b.height / 7).max(3);
                        let bot = SPGradient::new(SPColor::new(255, 255, 255, 0), SPColor::new(255, 255, 255, 45), 0);
                        g.fill_rounded_linear_gradient_rectangle(b.x + 2, b.y + b.height - bot_h - 2, b.width - 4, bot_h, 3, 0, &bot);
                    }
                }
                // Gloss sheen — independent overlay, active with any button style
                if self.glossy.get() && state != "active" {
                    draw_aero_sheen(g, b.x, b.y, b.width, b.height, R);
                }
                true
            }
            "appheader" | "appfooter" => {
                self.draw_header_footer(g, elm, b.x, b.y, b.width, b.height);
                true
            }
            "treeview" => {
                let bg = apply_tint_to_base(self.base.bg(scheme), elm.get_tint_color());
                let bg = if scheme == SP_COLOR_SCHEME_DARK { bg.lighten(0.18) } else { bg.lighten(-0.03) };
                g.fill_rectangle(b.x, b.y, b.width, b.height, bg);
                true
            }
            "viewstack" => {
                let bg = apply_tint_to_base(self.base.bg(scheme), elm.get_tint_color());
                g.fill_rectangle(b.x, b.y, b.width, b.height, bg);
                true
            }
            _ => false,
        }
    }

    fn draw_table_row(&self, g: &dyn SPGraphicsContext, table: &SPTableElement, _i: i32, x: i32, y: i32, width: i32, height: i32, row: i32) {
        let scheme = table.el.get_color_scheme();
        let base = apply_tint_to_base(self.base.bg(scheme), table.el.get_tint_color());
        if table.is_row_selected(row) {
            g.fill_rounded_rectangle(x + 2, y + 1, width - 4, height - 2, 5, with_alpha(self.base.accent(), 185));
            if self.glossy.get() {
                draw_aero_sheen(g, x + 2, y + 1, width - 4, height - 2, 5);
            }
        } else {
            let dark = scheme == SP_COLOR_SCHEME_DARK;
            let stripe = match (row % 2 == 0, dark) {
                (true, true) => 0.12,
                (true, false) => -0.10,
                (false, true) => 0.06,
                (false, false) => -0.05,
            };
            g.fill_rectangle(x, y, width, height, base.lighten(stripe));
        }
    }

    fn draw_table_header(&self, g: &dyn SPGraphicsContext, table: &SPTableElement, x: i32, y: i32, width: i32, height: i32) {
        let scheme = table.el.get_color_scheme();
        let base = apply_tint_to_base(self.base.bg(scheme), table.el.get_tint_color());
        g.draw_linear_gradient(&SPRectangle::new(x, y, width, height), &SPGradient::new(base.lighten(0.12), base.lighten(-0.06), 0));
        if self.glossy.get() {
            draw_aero_sheen(g, x, y, width, height, 0);
        }
        g.draw_line(x, y + height - 1, x + width, y + height - 1, sep_color(scheme));
    }

    fn draw_tab_bar(&self, g: &dyn SPGraphicsContext, tab_bar: &SPTabBarElement, x: i32, y: i32, width: i32, height: i32) {
        let base = apply_tint_to_base(self.base.bg(tab_bar.el.get_color_scheme()), tab_bar.el.get_tint_color());
        g.fill_rectangle(x, y, width, height, with_alpha(base, 220));
        if self.glossy.get() {
            draw_aero_sheen(g, x, y, width, height, 0);
        }
    }

    fn draw_tab_bar_tab(&self, g: &dyn SPGraphicsContext, _tab_bar: &SPTabBarElement, _tab: &SPTab, _index: i32, active: bool, x: i32, y: i32, width: i32, height: i32) {
        if !active {
            return;
        }
        // Tropical gradient pill
        let grad = SPGradient::new(self.base.tab_start(), self.base.tab_end(), 1);
        g.fill_rounded_linear_gradient_rectangle(x + 2, y + 2, width - 4, height - 3, 7, 1, &grad);
        if self.glossy.get() {
            draw_aero_sheen(g, x + 2, y + 2, width - 4, height - 3, 7);
        }
    }

    fn draw_tree_item(&self, g: &dyn SPGraphicsContext, tree_view: &SPTreeViewElement, item: &SPTreeItem, item_height: i32, x: i32, y: i32, _width: i32, _height: i32) {
        if !(item.is_selected() || item.is_highlighted()) {
            return;
        }
        let alpha = if item.is_selected() { 170 } else { 75 };
        let w = tree_view.el.get_width() - 8;
        g.fill_rounded_rectangle(x + 4, y + 2, w, item_height - 4, 6, with_alpha(self.base.accent(), alpha));
        if self.glossy.get() && item.is_selected() {
            draw_aero_sheen(g, x + 4, y + 2, w, item_height - 4, 6);
        }
    }

    fn draw_section_header(&self, g: &dyn SPGraphicsContext, tree_view: &SPTreeViewElement, title: &str, x: i32, y: i32, width: i32, height: i32) {
        let label = with_alpha(self.base.text_color(tree_view.el.get_color_scheme()), 160);
        let fs = tree_view.el.get_font_style();
        let style = SPFontStyle::new(&fs.get_family(), fs.get_height() - 2, fs.get_weight(), false, false);
        g.draw_string(title, &style, label, x + 10, y + 3, width - 10, height);
    }
}
