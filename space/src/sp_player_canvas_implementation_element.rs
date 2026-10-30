//! Port of `SPPlayerCanvasImplementationElement`: an element that can fill
//! the player canvas (visualizations, video...).

use crate::sp_element::ElementRef;
use crate::sp_visualization::SPVisualizationParam;

#[allow(unused_variables)]
pub trait SPPlayerCanvasImplementationElement {
    fn get_name(&self) -> String;
    fn as_element(&self) -> ElementRef;
    fn on_activated(&self) {}
    fn on_deactivated(&self) {}
    fn on_playback_changed(&self, playing: bool, pos_ms: i32, dur_ms: i32) {}
    fn on_audio_frame(&self, pcm: &[f32], fft: &[f32]) {}
    fn get_visualization_params(&self) -> Option<Vec<SPVisualizationParam>> {
        None
    }
}
