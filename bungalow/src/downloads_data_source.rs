//! Port of `DownloadsDataSource`: DownloadManager items filtered by status
//! group. Columns: Name | Type | Source URL | Status.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use space::sp_table_data_source::SPTableDataSource;

use crate::download_manager::{DLStatus, DownloadItem, DownloadManager};

const COLUMNS: [&str; 4] = ["Name", "Type", "Source URL", "Status"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    Ongoing,
    Completed,
}

pub struct DownloadsDataSource {
    dm: Option<Rc<DownloadManager>>,
    filter: Filter,
    /// Local copy updated on refresh().
    snapshot: RefCell<Vec<DownloadItem>>,
}

impl DownloadsDataSource {
    pub fn new(dm: Option<Rc<DownloadManager>>, filter: Filter) -> Self {
        let this = Self { dm, filter, snapshot: RefCell::new(Vec::new()) };
        this.refresh();
        this
    }

    /// Refreshes the snapshot from the DownloadManager (call on update).
    pub fn refresh(&self) {
        let mut snapshot = self.snapshot.borrow_mut();
        snapshot.clear();
        let Some(dm) = &self.dm else { return };
        for item in dm.items() {
            let ongoing = item.status == DLStatus::Ongoing;
            if (self.filter == Filter::Ongoing) == ongoing {
                snapshot.push(item);
            }
        }
    }

    fn item(&self, row: i32) -> Option<DownloadItem> {
        usize::try_from(row).ok().and_then(|row| self.snapshot.borrow().get(row).cloned())
    }

    /// The item id for a row (for the cancel action), 0 if invalid.
    pub fn get_item_id_for_row(&self, row: i32) -> i64 {
        self.item(row).map(|i| i.id).unwrap_or(0)
    }
}

impl SPTableDataSource for DownloadsDataSource {
    fn get_column_count(&self) -> i32 {
        4
    }
    fn get_row_count(&self) -> i32 {
        self.snapshot.borrow().len() as i32
    }
    fn get_text_for_column(&self, column: i32) -> String {
        usize::try_from(column).ok().and_then(|c| COLUMNS.get(c)).copied().unwrap_or_default().into()
    }
    fn get_text_for_cell(&self, column: i32, row: i32) -> String {
        let Some(item) = self.item(row) else { return String::new() };
        match column {
            0 => item.filename,
            1 => {
                // A friendly type label from the content type or extension.
                if let Some((_, sub)) = item.content_type.split_once('/') {
                    let sub = sub.split(';').next().unwrap_or_default();
                    if !sub.is_empty() && sub != "octet-stream" {
                        return sub.to_string();
                    }
                }
                let ext =
                    Path::new(&item.filename).extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
                if ext.is_empty() {
                    "File".into()
                } else {
                    ext
                }
            }
            2 => item.url,
            3 => match item.status {
                DLStatus::Completed => "Completed".into(),
                DLStatus::Failed if item.error_msg.is_empty() => "Failed".into(),
                DLStatus::Failed => format!("Failed: {}", item.error_msg),
                DLStatus::Cancelled => "Cancelled".into(),
                DLStatus::Ongoing if item.total_bytes > 0 => format!("{}%", (item.progress() * 100.0) as i32),
                DLStatus::Ongoing => "Downloading…".into(),
            },
            _ => String::new(),
        }
    }
    fn get_progress_for_row(&self, row: i32) -> f32 {
        match self.item(row) {
            Some(item) if item.status == DLStatus::Ongoing => item.progress(),
            _ => -1.0,
        }
    }
}
