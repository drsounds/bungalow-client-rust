//! Port of `ArtistOverView`.

use std::rc::Rc;

use space::impl_element_base;
use space::sp_element::{construct, ElementRef, SPElementBase, SPElementClass};
use space::sp_hbox_element::SPHBoxElement;
use space::sp_view_element::{SPViewClass, SPViewElement};


#[derive(Default)]
pub struct ArtistOverView {
    pub view: SPViewElement,
}

impl_element_base!(ArtistOverView, view);

impl ArtistOverView {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        SPViewElement::init_view(&rc.view);
        let this = rc.element().this();
        let header = SPHBoxElement::new(Some(&this));
        header.element().set_integer_attribute("height", 128);
        rc
    }
}

impl SPViewClass for ArtistOverView {
    fn view(&self) -> &SPViewElement {
        &self.view
    }
}

impl SPElementClass for ArtistOverView {
    view_element_overrides!();
}
