//! Hosts an app bundle with "AppType": "Swimming": a React (TSX) app built
//! with vite-plugin-swimming and run natively by the swimming framework
//! (QuickJS for the JS, Taffy for layout, tiny-skia for painting), with no
//! browser engine.
//!
//! The manifest's "Main" names the entry, relative to the bundle directory:
//! the Vite IIFE bundle (default "dist/bundle.js") or an .html file whose
//! classic scripts are run. bungalow:app:<app_id>:a:b maps to the in-app
//! route /a/b, delivered through the polyfilled `history` and a popstate
//! event, so routers that read `window.location` follow it.
//!
//! swimming's painter draws boxes but no glyphs yet, so text boxes from its
//! layout are drawn on top with the graphics context.

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use space::impl_element_base;
use space::sp_app_bundle::{resolve_inside, SPAppBundle};
use space::sp_color::SPColor;
use space::sp_element::{construct, ElementRef, SPElement, SPElementClass};
use space::sp_event_args::downcast_args;
use space::sp_font_style::SPFontStyle;
use space::sp_graphics_context::SPGraphicsContext;
use space::sp_image::{Pixel, SPImage};
use space::sp_mouse_event_args::SPMouseEventArgs;
use swimming::loader::run_source;
use swimming::JsBridge;
use swimming_core::LayoutBox;

const DEFAULT_MAIN: &str = "dist/bundle.js";
/// How often JS timers run and the app is repainted while it is shown.
const FRAME_INTERVAL: Duration = Duration::from_millis(33);
/// Apps not painted for this long are hidden and stop ticking.
const SHOWN_TIMEOUT: Duration = Duration::from_millis(250);

thread_local! {
    static APPS: RefCell<Vec<Weak<SPSwimmingAppElement>>> = const { RefCell::new(Vec::new()) };
}

struct Running {
    bridge: JsBridge,
    /// In-app path last delivered, e.g. "/a/b".
    route: String,
    size: (i32, i32),
}

pub struct SPSwimmingAppElement {
    pub el: SPElement,
    bundle: SPAppBundle,
    app: RefCell<Result<Running, String>>,
    error_handler: RefCell<Option<Rc<dyn Fn(&str)>>>,
    /// The last rendered frame; its texture is released when replaced.
    frame: RefCell<Option<SPImage>>,
    dirty: Cell<bool>,
    last_paint: Cell<Option<Instant>>,
    last_tick: Cell<Instant>,
}

impl_element_base!(SPSwimmingAppElement, el);

impl SPSwimmingAppElement {
    pub fn is_swimming(bundle: &SPAppBundle) -> bool {
        bundle.manifest.get("AppType").and_then(|t| t.as_str()) == Some("Swimming")
    }

    pub fn new(parent: Option<&ElementRef>, bundle: &SPAppBundle) -> Rc<Self> {
        let rc = construct(
            Self {
                el: SPElement::default(),
                bundle: bundle.clone(),
                app: RefCell::new(start(bundle)),
                error_handler: RefCell::new(None),
                frame: RefCell::new(None),
                dirty: Cell::new(true),
                last_paint: Cell::new(None),
                last_tick: Cell::new(Instant::now()),
            },
            parent,
        );
        rc.el.set_tag_name("swimmingapp");
        let weak = Rc::downgrade(&rc);
        rc.el.add_event_listener(
            "click",
            Rc::new(move |_, args| {
                let Some(this) = weak.upgrade() else { return 0 };
                let Some(mouse) = downcast_args::<SPMouseEventArgs>(args) else { return 0 };
                if mouse.get_button() == 0 {
                    this.pointer("click", mouse.get_x(), mouse.get_y());
                }
                1
            }),
        );
        APPS.with(|apps| apps.borrow_mut().push(Rc::downgrade(&rc)));
        rc
    }

    pub fn set_error_handler(&self, handler: Option<Rc<dyn Fn(&str)>>) {
        *self.error_handler.borrow_mut() = handler;
    }

