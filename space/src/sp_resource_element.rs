//! Port of `SPResourceElement`: draws a resource's name with an underline.

use std::cell::RefCell;
use std::rc::Rc;

use crate::impl_element_base;
use crate::libspace::SPRectangle;
use crate::sp_color::SPColor;
use crate::sp_element::{construct, ElementRef, SPElement, SPElementClass};
use crate::sp_graphics_context::SPGraphicsContext;
use crate::sp_resource::SPResource;

#[derive(Default)]
pub struct SPResourceElement {
    pub el: SPElement,
    resource: RefCell<Option<Rc<RefCell<SPResource>>>>,
}

impl_element_base!(SPResourceElement, el);

impl SPResourceElement {
    pub fn new(resource: Option<Rc<RefCell<SPResource>>>, parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.el.set_tag_name("resource");
        *rc.resource.borrow_mut() = resource;
        rc
    }
}

impl SPElementClass for SPResourceElement {
    fn paint(&self, x: i32, y: i32, g: &dyn SPGraphicsContext) {
        let font = self.el.get_font_style();
        let fg_color = self.el.get_color_attribute("fgcolor", SPColor::new(255, 255, 255, 255));
        let Some(resource) = self.resource.borrow().clone() else {
            return;
        };
        let abs_x = x + self.el.get_x();
        let abs_y = y + self.el.get_y();
        let (width, height) = (self.el.get_width(), self.el.get_height());
        self.el.set_absolute_bounds(SPRectangle::new(abs_x, abs_y, width, height));
        let title = resource.borrow().name.clone();
        let txt_size = g.measure_string(&title, &font);
        let top = abs_y + (height / 2) - (txt_size.height / 2);
        g.draw_string(&title, &font, fg_color, abs_x + 10, top, width, height);
        let bottom = abs_y + height - 1;
        g.draw_line(abs_x, bottom, abs_x + width, bottom, fg_color);
    }
}
