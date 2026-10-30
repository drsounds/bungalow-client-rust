//! Port of `SPStore`: a keyed collection of resources that notifies "update".

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::impl_event_args;
use crate::sp_object::SPObject;
use crate::sp_observable::SPObservable;
use crate::sp_resource::SPResource;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SPStoreEventType {
    Add,
    Update,
    Remove,
}

pub struct SPStoreEventArgs {
    pub type_: SPStoreEventType,
    pub object: Rc<RefCell<SPResource>>,
}
impl_event_args!(SPStoreEventArgs);

#[derive(Default)]
pub struct SPStore {
    pub observable: SPObservable,
    values: RefCell<BTreeMap<String, Rc<RefCell<SPResource>>>>,
}

impl SPStore {
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }

    pub fn get(&self, key: &str) -> Option<Rc<RefCell<SPResource>>> {
        self.values.borrow().get(key).cloned()
    }

    pub fn set(self: &Rc<Self>, key: &str, value: Rc<RefCell<SPResource>>) {
        self.values.borrow_mut().insert(key.to_string(), value.clone());
        let args = SPStoreEventArgs {
            type_: SPStoreEventType::Add,
            object: value,
        };
        let sender = SPObject::Other(self.clone());
        self.observable.notify("update", Some(&sender), &args);
    }

    pub fn contains(&self, key: &str) -> bool {
        self.values.borrow().contains_key(key)
    }

    /// Unimplemented in the C++ code as well.
    pub fn remove(&self, _key: &str) {}
}
