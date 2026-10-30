//! Port of the Win32 `SPMusicVisualizerElement` (windows/): a stand-in for
//! the OpenGL music visualizer. It keeps the wx element's API but renders a
//! placeholder, because the GL canvas and PCM/FFT capture are not available
//! in this backend either.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use space::impl_element_base;
use space::sp_color::SPColor;
use space::sp_element::{construct, ElementRef, SPElement, SPElementClass};
use space::sp_event_args::SPBasicEventArgs;
use space::sp_object::SPObject;
use space::sp_visualization::SPVisualization;

pub type VisualizationRef = Rc<RefCell<dyn SPVisualization>>;

pub struct SPMusicVisualizerElement {
    pub el: SPElement,
    visualization: RefCell<Option<VisualizationRef>>,
    fps: Cell<i32>,
}

impl Default for SPMusicVisualizerElement {
    fn default() -> Self {
        Self { el: SPElement::default(), visualization: RefCell::new(None), fps: Cell::new(60) }
    }
}

impl_element_base!(SPMusicVisualizerElement, el);

impl SPMusicVisualizerElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        rc.el.set_tag_name("musicvisualizer");
        rc.el.set_background_color(SPColor::new(0, 0, 0, 255));
        rc
    }

    pub fn set_visualization(&self, visualization: Option<VisualizationRef>) {
        *self.visualization.borrow_mut() = visualization;
        let this = self.el.this();
        let args = SPBasicEventArgs::new(Some(this.clone()));
        self.el.this().notify("visualizationChanged", Some(&SPObject::Element(this)), &args);
    }

    pub fn get_visualization(&self) -> Option<VisualizationRef> {
        self.visualization.borrow().clone()
    }

    pub fn set_fps(&self, fps: i32) {
        self.fps.set(fps);
    }

    pub fn get_fps(&self) -> i32 {
        self.fps.get()
    }
}

impl SPElementClass for SPMusicVisualizerElement {
    fn get_type(&self) -> String {
        "musicvisualizer".into()
    }
}
