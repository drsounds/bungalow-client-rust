//! Port of `UiDispatcher`: runs closures posted from worker threads on the
//! UI thread. The client drains the queue once per frame with [`run_pending`]
//! and installs a waker (e.g. a repaint request) with [`init_ui_dispatcher`].

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock};

type Job = Box<dyn FnOnce() + Send>;
type Waker = Box<dyn Fn() + Send + Sync>;

struct Dispatcher {
    sender: Mutex<Sender<Job>>,
    receiver: Mutex<Receiver<Job>>,
    waker: Mutex<Option<Waker>>,
}

fn dispatcher() -> &'static Dispatcher {
    static DISPATCHER: OnceLock<Dispatcher> = OnceLock::new();
    DISPATCHER.get_or_init(|| {
        let (sender, receiver) = channel();
        Dispatcher { sender: Mutex::new(sender), receiver: Mutex::new(receiver), waker: Mutex::new(None) }
    })
}

/// Sets the function that wakes the UI thread when work is posted.
pub fn init_ui_dispatcher(waker: Option<Waker>) {
    *dispatcher().waker.lock().unwrap() = waker;
}

/// Queues `f` to run on the UI thread. Safe to call from any thread.
pub fn post_to_ui(f: impl FnOnce() + Send + 'static) {
    let d = dispatcher();
    if d.sender.lock().unwrap().send(Box::new(f)).is_ok() {
        if let Some(waker) = d.waker.lock().unwrap().as_ref() {
            waker();
        }
    }
}

/// Runs every queued closure. Call on the UI thread.
pub fn run_pending() {
    loop {
        let job = dispatcher().receiver.lock().unwrap().try_recv();
        match job {
            Ok(job) => job(),
            Err(_) => break,
        }
    }
}
