//! Port of `SPPlaybackEventArgs.h`.

/// Carries current playback state: playing flag, position, and duration.
/// Dispatched on the application's playback observable whenever state changes.
#[derive(Debug, Clone, Copy)]
pub struct SPPlaybackStateEventArgs {
    pub playing: bool,
    /// Current playback position in milliseconds.
    pub position_ms: i32,
    /// Track duration in milliseconds (0 if unknown).
    pub duration_ms: i32,
}

impl SPPlaybackStateEventArgs {
    pub fn new(playing: bool, position_ms: i32, duration_ms: i32) -> Self {
        Self {
            playing,
            position_ms,
            duration_ms,
        }
    }
}
crate::impl_event_args!(SPPlaybackStateEventArgs, "SPPlaybackStateEventArgs");

/// Carries the seek target emitted by `SPSeekBarElement` on mouse-click.
#[derive(Debug, Clone, Copy)]
pub struct SPSeekEventArgs {
    position_ms: i32,
}

impl SPSeekEventArgs {
    pub fn new(position_ms: i32) -> Self {
        Self { position_ms }
    }
    pub fn get_position_ms(&self) -> i32 {
        self.position_ms
    }
}
crate::impl_event_args!(SPSeekEventArgs, "SPSeekEventArgs");

/// Carries a table row index, dispatched as "rowdblclicked" by `SPTableElement`.
#[derive(Debug, Clone, Copy)]
pub struct SPTableRowEventArgs {
    row: i32,
}

impl SPTableRowEventArgs {
    pub fn new(row: i32) -> Self {
        Self { row }
    }
    pub fn get_row(&self) -> i32 {
        self.row
    }
}
crate::impl_event_args!(SPTableRowEventArgs, "SPTableRowEventArgs");
