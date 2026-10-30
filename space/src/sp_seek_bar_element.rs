//! Port of `SPSeekBarElement`: a playback position slider dispatching "seek".

use std::cell::Cell;
use std::rc::Rc;

use crate::impl_element_base;
use crate::libspace::SPRectangle;
use crate::sp_color::SPColor;
use crate::sp_element::{construct, ElementRef, ElementRefExt, SPElement, SPElementClass};
use crate::sp_event_args::{downcast_args, SPEventArgs};
use crate::sp_graphics_context::SPGraphicsContext;
use crate::sp_mouse_event_args::SPMouseEventArgs;
use crate::sp_object::SPObject;
use crate::sp_playback_event_args::SPSeekEventArgs;

#[derive(Default)]
pub struct SPSeekBarElement {
    pub el: SPElement,
    pos_ms: Cell<i32>,
    dur_ms: Cell<i32>,
}

impl_element_base!(SPSeekBarElement, el);

impl SPSeekBarElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.el.set_tag_name("seekbar");
        rc
    }
    pub fn set_position(&self, pos_ms: i32) {
        self.pos_ms.set(pos_ms);
        self.el.invalidate();
    }
    pub fn set_duration(&self, dur_ms: i32) {
        self.dur_ms.set(dur_ms);
        self.el.invalidate();
    }
    pub fn get_position(&self) -> i32 {
        self.pos_ms.get()
    }
    pub fn get_duration(&self) -> i32 {
        self.dur_ms.get()
    }
}

impl SPElementClass for SPSeekBarElement {
    fn get_type(&self) -> String {
        "seekbar".into()
    }

    fn paint(&self, x: i32, y: i32, g: &dyn SPGraphicsContext) {
        let abs_x = x + self.el.get_x();
        let abs_y = y + self.el.get_y();
        let (w, h) = (self.el.get_width(), self.el.get_height());
        self.el.set_absolute_bounds(SPRectangle::new(abs_x, abs_y, w, h));
        g.fill_rounded_rectangle(abs_x, abs_y + h / 2 - 2, w, 4, 2, SPColor::new(80, 80, 80, 255));
        let (pos, dur) = (self.pos_ms.get(), self.dur_ms.get());
        if dur > 0 && pos >= 0 {
            let ratio = (pos as f32 / dur as f32).min(1.0);
            let fill_w = (ratio * w as f32) as i32;
            if fill_w > 0 {
                g.fill_rounded_rectangle(abs_x, abs_y + h / 2 - 2, fill_w, 4, 2, SPColor::new(95, 132, 180, 255));
            }
            let knob_x = abs_x + (ratio * w as f32) as i32 - 5;
            let knob_y = abs_y + h / 2 - 5;
            g.fill_rounded_rectangle(knob_x, knob_y, 10, 10, 5, SPColor::new(200, 200, 200, 255));
        }
    }

    fn mouse_down(&self, _sender: Option<&SPObject>, e: &dyn SPEventArgs) {
        let Some(me) = downcast_args::<SPMouseEventArgs>(e) else {
            return;
        };
        let dur = self.dur_ms.get();
        let w = self.el.get_width();
        if dur <= 0 || w <= 0 {
            return;
        }
        let rel_x = me.get_x() - self.el.get_absolute_bounds().x;
        let ratio = (rel_x as f32 / w as f32).clamp(0.0, 1.0);
        let seek_pos = (ratio * dur as f32) as i32;
        let this = self.el.this();
        self.el.dispatch_event("seek", &SPSeekEventArgs::new(seek_pos), Some(&this.as_object()));
    }
}
