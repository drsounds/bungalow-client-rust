//! Port of `SPAeroColorSlider`: a hue / saturation / lightness slider used
//! to tint the Frutiger Aero skin.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::impl_element_base;
use crate::impl_event_args;
use crate::libspace::SPRectangle;
use crate::sp_color::{SPColor, SPGradient};
use crate::sp_element::{construct, ElementRef, ElementRefExt, SPElement, SPElementClass};
use crate::sp_event_args::{downcast_args, SPEventArgs};
use crate::sp_graphics_context::SPGraphicsContext;
use crate::sp_mouse_event_args::SPMouseEventArgs;
use crate::sp_object::SPObject;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SPAeroSliderType {
    Hue,
    Saturation,
    Lightness,
}

pub struct SPAeroSliderEventArgs {
    pub value: f32,
}
impl_event_args!(SPAeroSliderEventArgs);

const TRACK_H: i32 = 12;
const KNOB_R: i32 = 8;
const PADDING: i32 = KNOB_R;

pub struct SPAeroColorSlider {
    pub el: SPElement,
    type_: SPAeroSliderType,
    value: Cell<f32>,
    linked_hue: Cell<f32>,
    linked_sat: Cell<f32>,
    dragging: Cell<bool>,
    pub on_change: RefCell<Option<Box<dyn Fn(f32)>>>,
    pub on_release: RefCell<Option<Box<dyn Fn()>>>,
}

impl_element_base!(SPAeroColorSlider, el);

impl SPAeroColorSlider {
    pub fn new(parent: Option<&ElementRef>, type_: SPAeroSliderType) -> Rc<Self> {
        let value = match type_ {
            SPAeroSliderType::Hue => 200.0,
            SPAeroSliderType::Saturation => 0.7,
            SPAeroSliderType::Lightness => 0.5,
        };
        let rc = construct(
            Self {
                el: SPElement::default(),
                type_,
                value: Cell::new(value),
                linked_hue: Cell::new(200.0),
                linked_sat: Cell::new(0.7),
                dragging: Cell::new(false),
                on_change: RefCell::new(None),
                on_release: RefCell::new(None),
            },
            parent,
        );
        rc.el.set_height(KNOB_R * 2 + 2);
        for (event, handler) in [
            ("mousedown", Self::mouse_down as fn(&Self, Option<&SPObject>, &dyn SPEventArgs)),
            ("mouseup", Self::mouse_up),
            ("mousemove", Self::on_mouse_move),
        ] {
            let weak = Rc::downgrade(&rc);
            rc.el.add_event_listener(
                event,
                Rc::new(move |sender, e| {
                    if let Some(this) = weak.upgrade() {
                        handler(&this, sender, e);
                    }
                    0
                }),
            );
        }
        rc
    }

    pub fn get_value(&self) -> f32 {
        self.value.get()
    }

    pub fn set_value(&self, v: f32) {
        let v = match self.type_ {
            SPAeroSliderType::Hue => v.rem_euclid(360.0),
            _ => v.clamp(0.0, 1.0),
        };
        self.value.set(v);
        self.el.invalidate();
    }

    pub fn set_linked_hue(&self, h: f32) {
        self.linked_hue.set(h);
        self.el.invalidate();
    }

    pub fn set_linked_sat(&self, s: f32) {
        self.linked_sat.set(s);
        self.el.invalidate();
    }

    fn apply_mouse_x(&self, abs_mouse_x: i32) {
        let bounds = self.el.get_absolute_bounds();
        let track_x = bounds.x + PADDING;
        let track_w = bounds.width - PADDING * 2;
        if track_w <= 0 {
            return;
        }
        let ratio = ((abs_mouse_x - track_x) as f32 / track_w as f32).clamp(0.0, 1.0);
        self.set_value(if self.type_ == SPAeroSliderType::Hue { ratio * 360.0 } else { ratio });
        if let Some(on_change) = self.on_change.borrow().as_ref() {
            on_change(self.value.get());
        }
        let this = self.el.this();
        self.el.dispatch_event("change", &SPAeroSliderEventArgs { value: self.value.get() }, Some(&this.as_object()));
    }

    fn on_mouse_move(&self, _sender: Option<&SPObject>, e: &dyn SPEventArgs) {
        if !self.dragging.get() {
            return;
        }
        if let Some(me) = downcast_args::<SPMouseEventArgs>(e) {
            self.apply_mouse_x(me.get_x());
        }
    }
}

impl SPElementClass for SPAeroColorSlider {
    fn get_type(&self) -> String {
        "aeroslider".into()
    }

