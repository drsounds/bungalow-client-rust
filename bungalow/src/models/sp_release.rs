//! Port of `SPRelease`: maps the `releases` table.

use std::rc::Rc;

use rusqlite::{params, Row};

use super::sp_db_connection::{opt_text, text, SPDBConnection};
use super::sp_label::SPLabel;
use super::sp_model::SPModel;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SPRelease {
    pub upc: String,
    pub name: String,
    /// Empty ↔ NULL.
    pub release_date: String,
    /// 0 ↔ NULL.
    pub label_id: i64,
    /// Empty ↔ NULL.
    pub cover_art: String,
    /// Populated by the caller.
    pub label: Option<Rc<SPLabel>>,
}

const COLUMNS: &str = "SELECT upc, name, release_date, label_id, cover_art FROM releases";

impl SPRelease {
    pub fn new(upc: &str, name: &str, release_date: &str, label_id: i64, cover_art: &str) -> Self {
        Self {
            upc: upc.into(),
            name: name.into(),
            release_date: release_date.into(),
            label_id,
            cover_art: cover_art.into(),
            label: None,
        }
    }

    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self::new(
            &text(row, 0),
            &text(row, 1),
            &text(row, 2),
            row.get::<_, Option<i64>>(3)?.unwrap_or(0),
            &text(row, 4),
        ))
    }

    pub fn find(conn: &SPDBConnection, upc: &str) -> Option<Self> {
        conn.query_one(&format!("{COLUMNS} WHERE upc = ?"), params![upc], Self::from_row)
    }

    pub fn find_all(conn: &SPDBConnection) -> Vec<Self> {
        conn.query(&format!("{COLUMNS} ORDER BY name"), [], Self::from_row)
    }
}

impl SPModel for SPRelease {
    fn save(&mut self, conn: &SPDBConnection) -> bool {
        conn.execute(
            "INSERT OR REPLACE INTO releases (upc, name, release_date, label_id, cover_art) VALUES (?, ?, ?, ?, ?)",
            params![
                self.upc,
                self.name,
                opt_text(&self.release_date),
                (self.label_id != 0).then_some(self.label_id),
                opt_text(&self.cover_art)
            ],
        )
    }
    fn remove(&mut self, conn: &SPDBConnection) -> bool {
        conn.execute("DELETE FROM releases WHERE upc = ?", params![self.upc])
    }
    fn is_persisted(&self) -> bool {
        !self.upc.is_empty()
    }
}
