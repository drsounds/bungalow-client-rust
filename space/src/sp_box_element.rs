//! Port of `SPBoxElement`: stretches its first child over its whole area.

use std::rc::Rc;

use crate::impl_element_base;
use crate::sp_element::{construct, ElementRef, SPElement, SPElementClass};

#[derive(Default)]
pub struct SPBoxElement {
    pub el: SPElement,
}

impl_element_base!(SPBoxElement, el);

impl SPBoxElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.el.set_tag_name("box");
        rc
    }
}

impl SPElementClass for SPBoxElement {
    fn get_type(&self) -> String {
        "box".into()
    }
    fn pack(&self) {
        self.el.pack_default();
        let Some(child) = self.el.get_child_nodes().into_iter().next() else {
            return;
        };
        let c = child.element();
        c.set_x(0);
        c.set_y(0);
        c.set_width(self.el.get_width());
        c.set_height(self.el.get_height());
        child.pack();
    }
}
