//! The `sp_audio_*` bridge used by [`super::local_media_service`]. The C++
//! client implements it with AVAudioEngine (macOS) and MCI (Windows); here it
//! uses rodio when the `audio` feature is enabled. Without the feature a
//! silent clock-based player keeps the transport state, so the UI behaves
//! the same but nothing is heard.
//!
//! The bridge is per thread, like the C++ globals that were only touched
//! from the UI thread.

use std::cell::RefCell;
use std::time::{Duration, Instant};

#[derive(Default)]
struct Clock {
    /// Seek baseline: position = offset + elapsed since last play/seek.
    offset: f64,
    started: Option<Instant>,
}

impl Clock {
    fn position(&self) -> f64 {
        self.offset + self.started.map(|s| s.elapsed().as_secs_f64()).unwrap_or(0.0)
    }
}

#[derive(Default)]
struct Player {
    loaded: bool,
    duration: f64,
    clock: Clock,
    #[cfg(feature = "audio")]
    output: Option<(rodio::OutputStream, rodio::Sink)>,
}

thread_local! {
    static PLAYER: RefCell<Player> = RefCell::new(Player::default());
}

/// Loads a local file path (not a URI). Returns true on success.
pub fn sp_audio_load(path: &str) -> bool {
    PLAYER.with(|p| {
        let mut p = p.borrow_mut();
        *p = Player::default();
        if !std::path::Path::new(path).is_file() {
            return false;
        }
        p.loaded = true;
        #[cfg(feature = "audio")]
        {
            use rodio::Source;
            let Ok(file) = std::fs::File::open(path) else { return false };
            let Ok(decoder) = rodio::Decoder::new(std::io::BufReader::new(file)) else { return false };
            p.duration = decoder.total_duration().map(|d| d.as_secs_f64()).unwrap_or(0.0);
            let Ok((stream, handle)) = rodio::OutputStream::try_default() else { return false };
            let Ok(sink) = rodio::Sink::try_new(&handle) else { return false };
            sink.pause();
            sink.append(decoder);
            p.output = Some((stream, sink));
        }
        true
    })
}

pub fn sp_audio_play() {
    PLAYER.with(|p| {
        let mut p = p.borrow_mut();
        if !p.loaded || p.clock.started.is_some() {
            return;
        }
        p.clock.started = Some(Instant::now());
        #[cfg(feature = "audio")]
        if let Some((_, sink)) = &p.output {
            sink.play();
        }
    });
}

pub fn sp_audio_pause() {
    PLAYER.with(|p| {
        let mut p = p.borrow_mut();
        p.clock.offset = p.clock.position();
        p.clock.started = None;
        #[cfg(feature = "audio")]
        if let Some((_, sink)) = &p.output {
            sink.pause();
        }
    });
}

pub fn sp_audio_stop() {
    PLAYER.with(|p| {
        let mut p = p.borrow_mut();
        p.clock = Clock::default();
        #[cfg(feature = "audio")]
        if let Some((_, sink)) = &p.output {
            sink.pause();
            let _ = sink.try_seek(Duration::ZERO);
        }
    });
}

pub fn sp_audio_seek(seconds: f64) {
    PLAYER.with(|p| {
        let mut p = p.borrow_mut();
        let mut target = seconds.max(0.0);
        if p.duration > 0.0 {
            target = target.min(p.duration);
        }
        p.clock.offset = target;
        if p.clock.started.is_some() {
            p.clock.started = Some(Instant::now());
        }
        #[cfg(feature = "audio")]
        if let Some((_, sink)) = &p.output {
            let _ = sink.try_seek(Duration::from_secs_f64(target));
        }
        let _ = Duration::ZERO;
    });
}

/// Total duration in seconds, 0.0 if unknown or nothing is loaded.
pub fn sp_audio_get_duration() -> f64 {
    PLAYER.with(|p| p.borrow().duration)
}

pub fn sp_audio_get_position() -> f64 {
    PLAYER.with(|p| {
        let p = p.borrow();
        #[cfg(feature = "audio")]
        if let Some((_, sink)) = &p.output {
            return sink.get_pos().as_secs_f64();
        }
        let position = p.clock.position();
        if p.duration > 0.0 {
            position.min(p.duration)
        } else {
            position
        }
    })
}

pub fn sp_audio_is_playing() -> bool {
    PLAYER.with(|p| p.borrow().clock.started.is_some())
}

/// Copies the latest PCM frame into `out`. Returns the sample count.
/// No backend exposes samples yet, so visualizers receive silence (as with MCI).
pub fn sp_audio_get_pcm(_out: &mut [f32]) -> usize {
    0
}

/// Copies the latest FFT magnitudes into `out`. Returns the bin count.
pub fn sp_audio_get_fft(_out: &mut [f32]) -> usize {
    0
}
