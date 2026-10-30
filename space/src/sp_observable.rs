//! Port of `SPObservable`.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::sp_event_args::SPEventArgs;
use crate::sp_object::SPObject;
use crate::sp_observer::{SPCallback, SPObserver};

#[derive(Default)]
pub struct SPObservable {
    observers: RefCell<Vec<SPObserver>>,
}

impl SPObservable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn dispatch_event(&self, event: &str, data: &dyn SPEventArgs, sender: Option<&SPObject>) {
        self.notify(event, sender, data);
    }

    /// Calls every listener registered for `event`. Listeners may add more
    /// listeners while being notified; those only see later events.
    pub fn notify(&self, event: &str, sender: Option<&SPObject>, data: &dyn SPEventArgs) -> i32 {
        let observers = self.observers.borrow().clone();
        for observer in observers.iter().filter(|o| o.get_event() == event) {
            (observer.get_callback())(sender, data);
        }
        0
    }

    pub fn add_event_listener(&self, event: &str, callback: SPCallback) {
        self.observers.borrow_mut().push(SPObserver::new(event, callback));
    }

    pub fn add_listener<F>(&self, event: &str, callback: F)
    where
        F: Fn(Option<&SPObject>, &dyn SPEventArgs) -> i32 + 'static,
    {
        self.add_event_listener(event, Rc::new(callback));
    }

    pub fn observer_count(&self) -> usize {
        self.observers.borrow().len()
    }
}

/// Builds a callback that forwards to `responder` while it is alive, the
/// counterpart of registering a member function with its `this` pointer.
pub fn responder<T: ?Sized + 'static>(
    responder: &Rc<T>,
    method: impl Fn(&T, Option<&SPObject>, &dyn SPEventArgs) -> i32 + 'static,
) -> SPCallback {
    let weak: Weak<T> = Rc::downgrade(responder);
    Rc::new(move |sender, args| match weak.upgrade() {
        Some(target) => method(&target, sender, args),
        None => 0,
    })
}
