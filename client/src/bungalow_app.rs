//! Port of `BungalowApp` (windows/): owns the database, download manager,
//! media services and playback queue, and builds the MainElement UI.
//!
//! It is the application context of the Space framework
//! ([`SPApplicationContext`]) and the handle the bungalow views use to reach
//! the app ([`BungalowAppHandle`]). The 500 ms position timer is driven by
//! [`BungalowApp::tick`], called from the frame loop.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use serde_json::Value;

use bungalow::bungalow_app_handle::{self, BungalowAppHandle};
use bungalow::database::Database;
use bungalow::download_manager::DownloadManager;
use bungalow::entity_resolvers::register_entity_resolvers;
use bungalow::models::sp_artist::SPArtist;
use bungalow::models::sp_db_connection::SPDBConnection;
use bungalow::models::sp_playable::SPPlayable;
use bungalow::models::sp_playable_track::SPPlayableTrack;
use bungalow::models::sp_recording::SPRecording;
use bungalow::models::sp_recording_artist::SPRecordingArtist;
use bungalow::models::sp_release::SPRelease;
use bungalow::models::sp_track::SPTrack;
use bungalow::platform_ui;
use bungalow::play_context::PlayContext;
use bungalow::play_queue_entry::PlayQueueEntry;
use bungalow::services::local_media_service::LocalMediaService;
use bungalow::services::sp_media_service::SPMediaService;
use bungalow::ui_dispatcher;
use bungalow::views::config_view::ConfigView;

use space::libspace::SPRectangle;
use space::sp_app_manager::SPAppManager;
use space::sp_application_context::{self, SPApplicationContext};
use space::sp_classic_skin::SPClassicSkin;
use space::sp_color::SPColor;
use space::sp_element::{ElementRef, SPElement, SPElementBase, SPElementClass, SP_COLOR_SCHEME_DARK};
use space::sp_frutiger_aero_skin::{AeroHeaderStyle, SPFrutigerAeroSkin};
use space::sp_message_type::SPMessageType;
use space::sp_modern_skin::SPModernSkin;
use space::sp_observable::SPObservable;
use space::sp_playback_event_args::SPPlaybackStateEventArgs;
use space::sp_skin::{SPButtonStyle, SPSkin};
use space::sp_uri_store::SPUriStore;
use space::sp_window_element::SPWindowElement;
use space::space::Space;

use crate::main_element::MainElement;
use crate::views::site_view::SiteView;

const POSITION_TIMER_INTERVAL: Duration = Duration::from_millis(500);

/// Playback and queue state (the C++ members after `m_developerMode`).
#[derive(Default)]
struct PlaybackState {
    /// Index into `queue_playables` of the playing entry.
    current: Option<usize>,
    queue_entries: Vec<PlayQueueEntry>,
    queue_playables: Vec<Option<SPPlayable>>,
    queue_index: i32,
    is_playing: bool,
    position_ms: i32,
    duration_ms: i32,
    track_title: String,
    artist_name: String,
    cover_art_path: String,
}

#[derive(Default)]
pub struct BungalowApp {
    space: RefCell<Option<Rc<Space>>>,
    window_element: RefCell<Option<Rc<SPWindowElement>>>,
    main: RefCell<Option<Rc<MainElement>>>,
    database: RefCell<Option<Rc<Database>>>,
    database_path: RefCell<String>,
    download_manager: RefCell<Option<Rc<DownloadManager>>>,
    media_services: RefCell<Vec<Rc<dyn SPMediaService>>>,
    developer_mode: Cell<bool>,
    state: RefCell<PlaybackState>,
    playback_events: SPObservable,
    /// When the position timer last fired; None while it is stopped.
    timer: Cell<Option<Instant>>,
}

/// The settings `initialize` reads from settings.json, with their defaults.
struct StartupSettings {
    color_scheme: i16,
    glossy: bool,
    skin_type: String,
    full_width_address_bar: bool,
    aero_hue: f32,
    aero_saturation: f32,
    aero_lightness: f32,
    aero_header_style: i32,
    button_style: i32,
}

