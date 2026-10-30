//! Port of `SPTabStackElement`: shows the child whose id is the active tab.

use std::cell::RefCell;
use std::rc::Rc;

use crate::impl_element_base;
use crate::sp_element::{construct, ElementRef, SPElement, SPElementClass};

#[derive(Default)]
pub struct SPTabStackElement {
    pub el: SPElement,
    tab_id: RefCell<String>,
}

impl_element_base!(SPTabStackElement, el);

impl SPTabStackElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        construct(Self::default(), parent)
    }

    pub fn get_tab_id(&self) -> String {
        self.tab_id.borrow().clone()
    }

    pub fn set_tab_id(&self, tab_id: &str) {
        *self.tab_id.borrow_mut() = tab_id.to_string();
        for child in self.el.get_child_nodes() {
            child.set_visible(child.element().get_id() == tab_id);
        }
        self.pack();
    }
}

impl SPElementClass for SPTabStackElement {
    fn get_type(&self) -> String {
        "tabstack".into()
    }
    fn pack(&self) {
        for child in self.el.get_child_nodes() {
            child.element().set_width(self.el.get_width());
            child.element().set_height(self.el.get_height());
            child.pack();
        }
    }
}