    fn paint(&self, px: i32, py: i32, g: &dyn SPGraphicsContext) {
        let abs_x = px + self.el.get_x();
        let abs_y = py + self.el.get_y();
        let (w, h) = (self.el.get_width(), self.el.get_height());
        self.el.set_absolute_bounds(SPRectangle::new(abs_x, abs_y, w, h));
        let track_y = abs_y + h / 2 - TRACK_H / 2;
        let track_x = abs_x + PADDING;
        let track_w = w - PADDING * 2;
        let (hue, sat, value) = (self.linked_hue.get(), self.linked_sat.get(), self.value.get());

        match self.type_ {
            SPAeroSliderType::Hue => {
                let stops = [0.0, 60.0, 120.0, 180.0, 240.0, 300.0, 360.0];
                let segments = 6;
                let seg_w = track_w / segments;
                for i in 0..segments {
                    let start = SPColor::from_hsl(stops[i as usize], 1.0, 0.55, 255);
                    let end = SPColor::from_hsl(stops[i as usize + 1], 1.0, 0.55, 255);
                    let sx = track_x + i * seg_w;
                    let sw = if i == segments - 1 { track_w - i * seg_w } else { seg_w };
                    g.fill_rounded_linear_gradient_rectangle(
                        sx,
                        track_y,
                        sw,
                        TRACK_H,
                        0,
                        0,
                        &SPGradient::new(start, end, 0),
                    );
                }
            }
            SPAeroSliderType::Saturation => {
                let gray = SPColor::from_hsl(hue, 0.0, 0.55, 255);
                let full = SPColor::from_hsl(hue, 1.0, 0.55, 255);
                g.fill_rounded_linear_gradient_rectangle(
                    track_x,
                    track_y,
                    track_w,
                    TRACK_H,
                    0,
                    0,
                    &SPGradient::new(gray, full, 0),
                );
            }
            SPAeroSliderType::Lightness => {
                let black = SPColor::from_hsl(hue, sat, 0.05, 255);
                let mid = SPColor::from_hsl(hue, sat, 0.55, 255);
                let white = SPColor::from_hsl(hue, sat * 0.3, 0.95, 255);
                let half = track_w / 2;
                g.fill_rounded_linear_gradient_rectangle(
                    track_x,
                    track_y,
                    half,
                    TRACK_H,
                    0,
                    0,
                    &SPGradient::new(black, mid, 0),
                );
                g.fill_rounded_linear_gradient_rectangle(
                    track_x + half,
                    track_y,
                    track_w - half,
                    TRACK_H,
                    0,
                    0,
                    &SPGradient::new(mid, white, 0),
                );
            }
        }
        g.draw_rounded_rectangle(track_x, track_y, track_w, TRACK_H, 4, SPColor::new(0, 0, 0, 60));

        let ratio = if self.type_ == SPAeroSliderType::Hue { value / 360.0 } else { value };
        let knob_cx = track_x + (ratio * track_w as f32) as i32;
        let knob_cy = abs_y + h / 2;
        g.fill_rounded_rectangle(
            knob_cx - KNOB_R + 1,
            knob_cy - KNOB_R + 1,
            KNOB_R * 2,
            KNOB_R * 2,
            KNOB_R,
            SPColor::new(0, 0, 0, 60),
        );
        g.fill_rounded_rectangle(
            knob_cx - KNOB_R,
            knob_cy - KNOB_R,
            KNOB_R * 2,
            KNOB_R * 2,
            KNOB_R,
            SPColor::new(255, 255, 255, 245),
        );
        let preview = match self.type_ {
            SPAeroSliderType::Hue => SPColor::from_hsl(value, 0.85, 0.55, 255),
            SPAeroSliderType::Saturation => SPColor::from_hsl(hue, value, 0.55, 255),
            SPAeroSliderType::Lightness => SPColor::from_hsl(hue, sat, value, 255),
        };
        g.draw_rounded_rectangle(knob_cx - KNOB_R, knob_cy - KNOB_R, KNOB_R * 2, KNOB_R * 2, KNOB_R, preview);
        g.draw_rounded_rectangle(
            knob_cx - KNOB_R + 1,
            knob_cy - KNOB_R + 1,
            KNOB_R * 2 - 2,
            KNOB_R * 2 - 2,
            KNOB_R - 1,
            SPColor::new(80, 80, 80, 120),
        );
    }

    fn mouse_down(&self, _sender: Option<&SPObject>, e: &dyn SPEventArgs) {
        let Some(me) = downcast_args::<SPMouseEventArgs>(e) else {
            return;
        };
        self.dragging.set(true);
        SPElement::set_mouse_capture(&self.el.this());
        self.apply_mouse_x(me.get_x());
    }

    fn mouse_up(&self, _sender: Option<&SPObject>, e: &dyn SPEventArgs) {
        let was_dragging = self.dragging.replace(false);
        SPElement::release_mouse_capture();
        if was_dragging {
            if let Some(me) = downcast_args::<SPMouseEventArgs>(e) {
                self.apply_mouse_x(me.get_x());
            }
            if let Some(on_release) = self.on_release.borrow().as_ref() {
                on_release();
            }
        }
    }
}
