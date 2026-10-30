//! Port of `SPModernSkin`: a procedural "liquid glass" skin.

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

/// Corner radius for the modern liquid glass look — tighter than the classic 25px pill.
const MODERN_RADIUS: i32 = 10;

/// Separator color appropriate for the current color scheme.
pub(crate) fn sep_color(scheme: i16) -> SPColor {
    if scheme == SP_COLOR_SCHEME_DARK {
        SPColor::new(255, 255, 255, 30)
    } else {
        SPColor::new(0, 0, 0, 25)
    }
}

/// Resolve an element's background color with optional tint and dark-mode boost.
pub(crate) fn resolve_background(elm: &SPElement, tint: Option<SPColor>) -> SPColor {
    let mut bg = elm.get_background_color();
    if let Some(tint) = tint {
        bg = bg.tint(&tint);
    }
    if elm.get_color_scheme() == SP_COLOR_SCHEME_DARK {
        bg = bg.lighten(2.0);
    }
    bg
}

fn with_alpha(color: SPColor, alpha: i16) -> SPColor {
    SPColor::new(color.get_r(), color.get_g(), color.get_b(), alpha)
}

/// Draws a liquid-glass specular + refraction pass over an element.
/// Specular: bright highlight along the top portion (light reflecting off glass).
/// Refraction: subtle brightening along the bottom (back-lit translucency).
pub(crate) fn draw_glass_sheen(g: &dyn SPGraphicsContext, x: i32, y: i32, w: i32, h: i32, r: i32) {
    let top_h = h * 2 / 5;
    let bot_h = h - top_h;
    let specular = SPGradient::new(SPColor::new(255, 255, 255, 90), SPColor::new(255, 255, 255, 0), 0);
    g.fill_rounded_linear_gradient_rectangle(x, y, w, top_h, r, 0, &specular);
    let refraction = SPGradient::new(SPColor::new(255, 255, 255, 0), SPColor::new(255, 255, 255, 26), 0);
    g.fill_rounded_linear_gradient_rectangle(x, y + top_h, w, bot_h, r, 0, &refraction);
}

pub struct SPModernSkin {
    base: SPSkinBase,
    glossy: Cell<bool>,
    button_style: Cell<SPButtonStyle>,
}

impl Default for SPModernSkin {
    fn default() -> Self {
        Self {
            base: SPSkinBase::new(),
            glossy: Cell::new(false),
            button_style: Cell::new(SPButtonStyle::Flat),
        }
    }
}

