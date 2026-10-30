//! Port of `SPTreeItem`: one row of a tree view (sidebar entry or section).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::sp_uri::SPUri;

#[derive(Debug, Default)]
pub struct SPTreeItem {
    highlighted: Cell<bool>,
    title: RefCell<String>,
    pub subtitle: RefCell<String>,
    icon_path: RefCell<String>,
    uri: Option<SPUri>,
    selected: Cell<bool>,
    is_section: Cell<bool>,
    collapsed: Cell<bool>,
    is_bookmark: Cell<bool>,
    children: RefCell<Vec<Rc<SPTreeItem>>>,
}

impl SPTreeItem {
    /// For section headers — no URI.
    pub fn section() -> Rc<Self> {
        Rc::new(Self { collapsed: Cell::new(true), ..Self::default() })
    }

    pub fn new(uri: SPUri) -> Rc<Self> {
        let text = if uri.get_address() == "bungalow:internal:start" { "Start" } else { "Loading..." };
        Rc::new(Self {
            title: RefCell::new(text.to_string()),
            uri: Some(uri),
            collapsed: Cell::new(true),
            ..Self::default()
        })
    }

    pub fn is_highlighted(&self) -> bool {
        self.highlighted.get()
    }
    pub fn set_highlighted(&self, value: bool) {
        self.highlighted.set(value);
    }
    pub fn is_selected(&self) -> bool {
        self.selected.get()
    }
    pub fn set_selected(&self, value: bool) {
        self.selected.set(value);
    }
    pub fn is_section(&self) -> bool {
        self.is_section.get()
    }
    pub fn set_is_section(&self, value: bool) {
        self.is_section.set(value);
    }
    pub fn is_collapsed(&self) -> bool {
        self.collapsed.get()
    }
    pub fn set_collapsed(&self, value: bool) {
        self.collapsed.set(value);
    }
    pub fn is_bookmark(&self) -> bool {
        self.is_bookmark.get()
    }
    pub fn set_is_bookmark(&self, value: bool) {
        self.is_bookmark.set(value);
    }
    pub fn add_child(&self, child: Rc<SPTreeItem>) {
        self.children.borrow_mut().push(child);
    }
    pub fn remove_child(&self, child: &Rc<SPTreeItem>) {
        self.children.borrow_mut().retain(|c| !Rc::ptr_eq(c, child));
    }
    pub fn clear_children(&self) {
        self.children.borrow_mut().clear();
    }
    pub fn has_children(&self) -> bool {
        !self.children.borrow().is_empty()
    }
    pub fn get_children(&self) -> Vec<Rc<SPTreeItem>> {
        self.children.borrow().clone()
    }
    pub fn get_icon_path(&self) -> String {
        self.icon_path.borrow().clone()
    }
    pub fn set_icon_path(&self, path: &str) {
        *self.icon_path.borrow_mut() = path.to_string();
    }
    pub fn get_text(&self) -> String {
        self.title.borrow().clone()
    }
    pub fn set_text(&self, value: &str) {
        *self.title.borrow_mut() = value.to_string();
    }
    pub fn get_uri(&self) -> Option<&SPUri> {
        self.uri.as_ref()
    }
}
