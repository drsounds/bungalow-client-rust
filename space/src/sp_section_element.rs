//! Port of `SPSectionElement`: a flexible vertical section.

use crate::sp_element::SPElementBase;
use std::rc::Rc;

use crate::impl_element_base;
use crate::sp_element::{construct, ElementRef, SPElementClass};
use crate::sp_vbox_element::SPVBoxElement;

#[derive(Default)]
pub struct SPSectionElement {
    pub vbox: SPVBoxElement,
}

impl_element_base!(SPSectionElement, vbox);

impl SPSectionElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.element().set_tag_name("section");
        rc.element().set_flex(1);
        rc
    }
}

impl SPElementClass for SPSectionElement {
    fn get_type(&self) -> String {
        "section".into()
    }
    fn pack(&self) {
        SPVBoxElement::pack_vbox(self.element());
    }
}
