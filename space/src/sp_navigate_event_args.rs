//! Port of `SPNavigateEventArgs`.

use crate::sp_uri::SPUri;

#[derive(Debug, Clone)]
pub struct SPNavigateEventArgs {
    uri: SPUri,
    url: String,
}

impl SPNavigateEventArgs {
    pub fn new(url: &str) -> Self {
        Self { uri: SPUri::new(url), url: url.to_string() }
    }
    pub fn get_url(&self) -> String {
        self.url.clone()
    }
    pub fn get_uri(&self) -> &SPUri {
        &self.uri
    }
}
crate::impl_event_args!(SPNavigateEventArgs);
