//! Port of `SPAppManager`: discovers app bundles and their extensions, and
//! resolves sp:// URIs to files inside them.
//!
//! Bundles are directories containing a manifest.json. Search paths are
//! scanned in the order they were added; a bundle found later replaces an
//! earlier one with the same identifier, so user folders can override the
//! bundled Apps.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use serde_json::Value;

use crate::sp_app_bundle::{
    lexically_normal, localized_field, resolve_inside, string_field, SPAppBundle, SPAppExtension,
};
use crate::sp_skin::SPSkin;
use crate::sp_xml_skin::SPXmlSkin;

const SP_SCHEME: &str = "sp://";

/// Folders inside an app bundle that hold extensions, and the BundleType
/// their manifests must declare.
const EXTENSION_FOLDERS: [(&str, &str); 1] = [("skins", "Skin")];

#[derive(Default)]
pub struct SPAppManager {
    search_paths: Vec<String>,
    bundles: Vec<SPAppBundle>,
}

thread_local! {
    static INSTANCE: RefCell<SPAppManager> = RefCell::new(SPAppManager::default());
}

fn read_json(file: &Path) -> Option<Value> {
    let text = fs::read_to_string(file).ok()?;
    match serde_json::from_str::<Value>(&text) {
        Ok(value) if value.is_object() => Some(value),
        Ok(_) => None,
        Err(e) => {
            log::error!("Invalid manifest {}: {e}", file.display());
            None
        }
    }
}

