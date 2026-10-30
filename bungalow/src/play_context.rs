//! Port of `PlayContext`: passed by a view to `BungalowApp::play_context()`
//! to start a full play action. Holds every entry (in display order) and the
//! one to start at.

use crate::play_queue_entry::PlayQueueEntry;

#[derive(Debug, Clone, Default)]
pub struct PlayContext {
    pub entries: Vec<PlayQueueEntry>,
    pub start_index: usize,
}