impl Default for StartupSettings {
    fn default() -> Self {
        Self {
            color_scheme: SP_COLOR_SCHEME_DARK,
            glossy: false,
            skin_type: "modern".into(),
            full_width_address_bar: true,
            aero_hue: 200.0,
            aero_saturation: 0.0,
            aero_lightness: 0.50,
            aero_header_style: AeroHeaderStyle::Glossy as i32,
            button_style: SPButtonStyle::Aqua as i32,
        }
    }
}

fn uri_basename(uri: &str) -> String {
    let name = uri.rsplit(['/', '\\']).next().unwrap_or(uri);
    match name.rfind('.') {
        Some(dot) => name[..dot].to_string(),
        None => name.to_string(),
    }
}

/// The resources directory (with views/app.xml) is next to the executable
/// or in an ancestor directory, e.g. the repo root when running from target/.
fn find_resources_dir() -> PathBuf {
    let mut candidates = Vec::new();
    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)) {
        candidates.push(exe_dir);
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd);
    }
    for start in &candidates {
        for dir in start.ancestors() {
            if dir.join("views").join("app.xml").exists() {
                return dir.to_path_buf();
            }
        }
    }
    candidates.into_iter().next().unwrap_or_else(|| PathBuf::from("."))
}

impl BungalowApp {
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }

    /// Builds the UI inside `window_element` and loads settings. Returns the
    /// MainElement. `self_rc` is this app, registered as the current
    /// application context.
    pub fn initialize(
        self: &Rc<Self>,
        space: &Rc<Space>,
        window_element: &Rc<SPWindowElement>,
    ) -> Result<Rc<MainElement>, String> {
        sp_application_context::set_current(Some(self.clone() as Rc<dyn SPApplicationContext>));
        bungalow_app_handle::set_current(Some(self.clone() as Rc<dyn BungalowAppHandle>));

        *self.space.borrow_mut() = Some(space.clone());
        *self.window_element.borrow_mut() = Some(window_element.clone());

        let weak = Rc::downgrade(self);
        SPElement::set_xul_error_handler(Some(Rc::new(move |msg: &str| {
            if let Some(app) = weak.upgrade() {
                app.show_error(msg);
            }
        })));

        self.media_services.borrow_mut().push(Rc::new(LocalMediaService::new()));

        let resources_dir = find_resources_dir();
        // Relative XUL paths resolve against the resources directory.
        std::env::set_current_dir(&resources_dir)
            .map_err(|e| format!("Cannot enter the resources directory {}: {e}", resources_dir.display()))?;

        // Discover app bundles (Apps/, ~/Bungalow[/Apps], ~/Documents/Bungalow[/Apps])
        // before creating the skin, which may come from one of them.
        let resources = resources_dir.to_string_lossy().replace('\\', "/");
        SPAppManager::with(|apps| {
            apps.add_default_search_paths(&resources, &platform_ui::home_dir(), &platform_ui::documents_dir());
            apps.scan();
        });

        // Use an existing bundled database, otherwise create one in the user data dir.
        let mut db_path = resources_dir.join("bungalow.db");
        if !db_path.exists() {
            let data_dir = PathBuf::from(platform_ui::user_data_dir());
            let _ = std::fs::create_dir_all(&data_dir);
            db_path = data_dir.join("bungalow.db");
        }
        *self.database_path.borrow_mut() = db_path.to_string_lossy().replace('\\', "/");
        let database = Rc::new(Database::new(&self.database_path.borrow())?);
        *self.database.borrow_mut() = Some(database.clone());
        *self.download_manager.borrow_mut() = Some(DownloadManager::new(Some(database.clone())));

        // Global URI entity store: views render the JSON-cached entity at once
        // and update when the resolver delivers fresh data.
        SPUriStore::with(|store| {
            store.set_persistence_directory(&format!("{}/cache/uris", platform_ui::user_data_dir()));
            store.set_scheduler(Some(std::sync::Arc::new(|task| ui_dispatcher::post_to_ui(task))));
            register_entity_resolvers(store, database.clone());
        });

        let window_ref: ElementRef = window_element.clone();
        let main = MainElement::new(Some(&window_ref));
        main.element().this().set_id("main");
        *self.main.borrow_mut() = Some(main.clone());

        let settings = self.read_startup_settings();

        let skin: Rc<dyn SPSkin> = if settings.skin_type == "frutiger-aero" {
            let aero = SPFrutigerAeroSkin::new();
            aero.set_header_style(AeroHeaderStyle::from_i32(settings.aero_header_style));
            Rc::new(aero)
        } else if settings.skin_type == "classic" {
            Rc::new(SPClassicSkin::new())
        } else {
            // "modern" and installed skin bundles are skin.xml skins; fall
            // back to procedural drawing if the bundle is missing.
            SPAppManager::create_skin(&settings.skin_type).unwrap_or_else(|| Rc::new(SPModernSkin::new()))
        };
        skin.set_glossy(settings.glossy);
        skin.set_button_style(SPButtonStyle::from_i32(settings.button_style));
        // The Now Playing colour sliders drive hue/saturation/lightness for
        // every skin, so restore them regardless of skin type.
        skin.base().set_hue(settings.aero_hue);
        skin.base().set_saturation(settings.aero_saturation);
        skin.base().set_lightness(settings.aero_lightness);
        let window = window_element.element();
        window.set_skin(Some(skin));
        window.set_color_scheme(settings.color_scheme);
        window.set_tint_color(Some(SPColor::from_hsl(
            settings.aero_hue,
            settings.aero_saturation,
            settings.aero_lightness,
            255,
        )));
        main.set_full_width_address_bar(settings.full_width_address_bar);
        window_ref.append_child(&(main.clone() as ElementRef));
        Ok(main)
    }

    fn read_startup_settings(&self) -> StartupSettings {
        let mut s = StartupSettings::default();
        let Ok(text) = std::fs::read_to_string(ConfigView::get_settings_path()) else { return s };
        let Ok(Value::Object(obj)) = serde_json::from_str::<Value>(&text) else { return s };
        if let Some(v) = obj.get("colorScheme").and_then(Value::as_i64) {
            s.color_scheme = v as i16;
        }
        if let Some(v) = obj.get("glossy").and_then(Value::as_bool) {
            s.glossy = v;
        }
        if let Some(v) = obj.get("developerMode").and_then(Value::as_bool) {
            self.developer_mode.set(v);
        }
        if let Some(v) = obj.get("skinType").and_then(Value::as_str) {
            s.skin_type = v.to_string();
        }
        if let Some(v) = obj.get("fullWidthAddressBar").and_then(Value::as_bool) {
            s.full_width_address_bar = v;
        }
        if let Some(v) = obj.get("aeroHue").and_then(Value::as_f64) {
            s.aero_hue = v as f32;
        }
        if let Some(v) = obj.get("aeroSaturation").and_then(Value::as_f64) {
            s.aero_saturation = v as f32;
        }
        if let Some(v) = obj.get("aeroLightness").and_then(Value::as_f64) {
            s.aero_lightness = v as f32;
        }
        if let Some(v) = obj.get("aeroHeaderStyle").and_then(Value::as_i64) {
            s.aero_header_style = v as i32;
        }
        if let Some(v) = obj.get("buttonStyle").and_then(Value::as_i64) {
            s.button_style = v as i32;
        }
        s
    }

    pub fn shutdown(&self) {
        sp_application_context::set_current(None);
        bungalow_app_handle::set_current(None);
        SPElement::set_xul_error_handler(None);
        self.stop_position_timer();
        self.media_services.borrow_mut().clear();
        let mut state = self.state.borrow_mut();
        state.queue_playables.clear();
        state.queue_entries.clear();
        state.current = None;
    }

    // ── Position timer ────────────────────────────────────────────────────

    fn start_position_timer(&self) {
        if self.timer.get().is_none() {
            self.timer.set(Some(Instant::now()));
        }
    }

    fn stop_position_timer(&self) {
        self.timer.set(None);
    }

    /// Fires the position timer when it is due. Returns how long until it
    /// is due again, so the frame loop can schedule a repaint, or None
    /// while it is stopped.
    pub fn tick(&self) -> Option<Duration> {
        let started = self.timer.get()?;
        let elapsed = started.elapsed();
        if elapsed < POSITION_TIMER_INTERVAL {
            return Some(POSITION_TIMER_INTERVAL - elapsed);
        }
        self.timer.set(Some(Instant::now()));
        self.on_position_timer();
        self.timer.get().map(|_| POSITION_TIMER_INTERVAL)
    }

    pub fn on_position_timer(&self) {
        let service = {
            let state = self.state.borrow();
            if !state.is_playing {
                return;
            }
            let Some(current) = state.current else { return };
            state.queue_playables[current].as_ref().and_then(|p| p.service.clone())
        };
        let next = {
            let mut state = self.state.borrow_mut();
            match &service {
                Some(svc) => {
                    state.position_ms = svc.get_position_ms();
                    if state.duration_ms == 0 {
                        state.duration_ms = svc.get_duration_ms();
                    }
                }
                None => state.position_ms += 500,
            }
            if state.duration_ms > 0 && state.position_ms >= state.duration_ms {
                state.position_ms = state.duration_ms;
                true
            } else {
                false
            }
        };
        if next {
            self.playback_next();
        } else {
            self.dispatch_playback_changed();
        }
    }

    // ── Accessors ─────────────────────────────────────────────────────────

    pub fn get_database_path(&self) -> String {
        self.database_path.borrow().clone()
    }

    pub fn get_media_services(&self) -> Vec<Rc<dyn SPMediaService>> {
        self.media_services.borrow().clone()
    }

    pub fn get_media_service_for_uri(&self, uri: &str) -> Option<Rc<dyn SPMediaService>> {
        self.media_services.borrow().iter().find(|s| s.accepts_uri(uri)).cloned()
    }

    pub fn is_playing(&self) -> bool {
        self.state.borrow().is_playing
    }

    pub fn get_position_ms(&self) -> i32 {
        self.state.borrow().position_ms
    }

    pub fn get_duration_ms(&self) -> i32 {
        self.state.borrow().duration_ms
    }

    pub fn get_cover_art_path(&self) -> String {
        self.state.borrow().cover_art_path.clone()
    }

    pub fn get_main(&self) -> Option<Rc<MainElement>> {
        self.main.borrow().clone()
    }

    // ── Develop menu ──────────────────────────────────────────────────────

    pub fn show_web_inspector(&self) {
        if let Some(view) = SiteView::get_active_instance() {
            view.show_inspector();
        }
    }

    pub fn reload_web_page(&self) {
        if let Some(view) = SiteView::get_active_instance() {
            view.reload();
        }
    }

    // ── Playback ──────────────────────────────────────────────────────────

    fn current_service(&self) -> Option<Rc<dyn SPMediaService>> {
        let state = self.state.borrow();
        let current = state.current?;
        state.queue_playables[current].as_ref().and_then(|p| p.service.clone())
    }

    fn stop_current_playback(&self) {
        self.stop_position_timer();
        if let Some(service) = self.current_service() {
            service.stop();
        }
        let mut state = self.state.borrow_mut();
        state.current = None;
        state.is_playing = false;
        state.position_ms = 0;
    }

    fn play_from_index(&self, mut idx: i32) {
        let service = {
            let mut state = self.state.borrow_mut();
            let size = state.queue_entries.len() as i32;
            while idx < size && !state.queue_entries[idx as usize].playable {
                idx += 1;
            }
            if idx >= size || idx < 0 {
                state.is_playing = false;
                drop(state);
                self.dispatch_playback_changed();
                return;
            }
            state.queue_index = idx;
            let i = idx as usize;
            let Some(service) = state.queue_playables[i].as_ref().and_then(|p| p.service.clone()) else {
                state.is_playing = false;
                drop(state);
                self.dispatch_playback_changed();
                return;
            };
            let entry = state.queue_entries[i].clone();
            state.track_title =
                if entry.display_title.is_empty() { uri_basename(&entry.uri) } else { entry.display_title.clone() };
            state.artist_name = entry.display_artist.clone();
            state.cover_art_path = entry.cover_art_path.clone();
            service.load(&entry.uri);
            service.play();
            state.current = Some(i);
            state.is_playing = true;
            state.position_ms = 0;
            service
        };
        self.state.borrow_mut().duration_ms = service.get_duration_ms();
        self.dispatch_playback_changed();
        self.start_position_timer();
    }

    pub fn play_track_id(&self, track_id: i64) {
        let Some(db) = self.database.borrow().clone() else { return };
        let conn = SPDBConnection::new(&db);

        let Some(pt) = SPPlayableTrack::find_by_track(&conn, track_id).into_iter().next() else {
            self.show_error("An error occurred");
            return;
        };
        let Some(playable) = SPPlayable::find(&conn, pt.playable_id) else {
            self.show_error("An error occurred");
            return;
        };

        let (mut title, mut artist, mut cover_art) = (String::new(), String::new(), String::new());
        if let Some(track) = SPTrack::find(&conn, track_id) {
            if let Some(recording) = SPRecording::find(&conn, &track.recording_id) {
                title = recording.name.clone();
                if !recording.version.is_empty() {
                    title += &format!(" ({})", recording.version);
                }
            }
            for ra in SPRecordingArtist::find_by_recording(&conn, &track.recording_id) {
                if ra.is_primary_artist || artist.is_empty() {
                    if let Some(a) = SPArtist::find(&conn, &ra.artist_id) {
                        artist = a.name.clone();
                    }
                    if ra.is_primary_artist {
                        break;
                    }
                }
            }
            if !track.release_id.is_empty() {
                if let Some(release) = SPRelease::find(&conn, &track.release_id) {
                    cover_art = release.cover_art.clone();
                }
            }
        }

        let entry = PlayQueueEntry {
            display_title: title,
            display_artist: artist,
            cover_art_path: cover_art,
            uri: playable.uri.clone(),
            playable: true,
        };
        self.play_context(PlayContext { entries: vec![entry], start_index: 0 });
    }

    pub fn play_uri(&self, uri: &str) {
        let entry = PlayQueueEntry {
            display_title: uri_basename(uri),
            uri: uri.to_string(),
            playable: true,
            ..Default::default()
        };
        self.play_context(PlayContext { entries: vec![entry], start_index: 0 });
    }

    fn dispatch_playback_changed(&self) {
        let args = {
            let state = self.state.borrow();
            SPPlaybackStateEventArgs::new(state.is_playing, state.position_ms, state.duration_ms)
        };
        self.playback_events.dispatch_event("playback:changed", &args, None);
    }

    fn invalidate_window(&self) {
        let Some(window) = self.window_element.borrow().clone() else { return };
        window.pack();
        if let Some(gc) = window.create_graphics() {
            window.invalidate_region(SPRectangle::new(0, 0, gc.get_width(), gc.get_height()));
        }
    }
}

