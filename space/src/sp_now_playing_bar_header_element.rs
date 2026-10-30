//! Port of `SPNowPlayingBarHeaderElement`: cover art next to the title.

use crate::sp_element::SPElementBase;
use std::cell::RefCell;
use std::rc::Rc;

use crate::impl_element_base;
use crate::libspace::SPMargin;
use crate::sp_element::{construct, ElementRef, SPElementClass};
use crate::sp_hbox_element::SPHBoxElement;
use crate::sp_image_element::SPImageElement;
use crate::sp_now_playing_bar_title_element::SPNowPlayingBarTitleElement;

#[derive(Default)]
pub struct SPNowPlayingBarHeaderElement {
    pub hbox: SPHBoxElement,
    cover_image: RefCell<Option<Rc<SPImageElement>>>,
    title: RefCell<Option<Rc<SPNowPlayingBarTitleElement>>>,
}

impl_element_base!(SPNowPlayingBarHeaderElement, hbox);

impl SPNowPlayingBarHeaderElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        let el = rc.element();
        let this = el.this();
        el.set_tag_name("nowplayingbarheader");
        el.set_gap(4);
        el.set_height(64);

        let cover = SPImageElement::new(Some(&this));
        cover.el.set_width(64);
        cover.el.set_height(64);
        el.append_child_default(&(cover.clone() as ElementRef));

        let title = SPNowPlayingBarTitleElement::new(Some(&this));
        title.element().set_flex(1);
        title.element().set_padding(SPMargin::default());
        el.append_child_default(&(title.clone() as ElementRef));

        *rc.cover_image.borrow_mut() = Some(cover);
        *rc.title.borrow_mut() = Some(title);
        rc
    }

    pub fn cover_image(&self) -> Option<Rc<SPImageElement>> {
        self.cover_image.borrow().clone()
    }
}

impl SPElementClass for SPNowPlayingBarHeaderElement {
    fn get_type(&self) -> String {
        "nowplayingbarheader".into()
    }
    fn pack(&self) {
        SPHBoxElement::pack_hbox(self.element());
    }
}
