//! Shared types of the Space framework (port of `libspace.h`).

use std::rc::Rc;

use crate::sp_event_args::SPEventArgs;
use crate::sp_object::SPObject;

pub const CONTROL_INPUT: i32 = 1;
pub const SP_EVENT_MOUSE_DOWN: i32 = 1;
pub const SP_EVENT_MOUSE_UP: i32 = 2;
pub const SP_EVENT_MOUSE_MOVE: i32 = 3;
pub const SP_EVENT_NAVIGATE: i32 = 4;

/// An event queued on [`crate::Space`] and routed through the element tree.
#[derive(Clone)]
pub struct SPEvent {
    pub type_: String,
    pub args: Rc<dyn SPEventArgs>,
    pub sender: Option<SPObject>,
}

impl SPEvent {
    pub fn new(type_: &str, args: Rc<dyn SPEventArgs>, sender: Option<SPObject>) -> Self {
        Self {
            type_: type_.to_string(),
            args,
            sender,
        }
    }
}

/// A message (subject, predicate, object) posted to [`crate::Space`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SPMessage {
    pub type_: String,
    pub subject_uri: String,
    pub predicate: String,
    pub object_uri: String,
    pub attribute: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SPMargin {
    pub left: i32,
    pub top: i32,
    pub bottom: i32,
    pub right: i32,
}

impl SPMargin {
    /// Same field order as the C++ aggregate initializer `{ left, top, bottom, right }`.
    pub const fn new(left: i32, top: i32, bottom: i32, right: i32) -> Self {
        Self { left, top, bottom, right }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SPPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SPRectangle {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub z: i32,
}

impl SPRectangle {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self { x, y, width, height, z: 0 }
    }

    /// Strict containment, as the element tree hit tests (`x > left && x < right`).
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x > self.x && x < self.x + self.width && y > self.y && y < self.y + self.height
    }
}

/// Parses `#rrggbb` (or `#aarrggbb`-sized strings) to an integer.
pub fn html_to_color(text: &str) -> u32 {
    let hex = text.strip_prefix('#').unwrap_or(text);
    let digits: String = hex.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
    u32::from_str_radix(&digits, 16).unwrap_or(0)
}
