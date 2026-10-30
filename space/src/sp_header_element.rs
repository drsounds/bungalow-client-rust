//! Port of `SPHeaderElement`: an image next to a title block.

use crate::sp_element::SPElementBase;
use std::cell::RefCell;
use std::rc::Rc;

use crate::impl_element_base;
use crate::libspace::SPMargin;
use crate::sp_element::{construct, ElementRef, SPElementClass};
use crate::sp_hbox_element::SPHBoxElement;
use crate::sp_image_element::SPImageElement;
use crate::sp_title_element::SPTitleElement;

#[derive(Default)]
pub struct SPHeaderElement {
    pub hbox: SPHBoxElement,
    pub title: RefCell<Option<Rc<SPTitleElement>>>,
    pub image: RefCell<Option<Rc<SPImageElement>>>,
}

impl_element_base!(SPHeaderElement, hbox);

impl SPHeaderElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        let el = rc.element();
        let this = el.this();
        el.set_tag_name("header");
        el.set_padding(SPMargin::new(5, 5, 5, 5));

        let image = SPImageElement::new(Some(&this));
        image.el.set("width", "128");
        image.el.set("height", "128");
        image.el.set("flex", "0");
        el.append_child_default(&(image.clone() as ElementRef));

        let title = SPTitleElement::new(Some(&this));
        title.element().set("flex", "1");
        title.set_title("Title");
        title.set_entity("Object");
        title.element().set("height", "100%");
        title.element().set("width", "100%");
        el.append_child_default(&(title.clone() as ElementRef));

        *rc.image.borrow_mut() = Some(image);
        *rc.title.borrow_mut() = Some(title);
        rc
    }
}

impl SPElementClass for SPHeaderElement {
    fn get_type(&self) -> String {
        "headerelement".into()
    }
    fn pack(&self) {
        SPHBoxElement::pack_hbox(self.element());
    }
}
