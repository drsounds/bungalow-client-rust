//! Port of `SPPlayable`: maps the `playables` table (id PK, uri UNIQUE).

use std::rc::Rc;

use rusqlite::{params, Row};

use super::sp_db_connection::{text, SPDBConnection};
use super::sp_model::SPModel;
use crate::services::sp_media_service::SPMediaService;

#[derive(Clone, Default)]
pub struct SPPlayable {
    pub id: i64,
    pub uri: String,
    /// The media service responsible for playing this URI, set by
    /// [`SPPlayable::resolve_service`].
    pub service: Option<Rc<dyn SPMediaService>>,
}

impl SPPlayable {
    pub fn new(uri: &str) -> Self {
        Self::with_id(0, uri)
    }

    pub fn with_id(id: i64, uri: &str) -> Self {
        Self { id, uri: uri.into(), service: None }
    }

    /// Sets `service` to the first entry of `services` that accepts the URI.
    pub fn resolve_service(&mut self, services: &[Rc<dyn SPMediaService>]) {
        self.service = services.iter().find(|s| s.accepts_uri(&self.uri)).cloned();
    }

    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self::with_id(row.get(0)?, &text(row, 1)))
    }

    pub fn find(conn: &SPDBConnection, id: i64) -> Option<Self> {
        conn.query_one("SELECT id, uri FROM playables WHERE id = ?", params![id], Self::from_row)
    }

    pub fn find_by_uri(conn: &SPDBConnection, uri: &str) -> Option<Self> {
        conn.query_one("SELECT id, uri FROM playables WHERE uri = ?", params![uri], Self::from_row)
    }

    pub fn find_all(conn: &SPDBConnection) -> Vec<Self> {
        conn.query("SELECT id, uri FROM playables ORDER BY id", [], Self::from_row)
    }
}

impl SPModel for SPPlayable {
    fn save(&mut self, conn: &SPDBConnection) -> bool {
        if !self.is_persisted() {
            let changed = conn.handle().execute("INSERT OR IGNORE INTO playables (uri) VALUES (?)", params![self.uri]);
            let Ok(changed) = changed else { return false };
            if changed > 0 {
                self.id = conn.last_insert_rowid();
            } else if let Some(existing) = Self::find_by_uri(conn, &self.uri) {
                self.id = existing.id;
            }
            true
        } else {
            conn.execute("UPDATE playables SET uri = ? WHERE id = ?", params![self.uri, self.id])
        }
    }
    fn remove(&mut self, conn: &SPDBConnection) -> bool {
        conn.execute("DELETE FROM playables WHERE id = ?", params![self.id])
    }
    fn is_persisted(&self) -> bool {
        self.id > 0
    }
}
