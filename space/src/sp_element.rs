//! Port of `SPNode` and `SPElement`, the base of every node in the element tree.
//!
//! The C++ classes form a pointer graph: parents own children, children keep a
//! raw pointer back to their parent, and subclasses override virtual methods.
//! Here an element is an `Rc<dyn SPElementClass>` ([`ElementRef`]):
//!
//! * [`SPElement`] holds the data of `SPNode` + `SPElement`. All fields use
//!   interior mutability, so every method takes `&self` and elements can call
//!   up and down the tree freely, like the C++ code does.
//! * [`SPElementClass`] holds the virtual methods. Each subclass is a struct
//!   that embeds its base class and overrides what the C++ class overrode.
//!   The base implementations stay callable as `SPElement::*_default`
//!   functions, the counterpart of `SPElement::pack()` style base calls.
//! * Children are strong references, parents weak ones.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::rc::{Rc, Weak};

use serde_json::{Map, Value};

use crate::libspace::{SPEvent, SPMargin, SPMessage, SPRectangle};
use crate::sp_color::{SPColor, SPGradient};
use crate::sp_event_args::{
    downcast_args, SPAttributeChangedEventArgs, SPBasicEventArgs, SPEventArgs, SPNodeTreeEventArgs,
};
use crate::sp_font_style::SPFontStyle;
use crate::sp_graphics_context::SPGraphicsContext;
use crate::sp_image::SPImage;
use crate::sp_mouse_event_args::SPMouseEventArgs;
use crate::sp_object::SPObject;
use crate::sp_observable::SPObservable;
use crate::sp_observer::SPCallback;
use crate::sp_skin::SPSkin;
use crate::sp_stylesheet::SPStylesheet;
use crate::sp_template::SPTemplate;
use crate::sp_uri_store::{to_string as status_to_string, SPUriEntry, SPUriStore, Subscription};
use crate::sp_view_element::SPViewClass;
use crate::sp_xml_parser::{parse_sp_xml, SPXmlNode};
use crate::space::Space;

pub const SP_COLOR_SCHEME_DEFAULT: i16 = 0;
pub const SP_COLOR_SCHEME_DARK: i16 = 1;
pub const SP_COLOR_SCHEME_LIGHT: i16 = 2;

pub const SP_STATE_NORMAL: i32 = 0;
pub const SP_STATE_HOVER: i32 = 1;
pub const SP_STATE_ACTIVE: i32 = 2;

pub type ElementRef = Rc<dyn SPElementClass>;
pub type WeakElementRef = Weak<dyn SPElementClass>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SPSizeScale {
    #[default]
    Undefined,
    Pixel,
    Percent,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SPUnitValue {
    pub value: f32,
    pub unit: SPSizeScale,
}

/// True when both references point at the same element.
pub fn same_element(a: &ElementRef, b: &ElementRef) -> bool {
    std::ptr::addr_eq(Rc::as_ptr(a), Rc::as_ptr(b))
}

thread_local! {
    static XUL_ERROR_HANDLER: RefCell<Option<Rc<dyn Fn(&str)>>> = const { RefCell::new(None) };
    static CAPTURED_ELEMENT: RefCell<Option<WeakElementRef>> = const { RefCell::new(None) };
}

/// Access to the element data of every element class.
pub trait SPElementBase: Any {
    fn element(&self) -> &SPElement;
    fn as_any(&self) -> &dyn Any;
}

/// Implements [`SPElementBase`] for a struct whose base element is reached
/// through the given field path, e.g. `impl_element_base!(SPButtonElement, el)`.
#[macro_export]
macro_rules! impl_element_base {
    ($type:ty, $($field:tt).+) => {
        impl $crate::sp_element::SPElementBase for $type {
            fn element(&self) -> &$crate::sp_element::SPElement {
                self.$($field).+.element()
            }
            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }
        }
    };
}

/// The virtual methods of `SPElement`.
#[allow(unused_variables)]
pub trait SPElementClass: SPElementBase {
    fn get_type(&self) -> String {
        "element".to_string()
    }

    /// Paints the element and its children at parent origin (x, y).
    fn paint(&self, x: i32, y: i32, gc: &dyn SPGraphicsContext) {
        self.element().paint_default(x, y, gc);
    }

    /// Paints the element's own content inside its absolute bounds.
    fn paint_content(&self, x: i32, y: i32, width: i32, height: i32, gc: &dyn SPGraphicsContext) {
        self.element().paint_content_default(x, y, width, height, gc);
    }

    fn pack(&self) {
        self.element().pack_default();
    }

    fn resize(&self, width: i32, height: i32) {
        self.element().resize_default(width, height);
    }

    /// `scroll(int x, int y)`: scrolls this element's own content.
    fn scroll_by(&self, x: i32, y: i32) {
        self.element().scroll_by_default(x, y);
    }

    /// `scroll(int scrollX, int scrollY, int x, int y)`: routes a wheel event at (x, y).
    fn scroll(&self, scroll_x: i32, scroll_y: i32, x: i32, y: i32) {
        self.element().scroll_default(scroll_x, scroll_y, x, y);
    }

    fn set_visible(&self, value: bool) {
        self.element().set_visible_default(value);
    }

    fn show(&self) {
        self.set_visible(true);
    }

    fn hide(&self) {
        self.set_visible(false);
    }

    fn notify(&self, event: &str, sender: Option<&SPObject>, data: &dyn SPEventArgs) -> i32 {
        self.element().notify_default(event, sender, data)
    }

    fn mouse_move(&self, x: i32, y: i32) {}

    fn mouse_click(&self, button: i32, x: i32, y: i32) {}

    fn mouse_down(&self, sender: Option<&SPObject>, e: &dyn SPEventArgs) {
        self.element().mouse_down_default(sender, e);
    }

    fn mouse_up(&self, sender: Option<&SPObject>, e: &dyn SPEventArgs) {
        self.element().mouse_up_default(sender, e);
    }

    fn on_click(&self, button: i32, x: i32, y: i32) {
        self.element().on_click_default(button, x, y);
    }

    fn process_event(&self, event: &SPEvent) {
        self.element().process_event_default(event);
    }

    fn process_message(&self, message: &SPMessage) {
        self.element().process_message_default(message);
    }

    fn request(&self, method: &str, uri: &str, data: &str) {
        self.element().request_default(method, uri, data);
    }

    fn on_load(&self) {}

    fn create_element(&self, tag_name: &str, parent: &ElementRef) -> ElementRef {
        crate::sp_element_factory::create_element(tag_name, parent)
    }

    /// The element that parsed children are appended to.
    fn get_slot_element(&self) -> ElementRef {
        self.element().this()
    }

    fn get_view_stack_element(&self) -> Option<ElementRef> {
        self.element().get_view_stack_element_default()
    }

    fn set_uri(&self, uri: &str) {
        self.element().set_uri_default(uri);
    }

    fn set_id(&self, value: &str) {
        self.element().set("id", value);
    }

    fn append_child(&self, child: &ElementRef) {
        self.element().append_child_default(child);
    }

    fn clear(&self) {
        self.element().clear_default();
    }

    fn create_graphics(&self) -> Option<Rc<dyn SPGraphicsContext>> {
        self.element().get_space().and_then(|space| space.get_graphics_context())
    }

    /// The view interface (`SPViewElement` and subclasses).
    fn as_view(&self) -> Option<&dyn SPViewClass> {
        None
    }
}

