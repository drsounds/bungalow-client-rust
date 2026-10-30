//! Port of `SPBookmarkEventArgs`.

#[derive(Debug, Clone)]
pub struct SPBookmarkEventArgs {
    uri: String,
    title: String,
}

impl SPBookmarkEventArgs {
    pub fn new(uri: &str, title: &str) -> Self {
        Self {
            uri: uri.to_string(),
            title: title.to_string(),
        }
    }
    pub fn get_uri(&self) -> &str {
        &self.uri
    }
    pub fn get_title(&self) -> &str {
        &self.title
    }
}
crate::impl_event_args!(SPBookmarkEventArgs, "SPBookmarkEventArgs");
