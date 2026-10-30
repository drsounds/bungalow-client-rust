//! Port of `Space`: the application object owning windows, the graphics
//! context and the event/message queues.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use crate::libspace::{SPEvent, SPMessage};
use crate::sp_element::{ElementRef, SPElementClass};
use crate::sp_event_args::SPEventArgs;
use crate::sp_graphics_context::SPGraphicsContext;
use crate::sp_object::SPObject;
use crate::sp_observable::SPObservable;
use crate::sp_skin::SPSkin;
use crate::sp_store::SPStore;
use crate::sp_window_element::SPWindowElement;

#[derive(Default)]
pub struct Space {
    observable: SPObservable,
    store: RefCell<Option<Rc<SPStore>>>,
    graphics_context: RefCell<Option<Rc<dyn SPGraphicsContext>>>,
    windows: RefCell<Vec<Rc<SPWindowElement>>>,
    events: RefCell<VecDeque<SPEvent>>,
    messages: RefCell<VecDeque<SPMessage>>,
}

impl Space {
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }

    pub fn observable(&self) -> &SPObservable {
        &self.observable
    }

    pub fn get_windows(&self) -> Vec<Rc<SPWindowElement>> {
        self.windows.borrow().clone()
    }

    pub fn add_window(&self, window: Rc<SPWindowElement>) {
        self.windows.borrow_mut().push(window);
    }

    pub fn get_store(&self) -> Option<Rc<SPStore>> {
        self.store.borrow().clone()
    }

    pub fn set_store(&self, value: Option<Rc<SPStore>>) {
        *self.store.borrow_mut() = value;
    }

    pub fn get_graphics_context(&self) -> Option<Rc<dyn SPGraphicsContext>> {
        self.graphics_context.borrow().clone()
    }

    pub fn set_graphics_context(&self, value: Option<Rc<dyn SPGraphicsContext>>) {
        *self.graphics_context.borrow_mut() = value;
    }

    pub fn create_window(self: &Rc<Self>, skin: Option<Rc<dyn SPSkin>>) -> Rc<SPWindowElement> {
        SPWindowElement::new(self, skin)
    }

    /// Notifies the listeners of the space, then every window.
    pub fn notify(&self, event: &str, sender: Option<&SPObject>, data: &dyn SPEventArgs) -> i32 {
        self.observable.notify(event, sender, data);
        for window in self.get_windows() {
            window.notify(event, sender, data);
        }
        0
    }

    pub fn queue_event(&self, event: SPEvent) {
        self.events.borrow_mut().push_back(event);
    }

    pub fn dequeue_event(&self) -> Option<SPEvent> {
        self.events.borrow_mut().pop_front()
    }

    pub fn dequeue_next_message(&self) -> Option<SPMessage> {
        self.messages.borrow_mut().pop_front()
    }

    pub fn post_message(&self, message: SPMessage) {
        self.messages.borrow_mut().push_back(message);
    }

    pub fn process_event(&self, event: &SPEvent) {
        for window in self.get_windows() {
            window.process_event(event);
        }
    }

    pub fn process_message(&self, message: &SPMessage) {
        for window in self.get_windows() {
            window.process_message(message);
        }
    }

    pub fn windows_as_elements(&self) -> Vec<ElementRef> {
        self.get_windows().into_iter().map(|w| w as ElementRef).collect()
    }
}
