//! Port of `SPInfoElement`: a dismissable message bar.

use crate::sp_element::SPElementBase;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::impl_element_base;
use crate::libspace::SPMargin;
use crate::sp_button_element::SPButtonElement;
use crate::sp_element::{construct, ElementRef, SPElementClass};
use crate::sp_hbox_element::SPHBoxElement;
use crate::sp_message_type::SPMessageType;
use crate::sp_text_element::SPTextElement;

#[derive(Default)]
pub struct SPInfoElement {
    pub hbox: SPHBoxElement,
    type_: Cell<SPMessageType>,
    text_element: RefCell<Option<Rc<SPTextElement>>>,
    btn_element: RefCell<Option<Rc<SPButtonElement>>>,
}

impl_element_base!(SPInfoElement, hbox);

impl SPInfoElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        let el = rc.element();
        let this = el.this();
        el.set_tag_name("info");
        el.set("height", "32");
        el.set_padding(SPMargin::default());

        let icon = SPTextElement::new(Some(&this));
        icon.el.set("width", "128");
        el.append_child_default(&(icon as ElementRef));

        let text = SPTextElement::new(Some(&this));
        text.el.set("flex", "1");
        el.append_child_default(&(text.clone() as ElementRef));

        let button = SPButtonElement::new(Some(&this));
        button.el.set("width", "64");
        button.el.set_inner_text("x");
        let weak = Rc::downgrade(&rc);
        button.el.add_event_listener(
            "click",
            Rc::new(move |_, _| {
                if let Some(this) = weak.upgrade() {
                    this.hide();
                    this.element().invalidate();
                }
                0
            }),
        );
        el.append_child_default(&(button.clone() as ElementRef));

        *rc.text_element.borrow_mut() = Some(text);
        *rc.btn_element.borrow_mut() = Some(button);
        rc
    }

    pub fn get_message_type(&self) -> SPMessageType {
        self.type_.get()
    }

    pub fn show_message(&self, type_: SPMessageType, text: &str) {
        self.set_message_type(type_);
        if let Some(t) = self.text_element.borrow().as_ref() {
            t.el.set_inner_text(text);
        }
        self.show();
        self.element().invalidate();
    }

    pub fn set_message_type(&self, type_: SPMessageType) {
        self.type_.set(type_);
        let bgcolor = match type_ {
            SPMessageType::Warning => "#ffffaa",
            SPMessageType::Info => "#00ffff",
            SPMessageType::Error => "#ffeeee",
            SPMessageType::Message => "#eeffee",
        };
        self.element().set("bgcolor", bgcolor);
        for child in self.element().get_child_nodes() {
            child.element().set("bgcolor", bgcolor);
        }
    }
}

impl SPElementClass for SPInfoElement {
    fn get_type(&self) -> String {
        "info".into()
    }
    fn pack(&self) {
        SPHBoxElement::pack_hbox(self.element());
    }
}
