//! Port of `SPTrackAudioFile`: the `track_audio_files` junction table.

use std::rc::Rc;

use rusqlite::{params, Row};

use super::sp_audio_file::SPAudioFile;
use super::sp_db_connection::SPDBConnection;
use super::sp_model::SPModel;
use super::sp_track::SPTrack;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SPTrackAudioFile {
    pub track_id: i64,
    pub audio_file_id: i64,
    /// Populated by the caller.
    pub track: Option<Rc<SPTrack>>,
    /// Populated by the caller.
    pub audio_file: Option<Rc<SPAudioFile>>,
}

const COLUMNS: &str = "SELECT track_id, audio_file_id FROM track_audio_files";

impl SPTrackAudioFile {
    pub fn new(track_id: i64, audio_file_id: i64) -> Self {
        Self { track_id, audio_file_id, track: None, audio_file: None }
    }

    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self::new(row.get(0)?, row.get(1)?))
    }

    pub fn find_by_track(conn: &SPDBConnection, track_id: i64) -> Vec<Self> {
        conn.query(&format!("{COLUMNS} WHERE track_id = ?"), params![track_id], Self::from_row)
    }

    pub fn find_by_audio_file(conn: &SPDBConnection, audio_file_id: i64) -> Vec<Self> {
        conn.query(&format!("{COLUMNS} WHERE audio_file_id = ?"), params![audio_file_id], Self::from_row)
    }

    pub fn find_all(conn: &SPDBConnection) -> Vec<Self> {
        conn.query(COLUMNS, [], Self::from_row)
    }
}

impl SPModel for SPTrackAudioFile {
    fn save(&mut self, conn: &SPDBConnection) -> bool {
        conn.execute(
            "INSERT OR IGNORE INTO track_audio_files (track_id, audio_file_id) VALUES (?, ?)",
            params![self.track_id, self.audio_file_id],
        )
    }
    fn remove(&mut self, conn: &SPDBConnection) -> bool {
        conn.execute(
            "DELETE FROM track_audio_files WHERE track_id = ? AND audio_file_id = ?",
            params![self.track_id, self.audio_file_id],
        )
    }
    fn is_persisted(&self) -> bool {
        self.track_id > 0 && self.audio_file_id > 0
    }
}
