//! Port of `SPApplicationContext`: what space elements may ask of the
//! application (playback state and controls).

use std::cell::RefCell;
use std::rc::Rc;

use crate::sp_observable::SPObservable;

pub trait SPApplicationContext {
    fn get_playback_events(&self) -> &SPObservable;
    fn has_current_playable(&self) -> bool;
    fn current_playable_uri(&self) -> String;
    fn get_track_title(&self) -> String;
    fn get_artist_name(&self) -> String;
    fn seek_to(&self, position_ms: i32);
    fn toggle_play_pause(&self);
    fn playback_prev(&self);
    fn playback_next(&self);
}

thread_local! {
    static CURRENT: RefCell<Option<Rc<dyn SPApplicationContext>>> = const { RefCell::new(None) };
}

/// `SPApplicationContext::current()`.
pub fn current() -> Option<Rc<dyn SPApplicationContext>> {
    CURRENT.with(|c| c.borrow().clone())
}

/// `SPApplicationContext::setCurrent()`.
pub fn set_current(context: Option<Rc<dyn SPApplicationContext>>) {
    CURRENT.with(|c| *c.borrow_mut() = context);
}
