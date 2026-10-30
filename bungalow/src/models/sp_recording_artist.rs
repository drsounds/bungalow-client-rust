//! Port of `SPRecordingArtist`: the `recording_artists` junction table.

use std::rc::Rc;

use rusqlite::{params, Row};

use super::sp_artist::SPArtist;
use super::sp_db_connection::{text, SPDBConnection};
use super::sp_model::SPModel;
use super::sp_recording::SPRecording;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SPRecordingArtist {
    pub recording_id: String,
    pub artist_id: String,
    pub is_primary_artist: bool,
    pub number: i32,
    /// Populated by the caller.
    pub recording: Option<Rc<SPRecording>>,
    /// Populated by the caller.
    pub artist: Option<Rc<SPArtist>>,
}

const COLUMNS: &str = "SELECT recording_id, artist_id, is_primary, number FROM recording_artists";

impl SPRecordingArtist {
    pub fn new(recording_id: &str, artist_id: &str, is_primary: bool, number: i32) -> Self {
        Self {
            recording_id: recording_id.into(),
            artist_id: artist_id.into(),
            is_primary_artist: is_primary,
            number,
            recording: None,
            artist: None,
        }
    }

    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self::new(&text(row, 0), &text(row, 1), row.get::<_, i32>(2)? != 0, row.get(3)?))
    }

    pub fn find_by_recording(conn: &SPDBConnection, recording_id: &str) -> Vec<Self> {
        conn.query(&format!("{COLUMNS} WHERE recording_id = ? ORDER BY number"), params![recording_id], Self::from_row)
    }

    pub fn find_by_artist(conn: &SPDBConnection, artist_id: &str) -> Vec<Self> {
        conn.query(&format!("{COLUMNS} WHERE artist_id = ?"), params![artist_id], Self::from_row)
    }

    pub fn find_all(conn: &SPDBConnection) -> Vec<Self> {
        conn.query(COLUMNS, [], Self::from_row)
    }
}

impl SPModel for SPRecordingArtist {
    fn save(&mut self, conn: &SPDBConnection) -> bool {
        conn.execute(
            "INSERT OR REPLACE INTO recording_artists (recording_id, artist_id, is_primary, number) VALUES (?, ?, ?, ?)",
            params![self.recording_id, self.artist_id, self.is_primary_artist as i32, self.number],
        )
    }
    fn remove(&mut self, conn: &SPDBConnection) -> bool {
        conn.execute(
            "DELETE FROM recording_artists WHERE recording_id = ? AND artist_id = ?",
            params![self.recording_id, self.artist_id],
        )
    }
    fn is_persisted(&self) -> bool {
        !self.recording_id.is_empty() && !self.artist_id.is_empty()
    }
}