impl SPApplicationContext for BungalowApp {
    fn get_playback_events(&self) -> &SPObservable {
        &self.playback_events
    }

    fn has_current_playable(&self) -> bool {
        self.state.borrow().current.is_some()
    }

    fn current_playable_uri(&self) -> String {
        let state = self.state.borrow();
        state.current.and_then(|i| state.queue_playables[i].as_ref().map(|p| p.uri.clone())).unwrap_or_default()
    }

    fn get_track_title(&self) -> String {
        self.state.borrow().track_title.clone()
    }

    fn get_artist_name(&self) -> String {
        self.state.borrow().artist_name.clone()
    }

    fn seek_to(&self, position_ms: i32) {
        let Some(service) = ({
            let state = self.state.borrow();
            state.current.map(|i| state.queue_playables[i].as_ref().and_then(|p| p.service.clone()))
        }) else {
            return;
        };
        self.state.borrow_mut().position_ms = position_ms;
        if let Some(service) = service {
            service.seek(position_ms);
        }
        self.dispatch_playback_changed();
    }

    fn toggle_play_pause(&self) {
        if self.state.borrow().current.is_none() {
            return;
        }
        let playing = {
            let mut state = self.state.borrow_mut();
            state.is_playing = !state.is_playing;
            state.is_playing
        };
        if let Some(service) = self.current_service() {
            if playing {
                service.play();
            } else {
                service.pause();
            }
        }
        if playing {
            self.start_position_timer();
        } else {
            self.stop_position_timer();
        }
        self.dispatch_playback_changed();
    }

