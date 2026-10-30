//! Port of `LocalMediaService`: plays local audio files ("file://" URIs).
//! Playback goes through the [`super::audio_bridge`] functions, the
//! counterpart of the `sp_audio_*` C bridge (AVAudioPlayerBridge / WinAudio).

use std::cell::{Cell, RefCell};

use super::audio_bridge as audio;
use super::sp_media_service::SPMediaService;
use super::sp_service::SPService;

#[derive(Default)]
pub struct LocalMediaService {
    loaded_uri: RefCell<String>,
    playing: Cell<bool>,
}

/// Strips the "file://" prefix to obtain a plain filesystem path.
/// "file:///Users/foo/bar.mp3" → "/Users/foo/bar.mp3"
fn uri_to_path(uri: &str) -> &str {
    uri.strip_prefix("file://").unwrap_or(uri)
}

impl LocalMediaService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_playing(&self) -> bool {
        self.playing.get()
    }
}

impl SPService for LocalMediaService {
    fn accepts_uri(&self, uri: &str) -> bool {
        uri.starts_with("file://")
    }
    fn start_login(&self) {}
    fn logout(&self) {}
    fn get_id(&self) -> String {
        "local".into()
    }
    fn get_name(&self) -> String {
        "Local Media".into()
    }
    fn is_logged_in(&self) -> bool {
        true
    }
    fn get_logged_in_user(&self) -> String {
        String::new()
    }
}

impl SPMediaService for LocalMediaService {
    fn load(&self, uri: &str) {
        *self.loaded_uri.borrow_mut() = uri.to_string();
        self.playing.set(false);
        audio::sp_audio_load(uri_to_path(uri));
    }
    fn play(&self) {
        if self.loaded_uri.borrow().is_empty() {
            return;
        }
        self.playing.set(true);
        audio::sp_audio_play();
    }
    fn pause(&self) {
        self.playing.set(false);
        audio::sp_audio_pause();
    }
    fn stop(&self) {
        self.playing.set(false);
        audio::sp_audio_stop();
    }
    fn seek(&self, pos_ms: i32) {
        audio::sp_audio_seek(pos_ms as f64 / 1000.0);
    }
    fn get_duration_ms(&self) -> i32 {
        (audio::sp_audio_get_duration() * 1000.0) as i32
    }
    fn get_position_ms(&self) -> i32 {
        (audio::sp_audio_get_position() * 1000.0) as i32
    }
}
