//! Port of `SPObserver`: one registered event listener.

use std::rc::Rc;

use crate::sp_event_args::SPEventArgs;
use crate::sp_object::SPObject;

/// A listener. In C++ this was a member-function pointer plus the responder
/// object; here the closure captures its responder (weakly, by convention).
pub type SPCallback = Rc<dyn Fn(Option<&SPObject>, &dyn SPEventArgs) -> i32>;

#[derive(Clone)]
pub struct SPObserver {
    event: String,
    callback: SPCallback,
}

impl SPObserver {
    pub fn new(event: &str, callback: SPCallback) -> Self {
        Self { event: event.to_string(), callback }
    }
    pub fn get_event(&self) -> &str {
        &self.event
    }
    pub fn get_callback(&self) -> SPCallback {
        self.callback.clone()
    }
}
