//! Port of `SPSave`: maps the `saves` table (soft-deletable references to a track).

use std::rc::Rc;

use rusqlite::{params, Row};

use super::sp_db_connection::{text, SPDBConnection};
use super::sp_model::SPModel;
use super::sp_track::SPTrack;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SPSave {
    pub id: i64,
    pub track_id: i64,
    pub saved_at: String,
    /// Empty string ↔ NULL in the database (not deleted).
    pub deleted_at: String,
    /// Populated by the caller.
    pub track: Option<Rc<SPTrack>>,
}

const COLUMNS: &str = "SELECT id, track_id, saved_at, deleted_at FROM saves";

impl SPSave {
    pub fn new(track_id: i64) -> Self {
        Self { track_id, ..Default::default() }
    }

    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            track_id: row.get(1)?,
            saved_at: text(row, 2),
            deleted_at: text(row, 3),
            track: None,
        })
    }

    pub fn find(conn: &SPDBConnection, id: i64) -> Option<Self> {
        conn.query_one(&format!("{COLUMNS} WHERE id = ?"), params![id], Self::from_row)
    }

    pub fn find_all(conn: &SPDBConnection) -> Vec<Self> {
        conn.query(&format!("{COLUMNS} ORDER BY saved_at DESC"), [], Self::from_row)
    }

    /// Saves WHERE deleted_at IS NULL.
    pub fn find_active(conn: &SPDBConnection) -> Vec<Self> {
        conn.query(&format!("{COLUMNS} WHERE deleted_at IS NULL ORDER BY saved_at DESC"), [], Self::from_row)
    }
}

impl SPModel for SPSave {
    fn save(&mut self, conn: &SPDBConnection) -> bool {
        if !self.is_persisted() {
            let ok = conn.execute(
                "INSERT INTO saves (track_id, saved_at, deleted_at) \
                 VALUES (?, COALESCE(NULLIF(?, ''), datetime('now')), NULLIF(?, ''))",
                params![self.track_id, self.saved_at, self.deleted_at],
            );
            if ok {
                self.id = conn.last_insert_rowid();
            }
            ok
        } else {
            conn.execute(
                "UPDATE saves SET track_id=?, deleted_at=NULLIF(?,'') WHERE id=?",
                params![self.track_id, self.deleted_at, self.id],
            )
        }
    }
    fn remove(&mut self, conn: &SPDBConnection) -> bool {
        conn.execute("DELETE FROM saves WHERE id = ?", params![self.id])
    }
    fn is_persisted(&self) -> bool {
        self.id > 0
    }
}
