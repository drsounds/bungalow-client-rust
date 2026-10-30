//! Port of `SPInputEventArgs`.

#[derive(Debug, Clone)]
pub struct SPInputEventArgs {
    text: String,
}

impl SPInputEventArgs {
    pub fn new(text: &str) -> Self {
        Self { text: text.to_string() }
    }
    pub fn get_text(&self) -> String {
        self.text.clone()
    }
}
crate::impl_event_args!(SPInputEventArgs);
