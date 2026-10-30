//! Port of `WhatsNewView`: the bungalow:whatsnew page.

use std::rc::Rc;

use space::impl_element_base;
use space::sp_container_view_element::SPContainerViewElement;
use space::sp_element::{construct, ElementRef, SPElementBase, SPElementClass};
use space::sp_view_element::{SPViewClass, SPViewElement};


#[derive(Default)]
pub struct WhatsNewView {
    pub container: SPContainerViewElement,
}

impl_element_base!(WhatsNewView, container);

impl WhatsNewView {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        SPViewElement::init_view(&rc.container.view);
        rc.element().set_attribute("src", "bungalow:whatsnew");
        rc
    }
}

impl SPViewClass for WhatsNewView {
    fn view(&self) -> &SPViewElement {
        &self.container.view
    }
    fn accepts_uri(&self, uri: &str) -> bool {
        uri == "bungalow:whatsnew"
    }
}

impl SPElementClass for WhatsNewView {
    fn get_type(&self) -> String {
        "whatsnewview".into()
    }
    view_element_overrides!();
}
