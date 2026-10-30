//! Port of `SPRequest`, `SPResponse` and `ISPRequestHandler`.

use std::any::Any;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SPResponse {
    pub content_type: String,
    pub status: i32,
    pub body: String,
}

/// Request handler for Space.
pub trait ISPRequestHandler {
    /// Returns whether this handles this type of URI.
    fn handles_uri(&self, _uri: &str) -> bool {
        false
    }
    /// Processes request.
    fn process_request(
        &self,
        _method: &str,
        _uri: &str,
        _headers: &BTreeMap<String, String>,
        _payload: Option<&dyn Any>,
    ) -> Option<SPResponse> {
        None
    }
}

thread_local! {
    static HANDLERS: RefCell<Vec<Rc<dyn ISPRequestHandler>>> = const { RefCell::new(Vec::new()) };
}

#[derive(Default)]
pub struct SPRequest {
    pub method: String,
    pub uri: String,
    pub headers: BTreeMap<String, String>,
    pub status: i32,
    pub response: Option<SPResponse>,
}

impl SPRequest {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_handler(handler: Rc<dyn ISPRequestHandler>) {
        HANDLERS.with(|h| h.borrow_mut().push(handler));
    }

    pub fn get_matching_handler_for_uri(&self, uri: &str) -> Option<Rc<dyn ISPRequestHandler>> {
        HANDLERS.with(|h| h.borrow().iter().find(|handler| handler.handles_uri(uri)).cloned())
    }

    pub fn request(
        &self,
        method: &str,
        uri: &str,
        headers: &BTreeMap<String, String>,
        data: Option<&dyn Any>,
    ) -> Option<SPResponse> {
        self.get_matching_handler_for_uri(uri)?.process_request(method, uri, headers, data)
    }
}

/// Port of `SPServer`: a placeholder for internal `bungalow:internal:` pages.
#[derive(Default)]
pub struct SPServer;

impl SPServer {
    pub fn request(&self, method: &str, uri: &str, reply: &dyn Fn(i32, String)) {
        if method == "GET" && uri == "bungalow:internal:home" {
            reply(200, String::new());
        }
    }
}
