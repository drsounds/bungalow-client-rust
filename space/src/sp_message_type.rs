//! Port of `SPMessageType`.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SPMessageType {
    #[default]
    Info,
    Warning,
    Error,
    Message,
}
