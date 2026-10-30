//! Port of `SPMusicVisualizerCanvasElement` (bungalow/): the canvas-slot
//! adapter that hosts an [`SPMusicVisualizerElement`] as a flex child and
//! wires the [`SPPlayerCanvasImplementationElement`] lifecycle into it.

use std::cell::RefCell;
use std::rc::Rc;

use space::impl_element_base;
use space::sp_color::SPColor;
use space::sp_element::{construct, ElementRef, SPElement, SPElementBase, SPElementClass};
use space::sp_player_canvas_implementation_element::SPPlayerCanvasImplementationElement;
use space::sp_visualization::SPVisualizationParam;

use super::sp_music_visualizer_element::{SPMusicVisualizerElement, VisualizationRef};

#[derive(Default)]
pub struct SPMusicVisualizerCanvasElement {
    pub el: SPElement,
    visualizer: RefCell<Option<Rc<SPMusicVisualizerElement>>>,
}

impl_element_base!(SPMusicVisualizerCanvasElement, el);

impl SPMusicVisualizerCanvasElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.el.set_tag_name("visualizercanvas");
        rc.el.set_background_color(SPColor::new(0, 0, 0, 255));
        let this = rc.el.this();
        let visualizer = SPMusicVisualizerElement::new(Some(&this));
        visualizer.element().set_flex(1);
        this.append_child(&(visualizer.clone() as ElementRef));
        *rc.visualizer.borrow_mut() = Some(visualizer);
        rc
    }

    /// Swaps the active visualization; forwarded to the embedded element.
    pub fn set_visualization(&self, visualization: Option<VisualizationRef>) {
        if let Some(visualizer) = self.visualizer.borrow().as_ref() {
            visualizer.set_visualization(visualization);
        }
    }

    pub fn get_visualizer_element(&self) -> Option<Rc<SPMusicVisualizerElement>> {
        self.visualizer.borrow().clone()
    }
}

impl SPElementClass for SPMusicVisualizerCanvasElement {
    fn get_type(&self) -> String {
        "visualizercanvas".into()
    }
}

impl SPPlayerCanvasImplementationElement for SPMusicVisualizerCanvasElement {
    fn get_name(&self) -> String {
        "Visualizer".into()
    }

    fn as_element(&self) -> ElementRef {
        self.el.this()
    }

    fn on_activated(&self) {
        self.el.this().set_visible(true);
        if let Some(visualizer) = self.visualizer.borrow().as_ref() {
            visualizer.set_visible(true);
        }
    }

    fn on_deactivated(&self) {
        if let Some(visualizer) = self.visualizer.borrow().as_ref() {
            visualizer.set_visible(false);
        }
        self.el.this().set_visible(false);
    }

    // on_audio_frame is a no-op: SPMusicVisualizerElement fetches PCM/FFT
    // data on its own render timer. on_playback_changed needs nothing extra
    // either, since the visualizer subscribes to playback events itself.

    fn get_visualization_params(&self) -> Option<Vec<SPVisualizationParam>> {
        let visualization = self.visualizer.borrow().as_ref()?.get_visualization()?;
        let params = visualization.borrow_mut().params().clone();
        Some(params)
    }

    fn set_visualization_param(&self, index: usize, param: SPVisualizationParam) {
        let Some(visualization) = self.visualizer.borrow().as_ref().and_then(|v| v.get_visualization()) else { return };
        let mut visualization = visualization.borrow_mut();
        if let Some(slot) = visualization.params().get_mut(index) {
            *slot = param;
        }
    }
}
