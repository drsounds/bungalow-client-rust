//! Port of `PlatformUi` (`space::platform`): native dialogs and well-known
//! folders. The folders use the `dirs` crate on every platform. Dialogs need
//! the windowing layer, so the client installs them with [`set_hooks`];
//! until it does they behave like a dismissed dialog.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use rusqlite::{params, Connection, OptionalExtension};

pub type MenuItems = Vec<(i32, String)>;

/// Dialog implementations supplied by the client.
pub trait PlatformHooks {
    /// Shows a popup menu at the cursor and returns the chosen id, or -1.
    fn show_popup_menu(&self, items: &MenuItems) -> i32;
    /// Shows a folder picker. Returns the chosen path, or "" if cancelled.
    fn show_folder_dialog(&self, title: &str, initial_path: &str) -> String;
    /// Shows a file picker for audio files. Returns the path, or "" if cancelled.
    fn show_open_audio_file_dialog(&self, title: &str) -> String;
    /// Shows a message box.
    fn show_message(&self, title: &str, message: &str);
}

thread_local! {
    static HOOKS: RefCell<Option<Rc<dyn PlatformHooks>>> = const { RefCell::new(None) };
}

pub fn set_hooks(hooks: Option<Rc<dyn PlatformHooks>>) {
    HOOKS.with(|h| *h.borrow_mut() = hooks);
}

fn hooks() -> Option<Rc<dyn PlatformHooks>> {
    HOOKS.with(|h| h.borrow().clone())
}

/// Shows a popup menu at the cursor and returns the chosen item id, or -1 if dismissed.
pub fn show_popup_menu(items: &MenuItems) -> i32 {
    hooks().map(|h| h.show_popup_menu(items)).unwrap_or(-1)
}

/// Shows the add (`track_id` == 0) or edit track dialog. Returns true if
/// changes were saved. Like the Windows build, adding imports one audio file
/// into the library and editing is not available yet.
pub fn show_add_track_dialog(db: Option<&Connection>, track_id: i32) -> bool {
    let Some(db) = db else { return false };
    let Some(hooks) = hooks() else { return false };
    if track_id > 0 {
        hooks.show_message("Edit Track", "Editing track metadata is not available in this build yet.");
        return false;
    }
    let path = hooks.show_open_audio_file_dialog("Add Track").replace('\\', "/");
    if path.is_empty() {
        return false;
    }
    import_audio_file(db, &path)
}

/// Adds `path` to files, audio_files and playables.
pub fn import_audio_file(db: &Connection, path: &str) -> bool {
    let name = Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let _ = db.execute("INSERT OR IGNORE INTO files(path) VALUES(?)", params![path]);
    let file_id: i64 = db
        .query_row("SELECT id FROM files WHERE path=?", params![path], |r| r.get(0))
        .optional()
        .ok()
        .flatten()
        .unwrap_or(0);
    if file_id == 0 {
        return false;
    }
    let _ = db.execute("INSERT OR IGNORE INTO audio_files(file_id, name) VALUES(?,?)", params![file_id, name]);
    let _ = db.execute("INSERT OR IGNORE INTO playables(uri) VALUES(?)", params![format!("file://{path}")]);
    true
}

/// Shows a folder picker. Returns the chosen path, or "" if cancelled.
pub fn show_folder_dialog(title: &str, initial_path: &str) -> String {
    hooks()
        .map(|h| h.show_folder_dialog(title, initial_path).replace('\\', "/"))
        .unwrap_or_default()
}

fn path_string(path: Option<std::path::PathBuf>) -> String {
    path.map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default()
}

/// Per-user application data directory (created if missing).
pub fn user_data_dir() -> String {
    let dir = format!("{}/Bungalow", path_string(dirs::data_dir()));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn home_dir() -> String {
    path_string(dirs::home_dir())
}

/// The user's Documents folder.
pub fn documents_dir() -> String {
    path_string(dirs::document_dir().or_else(|| dirs::home_dir().map(|h| h.join("Documents"))))
}
