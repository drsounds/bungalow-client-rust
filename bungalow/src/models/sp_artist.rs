//! Port of `SPArtist`: maps the `artists` table (isni TEXT PK, name TEXT NOT NULL).

use rusqlite::{params, Row};

use super::sp_db_connection::{text, SPDBConnection};
use super::sp_model::SPModel;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SPArtist {
    pub isni: String,
    pub name: String,
}

impl SPArtist {
    pub fn new(isni: &str, name: &str) -> Self {
        Self { isni: isni.into(), name: name.into() }
    }

    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self { isni: text(row, 0), name: text(row, 1) })
    }

    pub fn find(conn: &SPDBConnection, isni: &str) -> Option<Self> {
        conn.query_one("SELECT isni, name FROM artists WHERE isni = ?", params![isni], Self::from_row)
    }

    pub fn find_all(conn: &SPDBConnection) -> Vec<Self> {
        conn.query("SELECT isni, name FROM artists ORDER BY name", [], Self::from_row)
    }
}

impl SPModel for SPArtist {
    fn save(&mut self, conn: &SPDBConnection) -> bool {
        conn.execute("INSERT OR REPLACE INTO artists (isni, name) VALUES (?, ?)", params![self.isni, self.name])
    }
    fn remove(&mut self, conn: &SPDBConnection) -> bool {
        conn.execute("DELETE FROM artists WHERE isni = ?", params![self.isni])
    }
    fn is_persisted(&self) -> bool {
        !self.isni.is_empty()
    }
}
