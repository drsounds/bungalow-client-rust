//! Port of `SPFlowElement`: stacks visible children top to bottom at full width.

use std::rc::Rc;

use crate::impl_element_base;
use crate::sp_element::{construct, ElementRef, SPElement, SPElementClass};

#[derive(Default)]
pub struct SPFlowElement {
    pub el: SPElement,
}

impl_element_base!(SPFlowElement, el);

impl SPFlowElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.el.set_tag_name("flow");
        rc
    }
}

impl SPElementClass for SPFlowElement {
    fn get_type(&self) -> String {
        "flow".into()
    }
    fn pack(&self) {
        let mut top = 0;
        for child in self.el.get_child_nodes() {
            let c = child.element();
            if !c.is_visible() {
                continue;
            }
            c.set_y(top);
            c.set_width(self.el.get_width());
            child.pack();
            top += c.get_height();
        }
    }
}
