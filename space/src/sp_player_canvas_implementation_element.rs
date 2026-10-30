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
    /// A snapshot of the active visualization's parameters. The C++ code
    /// handed out a pointer to the live vector; here changes go back through
    /// [`Self::set_visualization_param`].
    fn get_visualization_params(&self) -> Option<Vec<SPVisualizationParam>> {
        None
    }
    /// Replaces the parameter at `index` (e.g. a slider moved).
    fn set_visualization_param(&self, index: usize, param: SPVisualizationParam) {}
}
