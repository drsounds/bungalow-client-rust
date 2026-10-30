//! Port of `LibraryDataSource`: the saved tracks (non-deleted rows of
//! `saves`) joined through tracks → recordings → releases and
//! recording_artists → artists. Columns: Name | Artists | Release.

use std::cell::RefCell;
use std::collections::HashMap;

use rusqlite::Connection;
use space::sp_table_data_source::SPTableDataSource;

use crate::play_queue_entry::PlayQueueEntry;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LibraryRow {
    /// Recording name.
    pub name: String,
    /// Comma-joined recording artist names.
    pub artists: String,
    pub release_name: String,
    pub track_id: i32,
    /// True if at least one playable_tracks entry exists.
    pub playable: bool,
}

const COLUMN_NAMES: [&str; 3] = ["Name", "Artists", "Release"];

// GROUP_CONCAT aggregates artist names per save row.
const QUERY: &str = "
    SELECT
        rec.name                                     AS name,
        COALESCE(GROUP_CONCAT(DISTINCT a.name), '')  AS artists,
        COALESCE(rel.name, '')                       AS release_name,
        t.id                                         AS track_id,
        CASE WHEN EXISTS (
            SELECT 1 FROM playable_tracks pt WHERE pt.track_id = t.id
        ) THEN 1 ELSE 0 END                          AS is_playable
    FROM saves s
    JOIN tracks             t   ON t.id            = s.track_id
    JOIN recordings         rec ON rec.isrc         = t.recording_id
    LEFT JOIN releases      rel ON rel.upc          = t.release_id
    LEFT JOIN recording_artists ra ON ra.recording_id = rec.isrc
    LEFT JOIN artists           a  ON a.isni          = ra.artist_id
    WHERE s.deleted_at IS NULL
    GROUP BY s.id
    ORDER BY s.saved_at DESC
";

#[derive(Default)]
pub struct LibraryDataSource {
    rows: RefCell<Vec<LibraryRow>>,
}

impl LibraryDataSource {
    pub fn new(db: Option<&Connection>) -> Self {
        let this = Self::default();
        if let Some(db) = db {
            this.load(db);
        }
        this
    }

    /// Reloads rows from the database (call after saves change).
    pub fn reload(&self, db: Option<&Connection>) {
        self.rows.borrow_mut().clear();
        if let Some(db) = db {
            self.load(db);
        }
    }

    fn load(&self, db: &Connection) {
        let Ok(mut stmt) = db.prepare(QUERY) else { return };
        let rows = stmt.query_map([], |r| {
            Ok(LibraryRow {
                name: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                artists: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                release_name: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                track_id: r.get(3)?,
                playable: r.get::<_, i32>(4)? != 0,
            })
        });
        if let Ok(rows) = rows {
            self.rows.borrow_mut().extend(rows.flatten());
        }
    }

    fn row(&self, row: i32) -> Option<LibraryRow> {
        usize::try_from(row).ok().and_then(|row| self.rows.borrow().get(row).cloned())
    }

    /// Builds a PlayQueueEntry for every row, resolving all track → URI
    /// mappings with a single query. Rows without a playable_tracks entry
    /// get playable = false and an empty uri.
    pub fn build_queue_entries(&self, db: Option<&Connection>) -> Vec<PlayQueueEntry> {
        let mut uri_map: HashMap<i32, String> = HashMap::new();
        if let Some(db) = db {
            if let Ok(mut stmt) = db.prepare(
                "SELECT pt.track_id, p.uri
                 FROM playable_tracks pt
                 JOIN playables p ON p.id = pt.playable_id
                 GROUP BY pt.track_id",
            ) {
                if let Ok(rows) = stmt.query_map([], |r| Ok((r.get::<_, i32>(0)?, r.get::<_, Option<String>>(1)?))) {
                    for (tid, uri) in rows.flatten() {
                        if let Some(uri) = uri {
                            uri_map.insert(tid, uri);
                        }
                    }
                }
            }
        }
        self.rows
            .borrow()
            .iter()
            .map(|row| {
                let mut e = PlayQueueEntry {
                    display_title: row.name.clone(),
                    display_artist: row.artists.clone(),
                    playable: row.playable,
                    ..Default::default()
                };
                if row.playable {
                    match uri_map.get(&row.track_id) {
                        Some(uri) => e.uri = uri.clone(),
                        // The flag said playable but no URI was found.
                        None => e.playable = false,
                    }
                }
                e
            })
            .collect()
    }
}

impl SPTableDataSource for LibraryDataSource {
    fn get_column_count(&self) -> i32 {
        3
    }
    fn get_row_count(&self) -> i32 {
        self.rows.borrow().len() as i32
    }
    fn get_text_for_column(&self, column: i32) -> String {
        usize::try_from(column).ok().and_then(|c| COLUMN_NAMES.get(c)).copied().unwrap_or_default().into()
    }
    fn get_row_uri(&self, index: i32) -> String {
        self.row(index).map(|r| format!("bungalow:track:{}", r.track_id)).unwrap_or_default()
    }
    fn get_text_for_cell(&self, column: i32, row: i32) -> String {
        let Some(r) = self.row(row) else { return String::new() };
        match column {
            0 => r.name,
            1 => r.artists,
            2 => r.release_name,
            _ => String::new(),
        }
    }
    fn get_track_id_for_row(&self, row: i32) -> i32 {
        self.row(row).map(|r| r.track_id).unwrap_or(0)
    }
    fn is_row_playable(&self, row: i32) -> bool {
        self.row(row).map(|r| r.playable).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;

    #[test]
    fn lists_saved_tracks() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_artist("1", "Queen");
        db.upsert_recording("GB1", "Bohemian Rhapsody", "");
        db.set_recording_artists(
            "GB1",
            &[crate::database::ArtistRelationship {
                artist_isni: "1".into(),
                artist_name: String::new(),
                is_primary: true,
                number: 0,
            }],
        );
        let track = db.upsert_track("GB1", "");
        db.ensure_save(track);
        let source = LibraryDataSource::new(Some(db.handle()));
        assert_eq!(source.get_row_count(), 1);
        assert_eq!(source.get_text_for_cell(1, 0), "Queen");
        assert_eq!(source.get_row_uri(0), format!("bungalow:track:{track}"));
        let entries = source.build_queue_entries(Some(db.handle()));
        assert!(!entries[0].playable);
    }
}
