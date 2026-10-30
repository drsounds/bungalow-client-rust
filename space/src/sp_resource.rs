//! Port of `SPResource`: a generic named entity with string attributes.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SPResource {
    pub id: String,
    pub type_: String,
    pub uri: String,
    pub name: String,
    pub collection_uri: String,
    pub creator_uris: Vec<String>,
    pub attributes: BTreeMap<String, String>,
}

impl SPResource {
    pub fn new(uri: &str) -> Self {
        Self { uri: uri.to_string(), ..Self::default() }
    }

    pub fn get(&self, key: &str, default_value: &str) -> String {
        match key {
            "uri" => self.uri.clone(),
            "name" => self.name.clone(),
            _ => self.attributes.get(key).cloned().unwrap_or_else(|| default_value.to_string()),
        }
    }

    pub fn set(&mut self, key: &str, value: &str) {
        if key == "name" {
            self.name = value.to_string();
        } else {
            self.attributes.insert(key.to_string(), value.to_string());
        }
    }
}
