//! Port of `SPMediaService`: a service that provides audio/media playback.

use super::sp_service::SPService;

/// The transport-control interface every media backend implements.
/// Methods take `&self` because services are shared (`Rc<dyn SPMediaService>`);
/// implementations keep their state in cells.
pub trait SPMediaService: SPService {
    /// Begins or resumes playback of the loaded URI.
    fn play(&self);

    /// Pauses playback without discarding the position.
    fn pause(&self);

    /// Loads a URI and prepares it for playback. Does not start playing.
    fn load(&self, uri: &str);

    /// Stops playback and resets the position to zero.
    fn stop(&self);

    /// Seeks to `pos` milliseconds from the start.
    fn seek(&self, pos: i32);

    /// Duration of the loaded track in milliseconds, 0 if unknown.
    fn get_duration_ms(&self) -> i32 {
        0
    }

    /// Playback position in milliseconds, 0 if unknown.
    fn get_position_ms(&self) -> i32 {
        0
    }
}
