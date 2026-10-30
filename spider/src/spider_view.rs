//! Port of `SpiderView`: an elua template served for the URIs it accepts.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use elua::{elua_loadfile, EluaContext};
use mlua::Lua;

/// A view in a [`crate::Spider`] server. Each view owns its own Lua state.
pub struct SpiderView {
    lua: Lua,
    elua_ctx: EluaContext,
    template: Option<PathBuf>,
    prefix: String,
}

impl SpiderView {
    /// Creates a view that accepts URIs starting with `prefix`.
    pub fn new(prefix: &str) -> Self {
        Self { lua: Lua::new(), elua_ctx: EluaContext::new(), template: None, prefix: prefix.to_string() }
    }

    pub fn set_template(&mut self, path: &Path) {
        self.template = Some(path.to_path_buf());
    }

    pub fn accepts_uri(&self, uri: &str) -> bool {
        uri.starts_with(&self.prefix)
    }

    /// Runs the template with `method`, `path` and `data` as globals and
    /// returns the markup it printed.
    pub fn process(&mut self, method: &str, path: &str, data: &BTreeMap<String, String>) -> Result<String, String> {
        let Some(template) = self.template.clone() else {
            return Ok(String::new());
        };
        let globals = self.lua.globals();
        let table = self.lua.create_table().map_err(|e| e.to_string())?;
        for (key, value) in data {
            table.set(key.as_str(), value.as_str()).map_err(|e| e.to_string())?;
        }
        globals.set("method", method).map_err(|e| e.to_string())?;
        globals.set("path", path).map_err(|e| e.to_string())?;
        globals.set("data", table).map_err(|e| e.to_string())?;

        let output = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
        let sink = output.clone();
        let print = self
            .lua
            .create_function(move |_, values: mlua::Variadic<mlua::Value>| {
                let mut out = sink.borrow_mut();
                for value in values.iter() {
                    out.push_str(&value.to_string().unwrap_or_default());
                }
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        globals.set("print", print).map_err(|e| e.to_string())?;

        let chunk = elua_loadfile(&self.lua, &template, &mut self.elua_ctx).map_err(|e| format!("Load error: {e}"))?;
        chunk.call::<()>(()).map_err(|e| format!("Runtime error: {e}\n{}", self.elua_ctx.output()))?;
        let result = output.borrow().clone();
        Ok(result)
    }
}
