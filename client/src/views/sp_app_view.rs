//! Port of `SPAppView` (wxwidgets/app/): the web-view canvas implementation
//! for [`super::sp_player_canvas_element::SPPlayerCanvasElement`].
//!
//! Presents a full SiteView with no navigation bar. While active, audio
//! data (PCM + FFT) is pushed each frame into a
//! `window.bungalowAudioFrame({pcm:[...], fft:[...]})` callback, and playback
//! state changes into `window.bungalowPlaybackChanged({...})`. Pages hosted
//! here register these callbacks to drive their own visualizations or
//! lyrics/metadata displays.

use std::fmt::Write;
use std::rc::Rc;

use space::impl_element_base;
use space::sp_element::{construct, ElementRef, SPElementBase};
use space::sp_player_canvas_implementation_element::SPPlayerCanvasImplementationElement;

use super::site_view::{SiteView, SiteViewClass};

/// Arrays are capped to keep the serialised payload small.
const MAX_SAMPLES: usize = 256;

#[derive(Default)]
pub struct SPAppView {
    pub site: SiteView,
    uri_prefix: String,
}

impl_element_base!(SPAppView, site);

impl SPAppView {
    /// URI prefix: "bungalow:player:app:" by default.
    pub fn new(parent: Option<&ElementRef>, uri_prefix: Option<&str>) -> Rc<Self> {
        let uri_prefix = uri_prefix.unwrap_or("bungalow:player:app:").to_string();
        let rc = construct(Self { uri_prefix: uri_prefix.clone(), ..Default::default() }, parent);
        let outer: Rc<dyn SiteViewClass> = rc.clone();
        rc.site.init_site(&outer, &uri_prefix, false, false, false);
        rc
    }
}

impl SiteViewClass for SPAppView {
    fn site(&self) -> &SiteView {
        &self.site
    }
    fn accepts_site_uri(&self, uri: &str) -> bool {
        uri.starts_with(&self.uri_prefix)
    }
}

site_view_subclass!(SPAppView, "appview");

fn push_floats(js: &mut String, values: &[f32]) {
    for (i, v) in values.iter().take(MAX_SAMPLES).enumerate() {
        if i > 0 {
            js.push(',');
        }
        let _ = write!(js, "{v:.4}");
    }
}

impl SPPlayerCanvasImplementationElement for SPAppView {
    fn get_name(&self) -> String {
        "App View".into()
    }

    fn as_element(&self) -> ElementRef {
        self.element().this()
    }

    fn on_activated(&self) {
        self.site.set_visible_site(true);
    }

    fn on_deactivated(&self) {
        self.site.set_visible_site(false);
    }

    fn on_playback_changed(&self, playing: bool, pos_ms: i32, dur_ms: i32) {
        self.site.run_script(&format!(
            "if(typeof window.bungalowPlaybackChanged==='function'){{\
             window.bungalowPlaybackChanged({{playing:{playing},positionMs:{pos_ms},durationMs:{dur_ms}}});}}"
        ));
    }

    /// Injects the latest PCM and FFT frame as JSON into the hosted page.
    fn on_audio_frame(&self, pcm: &[f32], fft: &[f32]) {
        let mut js = String::with_capacity(64 + (pcm.len().min(MAX_SAMPLES) + fft.len().min(MAX_SAMPLES)) * 10);
        js.push_str("if(typeof window.bungalowAudioFrame==='function'){window.bungalowAudioFrame({pcm:[");
        push_floats(&mut js, pcm);
        js.push_str("],fft:[");
        push_floats(&mut js, fft);
        js.push_str("]});}");
        self.site.run_script(&js);
    }
}
