//! Port of `SpaceView`: a Spider view that accepts every URI.

use std::rc::Rc;

use space::impl_element_base;
use space::sp_element::{construct, ElementRef, SPElementBase, SPElementClass};
use space::sp_spider_view_element::SPSpiderViewElement;
use space::sp_view_element::{SPViewClass, SPViewElement};

#[derive(Default)]
pub struct SpaceView {
    pub spider: SPSpiderViewElement,
}

impl_element_base!(SpaceView, spider);

impl SpaceView {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.spider.tab_bar_view.init_tab_bar_view();
        rc.element().set_clip_view(false);
        rc
    }
}

impl SPViewClass for SpaceView {
    fn view(&self) -> &SPViewElement {
        &self.spider.tab_bar_view.view
    }
    fn accepts_uri(&self, _uri: &str) -> bool {
        true
    }
    fn set_hash(&self, hash: &str) {
        self.spider.tab_bar_view.set_hash_tab_bar_view(hash);
    }
    fn navigate(&self, value: &str) {
        self.element().this().set_uri(value);
        self.element().set_attribute("src", value);
    }
}

impl SPElementClass for SpaceView {
    fn get_type(&self) -> String {
        "spaceview".into()
    }
    tab_bar_view_overrides!(spider.tab_bar_view);
}
