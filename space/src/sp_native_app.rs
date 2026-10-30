//! Port of `SPNativeApp`: runs an app bundle whose manifest declares
//! `"AppType": "Native"`: a Lua MVC app rendered with native XUL elements
//! instead of an embedded browser.
//!
//! Routes come from `<bundle>/routes.json`, with paths relative to the
//! bundle, and from the file each manifest `Features.<name>.Routes` names,
//! with paths relative to that file's folder:
//!
//! ```json
//! {"routes": [{"path": "^bungalow:app:x:(?P<name>\\w+)$",
//!              "view": "index.xml", "controller": "index.lua"}]}
//! ```
//!
//! A view or controller without an extension gets .xml or .lua. The
//! bundle's own routes come before feature routes, and the first route
//! whose path matches the whole URI handles it. The controller's function
//! named after its file (index.lua -> index) is called with
//! `request = {method, uri, action, data, arguments}`, where `arguments`
//! holds the captures from index 0 and by group name. The table it returns
//! is the view's model; its fields are also template variables.
//!
//! The app keeps one Lua state for its lifetime and each controller file its
//! own environment in it, so controllers keep state between requests.
//! Controllers run sandboxed: the safe standard libraries, `require()` of
//! .lua modules inside the bundle, and the libraries the manifest lists
//! under ApiPermissions ("io", "os"). A request that runs more than the
//! instruction limit fails, so a runaway controller cannot hang the app.
//!
//! Instead of a model, a controller can return
//! * `redirect(uri)` to navigate to uri (e.g. after handling a POST), or
//! * `render(view, model)` to render another view, named like a route's.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use mlua::{ChunkMode, Function, HookTriggers, Lua, Table, Value as LuaValue, VmState};
use regex::Regex;
use serde_json::{Map, Value};

use crate::sp_app_bundle::{lexically_normal, resolve_inside, string_field, SPAppBundle};
use crate::sp_template::SPTemplate;

const MAX_MODEL_DEPTH: usize = 32;
/// Instructions between instruction limit checks.
const HOOK_INTERVAL: u32 = 1000;

/// Globals of the base library that cannot reach outside the sandbox
/// (no load, loadfile, dofile or collectgarbage).
const BASE_FUNCTIONS: [&str; 19] = [
    "assert", "error", "getmetatable", "ipairs", "next", "pairs", "pcall", "print", "rawequal", "rawget", "rawlen",
    "rawset", "select", "setmetatable", "tonumber", "tostring", "type", "xpcall", "_VERSION",
];
const SAFE_LIBRARIES: [&str; 5] = ["coroutine", "math", "string", "table", "utf8"];
/// The parts of os available without the "os" permission.
const SAFE_OS_FUNCTIONS: [&str; 4] = ["clock", "date", "difftime", "time"];
/// Libraries an app can ask for with ApiPermissions.
const PERMISSION_LIBRARIES: [&str; 2] = ["io", "os"];

pub const DEFAULT_INSTRUCTION_LIMIT: i64 = 50_000_000;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Response {
    /// The rendered view; empty for a redirect
    pub xul: String,
    /// URI to navigate to instead, if not empty
    pub redirect: String,
}

struct Route {
    path: Regex,
    /// absolute path of the routes file's folder, which render() views are relative to
    base: PathBuf,
    view: PathBuf,
    controller: PathBuf,
}

pub struct SPNativeApp {
    root: PathBuf,
    routes: Vec<Route>,
    lua: Lua,
    sandbox: Table,
    result_meta: Table,
    /// path -> environment of the controller
    controllers: RefCell<HashMap<PathBuf, Table>>,
    instruction_limit: Rc<Cell<i64>>,
    /// run by the current request
    instructions: Rc<Cell<i64>>,
}

/// A bundle file's path for messages and Lua chunk names.
fn relative_name(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .to_string_lossy()
        .replace('\\', "/")
}

/// [`resolve_inside`], adding `default_extension` when relative has none
/// ("views/index" -> views/index.xml).
fn resolve_file(root: &Path, base: &Path, relative: &str, default_extension: &str) -> Option<PathBuf> {
    let mut path = resolve_inside(root, base, relative)?;
    if path.extension().is_none() {
        let mut name = path.into_os_string();
        name.push(default_extension);
        path = PathBuf::from(name);
    }
    Some(path)
}

