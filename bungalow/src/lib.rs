//! Port of the `bungalow/` directory of bungalow-client: the music library
//! database and models, media services, table data sources and views.
//!
//! Some platform pieces the views depend on (DownloadManager, PlatformUi,
//! UiDispatcher, the HTTP client) live in `windows/` in the C++ tree; they
//! are portable here and sit in this crate so the views can use them
//! without depending on the client.

pub mod bungalow_app_handle;
pub mod database;
pub mod download_manager;
pub mod downloads_data_source;
pub mod entity_resolvers;
pub mod http_client;
pub mod library_data_source;
pub mod local_files_data_source;
pub mod models;
pub mod platform_ui;
pub mod play_context;
pub mod play_queue_data_source;
pub mod play_queue_entry;
pub mod services;
pub mod ui_dispatcher;
pub mod views;

pub use database::Database;
