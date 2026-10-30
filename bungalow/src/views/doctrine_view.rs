//! Port of `DoctrineView`: shows bungalow:doctrine:* pages.

use std::cell::RefCell;
use std::rc::Rc;

use space::impl_element_base;
use space::libspace::SPMargin;
use space::sp_container_view_element::SPContainerViewElement;
use space::sp_element::{construct, ElementRef, SPElementBase, SPElementClass};
use space::sp_text_element::SPTextElement;
use space::sp_view_element::{SPViewClass, SPViewElement};

#[derive(Default)]
pub struct DoctrineView {
    pub container: SPContainerViewElement,
    pub text: RefCell<Option<Rc<SPTextElement>>>,
}

impl_element_base!(DoctrineView, container);

impl DoctrineView {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        SPViewElement::init_view(&rc.container.view);
        let el = rc.element();
        let this = el.this();
        el.set_padding(SPMargin::new(20, 20, 20, 20));
        let text = SPTextElement::new(Some(&this));
        text.el.set_string_attribute("width", "100%");
        this.append_child(&(text.clone() as ElementRef));
        *rc.text.borrow_mut() = Some(text);
        rc
    }
}

impl SPViewClass for DoctrineView {
    fn view(&self) -> &SPViewElement {
        &self.container.view
    }
    fn accepts_uri(&self, uri: &str) -> bool {
        uri.starts_with("bungalow:doctrine:")
    }
    fn navigate(&self, uri: &str) {
        // The C++ view splits the URI but does nothing with the parts yet.
        let _parts: Vec<&str> = uri.split(':').collect();
    }
}

impl SPElementClass for DoctrineView {
    fn get_type(&self) -> String {
        "view".into()
    }
    view_element_overrides!();
}
