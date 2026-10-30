//! The part of `BungalowApp` that code in this crate calls back into
//! (`BungalowApp::current()`). The client implements it and registers the
//! running app with [`set_current`].

use std::cell::RefCell;
use std::rc::Rc;

use crate::database::Database;
use crate::download_manager::DownloadManager;
use crate::play_context::PlayContext;
use crate::play_queue_entry::PlayQueueEntry;

pub trait BungalowAppHandle {
    fn get_database(&self) -> Option<Rc<Database>>;
    fn get_download_manager(&self) -> Option<Rc<DownloadManager>>;
    fn set_developer_mode(&self, value: bool);
    fn is_developer_mode(&self) -> bool;
    fn set_full_width_address_bar(&self, value: bool);
    /// Starts playing `context` (replaces the play queue).
    fn play_context(&self, context: PlayContext);
    fn play_queue_at_index(&self, index: i32);
    fn get_queue_entries(&self) -> Vec<PlayQueueEntry>;
    fn get_queue_index(&self) -> i32;
    fn navigate(&self, uri: &str);
    fn show_error(&self, message: &str);
}

thread_local! {
    static CURRENT: RefCell<Option<Rc<dyn BungalowAppHandle>>> = const { RefCell::new(None) };
}

pub fn current() -> Option<Rc<dyn BungalowAppHandle>> {
    CURRENT.with(|c| c.borrow().clone())
}

pub fn set_current(app: Option<Rc<dyn BungalowAppHandle>>) {
    CURRENT.with(|c| *c.borrow_mut() = app);
}
