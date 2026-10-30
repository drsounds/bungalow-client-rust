//! Port of `SPNativeAppElement`: hosts an [`SPNativeApp`] and renders its
//! responses as XUL.

use crate::sp_element::SPElementBase;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use serde_json::{Map, Value};

use crate::impl_element_base;
use crate::sp_app_bundle::SPAppBundle;
use crate::sp_element::{construct, ElementRef, SPElementClass};
use crate::sp_native_app::SPNativeApp;
use crate::sp_vbox_element::SPVBoxElement;
use crate::sp_window_element::SPWindowElement;
use crate::sp_xml_parser::parse_sp_xml;

const MAX_REDIRECTS: i32 = 8;

pub struct SPNativeAppElement {
    pub vbox: SPVBoxElement,
    bundle: SPAppBundle,
    app: RefCell<Result<SPNativeApp, String>>,
    error_handler: RefCell<Option<Rc<dyn Fn(&str)>>>,
    /// redirects being followed (navigation is synchronous)
    redirects: Cell<i32>,
}

impl_element_base!(SPNativeAppElement, vbox);

impl SPNativeAppElement {
    pub fn new(parent: Option<&ElementRef>, bundle: &SPAppBundle) -> Rc<Self> {
        construct(
            Self {
                vbox: SPVBoxElement::default(),
                bundle: bundle.clone(),
                app: RefCell::new(SPNativeApp::new(bundle)),
                error_handler: RefCell::new(None),
                redirects: Cell::new(0),
            },
            parent,
        )
    }

    pub fn set_error_handler(&self, handler: Option<Rc<dyn Fn(&str)>>) {
        *self.error_handler.borrow_mut() = handler;
    }

    pub fn restart(&self) {
        *self.app.borrow_mut() = SPNativeApp::new(&self.bundle);
    }

    fn report_error(&self, message: &str) {
        let handler = self.error_handler.borrow().clone();
        match handler {
            Some(handler) => handler(message),
            None => log::error!("{message}"),
        }
    }
}

impl SPElementClass for SPNativeAppElement {
    fn get_type(&self) -> String {
        "nativeapp".into()
    }

    fn pack(&self) {
        SPVBoxElement::pack_vbox(self.element());
    }

    fn request(&self, method: &str, uri: &str, data: &str) {
        let request_data = match serde_json::from_str::<Value>(data) {
            Ok(parsed @ Value::Object(_)) if !data.is_empty() => parsed,
            _ => Value::Object(Map::new()),
        };
        let response = match self.app.borrow().as_ref() {
            Ok(app) => app.handle(method, uri, &request_data),
            Err(error) => Err(error.clone()),
        };
        let response = match response {
            Ok(response) => response,
            Err(error) => return self.report_error(&error),
        };
        if response.redirect.is_empty() {
            match parse_sp_xml(&response.xul) {
                Ok(root) => self.element().load_xul_from_xml_doc(&root),
                Err(error) => self.report_error(&format!("The view for {uri} is not valid XUL: {error}")),
            }
            return;
        }
        if self.redirects.get() >= MAX_REDIRECTS {
            return self.report_error(&format!("Too many redirects, the last from {uri} to {}", response.redirect));
        }
        self.redirects.set(self.redirects.get() + 1);
        if let Some(window) = self.element().get_window_element() {
            if let Some(window) = window.as_any().downcast_ref::<SPWindowElement>() {
                window.navigate(&response.redirect);
            }
        }
        self.redirects.set(self.redirects.get() - 1);
    }
}
