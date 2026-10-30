//! Port of `SPUri`: splits `scheme:host:segments#hash` URIs.

use crate::sp_string::split;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SPUri {
    segments: Vec<String>,
    address: String,
    uri: String,
    protocol: String,
    entity: String,
    id: String,
    hash: String,
    host: String,
}

impl SPUri {
    pub fn new(uri: &str) -> Self {
        let uri_parts = split(uri, '#');
        let hash = uri_parts.get(1).cloned().unwrap_or_default();
        let address = uri_parts[0].clone();
        let uri_segments = split(&address, ':');
        Self {
            uri: uri.to_string(),
            hash,
            protocol: uri_parts[0].clone(),
            host: uri_segments.get(1).cloned().unwrap_or_default(),
            segments: uri_segments.iter().skip(2).cloned().collect(),
            address,
            entity: String::new(),
            id: String::new(),
        }
    }

    pub fn get_segments(&self) -> Vec<String> {
        self.segments.clone()
    }

    pub fn get_address(&self) -> String {
        self.address.clone()
    }

    pub fn get_protocol(&self) -> String {
        self.protocol.clone()
    }

    pub fn get_host(&self) -> String {
        self.host.clone()
    }

    pub fn get_entity(&self) -> String {
        self.entity.clone()
    }

    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    /// The fragment, or "overview" when the URI has none.
    pub fn get_hash(&self) -> String {
        if self.hash.is_empty() {
            "overview".to_string()
        } else {
            self.hash.clone()
        }
    }

    pub fn get_uri(&self) -> String {
        self.uri.clone()
    }
}

impl std::fmt::Display for SPUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bungalow_uris() {
        let uri = SPUri::new("bungalow:artist:1#albums");
        assert_eq!(uri.get_address(), "bungalow:artist:1");
        assert_eq!(uri.get_host(), "artist");
        assert_eq!(uri.get_segments(), vec!["1"]);
        assert_eq!(uri.get_hash(), "albums");
        assert_eq!(SPUri::new("bungalow:start").get_hash(), "overview");
        assert_eq!(SPUri::new("bungalow").get_host(), "");
    }
}
