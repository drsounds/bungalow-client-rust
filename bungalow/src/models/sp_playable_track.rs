//! Port of `SPPlayableTrack`: the `playable_tracks` junction table.

use std::rc::Rc;

use rusqlite::{params, Row};

use super::sp_db_connection::SPDBConnection;
use super::sp_model::SPModel;
use super::sp_playable::SPPlayable;
use super::sp_track::SPTrack;

#[derive(Clone, Default)]
pub struct SPPlayableTrack {
    pub playable_id: i64,
    pub track_id: i64,
    /// Populated by the caller.
    pub playable: Option<Rc<SPPlayable>>,
    /// Populated by the caller.
    pub track: Option<Rc<SPTrack>>,
}

const COLUMNS: &str = "SELECT playable_id, track_id FROM playable_tracks";

impl SPPlayableTrack {
    pub fn new(playable_id: i64, track_id: i64) -> Self {
        Self { playable_id, track_id, playable: None, track: None }
    }

    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self::new(row.get(0)?, row.get(1)?))
    }

    pub fn find_by_playable(conn: &SPDBConnection, playable_id: i64) -> Vec<Self> {
        conn.query(&format!("{COLUMNS} WHERE playable_id = ?"), params![playable_id], Self::from_row)
    }

    pub fn find_by_track(conn: &SPDBConnection, track_id: i64) -> Vec<Self> {
        conn.query(&format!("{COLUMNS} WHERE track_id = ?"), params![track_id], Self::from_row)
    }

    pub fn find_all(conn: &SPDBConnection) -> Vec<Self> {
        conn.query(COLUMNS, [], Self::from_row)
    }
}

impl SPModel for SPPlayableTrack {
    fn save(&mut self, conn: &SPDBConnection) -> bool {
        conn.execute(
            "INSERT OR IGNORE INTO playable_tracks (playable_id, track_id) VALUES (?, ?)",
            params![self.playable_id, self.track_id],
        )
    }
    fn remove(&mut self, conn: &SPDBConnection) -> bool {
        conn.execute(
            "DELETE FROM playable_tracks WHERE playable_id = ? AND track_id = ?",
            params![self.playable_id, self.track_id],
        )
    }
    fn is_persisted(&self) -> bool {
        self.playable_id > 0 && self.track_id > 0
    }
}
