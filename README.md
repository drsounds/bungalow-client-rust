# Bungalow (Rust)

A Rust port of [bungalow-client](https://github.com/drsounds/bungalow-client)
(the `feature/cef` branch, also known as bungalow-cef). The code keeps the C++
class structure: one Rust module per C++ class, with the same names in
`snake_case`, in a crate per C++ library.

- Lua scripting uses [mlua](https://github.com/mlua-rs/mlua) (Lua 5.4, vendored).
- The window and the Space renderer run on [egui/eframe](https://github.com/emilk/egui).
- Embedded web content (HTML5 app bundles, web sites) uses
  [Wef](https://crates.io/crates/wef), an offscreen CEF wrapper, instead of
  CEF's native child windows.
- The `linux/` shell is not ported yet.

## Layout

| C++ directory  | Rust crate   | What it is |
|----------------|--------------|------------|
| `elua/`        | `elua`       | Lua runtime (`Elua` on mlua) |
| `space/`       | `space`      | The Space UI framework: elements, skins, XML views, app bundles, URIs |
| `spider/`      | `spider`     | Spider views (Lua-scripted views) |
| `bungalow/`    | `bungalow`   | The Bungalow library: database, models, services, data sources, views |
| `windows/`     | `client`     | The executable: window, graphics context, app, browser views and the browser host |
| `Apps/`, `views/`, `icons/` | (same) | Resources, copied as they are |

Platform-neutral pieces of `windows/` moved into the `bungalow` crate so they do
not depend on the window toolkit: `DownloadManager`, `WinHttpClient`
(`http_client`), `UiDispatcher`, `PlatformUi` (`platform_ui`, behind hooks)
and the `sp_audio_*` bridge of `WinAudio` (`services/audio_bridge`, played
with rodio behind the `audio` feature).

The rest of `windows/` maps to `client/src`:

| C++ (`windows/`)               | Rust (`client/src/`) |
|--------------------------------|----------------------|
| `wtl_main.cpp`                 | `main.rs` (`MainFrame`, an `eframe::App`) |
| `NativeApplication`            | `native_application.rs` |
| `Win32GraphicsContext`         | `egui_graphics_context.rs` (`EguiGraphicsContext`) |
| `BungalowApp`                  | `bungalow_app.rs` |
| `MainElement`                  | `main_element.rs` |
| `FaviconCache`                 | `favicon_cache.rs` |
| `PlatformUi.cpp` (dialogs)     | `platform.rs` (`ClientPlatformHooks`, rfd dialogs, egui popup menus) |
| `SiteView`, `AppView`, `CuratorView`, `StoreView`, `PlayerView`, `SPAppView` | `views/` |
| `SPPlayerCanvasElement`, `SPMusicVisualizerElement` | `views/` |
| `cefhost/CefRuntime`           | `wefhost/wef_runtime.rs` |
| `cefhost/CefBrowserWindow`     | `wefhost/browser_window.rs` |
| `cefhost/CefAppBrowser`        | `wefhost/app_browser.rs` |
| `cefhost/CefSiteBrowser`       | `wefhost/site_browser.rs` |
| `cefhost/SpSchemeHandler`      | `wefhost/sp_scheme_server.rs` |

### How the C++ idioms are mapped

- **Class hierarchy.** Elements are `Rc<T>`; each type embeds its base
  (`el: SPElement`, `view: SPViewElement`, ...) and implements the
  `SPElementClass` trait for the virtual methods it overrode. Parents own
  their children, children hold weak parent links, and
  `ElementRefExt::downcast::<T>()` replaces `dynamic_cast`.
- **Events.** `addEventListener` callbacks are
  `Rc<dyn Fn(Option<&SPObject>, &dyn SPEventArgs) -> i32>`; event arguments are
  downcast with `downcast_args::<T>()`.
- **Singletons** (`SPAppManager::instance()`, the URI store, the UI dispatcher)
  are thread-locals with accessor functions.
- **Link-time hooks** (`createEmbeddedAppBrowser`, the platform dialogs) are
  traits that the client registers at startup
  (`space::sp_embedded_browser::set_backend`, `bungalow::platform_ui::set_hooks`).
- **Win32 timers** are deadlines checked by the frame loop
  (`NativeApplication::timer`), which also asks egui to repaint when one is due.

## Building

```sh
cargo build --release          # Space UI, library and views; no embedded browser
cargo run -p bungalow-client
```

Optional features of the `bungalow-client` package:

- `wef`: embedded browsers for HTML5 app bundles and web sites. Needs the CEF
  binary distribution that Wef expects: set `CEF_ROOT` (default `~/.cef`) as
  described in the [Wef README](https://crates.io/crates/wef), and put the CEF
  runtime files next to the executable (`cargo wef build` does this).
- `audio`: local playback through rodio (needs ALSA headers on Linux, e.g.
  `libasound2-dev`).
- `swimming` (default): runs Swimming app bundles, see below. Builds QuickJS
  from source, so it needs a C compiler.

```sh
cargo run -p bungalow-client --features wef,audio
```

Without `wef`, web sites open in the system browser and app bundles take the
same fallback as the C++ build without CEF. Without `audio`, playback state and
the queue work but nothing is played.

Linux builds need the usual eframe dependencies (X11 or Wayland, OpenGL,
`libxkbcommon-x11`). File dialogs go through the XDG desktop portal.

## App types

An app bundle's `manifest.json` picks how `bungalow:app:<id>` URIs run:

| `AppType`    | Runs as |
|--------------|---------|
| (none)       | HTML5: `index.html` in an embedded browser (`wef`), or the remote `https://<id>.apps.bungalow.se` |
| `"Native"`   | Lua MVC app rendered with native XUL elements (`SPNativeApp`) |
| `"Swimming"` | React (TSX) app rendered natively by [swimming](https://github.com/drsounds/swimming), with no browser engine (`views/sp_swimming_app_element.rs`) |

A Swimming app is built with `vite-plugin-swimming` into one IIFE bundle,
which `"Main"` names (default `dist/bundle.js`; an `.html` entry with classic
scripts also works):

```json
{
    "BundleType": "Application",
    "BundleIdentifier": "hello",
    "AppName": { "en": "Hello" },
    "AppType": "Swimming",
    "Main": "dist/bundle.js"
}
```

`bungalow:app:hello:albums:1` is delivered to the app as the route
`/albums/1` (`history.replaceState` plus a `popstate` event, which
`BrowserRouter` follows). The app gets left clicks, and its JS timers run
while it is shown. Reload (Develop menu) restarts it with a fresh JS
runtime. Since swimming does not draw glyphs yet, the text boxes of its
layout are drawn by the Space graphics context. `Apps/swimmingtest` is a
test app (open `bungalow:app:swimmingtest`).

## Differences from the C++ client

Platform and toolkit:

- **Popup menus are asynchronous.** `TrackPopupMenu` blocks in a nested modal
  loop, which egui cannot do, so `platform_ui::show_popup_menu` takes a
  callback and the menu is drawn by the frame loop. Callers that used the
  return value now continue in the callback.
- **Dialogs** (folder and file pickers, message boxes, the consent prompt for
  new web hosts) use [rfd](https://crates.io/crates/rfd) through the platform
  hooks.
- **Text inputs** are egui `TextEdit`s laid over the Space input elements,
  standing in for the Win32 `EDIT` controls.
- **The caption** is drawn by the client (File and Develop menus, title,
  window buttons), as in `wtl_main.cpp`. Its text color follows the
  brightness of the app header behind it, since there is no OS caption color
  to fall back on.
- **Startup** happens in the first frame, because egui has no fonts before
  then and the Space lays out text as it starts.

Embedded browsers under Wef:

- **No `sp://` scheme.** Wef cannot register custom schemes, so bundles are
  served by a loopback HTTP server at `http://<bundle>.localhost:<port>/`, one
  origin per bundle, and the shared API bundle at `http://import.localhost:<port>/`
  (readable from any origin, as before). `Apps/import/scripts/sp.js` recognises
  this hosting mode and maps `sp://<bundle>/x` to it.
- **No per-app CEF profiles.** All browsers share Wef's profile; the separate
  origins still keep each app's storage apart.
- **No request filter.** Instead of `OnBeforeResourceLoad`, app pages get a
  Content-Security-Policy that allows the same sources (the bundle, the shared
  API bundle, `data:`, `blob:`, and the allowed https/wss hosts with their
  subdomains).
- **Navigation is checked after the fact.** Wef has no `OnBeforeBrowse`, so a
  page that leaves its app bundle, or a site that moves to a host that has not
  been allowed, is noticed when the address changes; the browser goes back
  and the host view gets the same callback as before. Redirects of a page the
  host loaded are allowed, as CEF did. `bungalow:` links are caught by a click
  handler in the page.
- **Startup scripts** are injected into app pages by the server (before the
  page's own scripts) and into site pages when loading starts.
- **No DevTools.** Wef has no inspector; Show Inspector reports that.
- **No download interception.** Downloads started inside a site browser are not
  handed to the download manager.
- **Dark mode** sets the page's `color-scheme`; `prefers-color-scheme` media
  queries do not follow it.
- Keyboard input is limited to what Wef forwards (text, editing and navigation
  keys); clipboard, select-all, undo and redo shortcuts are mapped to frame
  commands.

Small fixes and choices in the port:

- An element tree without a color scheme falls back to the light scheme where
  the C++ code threw `"Invalid color scheme"`.
- `SPTitleElement::set_entity` writes the entity label (the C++ setter wrote the
  title label).
- `SPColor::from_html` also accepts `#rgb` and `#rrggbbaa`; colors without
  alpha are opaque.
- `MainElement` creates the info element but does not add it, as in the C++
  code.
- The bookmark sections of the sidebar stay disabled, as in the C++ code
  (`if (false)`).