    /// Starts the app again from its bundle, e.g. after it was rebuilt.
    pub fn restart(&self) {
        *self.app.borrow_mut() = start(&self.bundle);
        self.dirty.set(true);
    }

    /// Shows the in-app route `path` ("/a/b"), or reports why the app is
    /// not running. Returns false on error.
    pub fn navigate(&self, path: &str) -> bool {
        let result = match &mut *self.app.borrow_mut() {
            Ok(running) if running.route == path => Ok(()),
            Ok(running) => {
                running.route = path.to_string();
                running.bridge.eval_script(&route_script(path))
            }
            Err(error) => Err(error.clone()),
        };
        self.dirty.set(true);
        self.el.invalidate();
        match result {
            Ok(()) => true,
            Err(error) => {
                self.report_error(&format!("{}: {error}", self.bundle.identifier));
                false
            }
        }
    }

    fn report_error(&self, message: &str) {
        let handler = self.error_handler.borrow().clone();
        match handler {
            Some(handler) => handler(message),
            None => log::error!("{message}"),
        }
    }

    /// Sends a pointer event at absolute window coordinates to the app.
    fn pointer(&self, event: &str, x: i32, y: i32) {
        let bounds = self.el.get_absolute_bounds();
        if let Ok(running) = &*self.app.borrow() {
            if let Err(error) = running.bridge.pointer(event, (x - bounds.x) as f32, (y - bounds.y) as f32) {
                log::warn!("{}: {event} handler failed: {error}", self.bundle.identifier);
            }
        }
        self.dirty.set(true);
        self.el.invalidate();
    }

    fn is_shown(&self) -> bool {
        self.el.is_visible() && self.last_paint.get().is_some_and(|t| t.elapsed() < SHOWN_TIMEOUT)
    }

    fn tick(&self) {
        let elapsed = self.last_tick.get().elapsed();
        self.last_tick.set(Instant::now());
        if let Ok(running) = &*self.app.borrow() {
            let elapsed_ms = elapsed.as_millis().min(u32::MAX as u128) as u32;
            if let Err(error) = running.bridge.tick(elapsed_ms) {
                log::warn!("{}: timer failed: {error}", self.bundle.identifier);
            }
        }
        self.dirty.set(true);
        self.el.invalidate();
    }

    /// Runs the JS timers of the apps being shown. Returns how long until
    /// the next frame is due, or None if no app is shown.
    pub fn tick_all() -> Option<Duration> {
        let apps: Vec<Rc<SPSwimmingAppElement>> = APPS.with(|apps| {
            let mut apps = apps.borrow_mut();
            apps.retain(|w| w.strong_count() > 0);
            apps.iter().filter_map(Weak::upgrade).collect()
        });
        let mut next: Option<Duration> = None;
        for app in apps.iter().filter(|a| a.is_shown()) {
            let since = app.last_tick.get().elapsed();
            let wait = if since >= FRAME_INTERVAL {
                app.tick();
                FRAME_INTERVAL
            } else {
                FRAME_INTERVAL - since
            };
            next = Some(next.map_or(wait, |n| n.min(wait)));
        }
        next
    }

    /// Lays out and renders the app at `width` x `height` if anything
    /// changed since the last frame.
    fn render(&self, width: i32, height: i32, gc: &dyn SPGraphicsContext) {
        let mut app = self.app.borrow_mut();
        let Ok(running) = &mut *app else { return };
        if running.size != (width, height) {
            running.size = (width, height);
            running.bridge.resize(width as f32, height as f32);
            self.dirty.set(true);
        }
        if !self.dirty.replace(false) {
            return;
        }
        let Some(pixmap) = running.bridge.render() else { return };
        let mut image = SPImage::new(pixmap.width(), pixmap.height());
        for (out, pixel) in image.pixels.iter_mut().zip(pixmap.pixels()) {
            let c = pixel.demultiply();
            *out = Pixel { r: c.red(), g: c.green(), b: c.blue(), a: c.alpha() };
        }
        if let Some(old) = self.frame.replace(Some(image)) {
            gc.release_image(&old);
        }
    }

