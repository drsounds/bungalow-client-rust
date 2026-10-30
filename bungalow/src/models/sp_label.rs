//! Port of `SPLabel`: maps the `labels` table (id PK autoincrement, name).

use rusqlite::{params, Row};

use super::sp_db_connection::{text, SPDBConnection};
use super::sp_model::SPModel;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SPLabel {
    pub id: i64,
    pub name: String,
}

impl SPLabel {
    pub fn new(name: &str) -> Self {
        Self { id: 0, name: name.into() }
    }

    pub fn with_id(id: i64, name: &str) -> Self {
        Self { id, name: name.into() }
    }

    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self { id: row.get(0)?, name: text(row, 1) })
    }

    pub fn find(conn: &SPDBConnection, id: i64) -> Option<Self> {
        conn.query_one("SELECT id, name FROM labels WHERE id = ?", params![id], Self::from_row)
    }

    pub fn find_all(conn: &SPDBConnection) -> Vec<Self> {
        conn.query("SELECT id, name FROM labels ORDER BY name", [], Self::from_row)
    }
}

impl SPModel for SPLabel {
    fn save(&mut self, conn: &SPDBConnection) -> bool {
        if !self.is_persisted() {
            let ok = conn.execute("INSERT INTO labels (name) VALUES (?)", params![self.name]);
            if ok {
                self.id = conn.last_insert_rowid();
            }
            ok
        } else {
            conn.execute("UPDATE labels SET name = ? WHERE id = ?", params![self.name, self.id])
        }
    }
    fn remove(&mut self, conn: &SPDBConnection) -> bool {
        conn.execute("DELETE FROM labels WHERE id = ?", params![self.id])
    }
    fn is_persisted(&self) -> bool {
        self.id > 0
    }
}
