//! The part of `BungalowApp` that views in this crate call back into
//! (`BungalowApp::current()`). The client implements it and registers the
//! running app with [`set_current`].

use std::cell::RefCell;
use std::rc::Rc;

use crate::play_context::PlayContext;

pub trait BungalowAppHandle {
    fn set_developer_mode(&self, value: bool);
    fn set_full_width_address_bar(&self, value: bool);
    /// Starts playing `context` (replaces the play queue).
    fn play_context(&self, context: PlayContext);
}

thread_local! {
    static CURRENT: RefCell<Option<Rc<dyn BungalowAppHandle>>> = const { RefCell::new(None) };
}

pub fn current() -> Option<Rc<dyn BungalowAppHandle>> {
    CURRENT.with(|c| c.borrow().clone())
}

pub fn set_current(app: Option<Rc<dyn BungalowAppHandle>>) {
    CURRENT.with(|c| *c.borrow_mut() = app);
}