/// A new globals table that falls back to the sandbox.
fn new_environment(lua: &Lua, sandbox: &Table) -> mlua::Result<Table> {
    let env = lua.create_table()?;
    let meta = lua.create_table()?;
    meta.set("__index", sandbox.clone())?;
    env.set_metatable(Some(meta));
    Ok(env)
}

fn load_file(lua: &Lua, root: &Path, file: &Path, env: Table) -> mlua::Result<Function> {
    let name = relative_name(root, file);
    let source = fs::read(file).map_err(|_| mlua::Error::runtime(format!("cannot read {name}")))?;
    lua.load(source)
        .set_name(format!("@{name}"))
        .set_mode(ChunkMode::Text)
        .set_environment(env)
        .into_function()
}

/// Converts a controller result to JSON. Tables with exactly the keys 1..n
/// become arrays; functions and other non-data values become null.
fn to_json(value: &LuaValue, depth: usize) -> Result<Value, String> {
    let table = match value {
        LuaValue::Boolean(b) => return Ok(Value::Bool(*b)),
        LuaValue::Integer(i) => return Ok(Value::from(*i)),
        LuaValue::Number(n) => return Ok(serde_json::Number::from_f64(*n).map(Value::Number).unwrap_or(Value::Null)),
        LuaValue::String(s) => return Ok(Value::String(s.to_string_lossy().to_string())),
        LuaValue::Table(table) => table,
        _ => return Ok(Value::Null),
    };
    if depth >= MAX_MODEL_DEPTH {
        return Err("The controller result is nested too deeply".into());
    }

    let mut object = Map::new();
    let mut sequence_keys = 0i64;
    let mut max_key = 0i64;
    for pair in table.pairs::<LuaValue, LuaValue>() {
        let (key, item) = pair.map_err(|e| e.to_string())?;
        let key = match key {
            LuaValue::Integer(n) => {
                if n >= 1 {
                    sequence_keys += 1;
                    max_key = max_key.max(n);
                }
                n.to_string()
            }
            LuaValue::String(s) => s.to_string_lossy().to_string(),
            _ => continue,
        };
        object.insert(key, to_json(&item, depth + 1)?);
    }

    if sequence_keys == 0 || sequence_keys as usize != object.len() || max_key != sequence_keys {
        return Ok(Value::Object(object));
    }
    Ok(Value::Array(
        (1..=max_key)
            .map(|i| object.remove(&i.to_string()).unwrap_or(Value::Null))
            .collect(),
    ))
}

impl SPNativeApp {
    pub fn is_native(bundle: &SPAppBundle) -> bool {
        string_field(&bundle.manifest, "AppType") == "Native"
    }

    /// Fails if a routes file is invalid.
    pub fn new(bundle: &SPAppBundle) -> Result<Self, String> {
        let root = lexically_normal(Path::new(&bundle.path));
        let lua = Lua::new();
        let result_meta = lua.create_table().map_err(|e| e.to_string())?;
        let instructions = Rc::new(Cell::new(0));
        let instruction_limit = Rc::new(Cell::new(DEFAULT_INSTRUCTION_LIMIT));
        let sandbox = Self::open_sandbox(&lua, &root, &bundle.manifest, &result_meta).map_err(|e| e.to_string())?;

        // Coroutines inherit the hook.
        {
            let (instructions, limit) = (instructions.clone(), instruction_limit.clone());
            lua.set_hook(HookTriggers::new().every_nth_instruction(HOOK_INTERVAL), move |_, _| {
                instructions.set(instructions.get() + HOOK_INTERVAL as i64);
                if instructions.get() > limit.get() {
                    return Err(mlua::Error::runtime("the request exceeded the instruction limit"));
                }
                Ok(VmState::Continue)
            });
        }

        let mut app = Self {
            root: root.clone(),
            routes: Vec::new(),
            lua,
            sandbox,
            result_meta,
            controllers: RefCell::default(),
            instruction_limit,
            instructions,
        };

        let routes_file = root.join("routes.json");
        if routes_file.is_file() {
            app.load_routes(&routes_file)?;
        }

        if let Some(Value::Object(features)) = bundle.manifest.get("Features") {
            for (name, feature) in features {
                let routes = string_field(feature, "Routes");
                let file = (!routes.is_empty())
                    .then(|| resolve_inside(&root, &root, &routes))
                    .flatten()
                    .ok_or_else(|| format!("Feature \"{name}\" needs a Routes file inside the app"))?;
                app.load_routes(&file)?;
            }
        }
        Ok(app)
    }

