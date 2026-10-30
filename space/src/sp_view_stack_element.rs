//! Port of `SPViewStackElement`: shows the one child view that accepts the
//! current URI and keeps back/forward history.

use crate::sp_element::SPElementBase;
use std::cell::RefCell;
use std::rc::Rc;

use crate::impl_element_base;
use crate::sp_element::{construct, same_element, ElementRef, SPElementClass, WeakElementRef};
use crate::sp_uri::SPUri;
use crate::sp_vbox_element::SPVBoxElement;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavigationIntent {
    Normal,
    Back,
    Forward,
}

#[derive(Default)]
pub struct SPViewStackElement {
    pub vbox: SPVBoxElement,
    history: RefCell<Vec<String>>,
    future: RefCell<Vec<String>>,
    active_view: RefCell<Option<WeakElementRef>>,
    current_uri: RefCell<String>,
}

impl_element_base!(SPViewStackElement, vbox);

impl SPViewStackElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.element().set_tag_name("viewstack");
        rc
    }

    pub fn register_view(&self, view: &ElementRef) {
        self.append_child(view);
    }

    /// Main navigation handler inside bungalow. Records the current URI in
    /// the history stack before navigating, and clears the forward stack.
    pub fn navigate(&self, value: &str) {
        {
            let current = self.current_uri.borrow();
            if !current.is_empty() && *current != value {
                self.history.borrow_mut().push(current.clone());
                self.future.borrow_mut().clear();
            }
        }
        *self.current_uri.borrow_mut() = value.to_string();
        self.navigate_internal(value, NavigationIntent::Normal);
    }

    pub fn go_back(&self) {
        let Some(previous) = self.history.borrow_mut().pop() else {
            return;
        };
        let current = self.current_uri.replace(previous.clone());
        self.future.borrow_mut().push(current);
        self.navigate_internal(&previous, NavigationIntent::Back);
    }

    pub fn go_forward(&self) {
        let Some(next) = self.future.borrow_mut().pop() else {
            return;
        };
        let current = self.current_uri.replace(next.clone());
        self.history.borrow_mut().push(current);
        self.navigate_internal(&next, NavigationIntent::Forward);
    }

    pub fn can_go_back(&self) -> bool {
        !self.history.borrow().is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.future.borrow().is_empty()
    }

    pub fn get_current_uri(&self) -> String {
        self.current_uri.borrow().clone()
    }

    pub fn get_active_view(&self) -> Option<ElementRef> {
        self.active_view.borrow().as_ref().and_then(|w| w.upgrade())
    }

    fn navigate_internal(&self, value: &str, intent: NavigationIntent) {
        let uri = SPUri::new(value);
        let address = uri.get_address();
        let children = self.element().get_child_nodes();
        let matched = children
            .iter()
            .filter(|child| child.as_view().is_some_and(|view| view.accepts_uri(&address)))
            .last()
            .cloned();
        for child in &children {
            let is_match = matched.as_ref().is_some_and(|m| same_element(m, child));
            if !is_match {
                child.set_visible(false);
                continue;
            }
            child.set_visible(true);
            if let Some(view) = child.as_view() {
                match intent {
                    NavigationIntent::Back => view.navigate_back(&address),
                    NavigationIntent::Forward => view.navigate_forward(&address),
                    NavigationIntent::Normal => view.navigate(&address),
                }
                view.set_hash(&uri.get_hash());
            }
            child.element().set_flex(1);
            *self.active_view.borrow_mut() = Some(Rc::downgrade(child));
        }
        self.element().invalidate();
        self.pack();
    }
}

impl SPElementClass for SPViewStackElement {
    fn get_type(&self) -> String {
        "viewstack".into()
    }
    fn pack(&self) {
        SPVBoxElement::pack_vbox(self.element());
    }
    fn append_child(&self, child: &ElementRef) {
        self.element().append_child_default(child);
        child.set_visible(false);
        child.element().set_flex(1);
        self.pack();
    }
}
