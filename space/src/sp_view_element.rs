//! Port of `SPViewElement`, the base of every navigable view.
//!
//! Views are elements with extra virtual methods (`navigate`, `acceptsUri`,
//! `setHash`...). Those live in [`SPViewClass`]; an element exposes it through
//! [`crate::sp_element::SPElementClass::as_view`], the counterpart of
//! `dynamic_cast<SPViewElement *>`.

use crate::sp_element::SPElementBase;
use std::cell::RefCell;
use std::rc::Rc;

use crate::impl_element_base;
use crate::sp_element::{construct, ElementRef, SPElementClass};
use crate::sp_vbox_element::SPVBoxElement;

/// The virtual methods `SPViewElement` adds to `SPElement`.
#[allow(unused_variables)]
pub trait SPViewClass {
    /// The `SPViewElement` part of this view.
    fn view(&self) -> &SPViewElement;

    fn navigate(&self, uri: &str) {}
    fn navigate_back(&self, uri: &str) {
        self.navigate(uri);
    }
    fn navigate_forward(&self, uri: &str) {
        self.navigate(uri);
    }
    fn accepts_uri(&self, uri: &str) -> bool {
        false
    }
    fn get_hash(&self) -> String {
        self.view().hash.borrow().clone()
    }
    fn set_hash(&self, hash: &str) {
        self.view().set_hash_default(hash);
    }
}

#[derive(Default)]
pub struct SPViewElement {
    pub vbox: SPVBoxElement,
    hash: RefCell<String>,
}

impl_element_base!(SPViewElement, vbox);

impl SPViewElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        Self::init_view(&rc);
        rc
    }

    /// The `SPViewElement(SPElement *parent)` constructor body.
    pub fn init_view(view: &SPViewElement) {
        view.element().set_tag_name("view");
        view.element().set_clip_view(true);
    }

    pub fn set_hash_default(&self, hash: &str) {
        *self.hash.borrow_mut() = hash.to_string();
    }

    /// `SPViewElement::pack()`: packs as a vbox, then packs each child again.
    pub fn pack_view(el: &crate::sp_element::SPElement) {
        SPVBoxElement::pack_vbox(el);
        for child in el.get_child_nodes() {
            child.pack();
        }
    }

    /// `SPViewElement::scroll()`: scrolls the clipped content.
    pub fn scroll_view(el: &crate::sp_element::SPElement, scroll_x: i32, scroll_y: i32) {
        if el.get_clip_view() {
            el.set_scroll_x(el.get_scroll_x() + scroll_x);
            el.set_scroll_y(el.get_scroll_y() + scroll_y);
            el.invalidate();
        }
    }
}

impl SPViewClass for SPViewElement {
    fn view(&self) -> &SPViewElement {
        self
    }
}

impl SPElementClass for SPViewElement {
    fn get_type(&self) -> String {
        "view".into()
    }
    fn pack(&self) {
        Self::pack_view(self.element());
    }
    fn scroll(&self, scroll_x: i32, scroll_y: i32, _x: i32, _y: i32) {
        Self::scroll_view(self.element(), scroll_x, scroll_y);
    }
    fn as_view(&self) -> Option<&dyn SPViewClass> {
        Some(self)
    }
}
