//! Port of `SPUriStore`: a redux-lite global store for URI entities (mostly
//! `bungalow:` URIs).
//!
//! State is a map of URI -> [`SPUriEntry`], changed only through
//! [`SPUriStore::dispatch`] and the pure [`SPUriStore::reduce`] function.
//! Entities are persisted as JSON files, so a view can render the last known
//! data immediately on startup while [`SPUriStore::resolve_uri`] revalidates it
//! in the background (stale-while-revalidate).
//!
//! The store is per UI thread (`SPUriStore::with`) and every method takes
//! `&self`, so listeners may call back into the store while being notified.
//! Resolvers may complete on any thread; their results are handed to the
//! scheduler, which the application points at its UI thread.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use serde_json::{json, Value};

// Longest cache file name before falling back to a hashed name; keeps paths
// well inside the Windows MAX_PATH limit.
const MAX_FILE_NAME_LENGTH: usize = 120;

/// Lifecycle of a URI entity in the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SPUriStatus {
    /// nothing known yet
    #[default]
    Empty,
    /// data hydrated from the on-disk JSON cache, not yet revalidated
    Cached,
    /// a resolver is fetching; `data` still holds the last known value
    Loading,
    /// data fetched during this session
    Fresh,
    /// the last fetch failed; `data` still holds the last known value
    Error,
}

