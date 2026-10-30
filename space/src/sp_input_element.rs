//! Port of `SPInputElement`: a text field. The backend draws the edit control
//! (`draw_input_element`) and reports typed text back through `set_value`.

use std::cell::RefCell;
use std::rc::Rc;

use crate::impl_element_base;
use crate::sp_element::{construct, ElementRef, SPElement, SPElementClass};
use crate::sp_graphics_context::SPGraphicsContext;

#[derive(Default)]
pub struct SPInputElement {
    pub el: SPElement,
    value: RefCell<String>,
}

impl_element_base!(SPInputElement, el);

impl SPInputElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.el.set_tag_name("input");
        rc
    }

    pub fn get_value(&self) -> String {
        self.value.borrow().clone()
    }

    pub fn set_value(&self, value: &str) {
        *self.value.borrow_mut() = value.to_string();
        if let Some(gc) = self.create_graphics() {
            gc.set_control_attribute(&self.el.this(), "value", value);
        }
    }
}

impl SPElementClass for SPInputElement {
    fn get_type(&self) -> String {
        "input".into()
    }
    fn paint(&self, x: i32, y: i32, g: &dyn SPGraphicsContext) {
        self.el.paint_default(x, y, g);
        let this = self.el.this();
        let absolute = self.el.get_absolute_bounds();
        let bounds = match self.el.get_skin() {
            Some(skin) => skin.get_content_bounds(&this, &absolute),
            None => absolute,
        };
        g.draw_input_element(&this, bounds.x, bounds.y, bounds.width, bounds.height);
    }
}
