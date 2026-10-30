//! Port of `SPObject`: anything that can send an event.

use std::any::Any;
use std::rc::Rc;

use crate::sp_element::ElementRef;

/// The sender of an event. Elements are the usual senders; other objects
/// (the [`crate::Space`], application objects) are passed as `Other`.
#[derive(Clone)]
pub enum SPObject {
    Element(ElementRef),
    Other(Rc<dyn Any>),
}

impl SPObject {
    pub fn element(&self) -> Option<&ElementRef> {
        match self {
            SPObject::Element(element) => Some(element),
            SPObject::Other(_) => None,
        }
    }

    pub fn get_type(&self) -> String {
        match self {
            SPObject::Element(element) => element.get_type(),
            SPObject::Other(_) => "Undefined".to_string(),
        }
    }
}

impl From<ElementRef> for SPObject {
    fn from(element: ElementRef) -> Self {
        SPObject::Element(element)
    }
}

impl From<&ElementRef> for SPObject {
    fn from(element: &ElementRef) -> Self {
        SPObject::Element(element.clone())
    }
}
