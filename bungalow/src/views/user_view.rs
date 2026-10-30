//! Port of `UserView`: shows bungalow:user:* pages.

use std::cell::RefCell;
use std::rc::Rc;

use space::impl_element_base;
use space::libspace::SPMargin;
use space::sp_element::{construct, ElementRef, SPElementBase, SPElementClass};
use space::sp_header_element::SPHeaderElement;
use space::sp_view_element::{SPViewClass, SPViewElement};


#[derive(Default)]
pub struct UserView {
    pub view: SPViewElement,
    pub header: RefCell<Option<Rc<SPHeaderElement>>>,
}

impl_element_base!(UserView, view);

impl UserView {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        SPViewElement::init_view(&rc.view);
        let el = rc.element();
        let this = el.this();
        el.set_padding(SPMargin::new(20, 20, 20, 20));
        let header = SPHeaderElement::new(Some(&this));
        if let Some(title) = header.title.borrow().as_ref() {
            title.set_title("Title");
            title.set_entity("User");
        }
        header.element().set("width", "100%");
        header.element().set("height", "180");
        this.append_child(&(header.clone() as ElementRef));
        *rc.header.borrow_mut() = Some(header);
        rc
    }
}

impl SPViewClass for UserView {
    fn view(&self) -> &SPViewElement {
        &self.view
    }
    fn accepts_uri(&self, uri: &str) -> bool {
        uri.starts_with("bungalow:user:")
    }
    fn navigate(&self, _uri: &str) {
        if let Some(title) = self.header.borrow().as_ref().and_then(|h| h.title.borrow().clone()) {
            title.set_title("User");
        }
    }
}

impl SPElementClass for UserView {
    view_element_overrides!();
}
