//! Port of `SPLinkElement`.

use crate::sp_element::SPElementBase;
use std::rc::Rc;

use crate::impl_element_base;
use crate::sp_element::{construct, ElementRef, SPElementClass};
use crate::sp_text_element::SPTextElement;

#[derive(Default)]
pub struct SPLinkElement {
    pub text: SPTextElement,
}

impl_element_base!(SPLinkElement, text);

impl SPLinkElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.element().set_tag_name("link");
        rc
    }
}

impl SPElementClass for SPLinkElement {
    fn get_type(&self) -> String {
        "link".into()
    }
}