    /// Lua instructions one request may run (approximately).
    pub fn set_instruction_limit(&self, value: i64) {
        self.instruction_limit.set(value);
    }

    fn load_routes(&mut self, path: &Path) -> Result<(), String> {
        let name = relative_name(&self.root, path);
        let text = fs::read_to_string(path).map_err(|_| format!("Cannot read {name}"))?;
        // An empty routes file declares no routes.
        if text.trim().is_empty() {
            return Ok(());
        }
        let json: Value = serde_json::from_str(&text).map_err(|e| format!("{name} is not valid JSON: {e}"))?;
        let Some(Value::Array(routes)) = json.get("routes") else {
            return Err(format!("{name} has no \"routes\" array"));
        };

        let base = path.parent().unwrap_or(&self.root).to_path_buf();
        for entry in routes {
            let pattern = string_field(entry, "path");
            let view = string_field(entry, "view");
            let controller = string_field(entry, "controller");
            if pattern.is_empty() || view.is_empty() || controller.is_empty() {
                return Err(format!("{name}: every route needs a path, view and controller"));
            }
            let (Some(view), Some(controller)) = (
                resolve_file(&self.root, &base, &view, ".xml"),
                resolve_file(&self.root, &base, &controller, ".lua"),
            ) else {
                return Err(format!("{name}: route {pattern} refers to a file outside the app"));
            };
            // The whole URI must match, like std::regex_match.
            let path = Regex::new(&format!("^(?:{pattern})$"))
                .map_err(|e| format!("{name}: invalid route path {pattern}: {e}"))?;
            self.routes.push(Route {
                path,
                base: base.clone(),
                view,
                controller,
            });
        }
        Ok(())
    }

    fn open_sandbox(lua: &Lua, root: &Path, manifest: &Value, result_meta: &Table) -> mlua::Result<Table> {
        let globals = lua.globals();
        let mut permissions = BTreeSet::new();
        if let Some(Value::Array(requested)) = manifest.get("ApiPermissions") {
            for entry in requested {
                let permission = entry.as_str().map(str::to_string).unwrap_or_else(|| entry.to_string());
                if PERMISSION_LIBRARIES.contains(&permission.as_str()) {
                    permissions.insert(permission);
                } else {
                    log::warn!("Ignoring unknown ApiPermission {permission} in {}", root.display());
                }
            }
        }

        let sandbox = lua.create_table()?;
        for name in BASE_FUNCTIONS.iter().chain(SAFE_LIBRARIES.iter()) {
            sandbox.set(*name, globals.get::<LuaValue>(*name)?)?;
        }
        for name in &permissions {
            sandbox.set(name.as_str(), globals.get::<LuaValue>(name.as_str())?)?;
        }
        if !permissions.contains("os") {
            let os: Table = globals.get("os")?;
            let safe_os = lua.create_table()?;
            for name in SAFE_OS_FUNCTIONS {
                safe_os.set(name, os.get::<LuaValue>(name)?)?;
            }
            sandbox.set("os", safe_os)?;
        }
        sandbox.set("_G", sandbox.clone())?;

        // require(name): loads <bundle>/<name with dots as slashes>.lua once,
        // in its own environment.
        let loaded = lua.create_table()?;
        let (require_root, require_sandbox) = (root.to_path_buf(), sandbox.clone());
        sandbox.set(
            "require",
            lua.create_function(move |lua, name: String| {
                let cached: LuaValue = loaded.get(name.as_str())?;
                if !cached.is_nil() {
                    return Ok(cached);
                }
                let relative = format!("{}.lua", name.replace('.', "/"));
                let Some(file) = resolve_inside(&require_root, &require_root, &relative) else {
                    return Err(mlua::Error::runtime(format!("module '{name}' is outside the app")));
                };
                let env = new_environment(lua, &require_sandbox)?;
                let chunk = load_file(lua, &require_root, &file, env)?;
                let mut value: LuaValue = chunk.call(name.as_str())?;
                if value.is_nil() {
                    value = LuaValue::Boolean(true);
                }
                loaded.set(name.as_str(), value.clone())?;
                Ok(value)
            })?,
        )?;

        // redirect(uri): a controller result that navigates to uri.
        let meta = result_meta.clone();
        sandbox.set(
            "redirect",
            lua.create_function(move |lua, uri: String| {
                let result = lua.create_table()?;
                result.set("kind", "redirect")?;
                result.set("uri", uri)?;
                result.set_metatable(Some(meta.clone()));
                Ok(result)
            })?,
        )?;

        // render(view, model): a controller result that renders another view.
        let meta = result_meta.clone();
        sandbox.set(
            "render",
            lua.create_function(move |lua, (view, model): (String, Option<Table>)| {
                let result = lua.create_table()?;
                result.set("kind", "render")?;
                result.set("view", view)?;
                result.set("model", model)?;
                result.set_metatable(Some(meta.clone()));
                Ok(result)
            })?,
        )?;
        Ok(sandbox)
    }

