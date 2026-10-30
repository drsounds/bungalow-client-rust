//! Port of `SPClassicSkin`: the glossy 2009-era Bungalow look.

use std::cell::Cell;

use crate::libspace::SPRectangle;
use crate::sp_application_context;
use crate::sp_color::{SPColor, SPGradient};
use crate::sp_element::{ElementRef, SP_COLOR_SCHEME_DARK};
use crate::sp_font_style::SPFontStyle;
use crate::sp_graphics_context::SPGraphicsContext;
use crate::sp_skin::{SPButtonStyle, SPSkin, SPSkinBase};
use crate::sp_tab_bar_element::{SPTab, SPTabBarElement, SPTabToolButton};
use crate::sp_table_element::SPTableElement;
use crate::sp_tree_item::SPTreeItem;
use crate::sp_tree_view_element::SPTreeViewElement;

pub struct SPClassicSkin {
    base: SPSkinBase,
    light_color: SPColor,
    shadow_color: SPColor,
    glossy: Cell<bool>,
    button_style: Cell<SPButtonStyle>,
}

impl Default for SPClassicSkin {
    fn default() -> Self {
        Self {
            base: SPSkinBase::new(),
            light_color: SPColor::new(170, 170, 170, 255),
            shadow_color: SPColor::new(120, 120, 120, 255),
            glossy: Cell::new(true),
            button_style: Cell::new(SPButtonStyle::Gradient),
        }
    }
}

impl SPClassicSkin {
    pub fn new() -> Self {
        Self::default()
    }

    fn linear(g: &dyn SPGraphicsContext, x: i32, y: i32, w: i32, h: i32, gradient: SPGradient) {
        g.draw_linear_gradient(&SPRectangle::new(x, y, w, h), &gradient);
    }
}

