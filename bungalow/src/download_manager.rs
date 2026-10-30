//! Port of `DownloadManager`: downloads files on worker threads (ureq
//! instead of WinHTTP) and delivers every state change and callback on the
//! UI thread through [`crate::ui_dispatcher`].
//!
//! Workers cannot hold the manager (it lives on the UI thread), so they send
//! [`WorkerEvent`]s over a channel and post a closure that drains it.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use rusqlite::{params, Connection};
use serde_json::Value;

use crate::database::Database;
use crate::http_client::http_request;
use crate::platform_ui;
use crate::ui_dispatcher::post_to_ui;

const AUDIO_EXTS: &[&str] = &[".mp3", ".mp4", ".m4a", ".aac", ".wav", ".flac", ".ogg", ".opus", ".wma"];

const DOWNLOAD_EXTS: &[&str] = &[
    ".mp3", ".mp4", ".m4a", ".aac", ".wav", ".flac", ".ogg", ".opus", ".wma", ".zip", ".tar", ".gz", ".bz2", ".xz",
    ".rar", ".7z", ".pdf", ".dmg", ".pkg", ".exe", ".deb", ".rpm", ".avi", ".mkv", ".mov", ".wmv", ".webm",
];

const AUDIO_MIME_TYPES: &[&str] = &["audio/", "video/"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DLStatus {
    Ongoing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DownloadItem {
    pub id: i64,
    pub url: String,
    pub filename: String,
    pub file_path: String,
    pub content_type: String,
    pub status: DLStatus,
    pub total_bytes: i64,
    pub done_bytes: i64,
    pub error_msg: String,
}

impl DownloadItem {
    pub fn progress(&self) -> f32 {
        if self.total_bytes > 0 {
            self.done_bytes as f32 / self.total_bytes as f32
        } else {
            0.0
        }
    }
}

/// (url, is_download, filename, content_type)
pub type CheckCallback = Box<dyn FnOnce(&str, bool, &str, &str)>;

enum WorkerEvent {
    Checked { request: u64, url: String, is_download: bool, filename: String, content_type: String },
    CannotCreateFile { id: i64 },
    Progress { id: i64, done: i64, total: i64 },
    Finished { id: i64, ok: bool, cancelled: bool, error: String, tmp_path: String, done: i64, total: i64 },
}

thread_local! {
    static MANAGERS: RefCell<Vec<Weak<DownloadManager>>> = const { RefCell::new(Vec::new()) };
}

pub struct DownloadManager {
    db: Option<Rc<Database>>,
    items: RefCell<Vec<DownloadItem>>,
    next_item_id: Cell<i64>,
    cancel_flags: RefCell<HashMap<i64, Arc<AtomicBool>>>,
    on_update: RefCell<Option<Rc<dyn Fn()>>>,
    checks: RefCell<HashMap<u64, CheckCallback>>,
    next_check: Cell<u64>,
    sender: Sender<WorkerEvent>,
    receiver: Receiver<WorkerEvent>,
}

impl DownloadManager {
    pub fn new(db: Option<Rc<Database>>) -> Rc<Self> {
        let (sender, receiver) = channel();
        let this = Rc::new(Self {
            db,
            items: RefCell::new(Vec::new()),
            next_item_id: Cell::new(1),
            cancel_flags: RefCell::new(HashMap::new()),
            on_update: RefCell::new(None),
            checks: RefCell::new(HashMap::new()),
            next_check: Cell::new(1),
            sender,
            receiver,
        });
        this.load();
        MANAGERS.with(|m| m.borrow_mut().push(Rc::downgrade(&this)));
        this
    }

    fn conn(&self) -> Option<&Connection> {
        self.db.as_deref().map(Database::handle)
    }

    fn load(&self) {
        let Some(db) = self.conn() else { return };
        let Ok(mut stmt) = db.prepare(
            "SELECT id, url, filename, file_path, content_type, status, total_bytes, done_bytes FROM downloads ORDER BY id",
        ) else {
            return;
        };
        let text = |r: &rusqlite::Row, i| r.get::<_, Option<String>>(i).map(Option::unwrap_or_default);
        let rows = stmt.query_map([], |r| {
            let status = text(r, 5)?;
            let (status, error_msg) = match status.as_str() {
                "completed" => (DLStatus::Completed, String::new()),
                "failed" => (DLStatus::Failed, String::new()),
                "cancelled" => (DLStatus::Cancelled, String::new()),
                _ => (DLStatus::Failed, "Interrupted".to_string()),
            };
            Ok(DownloadItem {
                id: r.get(0)?,
                url: text(r, 1)?,
                filename: text(r, 2)?,
                file_path: text(r, 3)?,
                content_type: text(r, 4)?,
                status,
                total_bytes: r.get(6)?,
                done_bytes: r.get(7)?,
                error_msg,
            })
        });
        let Ok(rows) = rows else { return };
        for item in rows.flatten() {
            if item.id >= self.next_item_id.get() {
                self.next_item_id.set(item.id + 1);
            }
            self.items.borrow_mut().push(item);
        }
    }

    pub fn items(&self) -> Vec<DownloadItem> {
        self.items.borrow().clone()
    }

    pub fn set_update_callback(&self, cb: Option<Rc<dyn Fn()>>) {
        *self.on_update.borrow_mut() = cb;
    }

    fn notify_update(&self) {
        let cb = self.on_update.borrow().clone();
        if let Some(cb) = cb {
            cb();
        }
    }

    fn wake(sender: &Sender<WorkerEvent>, event: WorkerEvent) {
        if sender.send(event).is_ok() {
            post_to_ui(Self::pump_all);
        }
    }

    /// Delivers queued worker events to every live manager on this thread.
    pub fn pump_all() {
        let managers: Vec<Rc<DownloadManager>> = MANAGERS.with(|m| {
            let mut m = m.borrow_mut();
            m.retain(|w| w.strong_count() > 0);
            m.iter().filter_map(Weak::upgrade).collect()
        });
        for manager in managers {
            manager.pump();
        }
    }

    fn pump(&self) {
        while let Ok(event) = self.receiver.try_recv() {
            self.handle(event);
        }
    }

    fn handle(&self, event: WorkerEvent) {
        match event {
            WorkerEvent::Checked { request, url, is_download, filename, content_type } => {
                let cb = self.checks.borrow_mut().remove(&request);
                if let Some(cb) = cb {
                    if is_download {
                        cb(&url, true, &filename, &content_type);
                    } else {
                        cb(&url, false, "", "");
                    }
                }
            }
            WorkerEvent::CannotCreateFile { id } => {
                self.cancel_flags.borrow_mut().remove(&id);
                if self.update_item(id, |it| {
                    it.status = DLStatus::Failed;
                    it.error_msg = "Cannot create file".into();
                }) {
                    self.persist_item(id);
                    self.notify_update();
                }
            }
            WorkerEvent::Progress { id, done, total } => {
                let changed = self.update_item(id, |it| {
                    if it.status != DLStatus::Ongoing {
                        return;
                    }
                    it.done_bytes = done;
                    if total > 0 {
                        it.total_bytes = total;
                    }
                });
                if changed {
                    self.notify_update();
                }
            }
            WorkerEvent::Finished { id, ok, cancelled, error, tmp_path, done, total } => {
                self.cancel_flags.borrow_mut().remove(&id);
                if !self.update_item(id, |it| {
                    it.done_bytes = done;
                    if total > 0 {
                        it.total_bytes = total;
                    }
                }) {
                    return;
                }
                if ok {
                    self.on_download_complete(id, &tmp_path);
                } else {
                    self.update_item(id, |it| {
                        if cancelled {
                            it.status = DLStatus::Cancelled;
                        } else {
                            it.status = DLStatus::Failed;
                            it.error_msg = error.clone();
                        }
                    });
                    self.persist_item(id);
                    self.notify_update();
                }
            }
        }
    }

    fn update_item(&self, id: i64, f: impl FnOnce(&mut DownloadItem)) -> bool {
        let mut items = self.items.borrow_mut();
        match items.iter_mut().find(|i| i.id == id) {
            Some(item) => {
                f(item);
                true
            }
            None => false,
        }
    }

    fn find_item(&self, id: i64) -> Option<DownloadItem> {
        self.items.borrow().iter().find(|i| i.id == id).cloned()
    }

    /// Checks (with a HEAD request) whether `url` is a download and calls
    /// `cb` on the UI thread with the result.
    pub fn check_url(&self, url: &str, cb: CheckCallback) {
        if !Self::might_be_download(url) {
            cb(url, false, "", "");
            return;
        }
        let request = self.next_check.get();
        self.next_check.set(request + 1);
        self.checks.borrow_mut().insert(request, cb);
        let sender = self.sender.clone();
        let url = url.to_string();
        std::thread::spawn(move || {
            let res = http_request(&url, "HEAD", None, None, None);
            let mut is_download = false;
            let mut filename = String::new();
            if res.ok {
                is_download = (200..300).contains(&res.status)
                    && (res.content_disposition.contains("attachment")
                        || Self::is_audio_content_type(&res.content_type)
                        || res.content_type.contains("application/octet-stream"));
                filename = Self::guess_filename(&url, &res.content_disposition);
            }
            Self::wake(
                &sender,
                WorkerEvent::Checked { request, url, is_download, filename, content_type: res.content_type },
            );
        });
    }

    pub fn start_download(&self, url: &str, filename: &str, content_type: &str) -> i64 {
        let id = self.next_item_id.get();
        self.next_item_id.set(id + 1);
        let item = DownloadItem {
            id,
            url: url.to_string(),
            filename: if filename.is_empty() { Self::guess_filename(url, "") } else { filename.to_string() },
            file_path: String::new(),
            content_type: content_type.to_string(),
            status: DLStatus::Ongoing,
            total_bytes: 0,
            done_bytes: 0,
            error_msg: String::new(),
        };
        self.persist_new(&item);
        self.items.borrow_mut().push(item.clone());
        self.notify_update();

        let downloads = Self::get_downloads_folder();
        let _ = fs::create_dir_all(&downloads);
        let tmp_path = Self::unique_file_path(&downloads, &format!(".{}.part", item.filename));

        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel_flags.borrow_mut().insert(id, cancel.clone());

        let sender = self.sender.clone();
        let url = url.to_string();
        std::thread::spawn(move || {
            let Ok(mut file) = fs::File::create(&tmp_path) else {
                Self::wake(&sender, WorkerEvent::CannotCreateFile { id });
                return;
            };
            let mut done: i64 = 0;
            let total = Cell::new(0i64);
            let mut chunks = 0;
            let mut write_failed = false;
            let res = {
                let mut on_headers = |h: &crate::http_client::HttpResult| total.set(h.content_length);
                let mut on_data = |d: &[u8]| {
                    use std::io::Write;
                    if file.write_all(d).is_err() {
                        write_failed = true;
                        return false;
                    }
                    done += d.len() as i64;
                    chunks += 1;
                    if chunks % 8 == 0 {
                        Self::wake(&sender, WorkerEvent::Progress { id, done, total: total.get() });
                    }
                    true
                };
                http_request(&url, "GET", Some(&mut on_data), Some(&cancel), Some(&mut on_headers))
            };
            drop(file);
            let cancelled = res.cancelled && !write_failed && cancel.load(Ordering::Relaxed);
            let ok = res.ok && !res.cancelled && (200..300).contains(&res.status);
            let error = if write_failed {
                "Cannot write file".to_string()
            } else if res.ok {
                format!("HTTP {}", res.status)
            } else {
                res.error
            };
            if !ok {
                let _ = fs::remove_file(&tmp_path);
            }
            Self::wake(&sender, WorkerEvent::Finished { id, ok, cancelled, error, tmp_path, done, total: total.get() });
        });
        id
    }

    pub fn cancel_download(&self, id: i64) {
        if let Some(flag) = self.cancel_flags.borrow().get(&id) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    fn on_download_complete(&self, id: i64, tmp_path: &str) {
        let Some(item) = self.find_item(id) else { return };
        let dest_dir = Self::get_downloads_folder();
        let _ = fs::create_dir_all(&dest_dir);
        let dest_path = Self::unique_file_path(&dest_dir, &item.filename);

        if fs::rename(tmp_path, &dest_path).is_err() {
            let copied = fs::copy(tmp_path, &dest_path);
            let _ = fs::remove_file(tmp_path);
            if let Err(e) = copied {
                self.update_item(id, |it| {
                    it.status = DLStatus::Failed;
                    it.error_msg = e.to_string();
                });
                self.persist_item(id);
                self.notify_update();
                return;
            }
        }

        self.update_item(id, |it| {
            it.file_path = dest_path.clone();
            it.status = DLStatus::Completed;
            if it.total_bytes > 0 {
                it.done_bytes = it.total_bytes;
            }
        });
        let item = self.find_item(id).unwrap_or(item);

        if let Some(db) = self.conn() {
            let _ = db.execute(
                "UPDATE downloads SET status='completed', file_path=?, done_bytes=?, \
                 total_bytes=?, completed_at=datetime('now') WHERE id=?",
                params![item.file_path, item.done_bytes, item.total_bytes, item.id],
            );
        }

        if Self::is_audio_filename(&item.filename) || Self::is_audio_content_type(&item.content_type) {
            let music_dir = Self::get_music_folder();
            let _ = fs::create_dir_all(&music_dir);
            let music_path = Self::unique_file_path(&music_dir, &item.filename);
            if fs::copy(&dest_path, &music_path).is_ok() {
                self.import_audio_file(&music_path);
            }
        }
        self.notify_update();
    }

    fn import_audio_file(&self, file_path: &str) {
        if let Some(db) = self.conn() {
            platform_ui::import_audio_file(db, file_path);
        }
    }

    fn persist_new(&self, item: &DownloadItem) {
        let Some(db) = self.conn() else { return };
        let _ = db.execute(
            "INSERT INTO downloads(id, url, filename, content_type, status, total_bytes, done_bytes) \
             VALUES(?,?,?,?,'ongoing',0,0)",
            params![item.id, item.url, item.filename, item.content_type],
        );
    }

    fn persist_item(&self, id: i64) {
        if let Some(item) = self.find_item(id) {
            self.persist_update(&item);
        }
    }

    fn persist_update(&self, item: &DownloadItem) {
        let Some(db) = self.conn() else { return };
        let status = match item.status {
            DLStatus::Ongoing => "ongoing",
            DLStatus::Completed => "completed",
            DLStatus::Failed => "failed",
            DLStatus::Cancelled => "cancelled",
        };
        let _ = db.execute(
            "UPDATE downloads SET status=?, total_bytes=?, done_bytes=?, file_path=? WHERE id=?",
            params![status, item.total_bytes, item.done_bytes, item.file_path, item.id],
        );
    }

    fn extension(path: &str) -> Option<String> {
        path.rfind('.').map(|dot| path[dot..].to_ascii_lowercase())
    }

    pub fn might_be_download(url: &str) -> bool {
        let path = url.split('?').next().unwrap_or_default();
        Self::extension(path).is_some_and(|ext| DOWNLOAD_EXTS.contains(&ext.as_str()))
    }

    pub fn guess_filename(url: &str, content_disposition: &str) -> String {
        if let Some(pos) = content_disposition.find("filename=") {
            let mut name = &content_disposition[pos + 9..];
            name = name.strip_prefix('"').unwrap_or(name);
            if let Some(end) = name.find('"') {
                name = &name[..end];
            }
            if !name.is_empty() {
                return name.to_string();
            }
        }
        let path = url.split('?').next().unwrap_or_default();
        let name = path.rsplit('/').next().unwrap_or_default();
        if name.is_empty() { "download" } else { name }.to_string()
    }

    pub fn is_audio_content_type(content_type: &str) -> bool {
        AUDIO_MIME_TYPES.iter().any(|m| content_type.starts_with(m))
    }

    pub fn is_audio_filename(filename: &str) -> bool {
        Self::extension(filename).is_some_and(|ext| AUDIO_EXTS.contains(&ext.as_str()))
    }

    pub fn unique_file_path(dir: &str, filename: &str) -> String {
        let path = format!("{dir}/{filename}");
        if !Path::new(&path).exists() {
            return path;
        }
        let (stem, ext) = match filename.rfind('.') {
            Some(dot) => (&filename[..dot], &filename[dot..]),
            None => (filename, ""),
        };
        (2..1000)
            .map(|i| format!("{dir}/{stem} ({i}){ext}"))
            .find(|candidate| !Path::new(candidate).exists())
            .unwrap_or(path)
    }

    pub fn get_downloads_folder() -> String {
        let v = read_string_setting("downloadsFolder");
        if v.is_empty() {
            format!("{}/Downloads", platform_ui::home_dir())
        } else {
            v
        }
    }

    pub fn get_music_folder() -> String {
        let v = read_string_setting("musicFolder");
        if v.is_empty() {
            format!("{}/Music", platform_ui::home_dir())
        } else {
            v
        }
    }
}

impl Drop for DownloadManager {
    fn drop(&mut self) {
        for flag in self.cancel_flags.borrow().values() {
            flag.store(true, Ordering::Relaxed);
        }
    }
}

fn read_string_setting(key: &str) -> String {
    let Ok(text) = fs::read_to_string(crate::views::config_view::ConfigView::get_settings_path()) else {
        return String::new();
    };
    serde_json::from_str::<Value>(&text)
        .ok()
        .and_then(|obj| obj.get(key).and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_urls() {
        assert!(DownloadManager::might_be_download("https://x.se/a/song.MP3?x=1"));
        assert!(!DownloadManager::might_be_download("https://x.se/page.html"));
        assert_eq!(DownloadManager::guess_filename("https://x.se/a/b.zip?q", ""), "b.zip");
        assert_eq!(DownloadManager::guess_filename("https://x.se/", "attachment; filename=\"c.mp3\""), "c.mp3");
        assert_eq!(DownloadManager::guess_filename("https://x.se/", ""), "download");
        assert!(DownloadManager::is_audio_content_type("audio/mpeg"));
    }

    #[test]
    fn unique_paths() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path().to_string_lossy().to_string();
        let first = DownloadManager::unique_file_path(&dir, "a.mp3");
        fs::write(&first, b"x").unwrap();
        assert_eq!(DownloadManager::unique_file_path(&dir, "a.mp3"), format!("{dir}/a (2).mp3"));
    }
}
