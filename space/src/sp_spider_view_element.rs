//! Port of `SPSpiderViewElement`: a tab bar view whose content is loaded
//! from its URI (the `src` attribute) through the Spider templates.

use crate::sp_element::SPElementBase;
use std::rc::Rc;

use crate::impl_element_base;
use crate::sp_element::{construct, ElementRef, SPElementClass};
use crate::sp_event_args::SPEventArgs;
use crate::sp_object::SPObject;
use crate::sp_tab_bar_view_element::SPTabBarViewElement;
use crate::sp_view_element::{SPViewClass, SPViewElement};

#[derive(Default)]
pub struct SPSpiderViewElement {
    pub tab_bar_view: SPTabBarViewElement,
}

impl_element_base!(SPSpiderViewElement, tab_bar_view);

impl SPSpiderViewElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.tab_bar_view.init_tab_bar_view();
        rc
    }
}

impl SPViewClass for SPSpiderViewElement {
    fn view(&self) -> &SPViewElement {
        &self.tab_bar_view.view
    }
    fn accepts_uri(&self, uri: &str) -> bool {
        self.tab_bar_view.accepts_uri_default(uri)
    }
    fn set_hash(&self, hash: &str) {
        self.tab_bar_view.set_hash_tab_bar_view(hash);
    }
    fn navigate(&self, uri: &str) {
        let base_uri = uri.split('#').next().unwrap_or_default();
        self.element().set_attribute("src", base_uri);
    }
}

impl SPElementClass for SPSpiderViewElement {
    fn get_type(&self) -> String {
        "spiderview".into()
    }
    fn pack(&self) {
        SPViewElement::pack_view(self.element());
    }
    fn scroll(&self, scroll_x: i32, scroll_y: i32, _x: i32, _y: i32) {
        SPViewElement::scroll_view(self.element(), scroll_x, scroll_y);
    }
    fn mouse_down(&self, sender: Option<&SPObject>, e: &dyn SPEventArgs) {
        self.element().mouse_down_default(sender, e);
    }
    fn get_view_stack_element(&self) -> Option<ElementRef> {
        Some(self.tab_bar_view.tab_stack())
    }
    fn get_slot_element(&self) -> ElementRef {
        self.tab_bar_view.tab_stack()
    }
    fn clear(&self) {
        self.tab_bar_view.clear_tab_bar_view();
    }
    fn as_view(&self) -> Option<&dyn SPViewClass> {
        Some(self)
    }
}