    /// Draws the text boxes of the last layout, which swimming does not paint.
    fn paint_text(&self, x: i32, y: i32, gc: &dyn SPGraphicsContext) {
        let app = self.app.borrow();
        let Ok(running) = &*app else { return };
        let engine = running.bridge.engine();
        let engine = engine.lock().unwrap();
        let Some(layout) = engine.layout() else { return };
        let family = self.el.get_string_attribute("font", "Tahoma");
        paint_text_box(&layout.root, x as f32, y as f32, &family, gc);
    }
}

fn paint_text_box(b: &LayoutBox, ox: f32, oy: f32, family: &str, gc: &dyn SPGraphicsContext) {
    let (x, y) = (ox + b.x, oy + b.y);
    if let Some(text) = b.text.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        let c = &b.computed;
        let font = SPFontStyle::new(family, c.font_size.round() as i32, if c.bold { 700 } else { 400 }, false, false);
        let alpha = (c.color[3] as f32 * c.opacity) as i16;
        let color = SPColor::new(c.color[0] as i16, c.color[1] as i16, c.color[2] as i16, alpha);
        gc.draw_string(text, &font, color, x as i32, y as i32, b.w as i32, b.h as i32);
    }
    for child in &b.children {
        paint_text_box(child, x, y, family, gc);
    }
}