impl SPModernSkin {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SPSkin for SPModernSkin {
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
        let type_ = element.get_type();
        let tint_color = elm.get_tint_color();
        let scheme = elm.get_color_scheme();
        let glossy = self.glossy.get();
        let b = *bounds;
        match type_.as_str() {
            "input" => true,
            "splitter" => {
                g.fill_rectangle(b.x, b.y, b.width, b.height, sep_color(scheme));
                true
            }
            "button" => {
                let mut btn_bg = elm.get_background_color();
                if let Some(tint) = tint_color {
                    if elm.get_string_attribute("variant", "default") == "primary" {
                        btn_bg = tint.alpha(1.0);
                    } else {
                        btn_bg = btn_bg.tint(&tint);
                    }
                }
                if scheme == SP_COLOR_SCHEME_DARK {
                    btn_bg = btn_bg.lighten(2.0);
                }
                if state == "active" {
                    btn_bg = btn_bg.lighten(-0.15);
                } else if state == "hover" {
                    btn_bg = btn_bg.lighten(0.05);
                }
                match self.button_style.get() {
                    SPButtonStyle::Flat => {
                        g.fill_rounded_rectangle(b.x, b.y, b.width, b.height, MODERN_RADIUS, with_alpha(btn_bg, 210));
                    }
                    SPButtonStyle::Gradient => {
                        let grad = SPGradient::new(btn_bg.lighten(0.10), btn_bg.lighten(-0.12), 0);
                        g.fill_rounded_linear_gradient_rectangle(b.x, b.y, b.width, b.height, MODERN_RADIUS, 0, &grad);
                    }
                    SPButtonStyle::ModernGradient => {
                        let (h, s, l) = btn_bg.to_hsl();
                        let sec = SPColor::from_hsl(h + 60.0, s * 0.85, l * 0.85, btn_bg.get_a());
                        let grad = SPGradient::new(btn_bg, sec, 0);
                        g.fill_rounded_linear_gradient_rectangle(b.x, b.y, b.width, b.height, MODERN_RADIUS, 0, &grad);
                    }
                    SPButtonStyle::Aqua => {
                        g.fill_rounded_rectangle(b.x, b.y, b.width, b.height, MODERN_RADIUS, with_alpha(btn_bg, 230));
                        let top_h = b.height * 2 / 5;
                        let hi_spec = SPGradient::new(SPColor::new(255, 255, 255, 165), SPColor::new(255, 255, 255, 0), 0);
                        g.fill_rounded_linear_gradient_rectangle(b.x + 3, b.y + 2, b.width - 6, top_h, MODERN_RADIUS, 0, &hi_spec);
                        let bot_h = (b.height / 7).max(3);
                        let bot_spec = SPGradient::new(SPColor::new(255, 255, 255, 0), SPColor::new(255, 255, 255, 45), 0);
                        g.fill_rounded_linear_gradient_rectangle(b.x + 2, b.y + b.height - bot_h - 2, b.width - 4, bot_h, 3, 0, &bot_spec);
                    }
                }
                // Gloss sheen — independent of button style
                if glossy && state != "active" {
                    draw_glass_sheen(g, b.x, b.y, b.width, b.height, MODERN_RADIUS);
                }
                // Glass rim
                let rim = if scheme == SP_COLOR_SCHEME_DARK {
                    SPColor::new(10, 10, 10, 60)
                } else {
                    SPColor::new(200, 200, 200, 100)
                };
                g.fill_rounded_rectangle(b.x, b.y, b.width, b.height, MODERN_RADIUS, rim);
                true
            }
            "appheader" => {
                let bg = resolve_background(elm, tint_color);
                g.fill_rectangle(b.x, b.y, b.width, b.height, with_alpha(bg, 230));
                if glossy {
                    draw_glass_sheen(g, b.x, b.y, b.width, b.height, 0);
                }
                g.draw_line(b.x, b.y + b.height - 1, b.x + b.width, b.y + b.height - 1, sep_color(scheme));
                true
            }
            "appfooter" => {
                let bg = resolve_background(elm, tint_color);
                g.fill_rectangle(b.x, b.y, b.width, b.height, with_alpha(bg, 220));
                if glossy {
                    draw_glass_sheen(g, b.x, b.y, b.width, b.height, 0);
                }
                g.draw_line(b.x, b.y, b.x + b.width, b.y, sep_color(scheme));
                true
            }
            "treeview" => {
                let bg = elm.get_background_color();
                let bg = if scheme == SP_COLOR_SCHEME_DARK { bg.lighten(0.15) } else { bg.lighten(-0.04) };
                g.fill_rectangle(b.x, b.y, b.width, b.height, bg);
                true
            }
            "view" | "viewstack" => {
                let mut bg = SPColor::new(220, 220, 220, 255);
                if scheme == SP_COLOR_SCHEME_DARK {
                    bg = SPColor::new(28, 28, 28, 255);
                    if let Some(tint) = tint_color {
                        bg = bg.tint(&tint);
                    }
                } else if let Some(tint) = tint_color {
                    let t = tint.get_a() as f32 / 255.0 * 0.25;
                    bg = bg.mix(&tint, t);
                }
                g.fill_rectangle(b.x, b.y, b.width, b.height, bg);
                true
            }
            _ => false,
        }
    }

