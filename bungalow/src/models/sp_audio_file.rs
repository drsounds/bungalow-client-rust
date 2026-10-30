//! Port of `SPAudioFile`: maps the `audio_files` table.

use std::rc::Rc;

use rusqlite::{params, Row};

use super::sp_db_connection::{opt_text, text, SPDBConnection};
use super::sp_file_row::SPFileRow;
use super::sp_model::SPModel;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SPAudioFile {
    pub id: i64,
    pub file_id: i64,
    pub name: String,
    pub version: String,
    pub artists: String,
    pub album: String,
    /// Milliseconds.
    pub duration: i64,
    pub cover_art: String,
    /// Populated by the caller.
    pub file: Option<Rc<SPFileRow>>,
}

const COLUMNS: &str = "SELECT id, file_id, name, version, artists, album, duration, cover_art FROM audio_files";

impl SPAudioFile {
    pub fn new(file_id: i64, name: &str, version: &str, artists: &str, album: &str, duration: i64, cover_art: &str) -> Self {
        Self {
            id: 0,
            file_id,
            name: name.into(),
            version: version.into(),
            artists: artists.into(),
            album: album.into(),
            duration,
            cover_art: cover_art.into(),
            file: None,
        }
    }

    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            file_id: row.get(1)?,
            name: text(row, 2),
            version: text(row, 3),
            artists: text(row, 4),
            album: text(row, 5),
            duration: row.get::<_, Option<i64>>(6)?.unwrap_or(0),
            cover_art: text(row, 7),
            file: None,
        })
    }

    pub fn find(conn: &SPDBConnection, id: i64) -> Option<Self> {
        conn.query_one(&format!("{COLUMNS} WHERE id = ?"), params![id], Self::from_row)
    }

    pub fn find_all(conn: &SPDBConnection) -> Vec<Self> {
        conn.query(&format!("{COLUMNS} ORDER BY name"), [], Self::from_row)
    }

    pub fn find_by_file(conn: &SPDBConnection, file_id: i64) -> Vec<Self> {
        conn.query(&format!("{COLUMNS} WHERE file_id = ?"), params![file_id], Self::from_row)
    }
}

impl SPModel for SPAudioFile {
    fn save(&mut self, conn: &SPDBConnection) -> bool {
        if !self.is_persisted() {
            let ok = conn.execute(
                "INSERT INTO audio_files (file_id, name, version, artists, album, duration, cover_art) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![
                    self.file_id,
                    opt_text(&self.name),
                    opt_text(&self.version),
                    opt_text(&self.artists),
                    opt_text(&self.album),
                    self.duration,
                    opt_text(&self.cover_art)
                ],
            );
            if ok {
                self.id = conn.last_insert_rowid();
            }
            ok
        } else {
            conn.execute(
                "UPDATE audio_files SET file_id=?, name=?, version=?, artists=?, \
                 album=?, duration=?, cover_art=? WHERE id=?",
                params![
                    self.file_id,
                    opt_text(&self.name),
                    opt_text(&self.version),
                    opt_text(&self.artists),
                    opt_text(&self.album),
                    self.duration,
                    opt_text(&self.cover_art),
                    self.id
                ],
            )
        }
    }
    fn remove(&mut self, conn: &SPDBConnection) -> bool {
        conn.execute("DELETE FROM audio_files WHERE id = ?", params![self.id])
    }
    fn is_persisted(&self) -> bool {
        self.id > 0
    }
}
