//! Port of `Database`: owns the SQLite connection and creates the full
//! application schema on first open.
//!
//! Schema overview
//! ───────────────
//!  recordings      – ISRC (PK), name, version
//!  artists         – ISNI (PK), name
//!  recording_artists – junction: recording_id → recordings, artist_id → artists,
//!                      is_primary, number
//!  labels          – id (PK autoincrement), name
//!  releases        – upc (PK), name, release_date, label_id → labels, cover_art
//!  tracks          – id (PK autoincrement), recording_id → recordings,
//!                    release_id → releases (nullable)
//!  release_artists – junction: release_id → releases, artist_id → artists,
//!                    number, is_primary
//!  saves           – id (PK autoincrement), track_id → tracks,
//!                    saved_at, deleted_at
//!  bookmarks       – id (PK autoincrement), uri (UNIQUE), title,
//!                    parent_id → bookmarks, created_at

use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    pub id: i32,
    pub uri: String,
    pub title: String,
    /// -1 means top-level.
    pub parent_id: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArtistRecord {
    pub isni: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecordingRecord {
    pub isrc: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReleaseRecord {
    pub upc: String,
    pub name: String,
    pub release_date: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArtistRelationship {
    pub artist_isni: String,
    pub artist_name: String,
    pub is_primary: bool,
    pub number: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackDetails {
    pub id: i32,
    pub recording: RecordingRecord,
    /// Empty upc = no release.
    pub release: ReleaseRecord,
    pub recording_artists: Vec<ArtistRelationship>,
    pub release_artists: Vec<ArtistRelationship>,
}

pub struct Database {
    db: Connection,
}

/// Binds optional text: empty string → SQL NULL.
fn opt_text(v: &str) -> Option<&str> {
    (!v.is_empty()).then_some(v)
}

impl Database {
    /// Opens (or creates) the SQLite database at `path`.
    pub fn new(path: &str) -> Result<Self, String> {
        let db = Connection::open(path).map_err(|e| format!("Failed to open database '{path}': {e}"))?;
        let this = Self { db };
        this.exec("PRAGMA foreign_keys = ON;")?;
        this.initialize()?;
        Ok(this)
    }

    /// An in-memory database with the full schema, for tests.
    pub fn open_in_memory() -> Result<Self, String> {
        Self::new(":memory:")
    }

    /// The raw connection, the counterpart of `handle()`.
    pub fn handle(&self) -> &Connection {
        &self.db
    }

    fn exec(&self, sql: &str) -> Result<(), String> {
        self.db.execute_batch(sql).map_err(|e| format!("SQLite error: {e}"))
    }

    fn initialize(&self) -> Result<(), String> {
        // Artists — must exist before recordings reference them
        self.exec(
            "CREATE TABLE IF NOT EXISTS artists (
                isni TEXT PRIMARY KEY,
                name TEXT NOT NULL
            );",
        )?;
        self.exec(
            "CREATE TABLE IF NOT EXISTS recordings (
                isrc    TEXT PRIMARY KEY,
                name    TEXT NOT NULL,
                version TEXT
            );",
        )?;
        self.exec(
            "CREATE TABLE IF NOT EXISTS recording_artists (
                recording_id TEXT    NOT NULL REFERENCES recordings(isrc) ON DELETE CASCADE,
                artist_id    TEXT    NOT NULL REFERENCES artists(isni)    ON DELETE CASCADE,
                is_primary   INTEGER NOT NULL DEFAULT 0,
                number       INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (recording_id, artist_id)
            );",
        )?;
        self.exec(
            "CREATE TABLE IF NOT EXISTS labels (
                id   INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT    NOT NULL
            );",
        )?;
        self.exec(
            "CREATE TABLE IF NOT EXISTS releases (
                upc          TEXT    PRIMARY KEY,
                name         TEXT    NOT NULL,
                release_date TEXT,
                label_id     INTEGER REFERENCES labels(id) ON DELETE SET NULL,
                cover_art    TEXT
            );",
        )?;
        // Tracks — bind a recording to a release
        self.exec(
            "CREATE TABLE IF NOT EXISTS tracks (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                recording_id TEXT    NOT NULL REFERENCES recordings(isrc)  ON DELETE CASCADE,
                release_id   TEXT    NOT NULL REFERENCES releases(upc)     ON DELETE CASCADE
            );",
        )?;
        self.exec(
            "CREATE TABLE IF NOT EXISTS release_artists (
                release_id TEXT    NOT NULL REFERENCES releases(upc)  ON DELETE CASCADE,
                artist_id  TEXT    NOT NULL REFERENCES artists(isni)  ON DELETE CASCADE,
                number     INTEGER NOT NULL DEFAULT 0,
                is_primary INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (release_id, artist_id)
            );",
        )?;
        // Saves — soft-deletable references to a track
        self.exec(
            "CREATE TABLE IF NOT EXISTS saves (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                track_id   INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                saved_at   TEXT    NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT
            );",
        )?;
        // Files — paths to local or remote media files
        self.exec(
            "CREATE TABLE IF NOT EXISTS files (
                id   INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT    NOT NULL UNIQUE
            );",
        )?;
        // Audio files — embedded/resolved metadata for a local file
        self.exec(
            "CREATE TABLE IF NOT EXISTS audio_files (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                file_id   INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
                name      TEXT,
                version   TEXT,
                artists   TEXT,
                album     TEXT,
                duration  INTEGER,
                cover_art TEXT
            );",
        )?;
        // Migration: add columns introduced after initial schema (ignored if they exist)
        let _ = self.db.execute_batch("ALTER TABLE audio_files ADD COLUMN version TEXT");
        let _ = self.db.execute_batch("ALTER TABLE audio_files ADD COLUMN cover_art TEXT");
        self.exec(
            "CREATE TABLE IF NOT EXISTS track_audio_files (
                track_id      INTEGER NOT NULL REFERENCES tracks(id)      ON DELETE CASCADE,
                audio_file_id INTEGER NOT NULL REFERENCES audio_files(id) ON DELETE CASCADE,
                PRIMARY KEY (track_id, audio_file_id)
            );",
        )?;
        // Playables — a URI that can be played directly
        self.exec(
            "CREATE TABLE IF NOT EXISTS playables (
                id  INTEGER PRIMARY KEY AUTOINCREMENT,
                uri TEXT    NOT NULL UNIQUE
            );",
        )?;
        self.exec(
            "CREATE TABLE IF NOT EXISTS playable_tracks (
                playable_id INTEGER NOT NULL REFERENCES playables(id) ON DELETE CASCADE,
                track_id    INTEGER NOT NULL REFERENCES tracks(id)    ON DELETE CASCADE,
                PRIMARY KEY (playable_id, track_id)
            );",
        )?;
        // Downloads — tracks file downloads from the web browser
        self.exec(
            "CREATE TABLE IF NOT EXISTS downloads (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                url          TEXT    NOT NULL,
                filename     TEXT    NOT NULL,
                file_path    TEXT,
                content_type TEXT,
                status       TEXT    NOT NULL DEFAULT 'ongoing',
                total_bytes  INTEGER NOT NULL DEFAULT 0,
                done_bytes   INTEGER NOT NULL DEFAULT 0,
                created_at   TEXT    NOT NULL DEFAULT (datetime('now')),
                completed_at TEXT
            );",
        )?;
        // Bookmarks — user-saved URIs displayed in the left sidebar
        self.exec(
            "CREATE TABLE IF NOT EXISTS bookmarks (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                uri        TEXT    NOT NULL UNIQUE,
                title      TEXT    NOT NULL,
                parent_id  INTEGER REFERENCES bookmarks(id) ON DELETE CASCADE,
                created_at TEXT    NOT NULL DEFAULT (datetime('now'))
            );",
        )?;
        // Accepted third-party domains — hostnames the user has acknowledged the warning for
        self.exec(
            "CREATE TABLE IF NOT EXISTS accepted_domains (
                hostname TEXT NOT NULL UNIQUE
            );",
        )?;

        // Migration: make tracks.release_id nullable (allows tracks without a release).
        let needs_migration = self
            .db
            .prepare("PRAGMA table_info(tracks);")
            .and_then(|mut stmt| {
                let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, i32>(3)?)))?;
                Ok(rows.flatten().any(|(col, notnull)| col == "release_id" && notnull != 0))
            })
            .unwrap_or(false);
        if needs_migration {
            let _ = self.db.execute_batch("PRAGMA foreign_keys = OFF;");
            let _ = self.db.execute_batch("BEGIN;");
            let ok = [
                "DROP TABLE IF EXISTS tracks_new;",
                "CREATE TABLE tracks_new (
                    id           INTEGER PRIMARY KEY AUTOINCREMENT,
                    recording_id TEXT    NOT NULL REFERENCES recordings(isrc) ON DELETE CASCADE,
                    release_id   TEXT    REFERENCES releases(upc) ON DELETE SET NULL
                );",
                "INSERT OR IGNORE INTO tracks_new SELECT id, recording_id, release_id FROM tracks;",
                "DROP TABLE tracks;",
                "ALTER TABLE tracks_new RENAME TO tracks;",
            ]
            .iter()
            .all(|sql| self.db.execute_batch(sql).is_ok());
            let _ = self.db.execute_batch(if ok { "COMMIT;" } else { "ROLLBACK;" });
            let _ = self.db.execute_batch("PRAGMA foreign_keys = ON;");
        }
        Ok(())
    }

    // ── Bookmark CRUD ─────────────────────────────────────────────────────────

    /// Adds a bookmark. `parent_id` = -1 for top-level entries. No-op if uri already exists.
    pub fn add_bookmark(&self, uri: &str, title: &str, parent_id: i32) {
        let _ = if parent_id == -1 {
            self.db.execute("INSERT OR IGNORE INTO bookmarks (uri, title) VALUES (?, ?);", params![uri, title])
        } else {
            self.db.execute(
                "INSERT OR IGNORE INTO bookmarks (uri, title, parent_id) VALUES (?, ?, ?);",
                params![uri, title, parent_id],
            )
        };
    }

    /// Removes a bookmark by URI. Children are cascade-deleted by the schema.
    pub fn remove_bookmark(&self, uri: &str) {
        let _ = self.db.execute("DELETE FROM bookmarks WHERE uri = ?;", params![uri]);
    }

    pub fn is_bookmarked(&self, uri: &str) -> bool {
        self.db
            .query_row("SELECT 1 FROM bookmarks WHERE uri = ? LIMIT 1;", params![uri], |_| Ok(()))
            .optional()
            .ok()
            .flatten()
            .is_some()
    }

    // ── Accepted third-party domains ──────────────────────────────────────────

    pub fn is_domain_accepted(&self, hostname: &str) -> bool {
        self.db
            .query_row("SELECT 1 FROM accepted_domains WHERE hostname = ? LIMIT 1;", params![hostname], |_| Ok(()))
            .optional()
            .ok()
            .flatten()
            .is_some()
    }

    pub fn add_accepted_domain(&self, hostname: &str) {
        let _ = self.db.execute("INSERT OR IGNORE INTO accepted_domains (hostname) VALUES (?);", params![hostname]);
    }

    /// The id of a bookmark, or -1 if not found.
    pub fn get_bookmark_id(&self, uri: &str) -> i32 {
        self.db.query_row("SELECT id FROM bookmarks WHERE uri = ? LIMIT 1;", params![uri], |r| r.get(0)).unwrap_or(-1)
    }

    /// Top-level bookmarks whose URI starts with `uri_prefix`, ordered by created_at.
    pub fn get_top_level_bookmarks(&self, uri_prefix: &str) -> Vec<Bookmark> {
        let pattern = format!("{uri_prefix}%");
        self.query_bookmarks(
            "SELECT id, uri, title FROM bookmarks WHERE uri LIKE ? AND parent_id IS NULL ORDER BY created_at;",
            params![pattern],
            -1,
        )
    }

    /// All child bookmarks of the given parent, ordered by created_at.
    pub fn get_child_bookmarks(&self, parent_id: i32) -> Vec<Bookmark> {
        self.query_bookmarks(
            "SELECT id, uri, title FROM bookmarks WHERE parent_id = ? ORDER BY created_at;",
            params![parent_id],
            parent_id,
        )
    }

    fn query_bookmarks(&self, sql: &str, args: &[&dyn rusqlite::ToSql], parent_id: i32) -> Vec<Bookmark> {
        let Ok(mut stmt) = self.db.prepare(sql) else {
            return Vec::new();
        };
        stmt.query_map(args, |r| Ok(Bookmark { id: r.get(0)?, uri: r.get(1)?, title: r.get(2)?, parent_id }))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default()
    }

    // ── Music metadata CRUD ───────────────────────────────────────────────────

    fn query_strings<T>(
        &self,
        sql: &str,
        args: &[&dyn rusqlite::ToSql],
        make: impl Fn(Vec<String>) -> T,
        n: usize,
    ) -> Vec<T> {
        let Ok(mut stmt) = self.db.prepare(sql) else {
            return Vec::new();
        };
        stmt.query_map(args, |r| {
            (0..n)
                .map(|i| r.get::<_, Option<String>>(i).map(Option::unwrap_or_default))
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map(|rows| rows.flatten().map(&make).collect())
        .unwrap_or_default()
    }

    /// All artists ordered by name.
    pub fn get_all_artists(&self) -> Vec<ArtistRecord> {
        self.query_strings("SELECT isni, name FROM artists ORDER BY name;", &[], artist_record, 2)
    }

    /// The artist with `isni`, or an empty isni if not found.
    pub fn get_artist(&self, isni: &str) -> ArtistRecord {
        self.query_strings("SELECT isni, name FROM artists WHERE isni = ?;", params![isni], artist_record, 2)
            .into_iter()
            .next()
            .unwrap_or_default()
    }

    /// All recordings ordered by name.
    pub fn get_all_recordings(&self) -> Vec<RecordingRecord> {
        self.query_strings(
            "SELECT isrc, name, COALESCE(version,'') FROM recordings ORDER BY name;",
            &[],
            recording_record,
            3,
        )
    }

    /// All releases ordered by name.
    pub fn get_all_releases(&self) -> Vec<ReleaseRecord> {
        self.query_strings(
            "SELECT upc, name, COALESCE(release_date,'') FROM releases ORDER BY name;",
            &[],
            release_record,
            3,
        )
    }

    pub fn upsert_artist(&self, isni: &str, name: &str) {
        let _ = self.db.execute(
            "INSERT INTO artists (isni, name) VALUES (?, ?)
              ON CONFLICT(isni) DO UPDATE SET name = excluded.name;",
            params![isni, name],
        );
    }

    pub fn upsert_recording(&self, isrc: &str, name: &str, version: &str) {
        let _ = self.db.execute(
            "INSERT INTO recordings (isrc, name, version) VALUES (?, ?, ?)
              ON CONFLICT(isrc) DO UPDATE SET name = excluded.name, version = excluded.version;",
            params![isrc, name, opt_text(version)],
        );
    }

    pub fn upsert_release(&self, upc: &str, name: &str, release_date: &str) {
        let _ = self.db.execute(
            "INSERT INTO releases (upc, name, release_date) VALUES (?, ?, ?)
              ON CONFLICT(upc) DO UPDATE SET name = excluded.name,
                release_date = excluded.release_date;",
            params![upc, name, opt_text(release_date)],
        );
    }

    /// Inserts or finds a track for the given recording / release combination.
    /// `release_upc` may be empty for recordings without a release.
    /// Returns the track id.
    pub fn upsert_track(&self, recording_isrc: &str, release_upc: &str) -> i64 {
        let existing = if release_upc.is_empty() {
            self.db
                .query_row(
                    "SELECT id FROM tracks WHERE recording_id = ? AND release_id IS NULL LIMIT 1;",
                    params![recording_isrc],
                    |r| r.get::<_, i64>(0),
                )
                .optional()
        } else {
            self.db
                .query_row(
                    "SELECT id FROM tracks WHERE recording_id = ? AND release_id = ? LIMIT 1;",
                    params![recording_isrc, release_upc],
                    |r| r.get::<_, i64>(0),
                )
                .optional()
        };
        match existing {
            Err(_) => return -1,
            Ok(Some(id)) => return id,
            Ok(None) => {}
        }
        let _ = self.db.execute(
            "INSERT INTO tracks (recording_id, release_id) VALUES (?, ?);",
            params![recording_isrc, opt_text(release_upc)],
        );
        self.db.last_insert_rowid()
    }

    /// Replaces all recording_artists rows for `recording_isrc`.
    pub fn set_recording_artists(&self, recording_isrc: &str, artists: &[ArtistRelationship]) {
        let _ = self.db.execute("DELETE FROM recording_artists WHERE recording_id = ?;", params![recording_isrc]);
        let Ok(mut ins) = self.db.prepare(
            "INSERT INTO recording_artists (recording_id, artist_id, is_primary, number)
              VALUES (?, ?, ?, ?);",
        ) else {
            return;
        };
        for a in artists {
            let _ = ins.execute(params![recording_isrc, a.artist_isni, a.is_primary as i32, a.number]);
        }
    }

    /// Replaces all release_artists rows for `release_upc`.
    pub fn set_release_artists(&self, release_upc: &str, artists: &[ArtistRelationship]) {
        let _ = self.db.execute("DELETE FROM release_artists WHERE release_id = ?;", params![release_upc]);
        let Ok(mut ins) = self.db.prepare(
            "INSERT INTO release_artists (release_id, artist_id, number, is_primary)
              VALUES (?, ?, ?, ?);",
        ) else {
            return;
        };
        for a in artists {
            let _ = ins.execute(params![release_upc, a.artist_isni, a.number, a.is_primary as i32]);
        }
    }

    /// Adds a save for `track_id`.
    /// If a non-deleted save already exists, it is left as-is and its id is returned.
    /// If a soft-deleted save exists, it is reactivated (deleted_at set to NULL).
    /// Otherwise a new save row is inserted. Returns the save id.
    pub fn ensure_save(&self, track_id: i64) -> i64 {
        let active = self
            .db
            .query_row(
                "SELECT id FROM saves WHERE track_id = ? AND deleted_at IS NULL LIMIT 1;",
                params![track_id],
                |r| r.get::<_, i64>(0),
            )
            .optional();
        if let Ok(Some(id)) = active {
            return id;
        }
        let deleted = self
            .db
            .query_row(
                "SELECT id FROM saves WHERE track_id = ? AND deleted_at IS NOT NULL LIMIT 1;",
                params![track_id],
                |r| r.get::<_, i64>(0),
            )
            .optional();
        if let Ok(Some(id)) = deleted {
            let _ = self
                .db
                .execute("UPDATE saves SET deleted_at = NULL, saved_at = datetime('now') WHERE id = ?;", params![id]);
            return id;
        }
        if self.db.execute("INSERT INTO saves (track_id) VALUES (?);", params![track_id]).is_err() {
            return -1;
        }
        self.db.last_insert_rowid()
    }

    /// Loads full details for a track (recording, release, artist relationships).
    pub fn get_track_details(&self, track_id: i32) -> TrackDetails {
        let mut out = TrackDetails {
            id: track_id,
            recording: RecordingRecord::default(),
            release: ReleaseRecord::default(),
            recording_artists: Vec::new(),
            release_artists: Vec::new(),
        };
        if let Some(row) = self
            .query_strings(
                "SELECT rec.isrc, rec.name, COALESCE(rec.version,''),
                        COALESCE(rel.upc,''), COALESCE(rel.name,''),
                        COALESCE(rel.release_date,'')
                 FROM tracks t
                 JOIN recordings rec ON rec.isrc = t.recording_id
                 LEFT JOIN releases rel ON rel.upc = t.release_id
                 WHERE t.id = ?;",
                params![track_id],
                |v| v,
                6,
            )
            .into_iter()
            .next()
        {
            out.recording = recording_record(row[0..3].to_vec());
            out.release = release_record(row[3..6].to_vec());
        }
        out.recording_artists = self.artist_relationships(
            "SELECT a.isni, a.name, ra.is_primary, ra.number
             FROM recording_artists ra
             JOIN artists a ON a.isni = ra.artist_id
             WHERE ra.recording_id = ?
             ORDER BY ra.number;",
            &out.recording.isrc,
        );
        if !out.release.upc.is_empty() {
            out.release_artists = self.artist_relationships(
                "SELECT a.isni, a.name, ra.is_primary, ra.number
                 FROM release_artists ra
                 JOIN artists a ON a.isni = ra.artist_id
                 WHERE ra.release_id = ?
                 ORDER BY ra.number;",
                &out.release.upc,
            );
        }
        out
    }

    fn artist_relationships(&self, sql: &str, key: &str) -> Vec<ArtistRelationship> {
        let Ok(mut stmt) = self.db.prepare(sql) else {
            return Vec::new();
        };
        stmt.query_map(params![key], |r| {
            Ok(ArtistRelationship {
                artist_isni: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                artist_name: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                is_primary: r.get::<_, i32>(2)? != 0,
                number: r.get(3)?,
            })
        })
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
    }
}

fn artist_record(mut v: Vec<String>) -> ArtistRecord {
    let name = v.pop().unwrap_or_default();
    ArtistRecord { isni: v.pop().unwrap_or_default(), name }
}

fn recording_record(v: Vec<String>) -> RecordingRecord {
    let mut it = v.into_iter();
    RecordingRecord {
        isrc: it.next().unwrap_or_default(),
        name: it.next().unwrap_or_default(),
        version: it.next().unwrap_or_default(),
    }
}

fn release_record(v: Vec<String>) -> ReleaseRecord {
    let mut it = v.into_iter();
    ReleaseRecord {
        upc: it.next().unwrap_or_default(),
        name: it.next().unwrap_or_default(),
        release_date: it.next().unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookmarks_and_tracks() {
        let db = Database::open_in_memory().unwrap();
        db.add_bookmark("bungalow:artist:1", "One", -1);
        let parent = db.get_bookmark_id("bungalow:artist:1");
        db.add_bookmark("bungalow:artist:2", "Two", parent);
        assert!(db.is_bookmarked("bungalow:artist:2"));
        assert_eq!(db.get_top_level_bookmarks("bungalow:").len(), 1);
        assert_eq!(db.get_child_bookmarks(parent)[0].title, "Two");
        db.remove_bookmark("bungalow:artist:1");
        assert!(!db.is_bookmarked("bungalow:artist:2"));

        db.upsert_artist("0001", "Artist");
        db.upsert_recording("SE001", "Song", "");
        let track = db.upsert_track("SE001", "");
        assert_eq!(db.upsert_track("SE001", ""), track);
        db.set_recording_artists(
            "SE001",
            &[ArtistRelationship {
                artist_isni: "0001".into(),
                artist_name: String::new(),
                is_primary: true,
                number: 0,
            }],
        );
        let save = db.ensure_save(track);
        assert_eq!(db.ensure_save(track), save);
        let details = db.get_track_details(track as i32);
        assert_eq!(details.recording.name, "Song");
        assert_eq!(details.recording_artists[0].artist_name, "Artist");
        assert!(details.release.upc.is_empty());
    }
}
