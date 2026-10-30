//! Port of `SPAppFooterElement`: playback controls and the seek bar.

use crate::sp_element::SPElementBase;
use std::cell::RefCell;
use std::rc::Rc;

use crate::impl_element_base;
use crate::libspace::SPMargin;
use crate::sp_application_context;
use crate::sp_button_element::SPButtonElement;
use crate::sp_color::SPColor;
use crate::sp_element::{construct, ElementRef, SPElementClass};
use crate::sp_event_args::downcast_args;
use crate::sp_hbox_element::SPHBoxElement;
use crate::sp_playback_event_args::{SPPlaybackStateEventArgs, SPSeekEventArgs};
use crate::sp_seek_bar_element::SPSeekBarElement;
use crate::sp_text_element::SPTextElement;

#[derive(Default)]
struct Parts {
    play_pause_btn: Option<Rc<SPButtonElement>>,
    pos_label: Option<Rc<SPTextElement>>,
    seek_bar: Option<Rc<SPSeekBarElement>>,
    dur_label: Option<Rc<SPTextElement>>,
}

#[derive(Default)]
pub struct SPAppFooterElement {
    pub hbox: SPHBoxElement,
    parts: RefCell<Parts>,
}

impl_element_base!(SPAppFooterElement, hbox);

fn on_click(button: &SPButtonElement, action: fn(&dyn sp_application_context::SPApplicationContext)) {
    button.el.add_event_listener(
        "click",
        Rc::new(move |_, _| {
            if let Some(app) = sp_application_context::current() {
                action(app.as_ref());
            }
            0
        }),
    );
}

impl SPAppFooterElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        let el = rc.element();
        let this = el.this();
        el.set_tag_name("footer");
        el.set_color_attribute("bgcolor", SPColor::new(57, 57, 57, 255));
        el.set_padding(SPMargin::new(4, 8, 4, 8));
        el.set_gap(6);

        let button = |text: &str, action: fn(&dyn sp_application_context::SPApplicationContext)| {
            let b = SPButtonElement::new(Some(&this));
            b.el.set_inner_text(text);
            b.el.set_width(28);
            b.el.set_height(22);
            on_click(&b, action);
            el.append_child_default(&(b.clone() as ElementRef));
            b
        };
        button("|<", |app| app.playback_prev());
        let play_pause = button(">", |app| app.toggle_play_pause());
        button(">|", |app| app.playback_next());

        let label = || {
            let l = SPTextElement::new(Some(&this));
            l.el.set_inner_text("0:00");
            l.el.set_width(38);
            l.el.set_height(22);
            l.el.set_foreground_color(SPColor::new(200, 200, 200, 255));
            l
        };
        let pos_label = label();
        el.append_child_default(&(pos_label.clone() as ElementRef));

        let seek_bar = SPSeekBarElement::new(Some(&this));
        seek_bar.el.set_flex(1);
        seek_bar.el.set_height(22);
        seek_bar.el.add_event_listener(
            "seek",
            Rc::new(|_, args| {
                if let (Some(seek), Some(app)) = (downcast_args::<SPSeekEventArgs>(args), sp_application_context::current()) {
                    app.seek_to(seek.get_position_ms());
                }
                0
            }),
        );
        el.append_child_default(&(seek_bar.clone() as ElementRef));

        let dur_label = label();
        el.append_child_default(&(dur_label.clone() as ElementRef));

        *rc.parts.borrow_mut() = Parts {
            play_pause_btn: Some(play_pause),
            pos_label: Some(pos_label),
            seek_bar: Some(seek_bar),
            dur_label: Some(dur_label),
        };

        if let Some(app) = sp_application_context::current() {
            let weak = Rc::downgrade(&rc);
            app.get_playback_events().add_listener("playback:changed", move |_, args| {
                if let (Some(this), Some(state)) = (weak.upgrade(), downcast_args::<SPPlaybackStateEventArgs>(args)) {
                    this.on_playback_changed(state);
                }
                0
            });
        }
        rc
    }

    pub fn on_playback_changed(&self, state: &SPPlaybackStateEventArgs) {
        let parts = self.parts.borrow();
        if let Some(b) = &parts.play_pause_btn {
            b.el.set_inner_text(if state.playing { "||" } else { ">" });
        }
        if let Some(l) = &parts.pos_label {
            l.el.set_inner_text(&Self::format_time(state.position_ms));
        }
        if let Some(l) = &parts.dur_label {
            l.el.set_inner_text(&Self::format_time(state.duration_ms));
        }
        if let Some(s) = &parts.seek_bar {
            s.set_duration(state.duration_ms);
            s.set_position(state.position_ms);
        }
        self.element().invalidate();
    }

    pub fn format_time(ms: i32) -> String {
        let total = ms.max(0) / 1000;
        format!("{}:{:02}", total / 60, total % 60)
    }
}

impl SPElementClass for SPAppFooterElement {
    fn get_type(&self) -> String {
        "appfooter".into()
    }
    fn pack(&self) {
        SPHBoxElement::pack_hbox(self.element());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_minutes_and_seconds() {
        assert_eq!(SPAppFooterElement::format_time(-5), "0:00");
        assert_eq!(SPAppFooterElement::format_time(61_000), "1:01");
        assert_eq!(SPAppFooterElement::format_time(600_500), "10:00");
    }
}
