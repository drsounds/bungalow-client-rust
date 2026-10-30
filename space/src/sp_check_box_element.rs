//! Port of `SPCheckBoxElement`.

use std::rc::Rc;

use crate::impl_element_base;
use crate::libspace::SPRectangle;
use crate::sp_color::SPColor;
use crate::sp_element::{construct, ElementRef, SPElement, SPElementClass};
use crate::sp_font_style::SPFontStyle;
use crate::sp_graphics_context::SPGraphicsContext;

const LABEL_GAP: i32 = 6;

#[derive(Default)]
pub struct SPCheckBoxElement {
    pub el: SPElement,
}

impl_element_base!(SPCheckBoxElement, el);

impl SPCheckBoxElement {
    pub const BOX_SIZE: i32 = 16;

    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.el.set_tag_name("checkbox");
        rc
    }

    pub fn is_checked(&self) -> bool {
        matches!(self.el.get_string_attribute("checked", "false").as_str(), "true" | "checked" | "1")
    }

    pub fn set_checked(&self, checked: bool) {
        self.el.set_string_attribute("checked", if checked { "true" } else { "false" });
        self.el.invalidate();
    }
}

impl SPElementClass for SPCheckBoxElement {
    fn get_type(&self) -> String {
        "checkbox".into()
    }

    fn pack(&self) {
        self.el.pack_default();
        if self.el.get_flex() > 0 {
            return;
        }
        if self.el.get_height() <= 0 {
            self.el.set_height(Self::BOX_SIZE + 4);
        }
        if self.el.get_width() <= 0 {
            let text = self.el.get_inner_text();
            let text_width = if text.is_empty() { 0 } else { LABEL_GAP + self.el.get_text_bounds().width };
            self.el.set_width(Self::BOX_SIZE + text_width);
        }
    }

    fn paint_content(&self, x: i32, y: i32, width: i32, height: i32, c: &dyn SPGraphicsContext) {
        let checked = self.is_checked();
        let state = if checked { "checked" } else { "" };
        let this = self.el.this();
        let box_size = Self::BOX_SIZE.min(height);
        let rect = SPRectangle::new(x, y + (height - box_size) / 2, box_size, box_size);
        let skin = self.el.get_skin();
        if !skin.as_ref().is_some_and(|skin| skin.draw_element(c, &this, state, &rect)) {
            let fg = self.el.get_foreground_color();
            c.fill_rectangle(rect.x, rect.y, rect.width, rect.height, SPColor::new(255, 255, 255, 255));
            c.draw_rectangle(rect.x, rect.y, rect.width, rect.height, fg);
            if checked {
                let (bx, by, s) = (rect.x, rect.y, rect.width);
                c.draw_line(bx + s / 4, by + s / 2, bx + s * 2 / 5, by + s * 3 / 4, fg);
                c.draw_line(bx + s * 2 / 5, by + s * 3 / 4, bx + s * 3 / 4, by + s / 4, fg);
            }
        }
        let text = self.el.get_inner_text();
        if text.is_empty() {
            return;
        }
        let font_size = self.el.get_integer_attribute("size", 11);
        let font_family = self.el.get_string_attribute("font", "Tahoma");
        let font = SPFontStyle::new(&font_family, font_size, font_size / 2, false, false);
        let text_size = c.measure_string(&text, &font);
        let text_x = x + box_size + LABEL_GAP;
        let color =
            skin.and_then(|skin| skin.get_text_color(&this, state)).unwrap_or_else(|| self.el.get_foreground_color());
        c.draw_string(
            &text,
            &font,
            color,
            text_x,
            y + (height - text_size.height) / 2,
            (width - box_size - LABEL_GAP).max(0),
            height,
        );
    }

    fn on_click(&self, button: i32, x: i32, y: i32) {
        self.set_checked(!self.is_checked());
        self.el.on_click_default(button, x, y);
    }
}
