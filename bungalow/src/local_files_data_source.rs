//! Port of `LocalFilesDataSource`: the audio_files table joined to files for
//! the path. Columns: Name | Version | Artists | Album | Duration.

use std::cell::RefCell;

use rusqlite::Connection;
use space::sp_table_data_source::SPTableDataSource;

use crate::play_queue_entry::PlayQueueEntry;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalFileRow {
    pub name: String,
    pub version: String,
    pub artists: String,
    pub album: String,
    /// Formatted "m:ss" or empty.
    pub duration: String,
    pub path: String,
}

const COLUMN_NAMES: [&str; 5] = ["Name", "Version", "Artists", "Album", "Duration"];

const QUERY: &str = "
    SELECT
        af.name,
        COALESCE(af.version, ''),
        COALESCE(af.artists, ''),
        COALESCE(af.album,   ''),
        af.duration,
        f.path
    FROM audio_files af
    JOIN files f ON f.id = af.file_id
    ORDER BY af.artists, af.name
";

pub fn format_duration(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let total = ms / 1000;
    format!("{}:{:02}", total / 60, total % 60)
}

#[derive(Default)]
pub struct LocalFilesDataSource {
    rows: RefCell<Vec<LocalFileRow>>,
}

impl LocalFilesDataSource {
    pub fn new(db: Option<&Connection>) -> Self {
        let this = Self::default();
        if let Some(db) = db {
            this.load(db);
        }
        this
    }

    pub fn reload(&self, db: Option<&Connection>) {
        self.rows.borrow_mut().clear();
        if let Some(db) = db {
            self.load(db);
        }
    }

    fn load(&self, db: &Connection) {
        let Ok(mut stmt) = db.prepare(QUERY) else { return };
        let text = |r: &rusqlite::Row, i| r.get::<_, Option<String>>(i).map(Option::unwrap_or_default);
        let rows = stmt.query_map([], |r| {
            Ok(LocalFileRow {
                name: text(r, 0)?,
                version: text(r, 1)?,
                artists: text(r, 2)?,
                album: text(r, 3)?,
                duration: format_duration(r.get::<_, Option<i64>>(4)?.unwrap_or(0)),
                path: text(r, 5)?,
            })
        });
        if let Ok(rows) = rows {
            self.rows.borrow_mut().extend(rows.flatten());
        }
    }

    /// The file path for a row, or "" if the row is invalid.
    pub fn get_file_path_for_row(&self, row: i32) -> String {
        usize::try_from(row)
            .ok()
            .and_then(|row| self.rows.borrow().get(row).map(|r| r.path.clone()))
            .unwrap_or_default()
    }

    /// A PlayQueueEntry for every row using file:// URIs.
    pub fn build_queue_entries(&self) -> Vec<PlayQueueEntry> {
        self.rows
            .borrow()
            .iter()
            .map(|row| PlayQueueEntry {
                display_title: row.name.clone(),
                display_artist: row.artists.clone(),
                uri: format!("file://{}", row.path),
                playable: !row.path.is_empty(),
                ..Default::default()
            })
            .collect()
    }
}

impl SPTableDataSource for LocalFilesDataSource {
    fn get_column_count(&self) -> i32 {
        5
    }
    fn get_row_count(&self) -> i32 {
        self.rows.borrow().len() as i32
    }
    fn get_text_for_column(&self, column: i32) -> String {
        usize::try_from(column).ok().and_then(|c| COLUMN_NAMES.get(c)).copied().unwrap_or_default().into()
    }
    fn get_text_for_cell(&self, column: i32, row: i32) -> String {
        let Some(r) = usize::try_from(row).ok().and_then(|row| self.rows.borrow().get(row).cloned()) else {
            return String::new();
        };
        match column {
            0 => r.name,
            1 => r.version,
            2 => r.artists,
            3 => r.album,
            4 => r.duration,
            _ => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_durations() {
        assert_eq!(format_duration(0), "");
        assert_eq!(format_duration(61_000), "1:01");
    }
}
