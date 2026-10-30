//! Port of `SPStylesheet`: a JSON map of `entity -> property -> value`.

use serde_json::Value;

#[derive(Debug, Clone)]
pub struct SPStylesheet {
    pt: Value,
}

impl Default for SPStylesheet {
    fn default() -> Self {
        Self::new()
    }
}

impl SPStylesheet {
    /// Reads `../../skin.json`, like the C++ constructor.
    pub fn new() -> Self {
        Self::from_file("../../skin.json")
    }

    pub fn from_file(path: &str) -> Self {
        let pt = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| match serde_json::from_str(&text) {
                Ok(value) => Some(value),
                Err(e) => {
                    log::error!("{e}");
                    None
                }
            })
            .unwrap_or_else(|| Value::Object(Default::default()));
        Self { pt }
    }

    pub fn from_json(pt: Value) -> Self {
        Self { pt }
    }

    pub fn get(&self, entity: &str, prop: &str) -> Option<String> {
        self.pt.get(entity)?.get(prop)?.as_str().map(str::to_string)
    }
}
