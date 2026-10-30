//! Port of `PlayQueueEntry`: display snapshot of one play-queue slot,
//! decoupled from SPPlayable. Built by data sources when a queue is built.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayQueueEntry {
    /// Track name (e.g. "Bohemian Rhapsody").
    pub display_title: String,
    /// Primary artist name (e.g. "Queen").
    pub display_artist: String,
    /// Optional cover art (empty for bulk-built queues).
    pub cover_art_path: String,
    /// Resolved playback URI (empty when !playable).
    pub uri: String,
    /// false → greyed out, skipped by next/prev.
    pub playable: bool,
}

impl Default for PlayQueueEntry {
    fn default() -> Self {
        Self {
            display_title: String::new(),
            display_artist: String::new(),
            cover_art_path: String::new(),
            uri: String::new(),
            playable: true,
        }
    }
}
