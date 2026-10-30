//! Port of `Spider`: dispatches requests to the registered views.

use std::collections::BTreeMap;
use std::rc::Rc;

use space::Space;

use crate::spider_view::SpiderView;

pub struct Spider {
    space: Rc<Space>,
    views: Vec<SpiderView>,
}

impl Spider {
    pub fn new(space: Rc<Space>) -> Self {
        Self { space, views: Vec::new() }
    }

    pub fn space(&self) -> &Rc<Space> {
        &self.space
    }

    /// Returns the output of the last registered view accepting `uri`, like
    /// the C++ loop that kept overwriting its result.
    pub fn request(&mut self, method: &str, uri: &str) -> String {
        let mut result = String::new();
        for view in self.views.iter_mut() {
            if view.accepts_uri(uri) {
                result = view.process(method, uri, &BTreeMap::new()).unwrap_or_else(|e| {
                    log::error!("{e}");
                    String::new()
                });
            }
        }
        result
    }

    pub fn register_view(&mut self, view: SpiderView) {
        self.views.push(view);
    }
}
