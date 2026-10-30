//! Port of `StartView`: the Spider view for bungalow:start.

use std::rc::Rc;

use space::impl_element_base;
use space::sp_element::{construct, ElementRef, SPElementBase, SPElementClass};
use space::sp_spider_view_element::SPSpiderViewElement;
use space::sp_view_element::{SPViewClass, SPViewElement};


#[derive(Default)]
pub struct StartView {
    pub spider: SPSpiderViewElement,
}

impl_element_base!(StartView, spider);

impl StartView {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.spider.tab_bar_view.init_tab_bar_view();
        rc.element().set_clip_view(false);
        rc.element().this().set_uri("bungalow:start");
        rc
    }
}

impl SPViewClass for StartView {
    fn view(&self) -> &SPViewElement {
        &self.spider.tab_bar_view.view
    }
    fn accepts_uri(&self, uri: &str) -> bool {
        self.spider.tab_bar_view.accepts_uri_default(uri)
    }
    fn set_hash(&self, hash: &str) {
        self.spider.tab_bar_view.set_hash_tab_bar_view(hash);
    }
    fn navigate(&self, uri: &str) {
        SPViewClass::navigate(&self.spider, uri);
    }
}

impl SPElementClass for StartView {
    fn get_type(&self) -> String {
        "startview".into()
    }
    tab_bar_view_overrides!(spider.tab_bar_view);
}
