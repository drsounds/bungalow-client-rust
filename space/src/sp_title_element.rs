//! Port of `SPTitleElement`: an entity label above a large title.

use crate::sp_element::SPElementBase;
use std::cell::RefCell;
use std::rc::Rc;

use crate::impl_element_base;
use crate::sp_color::SPColor;
use crate::sp_element::{construct, ElementRef, SPElementClass};
use crate::sp_text_element::SPTextElement;
use crate::sp_vbox_element::SPVBoxElement;

#[derive(Default)]
pub struct SPTitleElement {
    pub vbox: SPVBoxElement,
    title: RefCell<Option<Rc<SPTextElement>>>,
    entity: RefCell<Option<Rc<SPTextElement>>>,
}

impl_element_base!(SPTitleElement, vbox);

impl SPTitleElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        let el = rc.element();
        let this = el.this();
        el.set_tag_name("title");

        let entity = SPTextElement::new(Some(&this));
        entity.el.set_integer_attribute("height", 180);
        entity.el.set_color_attribute("fgcolor", SPColor::new(10, 10, 10, 1));
        entity.el.set_integer_attribute("size", 15);
        entity.el.set("width", "100%");
        el.append_child_default(&(entity.clone() as ElementRef));

        let title = SPTextElement::new(Some(&this));
        title.el.set_integer_attribute("height", 160);
        title.el.set_integer_attribute("size", 25);
        title.el.set("width", "100%");
        el.append_child_default(&(title.clone() as ElementRef));

        *rc.entity.borrow_mut() = Some(entity);
        *rc.title.borrow_mut() = Some(title);
        rc
    }

    pub fn get_title(&self) -> String {
        self.title.borrow().as_ref().map(|t| t.el.get_inner_text()).unwrap_or_default()
    }

    pub fn set_title(&self, title: &str) {
        if let Some(t) = self.title.borrow().as_ref() {
            t.el.set_inner_text(title);
        }
    }

    pub fn get_entity(&self) -> String {
        self.entity.borrow().as_ref().map(|t| t.el.get_inner_text()).unwrap_or_default()
    }

    /// The C++ setter wrote the title label; this writes the entity label.
    pub fn set_entity(&self, entity: &str) {
        if let Some(t) = self.entity.borrow().as_ref() {
            t.el.set_inner_text(entity);
        }
    }
}

impl SPElementClass for SPTitleElement {
    fn get_type(&self) -> String {
        "title".into()
    }
    fn pack(&self) {
        SPVBoxElement::pack_vbox(self.element());
    }
}
