//! Port of `SPFileRow`: maps the `files` table (id PK, path UNIQUE).

use rusqlite::{params, Row};

use super::sp_db_connection::{text, SPDBConnection};
use super::sp_model::SPModel;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SPFileRow {
    pub id: i64,
    pub path: String,
}

impl SPFileRow {
    pub fn new(path: &str) -> Self {
        Self { id: 0, path: path.into() }
    }

    pub fn with_id(id: i64, path: &str) -> Self {
        Self { id, path: path.into() }
    }

    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self { id: row.get(0)?, path: text(row, 1) })
    }

    pub fn find(conn: &SPDBConnection, id: i64) -> Option<Self> {
        conn.query_one("SELECT id, path FROM files WHERE id = ?", params![id], Self::from_row)
    }

    pub fn find_by_path(conn: &SPDBConnection, path: &str) -> Option<Self> {
        conn.query_one("SELECT id, path FROM files WHERE path = ?", params![path], Self::from_row)
    }

    pub fn find_all(conn: &SPDBConnection) -> Vec<Self> {
        conn.query("SELECT id, path FROM files ORDER BY path", [], Self::from_row)
    }
}

impl SPModel for SPFileRow {
    fn save(&mut self, conn: &SPDBConnection) -> bool {
        if !self.is_persisted() {
            let changed = conn.handle().execute("INSERT OR IGNORE INTO files (path) VALUES (?)", params![self.path]);
            let Ok(changed) = changed else { return false };
            if changed > 0 {
                self.id = conn.last_insert_rowid();
            } else if let Some(existing) = Self::find_by_path(conn, &self.path) {
                self.id = existing.id;
            }
            true
        } else {
            conn.execute("UPDATE files SET path = ? WHERE id = ?", params![self.path, self.id])
        }
    }
    fn remove(&mut self, conn: &SPDBConnection) -> bool {
        conn.execute("DELETE FROM files WHERE id = ?", params![self.id])
    }
    fn is_persisted(&self) -> bool {
        self.id > 0
    }
}
