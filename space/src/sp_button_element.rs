//! Port of `SPButtonElement`: a push button painted by the skin.

use std::rc::Rc;

use crate::impl_element_base;
use crate::libspace::SPRectangle;
use crate::sp_element::{construct, state_class, ElementRef, SPElement, SPElementClass};
use crate::sp_graphics_context::SPGraphicsContext;

#[derive(Default)]
pub struct SPButtonElement {
    pub el: SPElement,
}

impl_element_base!(SPButtonElement, el);

impl SPButtonElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.el.set_tag_name("button");
        rc
    }
}

impl SPElementClass for SPButtonElement {
    fn get_type(&self) -> String {
        "button".into()
    }
    fn pack(&self) {
        self.el.pack_default();
        self.el.resize_to_text(true, true);
    }
    fn paint(&self, x: i32, y: i32, c: &dyn SPGraphicsContext) {
        let bounds = SPRectangle::new(self.el.get_x() + x, self.el.get_y() + y, self.el.get_width(), self.el.get_height());
        self.el.set_absolute_bounds(bounds);
        if let Some(skin) = self.el.get_skin() {
            skin.draw_element(c, &self.el.this(), state_class(self.el.get_state()), &bounds);
        }
    }
}