    fn draw_table_row(&self, g: &dyn SPGraphicsContext, table: &SPTableElement, _i: i32, x: i32, y: i32, width: i32, height: i32, row: i32) {
        let tint_color = table.el.get_tint_color();
        let bg = resolve_background(&table.el, tint_color);
        let dark = table.el.get_color_scheme() == SP_COLOR_SCHEME_DARK;
        if table.is_row_selected(row) {
            // Accent pill for selected row
            let accent = tint_color.map(|t| t.alpha(1.0)).unwrap_or(SPColor::new(99, 149, 234, 255));
            g.fill_rounded_rectangle(x + 2, y + 1, width - 4, height - 2, 6, with_alpha(accent, 180));
        } else if row % 2 == 0 {
            // Very subtle stripe — nearly invisible, just enough to aid scanning
            g.fill_rectangle(x, y, width, height, if dark { bg.lighten(0.1) } else { bg.lighten(-0.1) });
        } else {
            g.fill_rectangle(x, y, width, height, if dark { bg.lighten(0.05) } else { bg.lighten(-0.05) });
        }
    }

    fn draw_table_header(&self, g: &dyn SPGraphicsContext, table: &SPTableElement, x: i32, y: i32, width: i32, height: i32) {
        let bg = resolve_background(&table.el, table.el.get_tint_color());
        let col_count = table
            .data_source()
            .map(|ds| ds.get_column_count())
            .filter(|c| *c > 0)
            .unwrap_or(1);
        let col_width = width / col_count;
        g.fill_rectangle(x, y, width, height, with_alpha(bg, 220));
        if self.glossy.get() {
            draw_glass_sheen(g, x, y, width, height, 0);
        }
        let divider = sep_color(table.el.get_color_scheme());
        for col in 1..col_count {
            let cx = x + col * col_width;
            g.draw_line(cx, y + 3, cx, y + height - 4, divider);
        }
        // Bottom separator
        g.draw_line(x, y + height - 1, x + width, y + height - 1, divider);
    }

    fn draw_tab_bar_tab(&self, g: &dyn SPGraphicsContext, tab_bar: &SPTabBarElement, _tab: &SPTab, _index: i32, active: bool, x: i32, y: i32, width: i32, height: i32) {
        // Inactive tabs: no fill, just spacing — they exist in the empty tab bar background
        if !active {
            return;
        }
        let bg = resolve_background(&tab_bar.el, tab_bar.el.get_tint_color());
        // Active tab: glass pill
        g.fill_rounded_rectangle(x + 2, y + 2, width - 4, height - 3, 7, with_alpha(bg, 200));
        if self.glossy.get() {
            draw_glass_sheen(g, x + 2, y + 2, width - 4, height - 3, 7);
        }
        let rim = if tab_bar.el.get_color_scheme() == SP_COLOR_SCHEME_DARK {
            bg.alpha(1.0)
        } else {
            bg.alpha(1.0).lighten(-0.2)
        };
        g.fill_rounded_rectangle(x + 2, y + 2, width - 4, height - 3, 7, rim);
    }

    fn draw_tab_bar(&self, g: &dyn SPGraphicsContext, tab_bar: &SPTabBarElement, x: i32, y: i32, width: i32, height: i32) {
        let bg = resolve_background(&tab_bar.el, tab_bar.el.get_tint_color());
        g.fill_rectangle(x, y, width, height, with_alpha(bg, 215));
        if self.glossy.get() {
            draw_glass_sheen(g, x, y, width, height, 0);
        }
    }

    fn draw_tree_item(&self, g: &dyn SPGraphicsContext, tree_view: &SPTreeViewElement, item: &SPTreeItem, item_height: i32, x: i32, y: i32, _width: i32, _height: i32) {
        if item.is_selected() || item.is_highlighted() {
            let alpha = if item.is_selected() { 160 } else { 70 };
            // Pill-shaped selection indicator inset from edges
            g.fill_rounded_rectangle(x + 4, y + 2, tree_view.el.get_width() - 8, item_height - 4, 6, SPColor::new(99, 149, 234, alpha));
        }
    }

    fn draw_section_header(&self, g: &dyn SPGraphicsContext, tree_view: &SPTreeViewElement, title: &str, x: i32, y: i32, width: i32, height: i32) {
        // Modern: no background fill — just a muted, small label
        let fs = tree_view.el.get_font_style();
        let style = SPFontStyle::new(&fs.get_family(), fs.get_height() - 2, fs.get_weight(), false, false);
        g.draw_string(title, &style, SPColor::new(150, 150, 150, 180), x + 10, y + 3, width - 10, height);
    }
}