/// Data of `SPNode` + `SPElement`.
pub struct SPElement {
    // ── SPNode ──────────────────────────────────────────────────────────────
    properties: RefCell<HashMap<String, Rc<dyn Any>>>,
    string_attributes: RefCell<BTreeMap<String, String>>,
    color_attributes: RefCell<HashMap<String, SPColor>>,
    integer_attributes: RefCell<HashMap<String, i32>>,
    parent: RefCell<Option<WeakElementRef>>,
    children: RefCell<Vec<ElementRef>>,
    tag_name: RefCell<String>,
    id: RefCell<String>,
    inner_text: RefCell<String>,
    name: RefCell<String>,
    observable: SPObservable,
    space: RefCell<Option<Weak<Space>>>,
    self_ref: RefCell<Option<WeakElementRef>>,
    // ── SPElement ───────────────────────────────────────────────────────────
    background_image: RefCell<Option<Rc<SPImage>>>,
    class_list: RefCell<Vec<String>>,
    skin: RefCell<Option<Rc<dyn SPSkin>>>,
    scrollable: Cell<bool>,
    pub(crate) visible: Cell<bool>,
    color_scheme: Cell<i16>,
    state: Cell<i32>,
    margins: Cell<SPMargin>,
    padding: Cell<SPMargin>,
    font_style: RefCell<SPFontStyle>,
    data: RefCell<String>,
    width: Cell<i32>,
    height: Cell<i32>,
    x: Cell<i32>,
    y: Cell<i32>,
    z: Cell<i32>,
    scroll_x: Cell<i32>,
    scroll_y: Cell<i32>,
    unit_width: Cell<SPUnitValue>,
    unit_height: Cell<SPUnitValue>,
    absolute_bounds: Cell<SPRectangle>,
    flex: Cell<i32>,
    clip_view: Cell<bool>,
    gradient: Cell<Option<SPGradient>>,
    has_default_gradient: Cell<bool>,
    tint_color: Cell<Option<SPColor>>,
    active_color: Cell<Option<SPColor>>,
    background_color: Cell<Option<SPColor>>,
    foreground_color: Cell<Option<SPColor>>,
    draw_border: Cell<bool>,
    parsing_element: RefCell<Option<WeakElementRef>>,
    parsing_depth: Cell<i32>,
    gap: Cell<i32>,
    skip_element: Cell<bool>,
    /// Re-renders the src template when its URI entity changes.
    src_subscription: RefCell<Subscription>,
    /// Keeps the inner text in sync with the "entity" attribute's URI.
    entity_subscription: RefCell<Subscription>,
}

impl Default for SPElement {
    fn default() -> Self {
        Self {
            properties: RefCell::default(),
            string_attributes: RefCell::default(),
            color_attributes: RefCell::default(),
            integer_attributes: RefCell::default(),
            parent: RefCell::new(None),
            children: RefCell::default(),
            tag_name: RefCell::new("node".to_string()),
            id: RefCell::default(),
            inner_text: RefCell::default(),
            name: RefCell::default(),
            observable: SPObservable::new(),
            space: RefCell::new(None),
            self_ref: RefCell::new(None),
            background_image: RefCell::new(None),
            class_list: RefCell::default(),
            skin: RefCell::new(None),
            scrollable: Cell::new(false),
            visible: Cell::new(true),
            color_scheme: Cell::new(SP_COLOR_SCHEME_DEFAULT),
            state: Cell::new(SP_STATE_NORMAL),
            margins: Cell::default(),
            padding: Cell::default(),
            font_style: RefCell::new(SPFontStyle::default()),
            data: RefCell::default(),
            width: Cell::new(0),
            height: Cell::new(0),
            x: Cell::new(0),
            y: Cell::new(0),
            z: Cell::new(0),
            scroll_x: Cell::new(0),
            scroll_y: Cell::new(0),
            unit_width: Cell::default(),
            unit_height: Cell::default(),
            absolute_bounds: Cell::default(),
            flex: Cell::new(0),
            clip_view: Cell::new(true),
            gradient: Cell::new(None),
            has_default_gradient: Cell::new(false),
            tint_color: Cell::new(None),
            active_color: Cell::new(None),
            background_color: Cell::new(None),
            foreground_color: Cell::new(None),
            draw_border: Cell::new(false),
            parsing_element: RefCell::new(None),
            parsing_depth: Cell::new(0),
            gap: Cell::new(0),
            skip_element: Cell::new(false),
            src_subscription: RefCell::default(),
            entity_subscription: RefCell::default(),
        }
    }
}

