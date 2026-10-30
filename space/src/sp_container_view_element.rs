//! Port of `SPContainerViewElement`.

use crate::sp_element::SPElementBase;
use std::rc::Rc;

use crate::impl_element_base;
use crate::sp_element::{construct, ElementRef, SPElementClass};
use crate::sp_view_element::{SPViewClass, SPViewElement};

#[derive(Default)]
pub struct SPContainerViewElement {
    pub view: SPViewElement,
}

impl_element_base!(SPContainerViewElement, view);

impl SPContainerViewElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        SPViewElement::init_view(&rc.view);
        rc
    }
}

impl SPViewClass for SPContainerViewElement {
    fn view(&self) -> &SPViewElement {
        &self.view
    }
}

impl SPElementClass for SPContainerViewElement {
    fn get_type(&self) -> String {
        "view".into()
    }
    fn pack(&self) {
        SPViewElement::pack_view(self.element());
    }
    fn scroll(&self, scroll_x: i32, scroll_y: i32, _x: i32, _y: i32) {
        SPViewElement::scroll_view(self.element(), scroll_x, scroll_y);
    }
    fn as_view(&self) -> Option<&dyn SPViewClass> {
        Some(self)
    }
}