fn subdirectories(dir: &Path) -> Vec<PathBuf> {
    let mut result: Vec<PathBuf> = fs::read_dir(dir)
        .map(|entries| entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default();
    result.sort();
    result
}

fn absolute(path: &Path) -> String {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    lexically_normal(&path).to_string_lossy().into_owned()
}

impl SPAppManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// The application-wide manager (`SPAppManager::instance()`).
    pub fn with<R>(f: impl FnOnce(&mut SPAppManager) -> R) -> R {
        INSTANCE.with(|m| f(&mut m.borrow_mut()))
    }

    /// Adds `<base>/Apps`, `~/Bungalow`, `~/Bungalow/Apps`, `<documents>/Bungalow`
    /// and `<documents>/Bungalow/Apps` (later paths win on duplicate ids).
    /// Empty arguments are skipped.
    pub fn add_default_search_paths(&mut self, base: &str, home: &str, documents: &str) {
        if !base.is_empty() {
            self.add_search_path(&Path::new(base).join("Apps").to_string_lossy());
        }
        // Bundles may sit directly in a Bungalow folder or in its Apps/ subfolder.
        // Subfolders without a manifest.json (such as Apps/ itself) are skipped.
        for root in [home, documents] {
            if root.is_empty() {
                continue;
            }
            let bungalow = Path::new(root).join("Bungalow");
            self.add_search_path(&bungalow.to_string_lossy());
            self.add_search_path(&bungalow.join("Apps").to_string_lossy());
        }
    }

    pub fn add_search_path(&mut self, dir: &str) {
        let normalized = lexically_normal(Path::new(dir)).to_string_lossy().into_owned();
        if !self.search_paths.contains(&normalized) {
            self.search_paths.push(normalized);
        }
    }

    pub fn get_search_paths(&self) -> &[String] {
        &self.search_paths
    }

    /// Rescans all search paths.
    pub fn scan(&mut self) {
        self.bundles.clear();
        for dir in self.search_paths.clone() {
            self.scan_directory(Path::new(&dir));
        }
    }

    fn scan_directory(&mut self, dir: &Path) {
        for bundle_dir in subdirectories(dir) {
            let Some(manifest) = read_json(&bundle_dir.join("manifest.json")) else {
                continue;
            };
            let mut identifier = string_field(&manifest, "BundleIdentifier");
            if identifier.is_empty() {
                identifier = bundle_dir.file_name().unwrap_or_default().to_string_lossy().into_owned();
            }
            let mut bundle = SPAppBundle {
                type_: string_field(&manifest, "BundleType"),
                identifier,
                path: absolute(&bundle_dir),
                names: localized_field(&manifest, "AppName"),
                descriptions: localized_field(&manifest, "AppDescription"),
                manifest,
                extensions: Vec::new(),
            };

            for (folder, type_) in EXTENSION_FOLDERS {
                for ext_dir in subdirectories(&bundle_dir.join(folder)) {
                    let Some(ext_manifest) = read_json(&ext_dir.join("manifest.json")) else {
                        continue;
                    };
                    let mut ext_type = string_field(&ext_manifest, "BundleType");
                    if ext_type.is_empty() {
                        ext_type = type_.to_string();
                    }
                    if ext_type != type_ {
                        log::warn!("Skipping {}: BundleType {ext_type} does not belong in {folder}", ext_dir.display());
                        continue;
                    }
                    let mut ext_identifier = string_field(&ext_manifest, "BundleIdentifier");
                    if ext_identifier.is_empty() {
                        ext_identifier = format!(
                            "{}.{}",
                            bundle.identifier,
                            ext_dir.file_name().unwrap_or_default().to_string_lossy()
                        );
                    }
                    // Extension manifests name themselves after their type,
                    // e.g. SkinName / SkinDescription.
                    bundle.extensions.push(SPAppExtension {
                        names: localized_field(&ext_manifest, &format!("{ext_type}Name")),
                        descriptions: localized_field(&ext_manifest, &format!("{ext_type}Description")),
                        type_: ext_type,
                        identifier: ext_identifier,
                        path: absolute(&ext_dir),
                        bundle_id: bundle.identifier.clone(),
                        manifest: ext_manifest,
                    });
                }
            }

            match self.bundles.iter_mut().find(|b| b.identifier == bundle.identifier) {
                Some(existing) => *existing = bundle,
                None => self.bundles.push(bundle),
            }
        }
    }

    pub fn get_bundles(&self) -> &[SPAppBundle] {
        &self.bundles
    }

    pub fn find_bundle(&self, identifier: &str) -> Option<&SPAppBundle> {
        self.bundles.iter().find(|b| b.identifier == identifier)
    }

    /// All extensions of the given type across every bundle.
    pub fn get_extensions(&self, type_: &str) -> Vec<&SPAppExtension> {
        self.bundles.iter().flat_map(|b| b.extensions.iter()).filter(|e| e.type_ == type_).collect()
    }

    pub fn find_extension(&self, type_: &str, identifier: &str) -> Option<&SPAppExtension> {
        self.get_extensions(type_).into_iter().find(|e| e.identifier == identifier)
    }

    /// Resolves sp://<bundle>/<path> to an absolute file path. Returns None
    /// if the bundle is unknown or the path escapes it.
    pub fn resolve(&self, uri: &str) -> Option<String> {
        let rest = uri.strip_prefix(SP_SCHEME)?;
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        let bundle = self.find_bundle(host)?;
        Self::resolve_in_bundle(&bundle.path, path)
    }

    /// Resolves a path relative to a bundle directory. Returns None if the
    /// path escapes the bundle (sp://app/../../secret).
    pub fn resolve_in_bundle(bundle_root: &str, relative_path: &str) -> Option<String> {
        let root = Path::new(bundle_root);
        resolve_inside(root, root, relative_path).map(|p| p.to_string_lossy().into_owned())
    }

    /// Creates the skin for a skin type. Accepts a skin extension
    /// identifier ("modern.skin") or its short form ("modern"). Returns
    /// None if no such skin bundle is installed or it fails to load.
    ///
    /// Uses the application-wide manager; it is not borrowed while the skin
    /// loads, because skin resources resolve through it too.
    pub fn create_skin(skin_type: &str) -> Option<Rc<dyn SPSkin>> {
        let ext_path = Self::with(|m| {
            m.find_extension("Skin", skin_type)
                .or_else(|| m.find_extension("Skin", &format!("{skin_type}.skin")))
                .map(|ext| ext.path.clone())
        })?;
        let skin = SPXmlSkin::new();
        let file = Path::new(&ext_path).join("skin.xml");
        if !skin.load(&file.to_string_lossy(), &ext_path) {
            return None;
        }
        Some(Rc::new(skin))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_bundles_and_extensions() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("Apps/hello");
        fs::create_dir_all(app.join("skins/dark")).unwrap();
        fs::write(app.join("manifest.json"), r#"{"BundleIdentifier":"hello","AppName":{"en":"Hello","sv":"Hej"}}"#)
            .unwrap();
        fs::write(app.join("skins/dark/manifest.json"), r#"{"SkinName":"Dark"}"#).unwrap();
        fs::write(app.join("index.html"), "").unwrap();

        let mut manager = SPAppManager::new();
        manager.add_default_search_paths(&dir.path().to_string_lossy(), "", "");
        manager.scan();
        let bundle = manager.find_bundle("hello").unwrap();
        assert_eq!(bundle.get_name("sv"), "Hej");
        assert_eq!(bundle.get_name("de"), "Hello");
        let ext = manager.find_extension("Skin", "hello.dark").unwrap();
        assert_eq!(ext.get_name("en"), "Dark");
        assert!(manager.resolve("sp://hello/index.html").unwrap().ends_with("index.html"));
        assert!(manager.resolve("sp://hello/../../etc/passwd").is_none());
        assert!(manager.resolve("sp://missing/index.html").is_none());
    }
}