impl SPSkin for SPClassicSkin {
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
        let b = *bounds;
        let mut bg_color = elm.get_background_color();
        let mut shadow_color = self.shadow_color;
        let tint_color = elm.get_tint_color();
        let dark = elm.get_color_scheme() == SP_COLOR_SCHEME_DARK;
        let glossy = self.glossy.get() && state != "active";
        if let Some(tint) = tint_color {
            bg_color = bg_color.tint(&tint);
            shadow_color = tint.lighten(-0.0);
        }
        match type_.as_str() {
            "input" => true,
            "splitter" => {
                g.fill_rectangle(b.x, b.y, b.width, b.height, shadow_color);
                true
            }
            "button" | "nowplayingbarheader" | "nowplayingbartitle" => {
                let radius = if type_ == "button" { 25 } else { 0 };
                let mut bg = elm.get_background_color();
                if let Some(tint) = tint_color {
                    bg = if elm.get_string_attribute("variant", "default") == "primary" {
                        tint.alpha(1.0)
                    } else {
                        bg.tint(&tint).alpha(1.0)
                    };
                }
                if dark {
                    bg = bg.lighten(10.0);
                }
                if state == "active" {
                    bg = bg.lighten(-0.15);
                } else if state == "hover" {
                    bg = bg.lighten(0.05);
                }
                let mut grad = SPGradient::new(bg.lighten(0.50), bg.lighten(-0.40), 0);
                if state == "active" {
                    grad = SPGradient::new(grad.get_end(), grad.get_start(), 0);
                }
                g.fill_rounded_linear_gradient_rectangle(b.x, b.y, b.width, b.height, radius, 0, &grad);
                if glossy {
                    let gloss = SPGradient::new(bg.lighten(0.2), bg.lighten(0.3), 0);
                    g.fill_rounded_linear_gradient_rectangle(b.x + 5, b.y + 2, b.width - 10, b.height / 2, radius, 0, &gloss);
                    let gloss2 = SPGradient::new(bg.lighten(-0.2).alpha(0.0), bg.lighten(0.0), 0);
                    g.fill_rounded_linear_gradient_rectangle(b.x + 10, b.y + b.height / 2 - 2, b.width - 20, b.height / 2, radius, 0, &gloss2);
                }
                g.draw_rounded_rectangle(b.x, b.y, b.width, b.height, radius, bg.lighten(-0.5));
                let text = elm.get_inner_text();
                if !text.is_empty() {
                    let font = elm.get_font_style();
                    let size = g.measure_string(&text, &font);
                    let abs = elm.get_absolute_bounds();
                    let x = elm.get_width() / 2 - size.width / 2 + abs.x;
                    let y = elm.get_height() / 2 - size.height / 2 + abs.y;
                    g.draw_string(&text, &font, SPColor::new(10, 10, 10, 255), x, y, size.width, size.height);
                }
                true
            }
            "appheader" => {
                // Re-get fresh background color (same pattern as button) to avoid
                // double-tinting from the outer bg computation at the top.
                let mut bg = elm.get_background_color();
                if dark {
                    bg = bg.lighten(2.0);
                }
                Self::linear(g, b.x, b.y, b.width, b.height, SPGradient::new(bg.lighten(0.1), bg.lighten(-0.1), 0));
                if glossy {
                    Self::linear(g, b.x, b.y + 1, b.width, b.height / 2, SPGradient::new(bg.lighten(0.2), bg.lighten(0.0), 0));
                }
                g.draw_line(b.x, b.y + b.height - 1, b.x + b.width, b.y + b.height - 1, bg.lighten(-0.5));
                true
            }
            "appfooter" => {
                Self::linear(g, b.x, b.y, b.width, b.height, SPGradient::new(bg_color.lighten(0.2), bg_color.lighten(0.1), 0));
                if glossy {
                    Self::linear(g, b.x, b.y + 2, b.width, b.height / 2, SPGradient::new(bg_color.lighten(0.2), bg_color.lighten(0.3), 0));
                    Self::linear(
                        g,
                        b.x,
                        b.y + b.height / 2 - 2,
                        b.width,
                        b.height / 2,
                        SPGradient::new(bg_color.lighten(-0.2).alpha(0.0), bg_color.lighten(0.0), 0),
                    );
                }
                true
            }
            "treeview" => {
                if dark {
                    bg_color = bg_color.lighten(0.28);
                }
                g.fill_rectangle(b.x, b.y, b.width, b.height, bg_color);
                true
            }
            "viewstack" => {
                g.fill_rectangle(b.x, b.y, b.width, b.height, bg_color);
                true
            }
            _ => false,
        }
    }

    fn draw_table_row(&self, g: &dyn SPGraphicsContext, table: &SPTableElement, _i: i32, x: i32, y: i32, width: i32, height: i32, row: i32) {
        if table.is_row_selected(row) {
            g.fill_rectangle(x, y, width, height, SPColor::new(166, 218, 251, 255));
            return;
        }
        if row % 2 == 0 {
            let mut bg = table.el.get_background_color();
            if let Some(tint) = table.el.get_tint_color() {
                bg = bg.tint(&tint);
            }
            // Semi-transparent darker tint for even rows
            let stripe = bg.lighten(-0.2).alpha(100.0 / 255.0);
            if !table.gradient_view.get() {
                g.fill_rectangle(x, y, width, height, stripe);
            }
        }
        if let (Some(app), Some(data_source)) = (sp_application_context::current(), table.data_source()) {
            if app.has_current_playable()
                && row < data_source.get_row_count()
                && data_source.get_row_uri(row) == app.current_playable_uri()
            {
                g.fill_rectangle(x, y, width, height, SPColor::new(0, 0, 0, 255));
            }
        }
    }

    fn draw_table_header(&self, g: &dyn SPGraphicsContext, table: &SPTableElement, x: i32, y: i32, width: i32, height: i32) {
        let mut bg = table.el.get_background_color();
        if let Some(tint) = table.el.get_tint_color() {
            bg = bg.tint(&tint);
        }
        if table.el.get_color_scheme() == SP_COLOR_SCHEME_DARK {
            bg = bg.lighten(0.2);
        }
        let col_count = table
            .data_source()
            .map(|ds| ds.get_column_count())
            .filter(|c| *c > 0)
            .unwrap_or(1);
        let col_width = width / col_count;
        // Draw each column header cell like a button (default gradient + gloss)
        for col in 0..col_count {
            let cx = x + col * col_width;
            let cw = if col == col_count - 1 { width - col * col_width } else { col_width };
            // Base button gradient: lighter top → darker bottom
            Self::linear(g, cx, y, cw, height, SPGradient::new(bg.lighten(0.1), bg.lighten(-0.2), 0));
            // Gloss upper highlight
            if self.glossy.get() {
                Self::linear(g, cx + 2, y + 1, cw - 4, height / 2, SPGradient::new(bg.lighten(0.25), bg.lighten(0.15), 0));
                Self::linear(
                    g,
                    cx + 2,
                    y + height / 2,
                    cw - 4,
                    height / 2 - 1,
                    SPGradient::new(bg.lighten(-0.15).alpha(0.0), bg.lighten(0.05), 0),
                );
            }
            // Column divider (shadow line on right edge)
            if col < col_count - 1 {
                g.draw_line(cx + cw - 1, y, cx + cw - 1, y + height - 1, bg.lighten(-0.5));
            }
        }
        // Bottom border — same shadow line as button border
        g.draw_line(x, y + height - 1, x + width, y + height - 1, bg.lighten(-0.5));
    }

    fn draw_tab_bar_tool_button(&self, g: &dyn SPGraphicsContext, tab_bar: &SPTabBarElement, _button: &SPTabToolButton, x: i32, y: i32, width: i32, height: i32) {
        let mut bg = tab_bar.el.get_background_color();
        let tint = tab_bar.el.get_tint_color();
        if let Some(tint) = tint {
            bg = bg.tint(&tint);
        }
        if tab_bar.el.get_color_scheme() == SP_COLOR_SCHEME_DARK {
            bg = bg.lighten(0.2);
        }
        // Subtle inset button appearance matching the tab bar style
        Self::linear(g, x + 1, y + 2, width - 2, height - 4, SPGradient::new(bg.lighten(0.05), bg.lighten(-0.15), 0));
        let border = tint.map(|t| t.lighten(-0.3)).unwrap_or(self.shadow_color);
        g.draw_rounded_rectangle(x + 1, y + 2, width - 2, height - 4, 20, border);
        // Vertical divider on the left
        g.draw_line(x, y, x, y + height - 1, border);
    }

    fn draw_tab_bar_tab(&self, g: &dyn SPGraphicsContext, tab_bar: &SPTabBarElement, _tab: &SPTab, index: i32, active: bool, x: i32, y: i32, width: i32, height: i32) {
        let mut bg = tab_bar.el.get_background_color();
        let mut active_bg = bg;
        let mut shadow = self.shadow_color;
        if let Some(tint) = tab_bar.el.get_tint_color() {
            bg = bg.tint(&tint);
            active_bg = bg;
            shadow = tint.lighten(-0.0);
        }
        if tab_bar.el.get_color_scheme() == SP_COLOR_SCHEME_DARK {
            shadow = shadow.lighten(-0.5);
        }
        if active {
            g.fill_rectangle(x, y, width, height, active_bg);
            if index > 0 {
                g.draw_line(x, y, x, y + height - 1, shadow);
            }
        }
        g.draw_line(x + width, y, x + width, y + height - 1, shadow);
    }

    fn draw_tab_bar(&self, g: &dyn SPGraphicsContext, tab_bar: &SPTabBarElement, x: i32, y: i32, width: i32, height: i32) {
        let mut shadow = self.shadow_color;
        let mut bg = tab_bar.el.get_background_color();
        if let Some(tint) = tab_bar.el.get_tint_color() {
            bg = bg.tint(&tint);
            shadow = tint.lighten(-0.0);
        }
        if tab_bar.el.get_color_scheme() == SP_COLOR_SCHEME_DARK {
            bg = bg.lighten(0.5);
            shadow = shadow.lighten(-0.5);
        }
        let _ = self.light_color;
        Self::linear(g, x, y, width, height, SPGradient::new(bg.lighten(0.1), bg.lighten(-0.1), 0));
        g.draw_line(x, y + height - 1, x + width, y + height - 1, shadow);
    }

    fn draw_tree_item(&self, g: &dyn SPGraphicsContext, tree_view: &SPTreeViewElement, item: &SPTreeItem, item_height: i32, x: i32, y: i32, _width: i32, _height: i32) {
        if item.is_selected() {
            g.fill_rectangle(x, y, tree_view.el.get_width(), item_height, SPColor::new(166, 218, 251, 255));
        }
        if item.is_highlighted() {
            g.fill_rectangle(x, y, tree_view.el.get_width(), item_height, SPColor::new(166, 218, 251, 128));
        }
    }

    fn draw_section_header(&self, g: &dyn SPGraphicsContext, tree_view: &SPTreeViewElement, title: &str, x: i32, y: i32, width: i32, height: i32) {
        // Muted label — slightly smaller font
        let fs = tree_view.el.get_font_style();
        let style = SPFontStyle::new(&fs.get_family(), fs.get_height() - 1, fs.get_weight(), false, false);
        g.draw_string(title, &style, SPColor::new(128, 128, 128, 200), x + 8, y + 4, width - 8, height);
    }
}
