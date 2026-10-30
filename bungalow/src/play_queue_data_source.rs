//! Port of `PlayQueueDataSource`: the play queue as a two column table
//! (Track | Artist). Unplayable entries are reported by `is_row_playable`
//! and the playing row by `is_row_active`.

use std::cell::{Cell, RefCell};

use space::sp_table_data_source::SPTableDataSource;

use crate::play_queue_entry::PlayQueueEntry;

pub struct PlayQueueDataSource {
    entries: RefCell<Vec<PlayQueueEntry>>,
    active_index: Cell<i32>,
}

impl Default for PlayQueueDataSource {
    fn default() -> Self {
        Self { entries: RefCell::new(Vec::new()), active_index: Cell::new(-1) }
    }
}

impl PlayQueueDataSource {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the displayed entries and sets the active row (-1 = none).
    pub fn set_entries(&self, entries: &[PlayQueueEntry], active_index: i32) {
        *self.entries.borrow_mut() = entries.to_vec();
        self.active_index.set(active_index);
    }

    fn entry(&self, row: i32) -> Option<PlayQueueEntry> {
        usize::try_from(row).ok().and_then(|row| self.entries.borrow().get(row).cloned())
    }
}

impl SPTableDataSource for PlayQueueDataSource {
    fn get_column_count(&self) -> i32 {
        2
    }
    fn get_row_count(&self) -> i32 {
        self.entries.borrow().len() as i32
    }
    fn get_text_for_column(&self, column: i32) -> String {
        if column == 0 { "Track" } else { "Artist" }.into()
    }
    fn get_text_for_cell(&self, column: i32, row: i32) -> String {
        self.entry(row).map(|e| if column == 0 { e.display_title } else { e.display_artist }).unwrap_or_default()
    }
    fn is_row_playable(&self, row: i32) -> bool {
        self.entry(row).map(|e| e.playable).unwrap_or(false)
    }
    fn is_row_active(&self, index: i32) -> bool {
        index == self.active_index.get()
    }
}
