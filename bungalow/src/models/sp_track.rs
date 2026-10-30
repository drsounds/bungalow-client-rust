//! Port of `SPTrack`: maps the `tracks` table, binding a recording to a release.

use std::rc::Rc;

use rusqlite::{params, Row};

use super::sp_db_connection::{text, SPDBConnection};
use super::sp_model::SPModel;
use super::sp_recording::SPRecording;
use super::sp_release::SPRelease;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SPTrack {
    pub id: i64,
    pub recording_id: String,
    pub release_id: String,
    /// Populated by the caller.
    pub recording: Option<Rc<SPRecording>>,
    /// Populated by the caller.
    pub release: Option<Rc<SPRelease>>,
}

const COLUMNS: &str = "SELECT id, recording_id, release_id FROM tracks";

impl SPTrack {
    pub fn new(recording_id: &str, release_id: &str) -> Self {
        Self::with_id(0, recording_id, release_id)
    }

    pub fn with_id(id: i64, recording_id: &str, release_id: &str) -> Self {
        Self { id, recording_id: recording_id.into(), release_id: release_id.into(), recording: None, release: None }
    }

    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self::with_id(row.get(0)?, &text(row, 1), &text(row, 2)))
    }

    pub fn find(conn: &SPDBConnection, id: i64) -> Option<Self> {
        conn.query_one(&format!("{COLUMNS} WHERE id = ?"), params![id], Self::from_row)
    }

    pub fn find_all(conn: &SPDBConnection) -> Vec<Self> {
        conn.query(COLUMNS, [], Self::from_row)
    }

    pub fn find_by_recording(conn: &SPDBConnection, recording_id: &str) -> Vec<Self> {
        conn.query(&format!("{COLUMNS} WHERE recording_id = ?"), params![recording_id], Self::from_row)
    }

    pub fn find_by_release(conn: &SPDBConnection, release_id: &str) -> Vec<Self> {
        conn.query(&format!("{COLUMNS} WHERE release_id = ?"), params![release_id], Self::from_row)
    }
}

impl SPModel for SPTrack {
    fn save(&mut self, conn: &SPDBConnection) -> bool {
        if !self.is_persisted() {
            let ok = conn.execute(
                "INSERT INTO tracks (recording_id, release_id) VALUES (?, ?)",
                params![self.recording_id, self.release_id],
            );
            if ok {
                self.id = conn.last_insert_rowid();
            }
            ok
        } else {
            conn.execute(
                "UPDATE tracks SET recording_id = ?, release_id = ? WHERE id = ?",
                params![self.recording_id, self.release_id, self.id],
            )
        }
    }
    fn remove(&mut self, conn: &SPDBConnection) -> bool {
        conn.execute("DELETE FROM tracks WHERE id = ?", params![self.id])
    }
    fn is_persisted(&self) -> bool {
        self.id > 0
    }
}