impl SPElementBase for SPElement {
    fn element(&self) -> &SPElement {
        self
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A plain `<element>`: the C++ `new SPElement(parent)`.
impl SPElementClass for SPElement {}

impl SPElement {
    /// `new SPElement(parent)`.
    pub fn new(parent: Option<&ElementRef>) -> Rc<SPElement> {
        construct(SPElement::default(), parent)
    }

    /// Runs the `SPElement(SPElement *parent)` constructor body on a freshly
    /// allocated element. Every element constructor calls this first.
    pub fn init(this: &ElementRef, parent: Option<&ElementRef>) {
        let el = this.element();
        *el.self_ref.borrow_mut() = Some(Rc::downgrade(this));
        el.set_scrollable(false);
        if let Some(parent) = parent {
            el.set_parent_node(Some(parent));
            let parent_el = parent.element();
            *el.font_style.borrow_mut() = parent_el.font_style.borrow().clone();
            el.absolute_bounds.set(SPRectangle::new(
                parent_el.get_x(),
                parent_el.get_y(),
                parent_el.get_width(),
                parent_el.get_height(),
            ));
        }
        el.visible.set(true);
        el.scroll_x.set(0);
        el.scroll_y.set(0);
        el.data.borrow_mut().clear();
        el.x.set(0);
        el.y.set(0);
        el.z.set(0);
        el.width.set(0);
        el.height.set(0);
        el.margins.set(SPMargin::default());
        el.padding.set(SPMargin::default());
        let weak = Rc::downgrade(this);
        el.add_event_listener(
            "attributechanged",
            Rc::new(move |_sender, args| {
                if let Some(this) = weak.upgrade() {
                    this.element().on_attribute_changed(args);
                }
                0
            }),
        );
    }

    /// This element as an [`ElementRef`].
    pub fn this(&self) -> ElementRef {
        self.self_ref
            .borrow()
            .as_ref()
            .and_then(Weak::upgrade)
            .expect("element used before SPElement::init or after it was dropped")
    }

    pub fn try_this(&self) -> Option<ElementRef> {
        self.self_ref.borrow().as_ref().and_then(Weak::upgrade)
    }

    // ── Static handlers ─────────────────────────────────────────────────────

    /// Set by the application layer to receive XUL load-failure notifications.
    pub fn set_xul_error_handler(handler: Option<Rc<dyn Fn(&str)>>) {
        XUL_ERROR_HANDLER.with(|h| *h.borrow_mut() = handler);
    }

    fn report_xul_error(message: &str) {
        let handler = XUL_ERROR_HANDLER.with(|h| h.borrow().clone());
        if let Some(handler) = handler {
            handler(message);
        }
    }

    /// Mouse capture: while set, the element receives all mousemove/mouseup
    /// events regardless of pointer position (used for drag operations).
    pub fn set_mouse_capture(element: &ElementRef) {
        CAPTURED_ELEMENT.with(|c| *c.borrow_mut() = Some(Rc::downgrade(element)));
    }

    pub fn release_mouse_capture() {
        CAPTURED_ELEMENT.with(|c| *c.borrow_mut() = None);
    }

    pub fn captured_element() -> Option<ElementRef> {
        CAPTURED_ELEMENT.with(|c| c.borrow().as_ref().and_then(Weak::upgrade))
    }

    fn is_captured(&self) -> bool {
        match (Self::captured_element(), self.try_this()) {
            (Some(captured), Some(this)) => same_element(&captured, &this),
            _ => false,
        }
    }

    // ── SPNode: tree ────────────────────────────────────────────────────────

    pub fn get_parent_node(&self) -> Option<ElementRef> {
        self.parent.borrow().as_ref().and_then(Weak::upgrade)
    }

    pub fn get_parent_element(&self) -> Option<ElementRef> {
        self.get_parent_node()
    }

    /// Panics like the C++ `throw "Cannot set parent to self"`.
    pub fn set_parent_node(&self, parent: Option<&ElementRef>) {
        if let (Some(parent), Some(this)) = (parent, self.try_this()) {
            assert!(!same_element(parent, &this), "Cannot set parent to self");
        }
        *self.parent.borrow_mut() = parent.map(Rc::downgrade);
    }

    /// A snapshot of the children, safe to iterate while the tree changes.
    pub fn get_child_nodes(&self) -> Vec<ElementRef> {
        self.children.borrow().clone()
    }

    pub fn child_count(&self) -> usize {
        self.children.borrow().len()
    }

    pub fn set_children(&self, value: Vec<ElementRef>) {
        *self.children.borrow_mut() = value;
    }

    pub fn append_child_default(&self, child: &ElementRef) {
        self.children.borrow_mut().push(child.clone());
        child.element().set_parent_node(Some(&self.this()));
        let this = self.this();
        let args = SPNodeTreeEventArgs::new(child.clone(), this.clone(), "append");
        this.notify("treechanged", Some(&SPObject::Element(this.clone())), &args);
    }

    pub fn remove_child(&self, child: &ElementRef) {
        self.children.borrow_mut().retain(|c| !same_element(c, child));
    }

    pub fn clear_default(&self) {
        loop {
            let last = self.children.borrow().last().cloned();
            match last {
                Some(child) => self.remove_child(&child),
                None => break,
            }
        }
    }

    pub fn get_deepest_node(&self, level: i32) -> ElementRef {
        match self.children.borrow().first().cloned() {
            Some(child) => child.element().get_deepest_node(level + 1),
            None => self.this(),
        }
    }

    pub fn get_deepest_element(&self, level: i32) -> ElementRef {
        self.get_deepest_node(level)
    }

    pub fn get_node_by_id(&self, id: &str) -> Option<ElementRef> {
        for child in self.get_child_nodes() {
            if child.element().get_string_attribute("id", "") == id {
                return Some(child);
            }
            for grand_child in child.element().get_child_nodes() {
                if let Some(result) = grand_child.element().get_node_by_id(id) {
                    return Some(result);
                }
            }
        }
        None
    }

    pub fn get_element_by_id(&self, id: &str) -> Option<ElementRef> {
        for child in self.get_child_nodes() {
            if child.element().get_string_attribute("id", "") == id {
                return Some(child);
            }
            if let Some(found) = child.element().get_element_by_id(id) {
                return Some(found);
            }
        }
        None
    }

    // ── SPNode: identity ────────────────────────────────────────────────────

    pub fn get_space(&self) -> Option<Rc<Space>> {
        if let Some(space) = self.space.borrow().as_ref().and_then(Weak::upgrade) {
            return Some(space);
        }
        self.get_parent_node().and_then(|parent| parent.element().get_space())
    }

    pub fn set_space(&self, space: &Rc<Space>) {
        *self.space.borrow_mut() = Some(Rc::downgrade(space));
    }

    pub fn get_tag_name(&self) -> String {
        self.tag_name.borrow().clone()
    }

    pub fn set_tag_name(&self, value: &str) {
        *self.tag_name.borrow_mut() = value.to_string();
    }

    pub fn get_id(&self) -> String {
        self.id.borrow().clone()
    }

    pub fn get_inner_text(&self) -> String {
        self.inner_text.borrow().clone()
    }

    pub fn set_inner_text(&self, value: &str) {
        *self.inner_text.borrow_mut() = value.to_string();
    }

    pub fn set_name(&self, value: &str) {
        self.set_string_attribute("name", value);
        *self.name.borrow_mut() = value.to_string();
    }

    pub fn get_name(&self) -> String {
        self.get_string_attribute("name", "")
    }

    pub fn get_uri(&self) -> String {
        self.get_string_attribute("uri", "")
    }

    pub fn set_uri_default(&self, uri: &str) {
        self.set_string_attribute("uri", uri);
        if let Some(view_stack) = self.this().get_view_stack_element() {
            view_stack.set_uri(uri);
        }
    }

    pub fn get_value(&self) -> String {
        String::new()
    }

    // ── SPNode: attributes ──────────────────────────────────────────────────

    pub fn get_string_attributes(&self) -> BTreeMap<String, String> {
        self.string_attributes.borrow().clone()
    }

    pub fn get_integer_attributes(&self) -> HashMap<String, i32> {
        self.integer_attributes.borrow().clone()
    }

    pub fn get_color_attributes(&self) -> HashMap<String, SPColor> {
        self.color_attributes.borrow().clone()
    }

    pub fn has_attribute(&self, attr: &str) -> bool {
        self.properties.borrow().contains_key(attr)
    }

    pub fn get(&self, attr: &str) -> String {
        self.get_string_attribute(attr, "")
    }

    pub fn set(&self, attr: &str, value: &str) {
        self.set_attribute(attr, value);
    }

    pub fn get_object(&self, attr: &str) -> Option<Rc<dyn Any>> {
        self.properties.borrow().get(attr).cloned()
    }

    pub fn set_object(&self, attr: &str, value: Rc<dyn Any>) {
        self.properties.borrow_mut().insert(attr.to_string(), value);
    }

    pub fn get_string_attribute(&self, attr: &str, default_value: &str) -> String {
        self.string_attributes.borrow().get(attr).cloned().unwrap_or_else(|| default_value.to_string())
    }

    pub fn set_string_attribute_silently(&self, attr: &str, value: &str) {
        self.string_attributes.borrow_mut().insert(attr.to_string(), value.to_string());
    }

    pub fn set_string_attribute(&self, attr: &str, value: &str) {
        let old_value =
            self.string_attributes.borrow_mut().insert(attr.to_string(), value.to_string()).unwrap_or_default();
        let args = SPAttributeChangedEventArgs::new(attr, value, &old_value, "string");
        self.notify_self("attributechanged", &args);
    }

    pub fn get_integer_attribute(&self, attr: &str, default_value: i32) -> i32 {
        self.integer_attributes.borrow().get(attr).copied().unwrap_or(default_value)
    }

    pub fn set_integer_attribute(&self, attr: &str, value: i32) {
        self.integer_attributes.borrow_mut().insert(attr.to_string(), value);
        let args = SPAttributeChangedEventArgs::new(attr, &value.to_string(), "0", "integer");
        self.notify_self("attributechanged", &args);
    }

    /// `SPNode::setAttribute`: stores the string and, for numbers and
    /// `#rgb`/`#rrggbb` colors, the typed attribute too.
    pub fn set_attribute(&self, attr: &str, value: &str) {
        if attr == "id" {
            *self.id.borrow_mut() = value.to_string();
        }
        if self.get_string_attribute(attr, "") == value {
            return;
        }
        self.set_string_attribute(attr, value);
        if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) {
            if let Ok(n) = value.parse::<i32>() {
                self.set_integer_attribute(attr, n);
            }
        }
        if is_html_color(value) {
            self.set_color_attribute(attr, SPColor::from_html(value));
        }
    }

    /// `SPNode::getColorAttribute`: own color, else the nearest ancestor's.
    fn get_node_color_attribute(&self, title: &str, default_value: SPColor) -> SPColor {
        let mut color = self.color_attributes.borrow().get(title).copied().unwrap_or_default();
        if color.empty() {
            if let Some(parent) = self.get_parent_node() {
                color = parent.element().get_color_attribute(title, default_value);
            }
        }
        if color.empty() {
            default_value
        } else {
            color
        }
    }

    fn set_node_color_attribute(&self, attr: &str, value: SPColor) {
        self.color_attributes.borrow_mut().insert(attr.to_string(), value);
        let args = SPAttributeChangedEventArgs::new(
            attr,
            &value.to_html_color(),
            &SPColor::default().to_html_color(),
            "color",
        );
        self.notify_self("attributechanged", &args);
    }

    pub fn get_color_attribute(&self, attr: &str, default_value: SPColor) -> SPColor {
        match attr {
            "bgcolor" => self.get_background_color(),
            "fgcolor" => self.get_foreground_color(),
            _ => self.get_node_color_attribute(attr, default_value),
        }
    }

    pub fn set_color_attribute(&self, attr: &str, value: SPColor) {
        match attr {
            "bgcolor" => self.background_color.set(Some(value)),
            "fgcolor" => self.foreground_color.set(Some(value)),
            _ => self.set_node_color_attribute(attr, value),
        }
    }

    // ── Events ──────────────────────────────────────────────────────────────

    pub fn observable(&self) -> &SPObservable {
        &self.observable
    }

    pub fn add_event_listener(&self, event: &str, callback: SPCallback) {
        self.observable.add_event_listener(event, callback);
    }

    /// `dispatchEvent`: notifies through the (virtual) `notify` of this element.
    pub fn dispatch_event(&self, event: &str, data: &dyn SPEventArgs, sender: Option<&SPObject>) {
        match self.try_this() {
            Some(this) => {
                this.notify(event, sender, data);
            }
            None => {
                self.observable.notify(event, sender, data);
            }
        }
    }

    fn notify_self(&self, event: &str, data: &dyn SPEventArgs) {
        match self.try_this() {
            Some(this) => {
                let sender = SPObject::Element(this.clone());
                this.notify(event, Some(&sender), data);
            }
            None => {
                self.observable.notify(event, None, data);
            }
        }
    }

    /// `SPElement::notify`: notifies the listeners, then routes mouse events
    /// to the visible children under the pointer (or holding the capture).
    pub fn notify_default(&self, event: &str, sender: Option<&SPObject>, data: &dyn SPEventArgs) -> i32 {
        let mut ret = self.observable.notify(event, sender, data);
        let Some(mouse) = downcast_args::<SPMouseEventArgs>(data) else {
            return ret;
        };
        let captured = Self::captured_element();
        for element in self.get_child_nodes() {
            let el = element.element();
            if !el.is_visible() {
                continue;
            }
            // Route to the captured element even when the pointer has left
            // its bounds (or its ancestors'), so drags end on release.
            let mut is_captured = false;
            let mut e = captured.clone();
            while let Some(current) = e {
                if same_element(&current, &element) {
                    is_captured = true;
                    break;
                }
                e = current.element().get_parent_element();
            }
            let in_bounds = el.get_absolute_bounds().contains(mouse.get_x(), mouse.get_y());
            if !(is_captured || in_bounds) {
                continue;
            }
            match event {
                "mousemove" => {
                    element.notify(event, sender, data);
                    ret = 1;
                }
                "mouseup" => {
                    el.set_state(SP_STATE_NORMAL);
                    element.notify(event, sender, data);
                    ret = 1;
                }
                "mousedown" => {
                    el.set_state(SP_STATE_ACTIVE);
                    element.notify(event, sender, data);
                    ret = 1;
                }
                "click" => {
                    el.set_state(SP_STATE_NORMAL);
                    ret = 1;
                    element.notify(event, Some(&SPObject::Element(element.clone())), data);
                }
                "load" => {
                    ret = 1;
                    element.notify(event, Some(&SPObject::Element(element.clone())), data);
                }
                _ => {}
            }
        }
        ret
    }

    fn on_attribute_changed(&self, e: &dyn SPEventArgs) {
        let Some(args) = downcast_args::<SPAttributeChangedEventArgs>(e) else {
            return;
        };
        let value = args.get_new_value().to_string();
        match args.get_attribute_name() {
            "bgcolor" | "fgcolor" => self.invalidate(),
            "flex" => self.flex.set(atoi(&value)),
            "gap" => self.gap.set(atoi(&value)),
            "width" => self.set_width(atoi(&value)),
            "height" => self.set_height(atoi(&value)),
            "src" => self.load_src(&value),
            "entity" => self.bind_entity(&value),
            _ => {}
        }
    }

    pub fn mouse_down_default(&self, sender: Option<&SPObject>, e: &dyn SPEventArgs) {
        let Some(me) = downcast_args::<SPMouseEventArgs>(e) else {
            return;
        };
        let (x, y) = (me.get_x(), me.get_y());
        self.set_state(SP_STATE_ACTIVE);
        self.notify_self("mousedown", e);
        for elm in self.get_child_nodes() {
            if !elm.element().is_visible() {
                continue;
            }
            if elm.element().get_absolute_bounds().contains(x, y) {
                elm.mouse_down(sender, e);
            }
        }
    }

    pub fn mouse_up_default(&self, sender: Option<&SPObject>, e: &dyn SPEventArgs) {
        let Some(me) = downcast_args::<SPMouseEventArgs>(e) else {
            return;
        };
        let (x, y) = (me.get_x(), me.get_y());
        self.notify_self("mouseup", e);
        self.set_state(SP_STATE_NORMAL);
        for elm in self.get_child_nodes() {
            if !elm.element().is_visible() {
                continue;
            }
            if elm.element().get_absolute_bounds().contains(x, y) {
                elm.mouse_up(sender, e);
            }
        }
    }

    pub fn on_click_default(&self, button: i32, x: i32, y: i32) {
        let bounds = self.get_absolute_bounds();
        // The C++ code subtracts the y bound from both coordinates.
        let me = SPMouseEventArgs::new(button, x - bounds.y, y - bounds.y);
        self.set_state(SP_STATE_NORMAL);
        self.notify_self("click", &me);
        for elm in self.get_child_nodes() {
            if !elm.element().is_visible() {
                continue;
            }
            if elm.element().get_absolute_bounds().contains(x, y) {
                elm.on_click(button, x, y);
            }
        }
    }

    pub fn process_event_default(&self, event: &SPEvent) {
        if let Some(mouse) = downcast_args::<SPMouseEventArgs>(event.args.as_ref()) {
            let bounds = self.get_absolute_bounds();
            let inside = mouse.get_x() > bounds.x
                && mouse.get_x() < bounds.x + self.get_width()
                && mouse.get_y() > bounds.y
                && mouse.get_y() < bounds.y + self.get_height();
            let this = self.try_this().map(SPObject::Element);
            match event.type_.as_str() {
                "mousedown" if inside => self.dispatch_event("mousedown", mouse, this.as_ref()),
                "mouseup" | "mousemove" if self.is_captured() || inside => {
                    self.dispatch_event(&event.type_, mouse, this.as_ref())
                }
                _ => {}
            }
        }
        for elm in self.get_child_nodes() {
            if !elm.element().is_visible() {
                continue;
            }
            elm.process_event(event);
        }
    }

    pub fn process_message_default(&self, message: &SPMessage) {
        for elm in self.get_child_nodes() {
            elm.process_message(message);
        }
    }

    // ── Styling ─────────────────────────────────────────────────────────────

    pub fn set_state(&self, value: i32) {
        self.state.set(value);
        self.invalidate();
    }

    pub fn get_state(&self) -> i32 {
        self.state.get()
    }

    pub fn get_skin(&self) -> Option<Rc<dyn SPSkin>> {
        if let Some(skin) = self.skin.borrow().clone() {
            return Some(skin);
        }
        if let Some(parent) = self.get_parent_element() {
            if let Some(skin) = parent.element().get_skin() {
                return Some(skin);
            }
        }
        self.get_window_element().and_then(|window| window.element().get_skin())
    }

    pub fn set_skin(&self, value: Option<Rc<dyn SPSkin>>) {
        *self.skin.borrow_mut() = value;
        self.apply_skin();
        self.notify_theme_changed();
    }

    pub fn apply_skin(&self) {
        let Some(skin) = self.skin.borrow().clone() else {
            return;
        };
        self.apply_color_attribute_from_skin(skin.as_ref(), "bgcolor", "body.background.color");
        self.apply_color_attribute_from_skin(skin.as_ref(), "fgcolor", "body.foreground.color");
        let type_ = self.get_type();
        self.apply_color_attribute_from_skin(skin.as_ref(), "bgcolor", &format!("{type_}body.background.color"));
        self.apply_color_attribute_from_skin(skin.as_ref(), "fgcolor", &format!("{type_}body.foreground.color"));
        let base = skin.base();
        if base.has_image("element.background.image") {
            *self.background_image.borrow_mut() = base.get_image("element.background.image", "");
        }
    }

    fn apply_color_attribute_from_skin(&self, skin: &dyn SPSkin, attr: &str, prop: &str) {
        let pixel = skin.base().colors.borrow().get(prop).copied();
        if let Some(pixel) = pixel {
            self.set(attr, &pixel.to_hex());
        }
    }

    pub fn apply_image_attribute_from_skin(&self, skin: &dyn SPSkin, attr: &str, prop: &str) {
        let image = skin.base().images.borrow().get(prop).cloned();
        if let Some(image) = image {
            self.set_object(attr, image);
        }
    }

    pub fn notify_theme_changed(&self) {
        // Only a root (window) change is a theme switch; per-element tweaks don't flush the cache.
        if self.get_parent_node().is_some() {
            return;
        }
        if let Some(graphics) = self.get_space().and_then(|s| s.get_graphics_context()) {
            graphics.theme_changed();
        }
    }

    fn get_type(&self) -> String {
        self.try_this().map(|t| t.get_type()).unwrap_or_else(|| "element".into())
    }

    pub fn get_active_color(&self) -> Option<SPColor> {
        if let Some(active) = self.get_tint_color() {
            return Some(active);
        }
        if self.get_type() == "window" {
            return None;
        }
        let mut ret = None;
        if let Some(window) = self.get_window_element() {
            ret = window.element().get_active_color();
        }
        if let Some(parent) = self.get_parent_element() {
            ret = parent.element().get_active_color();
        }
        ret
    }

    pub fn set_active_color(&self, value: SPColor) {
        self.active_color.set(Some(value));
    }

    pub fn get_tint_color(&self) -> Option<SPColor> {
        if let Some(tint) = self.tint_color.get() {
            return Some(tint);
        }
        if self.get_type() == "window" {
            return None;
        }
        let mut ret = None;
        if let Some(window) = self.get_window_element() {
            ret = window.element().get_tint_color();
        }
        if let Some(parent) = self.get_parent_element() {
            ret = parent.element().get_tint_color();
        }
        ret
    }

    pub fn set_tint_color(&self, value: Option<SPColor>) {
        self.tint_color.set(value);
        self.notify_theme_changed();
    }

    pub fn get_color_scheme(&self) -> i16 {
        let mut scheme = self.color_scheme.get();
        if scheme == SP_COLOR_SCHEME_DEFAULT {
            if let Some(parent) = self.get_parent_element() {
                scheme = parent.element().get_color_scheme();
                if scheme != SP_COLOR_SCHEME_DEFAULT {
                    return scheme;
                }
            } else if let Some(window) = self.get_window_element() {
                scheme = window.element().get_color_scheme();
            }
        }
        if !(SP_COLOR_SCHEME_DARK..=SP_COLOR_SCHEME_LIGHT).contains(&scheme) {
            // The C++ code throws "Invalid color scheme" here; a tree without
            // a window scheme falls back to light instead.
            return SP_COLOR_SCHEME_LIGHT;
        }
        scheme
    }

    pub fn set_color_scheme(&self, value: i16) {
        self.color_scheme.set(value);
        self.notify_theme_changed();
    }

    pub fn get_gradient(&self) -> Option<SPGradient> {
        self.gradient.get()
    }

    pub fn set_gradient(&self, value: SPGradient) {
        self.gradient.set(Some(value));
    }

    pub fn has_default_gradient(&self) -> bool {
        self.has_default_gradient.get()
    }

    pub fn set_has_default_gradient(&self, value: bool) {
        self.has_default_gradient.set(value);
    }

    pub fn generate_and_set_default_gradient(&self) {
        self.set_gradient(self.generate_default_gradient());
    }

    pub fn generate_default_gradient(&self) -> SPGradient {
        let bg_color = self.get_color_attribute("bgcolor", SPColor::new(28, 28, 28, 255));
        SPGradient::new(bg_color.lighten(0.1), bg_color.lighten(-0.1), 0)
    }

    pub fn get_background_color(&self) -> SPColor {
        let mut def = SPColor::new(0, 0, 0, 0);
        if let Some(tint) = self.get_tint_color() {
            if self.get_color_scheme() == SP_COLOR_SCHEME_DARK {
                def = tint;
            } else {
                let t = tint.get_a() as f32 / 255.0 * 0.25;
                def = def.mix(&tint, t);
            }
            def = def.alpha(1.0);
        }
        match self.get_color_scheme() {
            SP_COLOR_SCHEME_LIGHT => def = def.lighten(0.3),
            SP_COLOR_SCHEME_DARK => def = def.lighten(-0.1),
            _ => {}
        }
        self.background_color.get().unwrap_or(def)
    }

    pub fn set_background_color(&self, value: SPColor) {
        self.set_color_attribute("bgcolor", value);
    }

    pub fn get_foreground_color(&self) -> SPColor {
        let mut def = SPColor::new(28, 28, 28, 255);
        if self.get_color_scheme() == SP_COLOR_SCHEME_DARK {
            def = SPColor::new(200, 200, 200, 255);
        }
        if let Some(tint) = self.get_tint_color() {
            def = def.tint(&tint);
        }
        self.foreground_color.get().unwrap_or(def)
    }

    pub fn set_foreground_color(&self, value: SPColor) {
        self.set_color_attribute("fgcolor", value);
    }

    pub fn get_border_color(&self) -> SPColor {
        self.get_color_attribute("border-color", SPColor::new(0, 0, 0, 0))
    }

    pub fn set_border_color(&self, value: SPColor) {
        self.set_color_attribute("border-color", value);
    }

    pub fn set_draw_border(&self, value: bool) {
        self.draw_border.set(value);
    }

    pub fn set_background_image(&self, value: Option<Rc<SPImage>>) {
        *self.background_image.borrow_mut() = value;
    }

    pub fn get_background_image(&self) -> Option<Rc<SPImage>> {
        self.background_image.borrow().clone()
    }

    pub fn has_class(&self, value: &str) -> bool {
        self.class_list.borrow().iter().any(|c| c == value)
    }

    pub fn add_class(&self, value: &str) {
        self.class_list.borrow_mut().push(value.to_string());
    }

    pub fn remove_class(&self, value: &str) {
        let mut list = self.class_list.borrow_mut();
        if let Some(index) = list.iter().position(|c| c == value) {
            list.truncate(index);
        }
    }

    pub fn apply_stylesheet(&self, _style: &SPStylesheet) {}

    pub fn get_font_style(&self) -> SPFontStyle {
        self.font_style.borrow().clone()
    }

    pub fn set_font_style(&self, value: SPFontStyle) {
        *self.font_style.borrow_mut() = value;
    }

    // ── Geometry ────────────────────────────────────────────────────────────

    pub fn get_x(&self) -> i32 {
        self.x.get()
    }
    pub fn get_y(&self) -> i32 {
        self.y.get()
    }
    pub fn get_z(&self) -> i32 {
        self.z.get()
    }
    pub fn get_width(&self) -> i32 {
        self.width.get()
    }
    pub fn get_height(&self) -> i32 {
        self.height.get()
    }

    pub fn set_x(&self, x: i32) {
        self.x.set(x.max(0));
    }
    pub fn set_y(&self, y: i32) {
        self.y.set(y.max(0));
    }
    pub fn set_z(&self, z: i32) {
        self.z.set(z);
    }
    pub fn set_width(&self, width: i32) {
        self.width.set(width.max(1));
    }
    pub fn set_height(&self, height: i32) {
        self.height.set(height.max(1));
    }

    pub fn get_absolute_bounds(&self) -> SPRectangle {
        self.absolute_bounds.get()
    }

    pub fn set_absolute_bounds(&self, value: SPRectangle) {
        self.absolute_bounds.set(value);
    }

    pub fn get_margins(&self) -> SPMargin {
        self.margins.get()
    }
    pub fn set_margins(&self, value: SPMargin) {
        self.margins.set(value);
    }
    pub fn get_padding(&self) -> SPMargin {
        self.padding.get()
    }
    pub fn set_padding(&self, value: SPMargin) {
        self.padding.set(value);
    }

    pub fn get_flex(&self) -> i32 {
        self.flex.get()
    }
    pub fn set_flex(&self, value: i32) {
        self.flex.set(value);
    }
    pub fn get_gap(&self) -> i32 {
        self.gap.get()
    }
    pub fn set_gap(&self, value: i32) {
        self.gap.set(value);
    }

    pub fn get_unit_width(&self) -> SPUnitValue {
        self.unit_width.get()
    }
    pub fn get_unit_height(&self) -> SPUnitValue {
        self.unit_height.get()
    }
    pub fn set_unit_width(&self, value: SPUnitValue) {
        self.unit_width.set(value);
    }
    pub fn set_unit_height(&self, value: SPUnitValue) {
        self.unit_height.set(value);
    }

    pub fn is_visible(&self) -> bool {
        self.visible.get()
    }

    /// The base `SPElement::setVisible`.
    pub fn set_visible_default(&self, value: bool) {
        self.visible.set(value);
    }

    pub fn is_scrollable(&self) -> bool {
        self.scrollable.get()
    }
    pub fn set_scrollable(&self, value: bool) {
        self.scrollable.set(value);
    }
    pub fn get_scroll_x(&self) -> i32 {
        self.scroll_x.get()
    }
    pub fn get_scroll_y(&self) -> i32 {
        self.scroll_y.get()
    }
    pub fn set_scroll_x(&self, value: i32) {
        self.scroll_x.set(value);
    }
    pub fn set_scroll_y(&self, value: i32) {
        self.scroll_y.set(value);
    }
    pub fn get_clip_view(&self) -> bool {
        self.clip_view.get()
    }
    pub fn set_clip_view(&self, value: bool) {
        self.clip_view.set(value);
    }

    pub fn get_skip_element(&self) -> bool {
        self.skip_element.get()
    }
    pub fn set_skip_element(&self, value: bool) {
        self.skip_element.set(value);
    }

    pub fn increase_parsing_depth(&self) {
        self.parsing_depth.set(self.parsing_depth.get() + 1);
    }
    pub fn decrease_parsing_depth(&self) {
        self.parsing_depth.set(self.parsing_depth.get() - 1);
    }
    pub fn get_parsing_depth(&self) -> i32 {
        self.parsing_depth.get()
    }
    pub fn get_parsing_element(&self) -> Option<ElementRef> {
        self.parsing_element.borrow().as_ref().and_then(Weak::upgrade)
    }
    pub fn set_parsing_element(&self, value: Option<&ElementRef>) {
        *self.parsing_element.borrow_mut() = value.map(Rc::downgrade);
    }

    pub fn get_window_element(&self) -> Option<ElementRef> {
        let mut parent = self.get_parent_element();
        while let Some(current) = parent {
            if current.get_type() == "window" {
                return Some(current);
            }
            parent = current.element().get_parent_element();
        }
        None
    }

    pub fn invalidate(&self) {
        let rect = self.get_absolute_bounds();
        if let Some(window) = self.get_window_element() {
            if let Some(gc) = window.create_graphics() {
                gc.invalidate_region(rect);
            }
        }
    }

    pub fn resize_default(&self, width: i32, height: i32) {
        self.width.set(width);
        self.height.set(height);
        self.invalidate();
        self.this().pack();
    }

    pub fn scroll_default(&self, scroll_x: i32, scroll_y: i32, x: i32, y: i32) {
        for elm in self.get_child_nodes() {
            if elm.element().is_visible() && elm.element().get_absolute_bounds().contains(x, y) {
                elm.scroll(scroll_x, scroll_y, x, y);
            }
        }
    }

    pub fn scroll_by_default(&self, x: i32, y: i32) {
        if self.clip_view.get() {
            self.scroll_x.set(self.scroll_x.get() + x);
            self.scroll_y.set((self.scroll_y.get() + y).max(0));
            self.invalidate();
        }
    }

    pub fn scroll_to(&self, x: i32, y: i32) {
        if self.clip_view.get() {
            self.scroll_x.set(x);
            self.scroll_y.set(y.max(0));
            self.invalidate();
        }
    }

    pub fn get_text_bounds(&self) -> SPRectangle {
        let text = self.get_inner_text();
        if text.is_empty() {
            return SPRectangle::default();
        }
        match self.try_this().and_then(|this| this.create_graphics()) {
            Some(gc) => gc.measure_string(&text, &self.font_style.borrow()),
            None => SPRectangle::default(),
        }
    }

    pub fn resize_to_text(&self, width: bool, height: bool) {
        if self.get_flex() > 0 {
            return;
        }
        let bounds = self.get_text_bounds();
        if width && self.get_width() <= 0 {
            self.set_width(bounds.width * 4);
        }
        if height && self.get_height() <= 0 {
            self.set_height(bounds.height * 2);
        }
    }

    /// `SPElement::pack`: resolves unit sizes and packs the children.
    pub fn pack_default(&self) {
        if !self.get_inner_text().is_empty() {
            self.resize_to_text(false, true);
        }
        let unit_width = self.get_unit_width();
        if unit_width.value > 0.0 {
            match unit_width.unit {
                SPSizeScale::Undefined => {}
                SPSizeScale::Percent => {
                    if let Some(parent) = self.get_parent_element() {
                        self.set_width((parent.element().get_width() as f32 * unit_width.value / 100.0) as i32);
                    }
                }
                SPSizeScale::Pixel => self.set_width(unit_width.value as i32),
            }
        }
        let unit_height = self.get_unit_height();
        if unit_height.value > 0.0 {
            match unit_height.unit {
                SPSizeScale::Undefined => {}
                SPSizeScale::Percent => {
                    if let Some(parent) = self.get_parent_element() {
                        self.set_height((parent.element().get_height() as f32 * unit_height.value / 100.0) as i32);
                    }
                }
                SPSizeScale::Pixel => self.set_height(unit_height.value as i32),
            }
        }
        for child in self.get_child_nodes() {
            child.pack();
        }
    }

    // ── Painting ────────────────────────────────────────────────────────────

    /// `SPElement::paint(int x, int y, SPGraphicsContext *c)`.
    pub fn paint_default(&self, x: i32, y: i32, c: &dyn SPGraphicsContext) {
        if !self.is_visible() {
            return;
        }
        let x = x + self.get_x();
        let y = y + self.get_y();
        let width = self.get_width();
        let height = self.get_height();
        let bounds = SPRectangle::new(x, y, width, height);
        self.absolute_bounds.set(bounds);
        if self.clip_view.get() {
            c.set_clip(&bounds);
        }
        let this = self.this();
        this.paint_content(bounds.x, bounds.y, bounds.width, bounds.height, c);
        let border_color = self.get_border_color();
        if !border_color.empty() && self.draw_border.get() {
            c.draw_rectangle(x - self.get_scroll_x(), y - self.get_scroll_y(), width, height, border_color);
        }
        for child in self.get_child_nodes() {
            child.paint(bounds.x, bounds.y, c);
        }
        if self.clip_view.get() {
            c.declip_rect();
        }
    }

    /// `SPElement::paint(int x, int y, int width, int height, SPGraphicsContext *c)`.
    pub fn paint_content_default(&self, x: i32, y: i32, width: i32, height: i32, c: &dyn SPGraphicsContext) {
        let fg_color = self.get_foreground_color();
        let bg_color = self.get_background_color();
        let font_size = self.get_integer_attribute("size", 11);
        let font_family = self.get_string_attribute("font", "Tahoma");
        let state_class = state_class(self.get_state());

        let this = self.this();
        let drawn = self
            .get_skin()
            .is_some_and(|skin| skin.draw_element(c, &this, state_class, &SPRectangle::new(x, y, width, height)));
        if !drawn && bg_color.get_a() > 0 {
            c.fill_rectangle(x, y, width, height, bg_color);
        }
        let text = self.get_inner_text();
        if !text.is_empty() {
            c.draw_string(
                &text,
                &SPFontStyle::new(&font_family, font_size, font_size / 2, false, false),
                fg_color,
                x - self.get_scroll_x(),
                y - self.get_scroll_y(),
                self.get_width(),
                self.get_height(),
            );
        }
    }

    // ── Loading XUL ─────────────────────────────────────────────────────────

    pub fn get_src(&self) -> String {
        self.get_string_attribute("src", "")
    }

    pub fn set_src(&self, value: &str) {
        self.set_string_attribute("src", value);
    }

    pub fn refresh(&self) {
        let src = self.get_src();
        self.set_string_attribute_silently("src", "");
        self.set_attribute("src", &src);
    }

    pub fn load_src(&self, uri: &str) {
        if uri.is_empty() {
            return;
        }
        self.set_string_attribute_silently("src", uri);
        let this = self.this();
        let args = SPBasicEventArgs::new(Some(this.clone()));
        this.notify("unload", Some(&SPObject::Element(this.clone())), &args);
        this.request("GET", uri, "");
    }

    /// `SPElement::request`: renders `views/<route>.xml` for bungalow: URIs
    /// with whatever the URI store knows, and re-renders on updates.
    pub fn request_default(&self, method: &str, uri: &str, data: &str) {
        let prefix = "bungalow:";
        let Some(route) = uri.strip_prefix(prefix) else {
            self.load_xul("./views/error.xml");
            return;
        };
        let route = route.split(':').next().unwrap_or_default().to_string();
        if route != "start" && route != "artist" {
            self.load_xul("./views/error.xml");
            return;
        }
        let filename = format!("./views/{route}.xml");
        let Ok(source) = std::fs::read_to_string(&filename) else {
            self.load_xul("./views/error.xml");
            return;
        };

        let mut request_data = Value::Object(Map::new());
        if !data.is_empty() {
            if let Ok(parsed @ Value::Object(_)) = serde_json::from_str::<Value>(data) {
                request_data = parsed;
            }
        }
        let action = request_data.get("command").and_then(Value::as_str).unwrap_or_default().to_string();

        // Render whatever the store already knows (memory or the JSON cache)
        // right away, then re-render when the resolver delivers fresh data.
        self.src_subscription.borrow_mut().reset();
        if method == "GET" {
            SPUriStore::with(|store| {
                store.resolve_uri(uri, -1);
            });
        }
        self.render_template(&source, method, uri, &action, &request_data);
        if method == "GET" {
            let weak = Rc::downgrade(&self.this());
            let uri_owned = uri.to_string();
            let subscription = SPUriStore::with(|store| {
                store.subscribe(
                    uri,
                    Rc::new(move |_, _| {
                        if let Some(this) = weak.upgrade() {
                            this.element().render_template(&source, "GET", &uri_owned, "", &Value::Object(Map::new()));
                        }
                    }),
                )
            });
            *self.src_subscription.borrow_mut() = subscription;
        }
    }

    fn render_template(&self, source: &str, method: &str, uri: &str, action: &str, request_data: &Value) {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let entry = SPUriStore::with(|store| store.get(uri));
        let mut model = Map::new();
        model.insert("now".into(), Value::String(now));
        model.insert("entity".into(), entry.data.clone());
        model.insert("status".into(), Value::String(status_to_string(entry.status).into()));
        let model = Value::Object(model);

        match SPTemplate::render(source, method, uri, action, request_data, &model, false) {
            Ok(xul) => self.set_inner_xul(&xul),
            Err(error) => {
                Self::report_xul_error(&error);
                self.load_xul("./views/error.xml");
            }
        }
    }

    pub fn on_load_default(&self) {}

    /// Loads a XUL document from disk, falling back to the error view (but
    /// never retrying the error view itself).
    pub fn load_xul(&self, filename: &str) {
        const ERROR_VIEW: &str = "./views/error.xml";
        let root = std::fs::read_to_string(filename).ok().and_then(|contents| parse_sp_xml(&contents).ok());
        match root {
            Some(root) => self.load_xul_from_xml_doc(&root),
            None => {
                if filename != ERROR_VIEW {
                    Self::report_xul_error("An error occurred");
                    self.load_xul(ERROR_VIEW);
                }
            }
        }
    }

    pub fn set_inner_xul(&self, value: &str) {
        if let Ok(root) = parse_sp_xml(value) {
            self.load_xul_from_xml_doc(&root);
        }
    }

    /// `SPNode::setInnerXml`: parses children into this element.
    pub fn set_inner_xml(&self, value: &str) {
        self.set_inner_xul(value);
    }

    pub fn load_xul_from_xml_doc(&self, root: &SPXmlNode) {
        let this = self.this();
        this.clear();
        let slot = this.get_slot_element();
        for (key, value) in &root.attributes {
            slot.element().set(key, value);
        }
        for child in root.children.iter().filter(|c| !c.text_node) {
            let elm = self.parse_element(child, &slot, &this);
            slot.append_child(&elm);
        }
        this.pack();
        self.invalidate();
        let args = SPBasicEventArgs::new(Some(this.clone()));
        this.notify("load", Some(&SPObject::Element(this.clone())), &args);
    }

    pub fn parse_element(&self, xml_node: &SPXmlNode, parent: &ElementRef, document: &ElementRef) -> ElementRef {
        let this = self.this();
        let elm = this.create_element(&xml_node.name, parent);
        elm.set_visible(true);
        for (key, value) in &xml_node.attributes {
            elm.element().set(key, value);
            if key == "hidden" {
                elm.set_visible(false);
            }
            if key == "onclick" && elm.get_type() == "button" {
                let responder = Rc::downgrade(&this);
                elm.element().add_event_listener(
                    "click",
                    Rc::new(move |sender, _args| {
                        if let (Some(responder), Some(sender)) =
                            (responder.upgrade(), sender.and_then(SPObject::element))
                        {
                            responder.element().on_post_back_button_click(sender);
                        }
                        0
                    }),
                );
            }
        }
        for child in &xml_node.children {
            let slot = elm.get_slot_element();
            if child.text_node {
                slot.element().set_inner_text(&child.text);
            } else {
                let child_element = self.parse_element(child, &slot, document);
                slot.append_child(&child_element);
                this.pack();
            }
        }
        // The XML text above is only a fallback; put the cached entity back.
        let entity_uri = elm.element().get_string_attribute("entity", "");
        if !entity_uri.is_empty() {
            elm.element().bind_entity(&entity_uri);
        }
        elm.pack();
        elm
    }

    fn on_post_back_button_click(&self, button: &ElementRef) {
        let mut form_data = self.get_form_data();
        form_data.insert("command".into(), Value::String(button.element().get_string_attribute("onclick", "submit")));
        self.post_back(&Value::Object(form_data));
    }

    pub fn post_back(&self, payload: &Value) {
        let uri = self.get_src();
        self.this().request("POST", &uri, &payload.to_string());
    }

    /// Collects the values of every `<input name="...">` below this element.
    pub fn get_form_data(&self) -> Map<String, Value> {
        let mut form_data = Map::new();
        for child in self.get_child_nodes() {
            if let Some(input) = child.as_any().downcast_ref::<crate::sp_input_element::SPInputElement>() {
                form_data.insert(input.element().get_string_attribute("name", ""), Value::String(input.get_value()));
            }
            for (key, value) in child.element().get_form_data() {
                form_data.insert(key, value);
            }
        }
        form_data
    }

    // ── Entities ────────────────────────────────────────────────────────────

    /// `<text entity="bungalow:artist:1" entity-field="name">Fallback</text>`
    /// shows the cached field immediately and updates it once the entity
    /// has been fetched.
    pub fn bind_entity(&self, uri: &str) {
        self.entity_subscription.borrow_mut().reset();
        if uri.is_empty() {
            return;
        }
        let entry = SPUriStore::with(|store| store.resolve_uri(uri, -1));
        self.apply_entity(&entry);
        let weak = Rc::downgrade(&self.this());
        let subscription = SPUriStore::with(|store| {
            store.subscribe(
                uri,
                Rc::new(move |_, entry| {
                    if let Some(this) = weak.upgrade() {
                        this.element().apply_entity(entry);
                    }
                }),
            )
        });
        *self.entity_subscription.borrow_mut() = subscription;
    }

    fn apply_entity(&self, entry: &SPUriEntry) {
        self.set_string_attribute_silently("entity-status", status_to_string(entry.status));
        let field = self.get_string_attribute("entity-field", "name");
        let Some(value) = entry.data.as_object().and_then(|o| o.get(&field)) else {
            return;
        };
        let text = match value {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        if text != self.get_inner_text() {
            self.set_inner_text(&text);
            self.invalidate();
        }
    }

    // ── Children lookup ─────────────────────────────────────────────────────

    pub fn get_tab_bar_view_element(&self) -> Option<ElementRef> {
        self.get_child_nodes().into_iter().find(|c| c.get_type() == "tabbarview")
    }

    pub fn get_view_stack_element_default(&self) -> Option<ElementRef> {
        self.get_child_nodes().into_iter().find(|c| c.get_type() == "viewstack")
    }
}

/// Allocates an element and runs the base constructor, the equivalent of
/// `new T(parent)` up to the subclass constructor body.
pub fn construct<T: SPElementClass>(value: T, parent: Option<&ElementRef>) -> Rc<T> {
    let rc = Rc::new(value);
    let element: ElementRef = rc.clone();
    SPElement::init(&element, parent);
    rc
}

/// Convenience accessors on element references.
pub trait ElementRefExt {
    fn el(&self) -> &SPElement;
    fn downcast<T: 'static>(&self) -> Option<&T>;
    fn as_object(&self) -> SPObject;
}

impl ElementRefExt for ElementRef {
    fn el(&self) -> &SPElement {
        self.element()
    }
    fn downcast<T: 'static>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }
    fn as_object(&self) -> SPObject {
        SPObject::Element(self.clone())
    }
}

/// Upcasts a concrete element.
pub fn as_element<T: SPElementClass>(rc: &Rc<T>) -> ElementRef {
    rc.clone()
}

pub fn state_class(state: i32) -> &'static str {
    match state {
        SP_STATE_ACTIVE => "active",
        SP_STATE_HOVER => "hover",
        _ => "",
    }
}

/// `atoi`: leading integer of a string, 0 when there is none.
pub fn atoi(value: &str) -> i32 {
    let trimmed = value.trim_start();
    let mut end = 0;
    for (i, c) in trimmed.char_indices() {
        if c.is_ascii_digit() || (i == 0 && (c == '-' || c == '+')) {
            end = i + c.len_utf8();
        } else {
            break;
        }
    }
    trimmed[..end].parse().unwrap_or(0)
}

fn is_html_color(value: &str) -> bool {
    let Some(hex) = value.strip_prefix('#') else {
        return false;
    };
    (hex.len() == 6 || hex.len() == 3) && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atoi_reads_leading_integers() {
        assert_eq!(atoi("42"), 42);
        assert_eq!(atoi("100%"), 100);
        assert_eq!(atoi("-3px"), -3);
        assert_eq!(atoi("abc"), 0);
    }

    #[test]
    fn attributes_are_typed() {
        let el = SPElement::new(None);
        el.set("size", "12");
        el.set("border-color", "#ff0000");
        assert_eq!(el.get_integer_attribute("size", 0), 12);
        assert_eq!(el.get_color_attribute("border-color", SPColor::default()), SPColor::new(255, 0, 0, 255));
        el.set("width", "120");
        assert_eq!(el.get_width(), 120);
        el.set("flex", "2");
        assert_eq!(el.get_flex(), 2);
    }

    #[test]
    fn children_know_their_parent() {
        let parent: ElementRef = SPElement::new(None);
        let child: ElementRef = SPElement::new(Some(&parent));
        parent.append_child(&child);
        child.element().set_id("c");
        assert!(same_element(&child.element().get_parent_element().unwrap(), &parent));
        assert!(parent.element().get_element_by_id("c").is_some());
        parent.clear();
        assert_eq!(parent.element().child_count(), 0);
    }

    #[test]
    fn element_listeners_receive_events() {
        let el: ElementRef = SPElement::new(None);
        let hits = Rc::new(Cell::new(0));
        let counter = hits.clone();
        el.element().add_event_listener(
            "custom",
            Rc::new(move |_, _| {
                counter.set(counter.get() + 1);
                0
            }),
        );
        el.element().dispatch_event("custom", &SPBasicEventArgs::default(), None);
        assert_eq!(hits.get(), 1);
    }
}
