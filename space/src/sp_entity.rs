//! Port of `SPEntity.h` (namespace `Entify`): a registry of entity resolvers
//! returning promises. Superseded by [`crate::sp_uri_store`], kept for parity.

use std::any::Any;
use std::rc::Rc;

pub type PromiseCallback = Box<dyn Fn(&Promise, &dyn Any)>;

#[derive(Default)]
pub struct Promise {
    done: Option<PromiseCallback>,
    fail: Option<PromiseCallback>,
    always: Option<PromiseCallback>,
}

impl Promise {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn done(&mut self, callback: PromiseCallback) {
        self.done = Some(callback);
    }
    pub fn fail(&mut self, callback: PromiseCallback) {
        self.fail = Some(callback);
    }
    pub fn always(&mut self, callback: PromiseCallback) {
        self.always = Some(callback);
    }
    pub fn set_done(&self, data: &dyn Any) {
        if let Some(done) = &self.done {
            done(self, data);
        }
        if let Some(always) = &self.always {
            always(self, data);
        }
    }
    pub fn set_fail(&self, data: &dyn Any) {
        if let Some(fail) = &self.fail {
            fail(self, data);
        }
        if let Some(always) = &self.always {
            always(self, data);
        }
    }
}

pub trait Resolver {
    /// Does a request.
    fn request(&self, _method: &str, _uri: &str, _data: Option<&dyn Any>) -> Promise {
        Promise::new()
    }
    /// Checks if the current entity URI is covered by this resolver.
    fn accepts_uri(&self, _uri: &str) -> bool {
        false
    }
}

#[derive(Default)]
pub struct SPEntity {
    resolvers: Vec<Rc<dyn Resolver>>,
}

impl SPEntity {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_resolver(&mut self, resolver: Rc<dyn Resolver>) {
        self.resolvers.push(resolver);
    }

    /// Asks the first resolver that accepts `uri`.
    pub fn request(&self, method: &str, uri: &str, data: Option<&dyn Any>) -> Option<Promise> {
        self.resolvers
            .iter()
            .find(|r| r.accepts_uri(uri))
            .map(|r| r.request(method, uri, data))
    }
}
