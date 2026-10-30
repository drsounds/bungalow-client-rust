//! Port of `StartFeature`: a feature hosting the start view.

use std::rc::Rc;

use space::impl_element_base;
use space::sp_element::{construct, ElementRef, SPElementBase, SPElementClass};
use space::sp_feature_element::SPFeatureElement;

use super::start_view::StartView;

#[derive(Default)]
pub struct StartFeature {
    pub feature: SPFeatureElement,
}

impl_element_base!(StartFeature, feature);

impl StartFeature {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.element().set_tag_name("feature");
        let this = rc.element().this();
        let view: ElementRef = StartView::new(Some(&this));
        rc.feature.register_view(&view);
        rc
    }
}

impl SPElementClass for StartFeature {
    fn get_type(&self) -> String {
        self.feature.get_type()
    }
    fn pack(&self) {
        self.feature.pack();
    }
    fn append_child(&self, child: &ElementRef) {
        self.feature.append_child(child);
    }
}
