//! Port of `SPRecording`: maps the `recordings` table (isrc PK, name, version).

use rusqlite::{params, Row};

use super::sp_db_connection::{opt_text, text, SPDBConnection};
use super::sp_model::SPModel;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SPRecording {
    pub isrc: String,
    pub name: String,
    /// Empty string ↔ NULL in the database.
    pub version: String,
}

impl SPRecording {
    pub fn new(isrc: &str, name: &str, version: &str) -> Self {
        Self { isrc: isrc.into(), name: name.into(), version: version.into() }
    }

    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self { isrc: text(row, 0), name: text(row, 1), version: text(row, 2) })
    }

    pub fn find(conn: &SPDBConnection, isrc: &str) -> Option<Self> {
        conn.query_one("SELECT isrc, name, version FROM recordings WHERE isrc = ?", params![isrc], Self::from_row)
    }

    pub fn find_all(conn: &SPDBConnection) -> Vec<Self> {
        conn.query("SELECT isrc, name, version FROM recordings ORDER BY name", [], Self::from_row)
    }
}

impl SPModel for SPRecording {
    fn save(&mut self, conn: &SPDBConnection) -> bool {
        conn.execute(
            "INSERT OR REPLACE INTO recordings (isrc, name, version) VALUES (?, ?, ?)",
            params![self.isrc, self.name, opt_text(&self.version)],
        )
    }
    fn remove(&mut self, conn: &SPDBConnection) -> bool {
        conn.execute("DELETE FROM recordings WHERE isrc = ?", params![self.isrc])
    }
    fn is_persisted(&self) -> bool {
        !self.isrc.is_empty()
    }
}
