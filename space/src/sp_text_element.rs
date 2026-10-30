//! Port of `SPTextElement`.

use std::rc::Rc;

use crate::impl_element_base;
use crate::sp_element::{construct, ElementRef, SPElement, SPElementClass};

#[derive(Default)]
pub struct SPTextElement {
    pub el: SPElement,
}

impl_element_base!(SPTextElement, el);

impl SPTextElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.el.set_tag_name("text");
        rc
    }
}

impl SPElementClass for SPTextElement {
    fn get_type(&self) -> String {
        "text".into()
    }
}
