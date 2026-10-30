//! Port of `SPAppBundle.h`: app bundles and the extensions they contribute.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

/// An extension contributed by an app bundle, e.g. a skin found in
/// `<bundle>/skins/<name>/manifest.json`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SPAppExtension {
    /// Manifest BundleType, e.g. "Skin"
    pub type_: String,
    /// Manifest BundleIdentifier, e.g. "modern.skin"
    pub identifier: String,
    /// Absolute directory of the extension
    pub path: String,
    /// Identifier of the owning app bundle
    pub bundle_id: String,
    pub names: BTreeMap<String, String>,
    pub descriptions: BTreeMap<String, String>,
    pub manifest: Value,
}

impl SPAppExtension {
    pub fn get_name(&self, lang: &str) -> String {
        let name = localized(&self.names, lang);
        if name.is_empty() {
            self.identifier.clone()
        } else {
            name
        }
    }
    pub fn get_description(&self, lang: &str) -> String {
        localized(&self.descriptions, lang)
    }
}

/// An app bundle: a directory with a manifest.json, found in one of the
/// app search paths (see [`crate::sp_app_manager::SPAppManager`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SPAppBundle {
    /// Manifest BundleType, e.g. "Application"
    pub type_: String,
    /// Manifest BundleIdentifier; host of sp:// URIs
    pub identifier: String,
    /// Absolute directory of the bundle
    pub path: String,
    pub names: BTreeMap<String, String>,
    pub descriptions: BTreeMap<String, String>,
    pub manifest: Value,
    pub extensions: Vec<SPAppExtension>,
}

impl SPAppBundle {
    pub fn get_name(&self, lang: &str) -> String {
        let name = localized(&self.names, lang);
        if name.is_empty() {
            self.identifier.clone()
        } else {
            name
        }
    }
    pub fn get_description(&self, lang: &str) -> String {
        localized(&self.descriptions, lang)
    }
}

pub(crate) fn localized(values: &BTreeMap<String, String>, lang: &str) -> String {
    values.get(lang).or_else(|| values.get("en")).or_else(|| values.values().next()).cloned().unwrap_or_default()
}

/// Reads a localized field such as `{"AppName": {"en": "..."}}`; a plain
/// string is stored under "en".
pub(crate) fn localized_field(obj: &Value, key: &str) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    match obj.get(key) {
        Some(Value::String(s)) => {
            result.insert("en".to_string(), s.clone());
        }
        Some(Value::Object(map)) => {
            for (lang, value) in map {
                if let Some(s) = value.as_str() {
                    result.insert(lang.clone(), s.to_string());
                }
            }
        }
        _ => {}
    }
    result
}

pub(crate) fn string_field(obj: &Value, key: &str) -> String {
    obj.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

/// `std::filesystem::path::lexically_normal`.
pub fn lexically_normal(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(out.components().next_back(), Some(Component::Normal(_))) {
                    out.pop();
                } else if !matches!(out.components().next_back(), Some(Component::RootDir | Component::Prefix(_))) {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Resolves `relative` against `base`; None if the result escapes `root`
/// (or is `root` itself).
pub fn resolve_inside(root: &Path, base: &Path, relative: &str) -> Option<PathBuf> {
    let root = lexically_normal(root);
    let target = lexically_normal(&base.join(relative));
    let rest = target.strip_prefix(&root).ok()?;
    if rest.as_os_str().is_empty() {
        return None;
    }
    Some(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_rejects_escapes() {
        let root = Path::new("/apps/x");
        assert_eq!(resolve_inside(root, root, "views/a.xml"), Some(PathBuf::from("/apps/x/views/a.xml")));
        assert_eq!(resolve_inside(root, root, "../y/secret"), None);
        assert_eq!(resolve_inside(root, root, "a/../../x/b"), Some(PathBuf::from("/apps/x/b")));
        assert_eq!(resolve_inside(root, root, "."), None);
    }
}
