//! Port of `ArtistView`.

use std::rc::Rc;

use space::impl_element_base;
use space::sp_element::{construct, ElementRef, SPElementBase, SPElementClass};
use space::sp_tab_bar_view_element::SPTabBarViewElement;
use space::sp_view_element::{SPViewClass, SPViewElement};

#[derive(Default)]
pub struct ArtistView {
    pub tab_bar_view: SPTabBarViewElement,
}

impl_element_base!(ArtistView, tab_bar_view);

impl ArtistView {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.tab_bar_view.init_tab_bar_view();
        rc
    }
}

impl SPViewClass for ArtistView {
    fn view(&self) -> &SPViewElement {
        &self.tab_bar_view.view
    }
    fn accepts_uri(&self, uri: &str) -> bool {
        self.tab_bar_view.accepts_uri_default(uri)
    }
    fn set_hash(&self, hash: &str) {
        self.tab_bar_view.set_hash_tab_bar_view(hash);
    }
}

impl SPElementClass for ArtistView {
    fn get_type(&self) -> String {
        "artistview".into()
    }
    tab_bar_view_overrides!(tab_bar_view);
}
