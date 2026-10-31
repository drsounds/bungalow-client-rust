//! Port of `CefRuntime`: process-wide Chromium lifetime, through the `cef`
//! crate.
//!
//! The same [`BungalowApp`] serves every process: in the browser process it
//! schedules CEF's external message pump on the frame loop, in renderers it
//! installs `window.jsBridge`, the page side of `window.bungalow.navigate`.

use std::cell::Cell;
use std::os::raw::c_int;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use cef::args::Args;
use cef::*;

/// The renderer -> browser process message `jsBridge.navigate(uri)` sends.
pub const NAVIGATE_MESSAGE: &str = "bungalow.navigate";

thread_local! {
    static INITIALIZED: Cell<bool> = const { Cell::new(false) };
    #[cfg(target_os = "macos")]
    static LIBRARY: std::cell::RefCell<Option<library_loader::LibraryLoader>> =
        const { std::cell::RefCell::new(None) };
}

/// The frame loop that runs CEF's work (see [`set_pump_context`]).
static PUMP_CONTEXT: OnceLock<egui::Context> = OnceLock::new();

/// Lets CEF ask the frame loop for a frame when it has work.
pub fn set_pump_context(ctx: &egui::Context) {
    let _ = PUMP_CONTEXT.set(ctx.clone());
}

wrap_app! {
    struct BungalowApp;

    impl App {
        fn on_before_command_line_processing(
            &self,
            process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            let Some(command_line) = command_line else { return };
            if process_type.is_none_or(|t| t.to_string().is_empty()) {
                // Software compositing: the frames are copied into egui
                // textures anyway, and GPU compositing gets the view size
                // wrong while it resizes.
                command_line.append_switch(Some(&"disable-gpu".into()));
                command_line.append_switch(Some(&"disable-gpu-compositing".into()));
            }
            #[cfg(target_os = "macos")]
            command_line.append_switch(Some(&"use-mock-keychain".into()));
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(PumpScheduler::new())
        }

        fn render_process_handler(&self) -> Option<RenderProcessHandler> {
            Some(BridgeInstaller::new())
        }
    }
}

wrap_browser_process_handler! {
    struct PumpScheduler;

    impl BrowserProcessHandler {
        /// A second launch must not open a window of its own.
        fn on_already_running_app_relaunch(
            &self,
            _command_line: Option<&mut CommandLine>,
            _current_directory: Option<&CefString>,
        ) -> c_int {
            1
        }

        /// May be called on any thread: asks for the frame that runs the work.
        fn on_schedule_message_pump_work(&self, delay_ms: i64) {
            if let Some(ctx) = PUMP_CONTEXT.get() {
                ctx.request_repaint_after(Duration::from_millis(delay_ms.max(0) as u64));
            }
        }
    }
}

wrap_render_process_handler! {
    struct BridgeInstaller;

    impl RenderProcessHandler {
        /// Defines `window.jsBridge.navigate(uri)` in every new script context.
        fn on_context_created(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            context: Option<&mut V8Context>,
        ) {
            let Some(global) = context.and_then(|c| c.global()) else { return };
            let mut handler = NavigateFunction::new();
            let Some(mut navigate) = v8_value_create_function(Some(&"navigate".into()), Some(&mut handler)) else {
                return;
            };
            let Some(mut bridge) = v8_value_create_object(None, None) else { return };
            bridge.set_value_bykey(Some(&"navigate".into()), Some(&mut navigate), Default::default());
            global.set_value_bykey(Some(&"jsBridge".into()), Some(&mut bridge), Default::default());
        }
    }
}

wrap_v8_handler! {
    struct NavigateFunction;

    impl V8Handler {
        /// Sends the URI to the browser process; non-string arguments are
        /// ignored (`window.bungalow.navigate` already rejects them).
        fn execute(
            &self,
            _name: Option<&CefString>,
            _object: Option<&mut V8Value>,
            arguments: Option<&[Option<V8Value>]>,
            _retval: Option<&mut Option<V8Value>>,
            _exception: Option<&mut CefString>,
        ) -> c_int {
            let uri = arguments
                .and_then(|a| a.first())
                .and_then(Option::as_ref)
                .filter(|v| v.is_string() != 0)
                .map(|v| CefString::from(&v.string_value()));
            let frame = v8_context_get_current_context().and_then(|c| c.frame());
            let message = process_message_create(Some(&NAVIGATE_MESSAGE.into()));
            if let (Some(uri), Some(frame), Some(mut message)) = (uri, frame, message) {
                if let Some(args) = message.argument_list() {
                    args.set_string(0, Some(&uri));
                }
                frame.send_process_message(ProcessId::BROWSER, Some(&mut message));
            }
            1
        }
    }
}

/// Runs this process as a CEF subprocess (renderer, GPU, ...) when CEF
/// launched it as one. Returns true if it did, and the process must exit.
/// Call first thing in `main`.
pub fn execute_subprocess() -> bool {
    // macOS runs subprocesses from the helper bundles instead.
    #[cfg(not(target_os = "macos"))]
    {
        let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
        let args = Args::new();
        let mut app = BungalowApp::new();
        // -1 in the browser process, the exit code in a subprocess.
        return execute_process(Some(args.as_main_args()), Some(&mut app), std::ptr::null_mut()) >= 0;
    }
    #[allow(unreachable_code)]
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
    {
        let exe = match std::env::current_exe() {
            Ok(exe) => exe,
            Err(e) => {
                log::error!("Cannot locate the CEF framework: {e}");
                return false;
            }
        };
        let loader = library_loader::LibraryLoader::new(&exe, false);
        if !loader.load() {
            log::error!("Cannot load the CEF framework");
            return false;
        }
        LIBRARY.with(|l| *l.borrow_mut() = Some(loader));
    }

    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    let args = Args::new();
    let settings = Settings {
        no_sandbox: 1,
        windowless_rendering_enabled: 1,
        external_message_pump: 1,
        root_cache_path: CefString::from(root.to_string_lossy().as_ref()),
        ..Default::default()
    };
    let mut app = BungalowApp::new();
    let ok = cef::initialize(Some(args.as_main_args()), Some(&settings), Some(&mut app), std::ptr::null_mut()) == 1;
    if !ok {
        log::error!("CEF is unavailable: initialization failed");
    }
    INITIALIZED.with(|i| i.set(ok));
    ok
}

pub fn is_initialized() -> bool {
    INITIALIZED.with(Cell::get)
}

/// Runs pending CEF work. CEF uses an external message pump: it asks for
/// frames through [`PumpScheduler`], and every frame runs this.
pub fn do_message_loop_work() {
    if is_initialized() {
        cef::do_message_loop_work();
    }
}

/// Shuts CEF down. Call once on the main thread, after every browser has
/// been closed (see [`super::close_all_browsers`]).
pub fn shutdown() {
    if !is_initialized() {
        return;
    }
    cef::shutdown();
    INITIALIZED.with(|i| i.set(false));
    #[cfg(target_os = "macos")]
    LIBRARY.with(|l| l.borrow_mut().take());
}