    /// The controller's environment, running the file the first time.
    fn controller(&self, path: &Path) -> mlua::Result<Table> {
        if let Some(env) = self.controllers.borrow().get(path) {
            return Ok(env.clone());
        }
        let env = new_environment(&self.lua, &self.sandbox)?;
        load_file(&self.lua, &self.root, path, env.clone())?.call::<()>(())?;
        self.controllers.borrow_mut().insert(path.to_path_buf(), env.clone());
        Ok(env)
    }

    fn is_result(&self, table: &Table) -> bool {
        table
            .metatable()
            .is_some_and(|meta| meta.to_pointer() == self.result_meta.to_pointer())
    }

    fn request_table(&self, route: &Route, uri: &str, method: &str, action: &str, data: &Value) -> mlua::Result<Table> {
        let request = self.lua.create_table()?;
        request.set("method", method)?;
        request.set("uri", uri)?;
        request.set("action", action)?;
        request.set("data", SPTemplate::push_json(&self.lua, data)?)?;
        let arguments = self.lua.create_table()?;
        if let Some(captures) = route.path.captures(uri) {
            for (i, name) in route.path.capture_names().enumerate().skip(1) {
                let Some(m) = captures.get(i) else {
                    continue;
                };
                arguments.raw_set(i as i64 - 1, m.as_str())?;
                if let Some(name) = name {
                    arguments.raw_set(name, m.as_str())?;
                }
            }
        }
        request.set("arguments", arguments)?;
        Ok(request)
    }

