//! Port of `SPReleaseArtist`: the `release_artists` junction table.

use std::rc::Rc;

use rusqlite::{params, Row};

use super::sp_artist::SPArtist;
use super::sp_db_connection::{text, SPDBConnection};
use super::sp_model::SPModel;
use super::sp_release::SPRelease;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SPReleaseArtist {
    pub release_id: String,
    pub artist_id: String,
    pub number: i32,
    pub is_primary_artist: bool,
    /// Populated by the caller.
    pub release: Option<Rc<SPRelease>>,
    /// Populated by the caller.
    pub artist: Option<Rc<SPArtist>>,
}

const COLUMNS: &str = "SELECT release_id, artist_id, number, is_primary FROM release_artists";

impl SPReleaseArtist {
    pub fn new(release_id: &str, artist_id: &str, number: i32, is_primary: bool) -> Self {
        Self {
            release_id: release_id.into(),
            artist_id: artist_id.into(),
            number,
            is_primary_artist: is_primary,
            release: None,
            artist: None,
        }
    }

    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self::new(&text(row, 0), &text(row, 1), row.get(2)?, row.get::<_, i32>(3)? != 0))
    }

    pub fn find_by_release(conn: &SPDBConnection, release_id: &str) -> Vec<Self> {
        conn.query(&format!("{COLUMNS} WHERE release_id = ? ORDER BY number"), params![release_id], Self::from_row)
    }

    pub fn find_by_artist(conn: &SPDBConnection, artist_id: &str) -> Vec<Self> {
        conn.query(&format!("{COLUMNS} WHERE artist_id = ?"), params![artist_id], Self::from_row)
    }

    pub fn find_all(conn: &SPDBConnection) -> Vec<Self> {
        conn.query(COLUMNS, [], Self::from_row)
    }
}

impl SPModel for SPReleaseArtist {
    fn save(&mut self, conn: &SPDBConnection) -> bool {
        conn.execute(
            "INSERT OR REPLACE INTO release_artists (release_id, artist_id, number, is_primary) VALUES (?, ?, ?, ?)",
            params![self.release_id, self.artist_id, self.number, self.is_primary_artist as i32],
        )
    }
    fn remove(&mut self, conn: &SPDBConnection) -> bool {
        conn.execute(
            "DELETE FROM release_artists WHERE release_id = ? AND artist_id = ?",
            params![self.release_id, self.artist_id],
        )
    }
    fn is_persisted(&self) -> bool {
        !self.release_id.is_empty() && !self.artist_id.is_empty()
    }
}