    fn playback_prev(&self) {
        if self.state.borrow().position_ms > 3000 {
            self.seek_to(0);
            return;
        }
        let queue_index = self.state.borrow().queue_index;
        if queue_index < 0 {
            return;
        }
        self.stop_current_playback();
        let prev = {
            let state = self.state.borrow();
            let mut prev = queue_index - 1;
            while prev >= 0 && !state.queue_entries[prev as usize].playable {
                prev -= 1;
            }
            prev
        };
        self.play_from_index(if prev >= 0 { prev } else { queue_index });
    }

    fn playback_next(&self) {
        self.stop_current_playback();
        let (next, size) = {
            let state = self.state.borrow();
            (state.queue_index + 1, state.queue_entries.len() as i32)
        };
        if next < size {
            self.play_from_index(next);
        } else {
            self.dispatch_playback_changed();
        }
    }
}

impl BungalowAppHandle for BungalowApp {
    fn get_database(&self) -> Option<Rc<Database>> {
        self.database.borrow().clone()
    }

    fn get_download_manager(&self) -> Option<Rc<DownloadManager>> {
        self.download_manager.borrow().clone()
    }

    fn set_developer_mode(&self, value: bool) {
        self.developer_mode.set(value);
    }

    fn is_developer_mode(&self) -> bool {
        self.developer_mode.get()
    }

