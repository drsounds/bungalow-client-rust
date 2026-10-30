//! Port of `SPNowPlayingBarTitleElement`: track title and artist labels.

use crate::sp_element::SPElementBase;
use std::cell::RefCell;
use std::rc::Rc;

use crate::impl_element_base;
use crate::libspace::SPMargin;
use crate::sp_application_context;
use crate::sp_color::SPColor;
use crate::sp_element::{construct, ElementRef, SPElementClass};
use crate::sp_event_args::downcast_args;
use crate::sp_playback_event_args::SPPlaybackStateEventArgs;
use crate::sp_text_element::SPTextElement;
use crate::sp_vbox_element::SPVBoxElement;

#[derive(Default)]
pub struct SPNowPlayingBarTitleElement {
    pub vbox: SPVBoxElement,
    title_label: RefCell<Option<Rc<SPTextElement>>>,
    info_label: RefCell<Option<Rc<SPTextElement>>>,
}

impl_element_base!(SPNowPlayingBarTitleElement, vbox);

impl SPNowPlayingBarTitleElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        let el = rc.element();
        let this = el.this();
        el.set_tag_name("nowplayingbartitle");
        el.set_padding(SPMargin::new(8, 8, 8, 8));
        el.set_gap(4);
        el.set_height(170);
        el.set_background_color(SPColor::new(0, 0, 0, 0));

        let title = SPTextElement::new(Some(&this));
        title.el.set_background_color(SPColor::new(0, 0, 0, 0));
        title.el.set_inner_text("");
        title.el.set_height(18);
        title.el.set_integer_attribute("size", 12);
        el.append_child_default(&(title.clone() as ElementRef));

        let info = SPTextElement::new(Some(&this));
        info.el.set_background_color(SPColor::new(0, 0, 0, 0));
        info.el.set_inner_text("");
        info.el.set_height(14);
        info.el.set_integer_attribute("size", 10);
        el.append_child_default(&(info.clone() as ElementRef));

        *rc.title_label.borrow_mut() = Some(title);
        *rc.info_label.borrow_mut() = Some(info);

        if let Some(app) = sp_application_context::current() {
            let weak = Rc::downgrade(&rc);
            app.get_playback_events().add_listener("playback:changed", move |_, args| {
                if let Some(this) = weak.upgrade() {
                    if downcast_args::<SPPlaybackStateEventArgs>(args).is_some() {
                        this.on_playback_changed();
                    }
                }
                0
            });
        }
        rc
    }

    pub fn on_playback_changed(&self) {
        let Some(app) = sp_application_context::current() else {
            return;
        };
        let (title, info) = if app.has_current_playable() {
            (app.get_track_title(), app.get_artist_name())
        } else {
            ("Nothing playing".to_string(), String::new())
        };
        if let Some(label) = self.title_label.borrow().as_ref() {
            label.el.set_inner_text(&title);
        }
        if let Some(label) = self.info_label.borrow().as_ref() {
            label.el.set_inner_text(&info);
        }
        self.element().invalidate();
    }
}

impl SPElementClass for SPNowPlayingBarTitleElement {
    fn get_type(&self) -> String {
        "nowplayingbartitle".into()
    }
    fn pack(&self) {
        SPVBoxElement::pack_vbox(self.element());
    }
}
