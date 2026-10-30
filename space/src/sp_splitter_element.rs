//! Port of `SPSplitterElement`: a thin divider drawn by the skin.

use std::rc::Rc;

use crate::impl_element_base;
use crate::sp_element::{construct, ElementRef, SPElement, SPElementClass};
use crate::sp_graphics_context::SPGraphicsContext;

#[derive(Default)]
pub struct SPSplitterElement {
    pub el: SPElement,
}

impl_element_base!(SPSplitterElement, el);

impl SPSplitterElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.el.set_tag_name("splitter");
        rc.el.set_width(1);
        rc
    }
}

impl SPElementClass for SPSplitterElement {
    fn paint_content(&self, _x: i32, _y: i32, _width: i32, _height: i32, c: &dyn SPGraphicsContext) {
        if let Some(skin) = self.el.get_skin() {
            skin.draw_element(c, &self.el.this(), "", &self.el.get_absolute_bounds());
        }
    }
}
