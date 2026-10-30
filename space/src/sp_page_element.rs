//! Port of `SPPageElement`: a flexible vertical page.

use crate::sp_element::SPElementBase;
use std::rc::Rc;

use crate::impl_element_base;
use crate::sp_element::{construct, ElementRef, SPElementClass};
use crate::sp_vbox_element::SPVBoxElement;

#[derive(Default)]
pub struct SPPageElement {
    pub vbox: SPVBoxElement,
}

impl_element_base!(SPPageElement, vbox);

impl SPPageElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.element().set_tag_name("page");
        rc.element().set_flex(1);
        rc
    }
}

impl SPElementClass for SPPageElement {
    fn get_type(&self) -> String {
        "page".into()
    }
    fn pack(&self) {
        SPVBoxElement::pack_vbox(self.element());
    }
}
