//! Port of `TopListView`.

use std::rc::Rc;

use space::impl_element_base;
use space::sp_container_view_element::SPContainerViewElement;
use space::sp_element::{construct, ElementRef, SPElementBase, SPElementClass};
use space::sp_view_element::{SPViewClass, SPViewElement};

#[derive(Default)]
pub struct TopListView {
    pub container: SPContainerViewElement,
}

impl_element_base!(TopListView, container);

impl TopListView {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        SPViewElement::init_view(&rc.container.view);
        rc
    }
}

impl SPViewClass for TopListView {
    fn view(&self) -> &SPViewElement {
        &self.container.view
    }
    fn accepts_uri(&self, uri: &str) -> bool {
        uri == "bungalow:internal:toplist"
    }
}

impl SPElementClass for TopListView {
    fn get_type(&self) -> String {
        "toplistview".into()
    }
    fn pack(&self) {
        SPViewElement::pack_view(self.element());
    }
    /// The C++ view routes wheel events like a plain element.
    fn scroll(&self, scroll_x: i32, scroll_y: i32, x: i32, y: i32) {
        self.element().scroll_default(scroll_x, scroll_y, x, y);
    }
    fn as_view(&self) -> Option<&dyn SPViewClass> {
        Some(self)
    }
}
