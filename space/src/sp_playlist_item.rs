//! Port of `SPPlaylistItemResource`.

use crate::sp_resource::SPResource;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SPPlaylistItemResource {
    pub resource: SPResource,
    pub playlist_uri: String,
    pub node_uri: String,
    pub user_uri: String,
    pub comments: String,
    pub position: i32,
    pub added_at: u64,
    pub removed_at: u64,
}