/// Browser globals QuickJS lacks and swimming-web-compat expects the host to
/// provide: a `URL` covering what its memory history reads (absolute URLs
/// and paths resolved against a base), and `document.defaultView` on the
/// stand-in document it installs, which React Router's createBrowserHistory
/// reads.
const HOST_GLOBALS: &str = r##"
if (typeof document === "undefined") {
  let current;
  Object.defineProperty(globalThis, "document", {
    configurable: true,
    get: () => current,
    set: (value) => {
      if (value && typeof value === "object" && !("defaultView" in value)) value.defaultView = globalThis;
      current = value;
    },
  });
}
if (typeof URL === "undefined") {
  const ABSOLUTE = /^([a-z][a-z0-9+.-]*:)\/\/([^/?#]*)([^?#]*)(\?[^#]*)?(#.*)?$/i;
  const normalize = (path) => {
    const out = [];
    for (const part of path.split("/").slice(1)) {
      if (part === "..") out.pop();
      else if (part !== ".") out.push(part);
    }
    if (/\/\.{0,2}$/.test(path) && out[out.length - 1] !== "") out.push("");
    return "/" + out.join("/");
  };
  globalThis.URL = class URL {
    constructor(input, base) {
      input = String(input);
      let m = ABSOLUTE.exec(input);
      if (!m) {
        if (base === undefined) throw new TypeError("Invalid URL: " + input);
        const b = new URL(base);
        const path = /^[^?#]*/.exec(input)[0];
        const rest = input.slice(path.length);
        const search = /^\?[^#]*/.exec(rest);
        const hash = /#.*$/.exec(rest);
        m = [input, b.protocol, b.host,
          !path ? b.pathname : path[0] === "/" ? path : b.pathname.replace(/[^/]*$/, "") + path,
          search ? search[0] : path ? "" : b.search, hash ? hash[0] : ""];
      }
      this.protocol = m[1].toLowerCase();
      this.host = m[2];
      this.hostname = m[2].replace(/:\d*$/, "");
      this.port = (/:(\d*)$/.exec(m[2]) || ["", ""])[1];
      this.pathname = normalize(m[3] || "/");
      this.search = m[4] && m[4] !== "?" ? m[4] : "";
      this.hash = m[5] && m[5] !== "#" ? m[5] : "";
    }
    get origin() { return this.protocol + "//" + this.host; }
    get href() { return this.origin + this.pathname + this.search + this.hash; }
    toString() { return this.href; }
    toJSON() { return this.href; }
  };
}
"##;

/// Loads the bundle's entry into a new JS runtime.
fn start(bundle: &SPAppBundle) -> Result<Running, String> {
    let root = Path::new(&bundle.path);
    let main = bundle.manifest.get("Main").and_then(|m| m.as_str()).unwrap_or(DEFAULT_MAIN);
    let entry = resolve_inside(root, root, main)
        .filter(|p| p.is_file())
        .ok_or_else(|| format!("The app \"{}\" has no {main}; build it with vite-plugin-swimming.", bundle.identifier))?;
    let bridge = JsBridge::new(800.0, 600.0).map_err(|e| format!("Cannot start the JavaScript runtime: {e}"))?;
    bridge.eval_script(HOST_GLOBALS).map_err(|e| format!("Cannot start the JavaScript runtime: {e}"))?;
    run_source(&bridge, &entry.to_string_lossy()).map_err(|e| format!("The app \"{}\" failed to load: {e}", bundle.identifier))?;
    Ok(Running { bridge, route: "/".into(), size: (0, 0) })
}

/// Replaces the polyfilled history entry with `path` and tells the app.
fn route_script(path: &str) -> String {
    let path = serde_json::to_string(path).unwrap_or_else(|_| "\"/\"".into());
    format!(
        "if(typeof history!=='undefined'){{history.replaceState(null,'',{path});\
         if(typeof dispatchEvent==='function')dispatchEvent({{type:'popstate',state:null}});}}"
    )
}

impl SPElementClass for SPSwimmingAppElement {
    fn get_type(&self) -> String {
        "swimmingapp".into()
    }

    fn paint_content(&self, x: i32, y: i32, width: i32, height: i32, gc: &dyn SPGraphicsContext) {
        self.last_paint.set(Some(Instant::now()));
        if width <= 0 || height <= 0 {
            return;
        }
        self.render(width, height, gc);
        if let Some(frame) = self.frame.borrow().as_ref() {
            gc.draw_image(frame, x, y, frame.width as i32, frame.height as i32);
        }
        self.paint_text(x, y, gc);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle_in(name: &str, files: &[(&str, &str)]) -> SPAppBundle {
        let dir = std::env::temp_dir().join(format!("bungalow-swimming-{name}-{}", std::process::id()));
        for (file, content) in files {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }
        SPAppBundle {
            identifier: name.into(),
            path: dir.to_string_lossy().into_owned(),
            manifest: serde_json::json!({ "AppType": "Swimming" }),
            ..Default::default()
        }
    }

    fn texts(b: &LayoutBox, out: &mut Vec<String>) {
        out.extend(b.text.clone());
        b.children.iter().for_each(|c| texts(c, out));
    }

    #[test]
    fn route_script_escapes_the_path() {
        assert!(route_script("/a/\"b").contains(r#"history.replaceState(null,'',"/a/\"b")"#));
    }

    #[test]
    fn host_url_resolves_like_a_browser() {
        let script = r#"
            [ new URL("/a/./b/../c?x=1#h", "http://swimming.local").href,
              new URL("d", "http://h:8/a/b").href,
              new URL("?q", "http://h/a#x").href,
              new URL("HTTP://h:8/p").port ]
              .forEach(function (s) { __SWIMMING_NATIVE__.appendChild(0, __SWIMMING_NATIVE__.createText(s)); });
            __SWIMMING_NATIVE__.commit();
        "#;
        let bundle = bundle_in("url", &[("dist/bundle.js", script)]);
        let running = start(&bundle).unwrap();
        let mut found = Vec::new();
        texts(&running.bridge.engine().lock().unwrap().layout().unwrap().root, &mut found);
        assert_eq!(found, ["http://swimming.local/a/c?x=1#h", "http://h:8/a/d", "http://h/a?q", "8"]);
    }

    #[test]
    fn start_reports_a_missing_bundle() {
        let bundle = bundle_in("missing", &[]);
        assert!(SPSwimmingAppElement::is_swimming(&bundle));
        assert!(start(&bundle).err().unwrap().contains("dist/bundle.js"));
    }

    #[test]
    fn start_runs_the_bundle_and_routes_reach_it() {
        // Stands in for swimming-web-compat's history and swimming-react's
        // reconciler: shows location.pathname in a div.
        let script = r#"
            var n = __SWIMMING_NATIVE__, path = "/", listeners = [];
            globalThis.history = { replaceState: function (s, t, url) { path = url; } };
            globalThis.dispatchEvent = function (e) { listeners.forEach(function (f) { f(e); }); };
            var div = n.createNode("div", "width: 100px; height: 40px; background-color: #ff0000", "{}");
            var text = n.createText(path);
            n.appendChild(div, text);
            n.appendChild(0, div);
            listeners.push(function () { n.setText(text, path); n.commit(); });
            n.commit();
        "#;
        let bundle = bundle_in("routes", &[("dist/bundle.js", script)]);
        let running = start(&bundle).unwrap();
        running.bridge.eval_script(&route_script("/albums/1")).unwrap();

        let engine = running.bridge.engine();
        let mut found = Vec::new();
        texts(&engine.lock().unwrap().layout().unwrap().root, &mut found);
        assert_eq!(found, ["/albums/1"]);

        let pixmap = running.bridge.render().unwrap();
        let red = pixmap.pixel(10, 10).unwrap().demultiply();
        assert_eq!((red.red(), red.green(), red.blue()), (255, 0, 0));
    }

    /// Centers of the boxes with `tag`, in layout order.
    fn centers(b: &LayoutBox, tag: &str, ox: f32, oy: f32, out: &mut Vec<(f32, f32)>) {
        let (x, y) = (ox + b.x, oy + b.y);
        if b.tag == tag {
            out.push((x + b.w / 2.0, y + b.h / 2.0));
        }
        b.children.iter().for_each(|c| centers(c, tag, x, y, out));
    }

    /// The test app in Apps/swimmingtest, a React Router app built with
    /// vite-plugin-swimming.
    #[test]
    fn the_test_app_renders_clicks_ticks_and_routes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../Apps/swimmingtest");
        let manifest = std::fs::read_to_string(root.join("manifest.json")).unwrap();
        let bundle = SPAppBundle {
            identifier: "swimmingtest".into(),
            path: root.to_string_lossy().into_owned(),
            manifest: serde_json::from_str(&manifest).unwrap(),
            ..Default::default()
        };
        assert!(SPSwimmingAppElement::is_swimming(&bundle));
        let running = start(&bundle).unwrap();
        let engine = running.bridge.engine();
        let read = || {
            let mut found = Vec::new();
            texts(&engine.lock().unwrap().layout().unwrap().root, &mut found);
            found.concat()
        };
        assert!(read().contains("Swimming Test"));
        assert!(read().contains("Clicked 0 times"));

        let mut buttons = Vec::new();
        centers(&engine.lock().unwrap().layout().unwrap().root, "button", 0.0, 0.0, &mut buttons);
        let (x, y) = buttons[0];
        running.bridge.pointer("click", x, y).unwrap();
        assert!(read().contains("Clicked 1 times"), "{}", read());

        running.bridge.tick(1000).unwrap();
        assert!(read().contains("Running for 1 s"), "{}", read());

        let (x, y) = buttons[1];
        running.bridge.pointer("click", x, y).unwrap();
        assert!(read().contains("land here directly"), "{}", read());

        running.bridge.eval_script(&route_script("/")).unwrap();
        assert!(read().contains("Swimming Test"), "{}", read());
        running.bridge.eval_script(&route_script("/about")).unwrap();
        assert!(read().contains("land here directly"), "{}", read());
        running.bridge.eval_script(&route_script("/nowhere")).unwrap();
        assert!(read().contains("No page for /nowhere"), "{}", read());
    }
}
