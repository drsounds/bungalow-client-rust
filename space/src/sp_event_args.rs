//! Port of `SPEventArgs` and the event argument classes declared with it.

use std::any::Any;

use crate::sp_element::ElementRef;

/// Event payload. Concrete argument types are recovered with
/// [`downcast_args`], the counterpart of `dynamic_cast` in the C++ code.
pub trait SPEventArgs: Any {
    fn get_type(&self) -> String {
        "SPEventArgs".to_string()
    }
    fn as_any(&self) -> &dyn Any;
}

/// `dynamic_cast<T*>(args)`.
pub fn downcast_args<T: 'static>(args: &dyn SPEventArgs) -> Option<&T> {
    args.as_any().downcast_ref::<T>()
}

/// Implements [`SPEventArgs`] for a type, optionally with the C++ `getType()` name.
#[macro_export]
macro_rules! impl_event_args {
    ($type:ty) => {
        impl $crate::sp_event_args::SPEventArgs for $type {
            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }
        }
    };
    ($type:ty, $name:expr) => {
        impl $crate::sp_event_args::SPEventArgs for $type {
            fn get_type(&self) -> String {
                $name.to_string()
            }
            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }
        }
    };
}

/// Plain event arguments carrying only the sender.
#[derive(Clone, Default)]
pub struct SPBasicEventArgs {
    sender: Option<ElementRef>,
}

impl SPBasicEventArgs {
    pub fn new(sender: Option<ElementRef>) -> Self {
        Self { sender }
    }
    pub fn get_sender(&self) -> Option<ElementRef> {
        self.sender.clone()
    }
    pub fn set_sender(&mut self, value: Option<ElementRef>) {
        self.sender = value;
    }
}
impl_event_args!(SPBasicEventArgs);

/// A node was appended to or removed from a parent ("treechanged").
pub struct SPNodeTreeEventArgs {
    node: ElementRef,
    parent: ElementRef,
    action: String,
}

impl SPNodeTreeEventArgs {
    pub fn new(node: ElementRef, parent: ElementRef, action: &str) -> Self {
        Self { node, parent, action: action.to_string() }
    }
    pub fn get_node(&self) -> ElementRef {
        self.node.clone()
    }
    pub fn get_parent(&self) -> ElementRef {
        self.parent.clone()
    }
    pub fn get_action(&self) -> String {
        self.action.clone()
    }
    pub fn set_action(&mut self, value: &str) {
        self.action = value.to_string();
    }
}
impl_event_args!(SPNodeTreeEventArgs);

/// An attribute changed ("attributechanged").
#[derive(Debug, Clone)]
pub struct SPAttributeChangedEventArgs {
    attribute_name: String,
    new_value: String,
    old_value: String,
    attribute_type: String,
}

impl SPAttributeChangedEventArgs {
    pub fn new(attribute_name: &str, new_value: &str, old_value: &str, attribute_type: &str) -> Self {
        Self {
            attribute_name: attribute_name.to_string(),
            new_value: new_value.to_string(),
            old_value: old_value.to_string(),
            attribute_type: attribute_type.to_string(),
        }
    }
    pub fn get_attribute_name(&self) -> &str {
        &self.attribute_name
    }
    pub fn get_new_value(&self) -> &str {
        &self.new_value
    }
    pub fn get_old_value(&self) -> &str {
        &self.old_value
    }
    pub fn get_attribute_type(&self) -> &str {
        &self.attribute_type
    }
}
impl_event_args!(SPAttributeChangedEventArgs);

/// A key press routed through the tree ("keydown").
#[derive(Debug, Clone, Copy)]
pub struct SPKeyEventArgs {
    key_code: i32,
    cmd_down: bool,
}

impl SPKeyEventArgs {
    pub const KEY_UP: i32 = 0x10001;
    pub const KEY_DOWN: i32 = 0x10002;

    pub fn new(key_code: i32, cmd_down: bool) -> Self {
        Self { key_code, cmd_down }
    }
    pub fn get_key_code(&self) -> i32 {
        self.key_code
    }
    pub fn get_cmd_down(&self) -> bool {
        self.cmd_down
    }
}
impl_event_args!(SPKeyEventArgs);
