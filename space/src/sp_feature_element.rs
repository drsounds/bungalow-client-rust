//! Port of `SPFeatureElement`: a view stack of registered feature views.

use crate::sp_element::SPElementBase;
use std::cell::RefCell;
use std::rc::Rc;

use crate::impl_element_base;
use crate::sp_element::{construct, ElementRef, SPElementClass};
use crate::sp_view_stack_element::SPViewStackElement;

#[derive(Default)]
pub struct SPFeatureElement {
    pub stack: SPViewStackElement,
    views: RefCell<Vec<ElementRef>>,
}

impl_element_base!(SPFeatureElement, stack);

impl SPFeatureElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.element().set_tag_name("feature");
        rc
    }

    pub fn register_view(&self, view: &ElementRef) {
        self.views.borrow_mut().push(view.clone());
        self.append_child(view);
    }
}

impl SPElementClass for SPFeatureElement {
    fn get_type(&self) -> String {
        "viewstack".into()
    }
    fn pack(&self) {
        self.stack.pack();
    }
    fn append_child(&self, child: &ElementRef) {
        self.stack.append_child(child);
    }
}
