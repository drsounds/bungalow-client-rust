//! Port of `SPImageElement`.

use std::cell::RefCell;
use std::rc::Rc;

use crate::impl_element_base;
use crate::libspace::SPRectangle;
use crate::sp_element::{construct, ElementRef, SPElement, SPElementClass};
use crate::sp_graphics_context::SPGraphicsContext;
use crate::sp_image::SPImage;

#[derive(Default)]
pub struct SPImageElement {
    pub el: SPElement,
    uri: RefCell<String>,
    image: RefCell<Option<Rc<SPImage>>>,
}

impl_element_base!(SPImageElement, el);

impl SPImageElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.el.set_tag_name("img");
        rc
    }

    pub fn get_src(&self) -> String {
        self.uri.borrow().clone()
    }

    pub fn set_src(&self, value: &str) {
        *self.uri.borrow_mut() = value.to_string();
        if let Some(gc) = self.el.get_space().and_then(|space| space.get_graphics_context()) {
            *self.image.borrow_mut() = gc.load_image(value);
        }
    }

    pub fn set_image(&self, image: Option<Rc<SPImage>>) {
        *self.image.borrow_mut() = image;
    }
}

impl SPElementClass for SPImageElement {
    fn get_type(&self) -> String {
        "img".into()
    }
    fn paint(&self, x: i32, y: i32, g: &dyn SPGraphicsContext) {
        let abs_x = x + self.el.get_x();
        let abs_y = y + self.el.get_y();
        self.el
            .set_absolute_bounds(SPRectangle::new(abs_x, abs_y, self.el.get_width(), self.el.get_height()));
        if let Some(image) = self.image.borrow().as_ref() {
            g.draw_image(image, abs_x, abs_y, self.el.get_width(), self.el.get_height());
        }
    }
}
