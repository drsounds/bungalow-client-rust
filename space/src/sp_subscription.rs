//! Port of `SPSubscription`: an observable bound to a URI of a [`Space`].

use std::rc::Weak;

use crate::sp_observable::SPObservable;
use crate::space::Space;

pub struct SPSubscription {
    pub observable: SPObservable,
    pub uri: String,
    pub space: Weak<Space>,
}

impl SPSubscription {
    pub fn new(space: Weak<Space>, uri: &str) -> Self {
        Self {
            observable: SPObservable::new(),
            uri: uri.to_string(),
            space,
        }
    }
}
