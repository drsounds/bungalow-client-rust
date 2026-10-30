//! Port of `SPAppHeaderElement`.

use crate::sp_element::SPElementBase;
use std::rc::Rc;

use crate::impl_element_base;
use crate::sp_element::{construct, ElementRef, SPElementClass};
use crate::sp_hbox_element::SPHBoxElement;

#[derive(Default)]
pub struct SPAppHeaderElement {
    pub hbox: SPHBoxElement,
}

impl_element_base!(SPAppHeaderElement, hbox);

impl SPAppHeaderElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.element().set_tag_name("appheader");
        rc
    }
}

impl SPElementClass for SPAppHeaderElement {
    fn get_type(&self) -> String {
        "appheader".into()
    }
    fn pack(&self) {
        SPHBoxElement::pack_hbox(self.element());
    }
}
