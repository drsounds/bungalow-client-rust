//! Port of `SPTabBarViewElement`: a view with a tab bar on top and a tab
//! stack below. `<page>` children become tabs.

use crate::sp_element::SPElementBase;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::impl_element_base;
use crate::sp_element::{construct, ElementRef, ElementRefExt, SPElementClass};
use crate::sp_event_args::{downcast_args, SPEventArgs, SPNodeTreeEventArgs};
use crate::sp_object::SPObject;
use crate::sp_tab_bar_element::{SPTabBarElement, TabBarEventArgs};
use crate::sp_tab_stack_element::SPTabStackElement;
use crate::sp_uri::SPUri;
use crate::sp_view_element::{SPViewClass, SPViewElement};
use crate::sp_window_element::SPWindowElement;

#[derive(Default)]
pub struct SPTabBarViewElement {
    pub view: SPViewElement,
    tab_bar: RefCell<Option<Rc<SPTabBarElement>>>,
    tab_stack: RefCell<Option<Rc<SPTabStackElement>>>,
}

impl_element_base!(SPTabBarViewElement, view);

impl SPTabBarViewElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.init_tab_bar_view();
        rc
    }

    /// The `SPTabBarViewElement(SPElement *parent)` constructor body.
    pub fn init_tab_bar_view(&self) {
        SPViewElement::init_view(&self.view);
        let el = self.element();
        let this = el.this();
        el.set_tag_name("tabbarview");
        el.set_flex(1);
        let tab_bar = SPTabBarElement::new(Some(&this));
        let tab_stack = SPTabStackElement::new(Some(&this));
        tab_bar.element().set_height(28);
        let tab_bar_ref: ElementRef = tab_bar.clone();
        let tab_stack_ref: ElementRef = tab_stack.clone();
        el.append_child_default(&tab_bar_ref);
        el.append_child_default(&tab_stack_ref);
        tab_stack.element().set_flex(1);
        *self.tab_bar.borrow_mut() = Some(tab_bar.clone());
        *self.tab_stack.borrow_mut() = Some(tab_stack.clone());

        let view: Weak<dyn SPElementClass> = Rc::downgrade(&this);
        tab_bar.element().add_event_listener(
            "tabselected",
            Rc::new({
                let view = view.clone();
                move |_sender, e| {
                    if let (Some(view), Some(args)) = (view.upgrade(), downcast_args::<TabBarEventArgs>(e)) {
                        Self::on_tab_selected(&view, args);
                    }
                    0
                }
            }),
        );
        let (bar, stack) = (Rc::downgrade(&tab_bar), Rc::downgrade(&tab_stack));
        tab_stack.element().add_event_listener(
            "treechanged",
            Rc::new({
                let (view, bar, stack) = (view.clone(), bar.clone(), stack.clone());
                move |_sender, e| {
                    if let (Some(view), Some(bar), Some(stack)) = (view.upgrade(), bar.upgrade(), stack.upgrade()) {
                        Self::on_tree_changed(&view, &bar, &stack, e);
                    }
                    0
                }
            }),
        );
        el.add_event_listener(
            "unload",
            Rc::new({
                let bar = bar.clone();
                move |_, _| {
                    if let Some(bar) = bar.upgrade() {
                        bar.clear();
                    }
                    0
                }
            }),
        );
        el.add_event_listener(
            "load",
            Rc::new(move |_, _| {
                if let (Some(bar), Some(stack)) = (bar.upgrade(), stack.upgrade()) {
                    Self::on_load(&bar, &stack);
                }
                0
            }),
        );
    }

    pub fn tab_bar(&self) -> Rc<SPTabBarElement> {
        self.tab_bar.borrow().clone().expect("tab bar view not initialised")
    }

    pub fn tab_stack(&self) -> Rc<SPTabStackElement> {
        self.tab_stack.borrow().clone().expect("tab bar view not initialised")
    }

    fn on_load(tab_bar: &SPTabBarElement, tab_stack: &SPTabStackElement) {
        let Some(tab) = tab_bar.get_tabs().into_iter().next() else {
            return;
        };
        tab_stack.set_tab_id(&SPUri::new(&tab.uri).get_hash());
    }

    fn on_tab_selected(view: &ElementRef, args: &TabBarEventArgs) {
        if let Some(window) = view.el().get_window_element() {
            if let Some(window) = window.downcast::<SPWindowElement>() {
                window.navigate(&args.tab.uri);
            }
        }
    }

    fn on_tree_changed(view: &ElementRef, tab_bar: &SPTabBarElement, tab_stack: &SPTabStackElement, e: &dyn SPEventArgs) {
        let uri = view.el().get_uri();
        if uri.is_empty() {
            return;
        }
        let Some(args) = downcast_args::<SPNodeTreeEventArgs>(e) else {
            return;
        };
        let node = args.get_node();
        if node.el().get_tag_name() != "page" {
            return;
        }
        match args.get_action().as_str() {
            "remove" => tab_bar.remove(&node.el().get_uri()),
            "append" => {
                let id = node.el().get_id();
                let view_uri = format!("{uri}#{id}");
                node.set_uri(&view_uri);
                node.el().set_flex(1);
                debug_assert!(node
                    .el()
                    .get_parent_element()
                    .is_some_and(|p| std::ptr::addr_eq(Rc::as_ptr(&p), tab_stack as *const _)));
                tab_bar.add_tab(&id, &node.el().get_string_attribute("title", &id), &node.el().get_uri());
            }
            _ => {}
        }
    }

    pub fn accepts_uri_default(&self, uri: &str) -> bool {
        uri.starts_with(&self.element().get_uri())
    }

    pub fn set_hash_tab_bar_view(&self, hash: &str) {
        self.view.set_hash_default(hash);
        self.tab_stack().set_tab_id(hash);
    }

    pub fn clear_tab_bar_view(&self) {
        self.tab_bar().clear();
        self.tab_stack().clear();
    }
}

impl SPViewClass for SPTabBarViewElement {
    fn view(&self) -> &SPViewElement {
        &self.view
    }
    fn accepts_uri(&self, uri: &str) -> bool {
        self.accepts_uri_default(uri)
    }
    fn set_hash(&self, hash: &str) {
        self.set_hash_tab_bar_view(hash);
    }
    fn navigate(&self, value: &str) {
        self.tab_stack().set_tab_id(&SPUri::new(value).get_hash());
    }
}

impl SPElementClass for SPTabBarViewElement {
    fn get_type(&self) -> String {
        "tabbarview".into()
    }
    fn pack(&self) {
        SPViewElement::pack_view(self.element());
    }
    fn scroll(&self, scroll_x: i32, scroll_y: i32, _x: i32, _y: i32) {
        SPViewElement::scroll_view(self.element(), scroll_x, scroll_y);
    }
    fn mouse_down(&self, sender: Option<&SPObject>, e: &dyn SPEventArgs) {
        self.element().mouse_down_default(sender, e);
    }
    fn get_view_stack_element(&self) -> Option<ElementRef> {
        Some(self.tab_stack())
    }
    fn get_slot_element(&self) -> ElementRef {
        self.tab_stack()
    }
    fn clear(&self) {
        self.clear_tab_bar_view();
    }
    fn as_view(&self) -> Option<&dyn SPViewClass> {
        Some(self)
    }
}
