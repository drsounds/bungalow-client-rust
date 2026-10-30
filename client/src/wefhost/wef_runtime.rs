//! Port of `CefRuntime`: process-wide Chromium lifetime, through Wef.

use std::cell::Cell;
use std::path::PathBuf;

thread_local! {
    static INITIALIZED: Cell<bool> = const { Cell::new(false) };
    #[cfg(target_os = "macos")]
    static FRAMEWORK: std::cell::RefCell<Option<wef::FrameworkLoader>> = const { std::cell::RefCell::new(None) };
}

/// Runs this process as a CEF subprocess (renderer, GPU, ...) when CEF
/// launched it as one. Returns true if it did, and the process must exit.
/// Call first thing in `main`.
pub fn execute_subprocess() -> bool {
    // macOS runs subprocesses from the helper bundles instead.
    #[cfg(not(target_os = "macos"))]
    match wef::exec_process() {
        Ok(is_subprocess) => return is_subprocess,
        Err(e) => log::error!("CEF subprocess check failed: {e}"),
    }
    false
}

/// `cacheRoot()`: the root of every browser profile.
pub fn cache_root() -> PathBuf {
    PathBuf::from(bungalow::platform_ui::user_data_dir()).join("cef")
}

/// Starts CEF in the browser process. Returns false if CEF is unavailable;
/// app bundles then fall back like the non-CEF C++ build.
pub fn initialize() -> bool {
    let root = cache_root();
    if let Err(e) = std::fs::create_dir_all(&root) {
        log::warn!("Cannot create {}: {e}", root.display());
    }

    #[cfg(target_os = "macos")]
    match wef::FrameworkLoader::load_in_main() {
        Ok(loader) => FRAMEWORK.with(|f| *f.borrow_mut() = Some(loader)),
        Err(e) => {
            log::error!("Cannot load the CEF framework: {e}");
            return false;
        }
    }

    let settings = wef::Settings::new().root_cache_path(root.to_string_lossy().as_bytes().to_vec());
    let ok = match wef::init(settings) {
        Ok(()) => true,
        Err(e) => {
            log::error!("CEF is unavailable: {e}");
            false
        }
    };
    INITIALIZED.with(|i| i.set(ok));
    ok
}

pub fn is_initialized() -> bool {
    INITIALIZED.with(Cell::get)
}

/// Runs pending CEF work. Wef uses CEF's external message pump: on Windows
/// and macOS it is driven by the native event loop that eframe already runs,
/// on Linux by the default GLib main context, which nothing else iterates.
pub fn do_message_loop_work() {
    #[cfg(target_os = "linux")]
    glib_iteration();
}

/// `g_main_context_iteration(NULL, FALSE)`, looked up at run time: libcef
/// loads GLib, so the client does not need to link it itself.
#[cfg(target_os = "linux")]
fn glib_iteration() {
    use std::ffi::{c_char, c_int, c_void};
    use std::sync::OnceLock;

    type Iteration = unsafe extern "C" fn(*mut c_void, c_int) -> c_int;
    extern "C" {
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }
    static ITERATION: OnceLock<Option<Iteration>> = OnceLock::new();
    let iteration = ITERATION.get_or_init(|| {
        // RTLD_DEFAULT is the null handle on glibc.
        let symbol = unsafe { dlsym(std::ptr::null_mut(), c"g_main_context_iteration".as_ptr()) };
        if symbol.is_null() {
            log::warn!("GLib is not loaded; CEF work will not be pumped");
            None
        } else {
            Some(unsafe { std::mem::transmute::<*mut c_void, Iteration>(symbol) })
        }
    });
    if let Some(iteration) = iteration {
        // Bounded, so a busy CEF cannot starve the frame.
        for _ in 0..32 {
            if unsafe { iteration(std::ptr::null_mut(), 0) } == 0 {
                break;
            }
        }
    }
}

/// Shuts CEF down. Call once on the main thread, after every browser has
/// been closed (see [`super::close_all_browsers`]).
pub fn shutdown() {
    if !is_initialized() {
        return;
    }
    wef::shutdown();
    INITIALIZED.with(|i| i.set(false));
    #[cfg(target_os = "macos")]
    FRAMEWORK.with(|f| f.borrow_mut().take());
}
