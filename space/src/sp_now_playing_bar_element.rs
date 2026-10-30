//! Port of `SPNowPlayingBarElement`: the now-playing panel of the sidebar.
//! The C++ build optionally embeds a player canvas here
//! (`BUNGALOW_ENABLE_PLAYER_CANVAS`); the client adds it the same way.

use crate::sp_element::SPElementBase;
use std::cell::RefCell;
use std::rc::Rc;

use crate::impl_element_base;
use crate::libspace::SPMargin;
use crate::sp_application_context;
use crate::sp_element::{construct, ElementRef, SPElementClass};
use crate::sp_now_playing_bar_header_element::SPNowPlayingBarHeaderElement;
use crate::sp_vbox_element::SPVBoxElement;

#[derive(Default)]
pub struct SPNowPlayingBarElement {
    pub vbox: SPVBoxElement,
    header: RefCell<Option<Rc<SPNowPlayingBarHeaderElement>>>,
}

impl_element_base!(SPNowPlayingBarElement, vbox);

impl SPNowPlayingBarElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        let el = rc.element();
        el.set_tag_name("nowplayingbar");
        el.set_padding(SPMargin::default());
        el.set_gap(4);
        el.set_height(128 + 64);
        let header = SPNowPlayingBarHeaderElement::new(Some(&el.this()));
        el.append_child_default(&(header.clone() as ElementRef));
        *rc.header.borrow_mut() = Some(header);

        if let Some(app) = sp_application_context::current() {
            let weak = Rc::downgrade(&rc);
            app.get_playback_events().add_listener("playback:changed", move |_, _| {
                if let Some(this) = weak.upgrade() {
                    this.element().invalidate();
                }
                0
            });
        }
        rc
    }

    pub fn header(&self) -> Option<Rc<SPNowPlayingBarHeaderElement>> {
        self.header.borrow().clone()
    }
}

impl SPElementClass for SPNowPlayingBarElement {
    fn get_type(&self) -> String {
        "nowplayingbar".into()
    }
    fn pack(&self) {
        SPVBoxElement::pack_vbox(self.element());
        if let Some(header) = self.header() {
            header.pack();
        }
        self.element().set_height(self.element().get_width() + 100);
    }
}