pub fn to_string(status: SPUriStatus) -> &'static str {
    match status {
        SPUriStatus::Empty => "empty",
        SPUriStatus::Cached => "cached",
        SPUriStatus::Loading => "loading",
        SPUriStatus::Fresh => "fresh",
        SPUriStatus::Error => "error",
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SPUriEntry {
    /// null until something is known
    pub data: Value,
    pub status: SPUriStatus,
    /// ms since epoch of the data in `data`
    pub updated_at: i64,
    /// id of the fetch in flight / last fetch
    pub request_id: u64,
    pub error: String,
}

impl SPUriEntry {
    pub fn has_data(&self) -> bool {
        !self.data.is_null()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SPUriActionType {
    /// payload loaded from disk
    Hydrate,
    /// fetch started (request_id)
    Pending,
    /// fetch finished (request_id, payload)
    Fulfilled,
    /// fetch failed (request_id, error)
    Rejected,
    /// merge payload into the current data (optimistic update)
    Patch,
    /// mark stale so the next resolve_uri refetches
    Invalidate,
    /// forget the entity, including its cache file
    Evict,
}

#[derive(Debug, Clone)]
pub struct SPUriAction {
    pub type_: SPUriActionType,
    pub uri: String,
    pub payload: Value,
    pub error: String,
    pub request_id: u64,
    pub timestamp: i64,
}

impl SPUriAction {
    pub fn new(type_: SPUriActionType, uri: &str) -> Self {
        Self { type_, uri: uri.to_string(), payload: Value::Null, error: String::new(), request_id: 0, timestamp: 0 }
    }

    pub fn with_payload(mut self, payload: Value) -> Self {
        self.payload = payload;
        self
    }
}

pub type Listener = Rc<dyn Fn(&str, &SPUriEntry)>;
pub type ResolveCallback = Box<dyn FnOnce(Value) + Send>;
pub type RejectCallback = Box<dyn FnOnce(String) + Send>;
pub type Resolver = Rc<dyn Fn(&str, ResolveCallback, RejectCallback)>;
pub type Task = Box<dyn FnOnce() + Send>;
pub type Scheduler = Arc<dyn Fn(Task) + Send + Sync>;
pub type Clock = Rc<dyn Fn() -> i64>;

struct Slot {
    uri: String,
    listener: Listener,
    active: Cell<bool>,
}

/// Unsubscribes when dropped or reset.
#[derive(Default)]
pub struct Subscription {
    slot: Option<Rc<Slot>>,
}

impl Subscription {
    pub fn reset(&mut self) {
        if let Some(slot) = self.slot.take() {
            let weak = Rc::downgrade(&slot);
            drop(slot);
            let _ = STORE.try_with(|store| store.unsubscribe(&weak));
        }
    }

    pub fn active(&self) -> bool {
        self.slot.as_ref().is_some_and(|slot| slot.active.get())
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.reset();
    }
}

thread_local! {
    static STORE: SPUriStore = SPUriStore::new();
}

pub struct SPUriStore {
    entries: RefCell<BTreeMap<String, SPUriEntry>>,
    resolvers: RefCell<BTreeMap<String, Resolver>>,
    slots: RefCell<Vec<Rc<Slot>>>,
    directory: RefCell<String>,
    scheduler: RefCell<Scheduler>,
    clock: RefCell<Clock>,
    default_max_age: Cell<i64>,
    next_request_id: Cell<u64>,
}

fn system_now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn inline_scheduler() -> Scheduler {
    Arc::new(|task: Task| task())
}

fn fnv1a(text: &str) -> u64 {
    let mut hash: u64 = 14695981039346656037;
    for c in text.bytes() {
        hash ^= c as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    hash
}

/// Reversible, filesystem-safe encoding: bungalow:artist:1 -> bungalow%3Aartist%3A1
fn encode_file_name(uri: &str) -> String {
    let mut out = String::new();
    for c in uri.bytes() {
        if c.is_ascii_alphanumeric() || c == b'-' || c == b'_' || c == b'.' {
            out.push(c as char);
        } else {
            out.push_str(&format!("%{c:02X}"));
        }
    }
    if out.len() > MAX_FILE_NAME_LENGTH {
        out = format!("{}~{:016x}", &out[..MAX_FILE_NAME_LENGTH - 17], fnv1a(uri));
    }
    out + ".json"
}

impl Default for SPUriStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SPUriStore {
    pub fn new() -> Self {
        Self {
            entries: RefCell::default(),
            resolvers: RefCell::default(),
            slots: RefCell::default(),
            directory: RefCell::default(),
            scheduler: RefCell::new(inline_scheduler()),
            clock: RefCell::new(Rc::new(system_now)),
            default_max_age: Cell::new(5 * 60 * 1000),
            next_request_id: Cell::new(1),
        }
    }

    /// The store of the current (UI) thread: `SPUriStore::instance()`.
    pub fn with<R>(f: impl FnOnce(&SPUriStore) -> R) -> R {
        STORE.with(f)
    }

    /// Pure reducer: next state of one entry for an action addressed to it.
    pub fn reduce(mut state: SPUriEntry, action: &SPUriAction) -> SPUriEntry {
        match action.type_ {
            SPUriActionType::Hydrate => {
                // Never let a disk read clobber data that is already in memory.
                if state.status != SPUriStatus::Empty {
                    return state;
                }
                state.data = action.payload.clone();
                state.updated_at = action.timestamp;
                state.status = SPUriStatus::Cached;
            }
            SPUriActionType::Pending => {
                state.status = SPUriStatus::Loading;
                state.request_id = action.request_id;
                state.error.clear();
            }
            SPUriActionType::Fulfilled => {
                // Ignore responses from fetches that were superseded.
                if action.request_id != state.request_id {
                    return state;
                }
                state.data = action.payload.clone();
                state.updated_at = action.timestamp;
                state.status = SPUriStatus::Fresh;
                state.error.clear();
            }
            SPUriActionType::Rejected => {
                if action.request_id != state.request_id {
                    return state;
                }
                state.status = SPUriStatus::Error;
                state.error = action.error.clone();
            }
            SPUriActionType::Patch => {
                if state.data.is_object() && action.payload.is_object() {
                    merge_patch(&mut state.data, &action.payload);
                } else {
                    state.data = action.payload.clone();
                }
                state.updated_at = action.timestamp;
                if state.status == SPUriStatus::Empty {
                    state.status = SPUriStatus::Fresh;
                }
            }
            SPUriActionType::Invalidate => {
                if state.status == SPUriStatus::Fresh {
                    state.status = SPUriStatus::Cached;
                }
            }
            SPUriActionType::Evict => return SPUriEntry::default(),
        }
        state
    }

    pub fn dispatch(&self, mut action: SPUriAction) {
        if action.timestamp == 0 {
            action.timestamp = (self.clock.borrow())();
        }

        let previous = self.entries.borrow().get(&action.uri).cloned().unwrap_or_default();
        let next = Self::reduce(previous.clone(), &action);

        if action.type_ == SPUriActionType::Evict {
            self.entries.borrow_mut().remove(&action.uri);
            self.remove_cache_file(&action.uri);
        } else {
            self.entries.borrow_mut().insert(action.uri.clone(), next.clone());
        }

        if next == previous {
            return;
        }

        let data_changed = next.data != previous.data || next.updated_at != previous.updated_at;
        if data_changed && action.type_ != SPUriActionType::Hydrate && action.type_ != SPUriActionType::Evict {
            self.persist(&action.uri, &next);
        }

        self.notify(&action.uri, &next);
    }

    /// Current entry, hydrating it from the JSON cache on first access.
    pub fn get(&self, uri: &str) -> SPUriEntry {
        if !self.entries.borrow().contains_key(uri) {
            self.hydrate(uri);
        }
        self.entries.borrow().get(uri).cloned().unwrap_or_default()
    }

    /// Returns what is known about `uri` right now (possibly cached or empty)
    /// and starts a fetch through the matching resolver when the entry is
    /// missing, hydrated from disk, invalidated, failed, or older than
    /// `max_age_ms` (negative: use the default max age).
    pub fn resolve_uri(&self, uri: &str, max_age_ms: i64) -> SPUriEntry {
        let entry = self.get(uri);
        if uri.is_empty() || entry.status == SPUriStatus::Loading {
            return entry;
        }

        let max_age = if max_age_ms < 0 { self.default_max_age.get() } else { max_age_ms };
        let now = (self.clock.borrow())();
        if entry.status == SPUriStatus::Fresh && now - entry.updated_at <= max_age {
            return entry;
        }

        let Some(resolver) = self.find_resolver(uri) else {
            return entry;
        };

        let request_id = self.next_request_id.get();
        self.next_request_id.set(request_id + 1);
        let mut pending = SPUriAction::new(SPUriActionType::Pending, uri);
        pending.request_id = request_id;
        self.dispatch(pending);

        // Results may arrive on a worker thread; hop to the scheduler's thread
        // before touching the store.
        let scheduler = self.scheduler.borrow().clone();
        let resolve: ResolveCallback = {
            let scheduler = scheduler.clone();
            let uri = uri.to_string();
            Box::new(move |data: Value| {
                scheduler(Box::new(move || {
                    let mut action = SPUriAction::new(SPUriActionType::Fulfilled, &uri).with_payload(data);
                    action.request_id = request_id;
                    SPUriStore::with(|store| store.dispatch(action));
                }));
            })
        };
        let reject: RejectCallback = {
            let uri = uri.to_string();
            Box::new(move |error: String| {
                scheduler(Box::new(move || {
                    let mut action = SPUriAction::new(SPUriActionType::Rejected, &uri);
                    action.error = error;
                    action.request_id = request_id;
                    SPUriStore::with(|store| store.dispatch(action));
                }));
            })
        };

        resolver(uri, resolve, reject);
        self.get(uri)
    }

    /// Listen to changes of one URI, or of every URI when `uri` is empty.
    #[must_use]
    pub fn subscribe(&self, uri: &str, listener: Listener) -> Subscription {
        let slot = Rc::new(Slot { uri: uri.to_string(), listener, active: Cell::new(true) });
        self.slots.borrow_mut().push(slot.clone());
        Subscription { slot: Some(slot) }
    }

    fn unsubscribe(&self, slot: &Weak<Slot>) {
        if let Some(slot) = slot.upgrade() {
            slot.active.set(false);
            self.slots.borrow_mut().retain(|s| !Rc::ptr_eq(s, &slot));
        }
    }

    fn notify(&self, uri: &str, entry: &SPUriEntry) {
        // Listeners commonly re-render views, which subscribes new elements and
        // drops old ones, so iterate over a snapshot and skip slots that were
        // unsubscribed meanwhile.
        let snapshot = self.slots.borrow().clone();
        for slot in snapshot {
            if slot.active.get() && (slot.uri.is_empty() || slot.uri == uri) {
                (slot.listener)(uri, entry);
            }
        }
    }

    /// Register a resolver for URIs starting with `prefix`; longest prefix wins.
    pub fn register_resolver(&self, prefix: &str, resolver: Resolver) {
        self.resolvers.borrow_mut().insert(prefix.to_string(), resolver);
    }

    fn find_resolver(&self, uri: &str) -> Option<Resolver> {
        self.resolvers
            .borrow()
            .iter()
            .filter(|(prefix, _)| uri.starts_with(prefix.as_str()))
            .max_by_key(|(prefix, _)| prefix.len())
            .map(|(_, resolver)| resolver.clone())
    }

    /// Directory for the JSON cache; empty disables persistence.
    pub fn set_persistence_directory(&self, directory: &str) {
        *self.directory.borrow_mut() = directory.to_string();
        if !directory.is_empty() {
            let _ = fs::create_dir_all(directory);
        }
    }

    pub fn get_persistence_directory(&self) -> String {
        self.directory.borrow().clone()
    }

    pub fn set_scheduler(&self, scheduler: Option<Scheduler>) {
        *self.scheduler.borrow_mut() = scheduler.unwrap_or_else(inline_scheduler);
    }

    pub fn set_clock(&self, clock: Option<Clock>) {
        *self.clock.borrow_mut() = clock.unwrap_or_else(|| Rc::new(system_now));
    }

    pub fn set_default_max_age(&self, max_age_ms: i64) {
        self.default_max_age.set(max_age_ms);
    }

    /// Drop all in-memory state, listeners and resolvers (cache files stay).
    pub fn reset(&self) {
        for slot in self.slots.borrow_mut().drain(..) {
            slot.active.set(false);
        }
        self.entries.borrow_mut().clear();
        self.resolvers.borrow_mut().clear();
    }

    /// Path of the cache file for `uri` inside the persistence directory.
    pub fn cache_file_path(&self, uri: &str) -> String {
        let directory = self.directory.borrow();
        if directory.is_empty() {
            return String::new();
        }
        PathBuf::from(directory.as_str()).join(encode_file_name(uri)).to_string_lossy().into_owned()
    }

    fn hydrate(&self, uri: &str) {
        let path = self.cache_file_path(uri);
        if path.is_empty() {
            return;
        }
        let Ok(source) = fs::read_to_string(&path) else {
            return;
        };
        // A corrupt cache file is just a cache miss.
        let Ok(document) = serde_json::from_str::<Value>(&source) else {
            return;
        };
        // Hashed file names can collide; only accept the entity it was written for.
        if !document.is_object() || document.get("uri").and_then(Value::as_str) != Some(uri) {
            return;
        }
        let Some(data) = document.get("data") else {
            return;
        };
        let mut action = SPUriAction::new(SPUriActionType::Hydrate, uri).with_payload(data.clone());
        action.timestamp = document.get("updatedAt").and_then(Value::as_i64).unwrap_or(0);
        self.dispatch(action);
    }

    fn persist(&self, uri: &str, entry: &SPUriEntry) {
        let path = self.cache_file_path(uri);
        if path.is_empty() || !entry.has_data() {
            return;
        }
        let document = json!({
            "uri": uri,
            "updatedAt": entry.updated_at,
            "data": entry.data,
        });
        // Write to a temporary file and rename it so a crash never leaves a
        // half-written cache file behind.
        let temporary = format!("{path}.tmp");
        if fs::write(&temporary, document.to_string()).is_err() {
            return;
        }
        if fs::rename(&temporary, &path).is_err() {
            let _ = fs::remove_file(&temporary);
        }
    }

    fn remove_cache_file(&self, uri: &str) {
        let path = self.cache_file_path(uri);
        if !path.is_empty() {
            let _ = fs::remove_file(path);
        }
    }
}

/// RFC 7386 JSON merge patch, as `nlohmann::json::merge_patch`.
pub fn merge_patch(target: &mut Value, patch: &Value) {
    if let Value::Object(patch_map) = patch {
        if !target.is_object() {
            *target = Value::Object(Default::default());
        }
        let target_map = target.as_object_mut().unwrap();
        for (key, value) in patch_map {
            if value.is_null() {
                target_map.remove(key);
            } else {
                merge_patch(target_map.entry(key.clone()).or_insert(Value::Null), value);
            }
        }
    } else {
        *target = patch.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fulfilled_ignores_superseded_requests() {
        let mut pending = SPUriAction::new(SPUriActionType::Pending, "a");
        pending.request_id = 2;
        let state = SPUriStore::reduce(SPUriEntry::default(), &pending);
        let mut stale = SPUriAction::new(SPUriActionType::Fulfilled, "a").with_payload(json!(1));
        stale.request_id = 1;
        assert_eq!(SPUriStore::reduce(state.clone(), &stale).status, SPUriStatus::Loading);
        stale.request_id = 2;
        let fresh = SPUriStore::reduce(state, &stale);
        assert_eq!(fresh.status, SPUriStatus::Fresh);
        assert_eq!(fresh.data, json!(1));
    }

    #[test]
    fn resolve_notifies_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let store = SPUriStore::new();
        store.set_persistence_directory(dir.path().to_str().unwrap());
        store.register_resolver(
            "bungalow:artist:",
            Rc::new(|uri: &str, resolve: ResolveCallback, _| {
                resolve(json!({ "uri": uri, "name": "Test" }));
            }),
        );
        // resolve() dispatches through the thread's store, so use it here too.
        SPUriStore::with(|global| {
            global.reset();
            global.set_persistence_directory(dir.path().to_str().unwrap());
            global.register_resolver(
                "bungalow:artist:",
                Rc::new(|uri: &str, resolve: ResolveCallback, _| {
                    resolve(json!({ "uri": uri, "name": "Test" }));
                }),
            );
            let seen = Rc::new(Cell::new(0));
            let counter = seen.clone();
            let _subscription =
                global.subscribe("bungalow:artist:1", Rc::new(move |_, _| counter.set(counter.get() + 1)));
            let entry = global.resolve_uri("bungalow:artist:1", -1);
            assert_eq!(entry.status, SPUriStatus::Fresh);
            assert_eq!(entry.data["name"], "Test");
            assert!(seen.get() >= 2);
            assert!(std::path::Path::new(&global.cache_file_path("bungalow:artist:1")).exists());
        });
        // A fresh store hydrates from the cache file.
        let entry = store.get("bungalow:artist:1");
        assert_eq!(entry.status, SPUriStatus::Cached);
        assert_eq!(entry.data["name"], "Test");
    }

    #[test]
    fn long_names_are_hashed() {
        let name = encode_file_name(&"x".repeat(300));
        assert!(name.len() <= MAX_FILE_NAME_LENGTH + 5);
        assert!(name.contains('~'));
        assert_eq!(encode_file_name("bungalow:artist:1"), "bungalow%3Aartist%3A1.json");
    }
}
