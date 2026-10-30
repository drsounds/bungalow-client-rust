//! Port of the Win32 `SPPlayerCanvasElement` (windows/): the runtime slot
//! for an [`SPPlayerCanvasImplementationElement`]. It forwards playback
//! events to the active implementation and drives `on_audio_frame()` from a
//! timer while playing. The Win32 `SetTimer` becomes a deadline that the
//! frame loop checks with [`SPPlayerCanvasElement::tick_all`].

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use bungalow::services::audio_bridge::{sp_audio_get_fft, sp_audio_get_pcm};

use space::impl_element_base;
use space::sp_application_context;
use space::sp_element::{construct, ElementRef, SPElementBase, SPElementClass};
use space::sp_event_args::{downcast_args, SPBasicEventArgs};
use space::sp_object::SPObject;
use space::sp_playback_event_args::SPPlaybackStateEventArgs;
use space::sp_player_canvas_implementation_element::SPPlayerCanvasImplementationElement;
use space::sp_vbox_element::SPVBoxElement;

const MAX_PCM: usize = 1024;
const MAX_FFT: usize = 512;

thread_local! {
    /// Canvases whose audio timer is running.
    static ACTIVE_TIMERS: RefCell<Vec<Weak<SPPlayerCanvasElement>>> = const { RefCell::new(Vec::new()) };
}

pub struct SPPlayerCanvasElement {
    pub vbox: SPVBoxElement,
    current: RefCell<Option<Rc<dyn SPPlayerCanvasImplementationElement>>>,
    /// When the audio timer fires next; None while it is stopped.
    next_frame: Cell<Option<Instant>>,
    audio_fps: Cell<i32>,
    me: RefCell<Weak<SPPlayerCanvasElement>>,
}

impl Default for SPPlayerCanvasElement {
    fn default() -> Self {
        Self {
            vbox: SPVBoxElement::default(),
            current: RefCell::new(None),
            next_frame: Cell::new(None),
            audio_fps: Cell::new(30),
            me: RefCell::new(Weak::new()),
        }
    }
}

impl_element_base!(SPPlayerCanvasElement, vbox);

impl SPPlayerCanvasElement {
    pub fn new(parent: Option<&ElementRef>) -> Rc<Self> {
        let rc = construct(Self::default(), parent);
        *rc.me.borrow_mut() = Rc::downgrade(&rc);
        rc.element().set_tag_name("playercanvas");
        if let Some(context) = sp_application_context::current() {
            let weak = Rc::downgrade(&rc);
            context.get_playback_events().add_listener("playback:changed", move |_, args| {
                if let (Some(this), Some(state)) = (weak.upgrade(), downcast_args::<SPPlaybackStateEventArgs>(args)) {
                    this.on_playback_changed(state);
                }
                0
            });
        }
        rc
    }

    fn interval(&self) -> Duration {
        Duration::from_millis(1000 / self.audio_fps.get().max(1) as u64)
    }

    fn start_timer(&self) {
        if self.next_frame.get().is_some() {
            return;
        }
        self.next_frame.set(Some(Instant::now() + self.interval()));
        let me = self.me.borrow().clone();
        ACTIVE_TIMERS.with(|t| t.borrow_mut().push(me));
    }

    fn stop_timer(&self) {
        if self.next_frame.take().is_none() {
            return;
        }
        let me = self.me.borrow().clone();
        ACTIVE_TIMERS.with(|t| t.borrow_mut().retain(|w| !w.ptr_eq(&me) && w.strong_count() > 0));
    }

    /// Fires the audio timers that are due. Returns how long until the next
    /// one is due, or None if none is running.
    pub fn tick_all() -> Option<Duration> {
        let canvases: Vec<Rc<SPPlayerCanvasElement>> =
            ACTIVE_TIMERS.with(|t| t.borrow().iter().filter_map(Weak::upgrade).collect());
        let now = Instant::now();
        let mut next: Option<Duration> = None;
        for canvas in canvases {
            let Some(due) = canvas.next_frame.get() else { continue };
            if due <= now {
                canvas.next_frame.set(Some(now + canvas.interval()));
                canvas.on_audio_timer_fired();
            }
            if let Some(due) = canvas.next_frame.get() {
                let wait = due.saturating_duration_since(now);
                next = Some(next.map_or(wait, |n| n.min(wait)));
            }
        }
        next
    }

    pub fn set_implementation(&self, implementation: Option<Rc<dyn SPPlayerCanvasImplementationElement>>) {
        let this = self.element().this();
        if let Some(current) = self.current.borrow_mut().take() {
            current.on_deactivated();
            self.element().remove_child(&current.as_element());
        }
        if let Some(implementation) = &implementation {
            let el = implementation.as_element();
            el.element().set_flex(1);
            this.append_child(&el);
            implementation.on_activated();
            this.pack();
            self.element().invalidate();
        }
        *self.current.borrow_mut() = implementation;
        let args = SPBasicEventArgs::new(Some(this.clone()));
        this.notify("implementationChanged", Some(&SPObject::Element(this.clone())), &args);
    }

    pub fn get_implementation(&self) -> Option<Rc<dyn SPPlayerCanvasImplementationElement>> {
        self.current.borrow().clone()
    }

    pub fn set_audio_fps(&self, fps: i32) {
        if fps <= 0 {
            return;
        }
        self.audio_fps.set(fps);
        if self.next_frame.get().is_some() {
            self.stop_timer();
            self.start_timer();
        }
    }

    pub fn get_audio_fps(&self) -> i32 {
        self.audio_fps.get()
    }

    pub fn on_audio_timer_fired(&self) {
        let Some(current) = self.get_implementation() else { return };
        let mut pcm = [0f32; MAX_PCM];
        let mut fft = [0f32; MAX_FFT];
        let pcm_count = sp_audio_get_pcm(&mut pcm).min(MAX_PCM);
        let fft_count = sp_audio_get_fft(&mut fft).min(MAX_FFT);
        current.on_audio_frame(&pcm[..pcm_count], &fft[..fft_count]);
    }

    fn on_playback_changed(&self, state: &SPPlaybackStateEventArgs) {
        if state.playing {
            self.start_timer();
        } else {
            self.stop_timer();
            if let Some(current) = self.get_implementation() {
                current.on_audio_frame(&[], &[]);
            }
        }
        if let Some(current) = self.get_implementation() {
            current.on_playback_changed(state.playing, state.position_ms, state.duration_ms);
        }
    }
}

impl Drop for SPPlayerCanvasElement {
    fn drop(&mut self) {
        if let Some(current) = self.current.get_mut().take() {
            current.on_deactivated();
        }
    }
}

impl SPElementClass for SPPlayerCanvasElement {
    fn get_type(&self) -> String {
        "playercanvas".into()
    }
    fn pack(&self) {
        SPVBoxElement::pack_vbox(self.element());
    }
}