    fn set_full_width_address_bar(&self, value: bool) {
        let Some(main) = self.main.borrow().clone() else { return };
        main.set_full_width_address_bar(value);
        self.invalidate_window();
    }

    fn play_context(&self, mut context: PlayContext) {
        self.stop_current_playback();
        let services = self.get_media_services();
        let mut playables = Vec::with_capacity(context.entries.len());
        for entry in &mut context.entries {
            let mut playable = None;
            if entry.playable && !entry.uri.is_empty() {
                let mut p = SPPlayable::new(&entry.uri);
                p.resolve_service(&services);
                if p.service.is_some() {
                    playable = Some(p);
                } else {
                    entry.playable = false;
                }
            }
            playables.push(playable);
        }
        {
            let mut state = self.state.borrow_mut();
            state.queue_playables = playables;
            state.queue_entries = context.entries;
            state.queue_index = -1;
        }
        self.playback_events.dispatch_event("queue:changed", &space::sp_event_args::SPBasicEventArgs::new(None), None);
        self.play_from_index(context.start_index as i32);
    }

    fn play_queue_at_index(&self, index: i32) {
        {
            let state = self.state.borrow();
            if index < 0 || index >= state.queue_entries.len() as i32 || !state.queue_entries[index as usize].playable {
                return;
            }
        }
        self.stop_current_playback();
        self.play_from_index(index);
    }

    fn get_queue_entries(&self) -> Vec<PlayQueueEntry> {
        self.state.borrow().queue_entries.clone()
    }

    fn get_queue_index(&self) -> i32 {
        self.state.borrow().queue_index
    }

    fn navigate(&self, uri: &str) {
        if let Some(main) = self.main.borrow().clone() {
            main.navigate(uri);
        }
    }

    fn show_error(&self, message: &str) {
        if let Some(main) = self.main.borrow().clone() {
            main.show_message(SPMessageType::Error, message);
        }
    }
}