    /// Runs the route matching `uri`. `data` is the posted form data; its
    /// "command" is the request action. Fails if no route matches or the
    /// controller or view fails.
    pub fn handle(&self, method: &str, uri: &str, data: &Value) -> Result<Response, String> {
        let action = string_field(data, "command");
        let lua_error = |e: mlua::Error| match e {
            mlua::Error::RuntimeError(message) | mlua::Error::SyntaxError { message, .. } => message,
            mlua::Error::CallbackError { cause, .. } => cause.to_string(),
            other => other.to_string(),
        };
        let Some(route) = self.routes.iter().find(|r| r.path.is_match(uri)) else {
            return Err(format!("No route matches {uri}"));
        };

        let mut view_path = route.view.clone();
        let mut model = Value::Object(Map::new());
        self.instructions.set(0);
        let env = self.controller(&route.controller).map_err(lua_error)?;
        let function_name = route
            .controller
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let Ok(function) = env.get::<Function>(function_name.as_str()) else {
            return Err(format!(
                "{} does not define a function {function_name}()",
                relative_name(&self.root, &route.controller)
            ));
        };
        let request = self.request_table(route, uri, method, &action, data).map_err(lua_error)?;
        let mut result: LuaValue = function.call(request).map_err(lua_error)?;

        if let LuaValue::Table(table) = &result {
            if self.is_result(table) {
                let kind: String = table.get("kind").unwrap_or_default();
                if kind == "redirect" {
                    return Ok(Response {
                        xul: String::new(),
                        redirect: table.get("uri").unwrap_or_default(),
                    });
                }
                let view: String = table.get("view").unwrap_or_default();
                view_path = resolve_file(&self.root, &route.base, &view, ".xml")
                    .ok_or_else(|| format!("{function_name}() renders a view outside the app: {view}"))?;
                result = table.get("model").unwrap_or(LuaValue::Nil);
            }
        }
        match &result {
            LuaValue::Table(_) => model = to_json(&result, 0)?,
            LuaValue::Nil => {}
            _ => return Err(format!("{function_name}() must return a table, redirect(), render() or nil")),
        }

        let view = fs::read_to_string(&view_path)
            .map_err(|_| format!("Cannot read {}", relative_name(&self.root, &view_path)))?;
        let xul = SPTemplate::render(&view, method, uri, &action, data, &model, true)?;
        Ok(Response { xul, redirect: String::new() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bundle(files: &[(&str, &str)], manifest: Value) -> (tempfile::TempDir, SPAppBundle) {
        let dir = tempfile::tempdir().unwrap();
        for (name, content) in files {
            let path = dir.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        let bundle = SPAppBundle {
            path: dir.path().to_string_lossy().into_owned(),
            manifest,
            ..Default::default()
        };
        (dir, bundle)
    }

    const ROUTES: &str = r#"{"routes": [
        {"path": "bungalow:app:x:(?P<name>\\w+)", "view": "index", "controller": "index"},
        {"path": "bungalow:app:x", "view": "index", "controller": "home"}]}"#;

    #[test]
    fn routes_controllers_and_views() {
        let (_dir, bundle) = bundle(
            &[
                ("routes.json", ROUTES),
                (
                    "index.lua",
                    "count = 0\nfunction index(request)\n count = count + 1\n return { name = request.arguments.name, first = request.arguments[0], count = count, items = {'a', 'b'} }\nend",
                ),
                ("home.lua", "function home(request) return redirect('bungalow:app:x:home') end"),
                ("index.xml", "<text>${name} ${first} ${count} ${#items}</text>"),
            ],
            json!({ "AppType": "Native" }),
        );
        assert!(SPNativeApp::is_native(&bundle));
        let app = SPNativeApp::new(&bundle).unwrap();
        assert_eq!(app.handle("GET", "bungalow:app:x:joe", &json!({})).unwrap().xul, "<text>joe joe 1 2</text>");
        // Controllers keep state between requests.
        assert_eq!(app.handle("GET", "bungalow:app:x:joe", &json!({})).unwrap().xul, "<text>joe joe 2 2</text>");
        assert_eq!(app.handle("GET", "bungalow:app:x", &json!({})).unwrap().redirect, "bungalow:app:x:home");
        assert!(app.handle("GET", "bungalow:app:y", &json!({})).unwrap_err().contains("No route"));
    }

    #[test]
    fn sandbox_blocks_io_and_runaway_loops() {
        let (_dir, bundle) = bundle(
            &[
                ("routes.json", r#"{"routes": [{"path": "a:io", "view": "v", "controller": "io"}, {"path": "a:loop", "view": "v", "controller": "loop"}, {"path": "a:mod", "view": "v", "controller": "mod"}]}"#),
                ("io.lua", "function io(request) return { t = type(_G.io), os = type(os.execute) } end"),
                ("loop.lua", "function loop() while true do end end"),
                ("mod.lua", "local m = require('lib.util')\nfunction mod() return { v = m.value } end"),
                ("lib/util.lua", "return { value = 42 }"),
                ("v.xml", "${t}${os}${v}"),
            ],
            json!({}),
        );
        let app = SPNativeApp::new(&bundle).unwrap();
        assert_eq!(app.handle("GET", "a:io", &json!({})).unwrap().xul, "nilnil");
        assert_eq!(app.handle("GET", "a:mod", &json!({})).unwrap().xul, "42");
        app.set_instruction_limit(100_000);
        assert!(app.handle("GET", "a:loop", &json!({})).unwrap_err().contains("instruction limit"));
    }

    #[test]
    fn to_json_detects_arrays() {
        let lua = Lua::new();
        let value: LuaValue = lua.load("return { 1, 2, { a = true } }").eval().unwrap();
        assert_eq!(to_json(&value, 0).unwrap(), json!([1, 2, { "a": true }]));
        let value: LuaValue = lua.load("return { [1] = 1, [3] = 3 }").eval().unwrap();
        assert_eq!(to_json(&value, 0).unwrap(), json!({ "1": 1, "3": 3 }));
    }
}
