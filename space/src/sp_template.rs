//! Port of `SPTemplate`: renders Lua-templated XUL views.
//!
//! A template is elua markup plus two shorthands:
//! * a line starting with `%` is Lua code (`% for i, n do` counts to `n`),
//! * `${expr}` prints `expr`.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use mlua::{Lua, MultiValue, Value as LuaValue};
use regex::Regex;
use serde_json::Value;

use elua::{elua_loadbuffer, EluaContext};

pub struct SPTemplate;

fn convert_template(source: &str) -> String {
    static COUNT_LOOP: OnceLock<Regex> = OnceLock::new();
    let count_loop = COUNT_LOOP.get_or_init(|| Regex::new(r"^\s*for\s+([A-Za-z_]\w*)\s*,\s*(.+?)\s+do\s*$").unwrap());
    let mut converted = String::new();
    let mut lines = source.split('\n').peekable();

    while let Some(line) = lines.next() {
        let has_newline = lines.peek().is_some();
        let trimmed = line.trim_start_matches([' ', '\t', '\r']);
        if let Some(code) = trimmed.strip_prefix('%') {
            let code = match count_loop.captures(code) {
                Some(m) => format!("for {}=1,{} do", &m[1], &m[2]),
                None => code.to_string(),
            };
            converted.push_str("<%");
            converted.push_str(&code);
            converted.push_str("%>");
        } else {
            let mut rest = line;
            loop {
                let Some(open) = rest.find("${") else {
                    converted.push_str(rest);
                    break;
                };
                let Some(close) = rest[open + 2..].find('}').map(|c| c + open + 2) else {
                    converted.push_str(rest);
                    break;
                };
                converted.push_str(&rest[..open]);
                converted.push_str("<%=");
                converted.push_str(&rest[open + 2..close]);
                converted.push_str("%>");
                rest = &rest[close + 1..];
            }
            if has_newline {
                converted.push('\n');
            }
        }
    }
    converted
}

/// Escapes text so entity data is safe to interpolate into XUL markup.
pub fn escape_xml(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// `tostring()` of a Lua value, like `luaL_tolstring`.
pub fn lua_to_display(lua: &Lua, value: &LuaValue) -> String {
    match value {
        LuaValue::Nil => "nil".into(),
        LuaValue::Boolean(b) => b.to_string(),
        LuaValue::String(s) => s.to_string_lossy().to_string(),
        LuaValue::Integer(i) => i.to_string(),
        LuaValue::Number(n) => format_number(*n),
        other => lua
            .globals()
            .get::<mlua::Function>("tostring")
            .and_then(|f| f.call::<String>(other.clone()))
            .unwrap_or_default(),
    }
}

impl SPTemplate {
    /// With `model_globals`, each field of an object model is also a template
    /// variable, so `${name}` reads `model.name`.
    pub fn render(
        source: &str,
        method: &str,
        uri: &str,
        action: &str,
        data: &Value,
        model: &Value,
        model_globals: bool,
    ) -> Result<String, String> {
        let lua = Lua::new();
        Self::render_with(&lua, source, method, uri, action, data, model, model_globals).map_err(|e| e.to_string())
    }

    #[allow(clippy::too_many_arguments)]
    fn render_with(
        lua: &Lua,
        source: &str,
        method: &str,
        uri: &str,
        action: &str,
        data: &Value,
        model: &Value,
        model_globals: bool,
    ) -> mlua::Result<String> {
        let output = Rc::new(RefCell::new(String::new()));
        let globals = lua.globals();

        let sink = output.clone();
        globals.set(
            "print",
            lua.create_function(move |_, args: MultiValue| {
                append_print(&mut sink.borrow_mut(), &args);
                Ok(())
            })?,
        )?;
        globals.set(
            "escape",
            lua.create_function(|lua, value: LuaValue| {
                if value.is_nil() {
                    return Ok(String::new());
                }
                Ok(escape_xml(&lua_to_display(lua, &value)))
            })?,
        )?;
        globals.set("method", method)?;
        globals.set("uri", uri)?;
        globals.set("action", action)?;
        globals.set("data", Self::push_json(lua, data)?)?;
        globals.set("model", Self::push_json(lua, model)?)?;
        if model_globals {
            if let Value::Object(map) = model {
                for (key, item) in map {
                    globals.set(key.as_str(), Self::push_json(lua, item)?)?;
                }
            }
        }

        let converted = convert_template(source);
        let mut context = EluaContext::new();
        let chunk = elua_loadbuffer(lua, &converted, "spider-template", &mut context)?;
        chunk.call::<()>(())?;
        let result = output.borrow().clone();
        Ok(result)
    }

    /// Converts a JSON value to Lua (arrays become 1-based tables).
    pub fn push_json(lua: &Lua, value: &Value) -> mlua::Result<LuaValue> {
        Ok(match value {
            Value::Null => LuaValue::Nil,
            Value::Bool(b) => LuaValue::Boolean(*b),
            Value::Number(n) => match n.as_i64() {
                Some(i) => LuaValue::Integer(i),
                None => LuaValue::Number(n.as_f64().unwrap_or(0.0)),
            },
            Value::String(s) => LuaValue::String(lua.create_string(s)?),
            Value::Array(items) => {
                let table = lua.create_table_with_capacity(items.len(), 0)?;
                for (i, item) in items.iter().enumerate() {
                    table.raw_set(i + 1, Self::push_json(lua, item)?)?;
                }
                LuaValue::Table(table)
            }
            Value::Object(map) => {
                let table = lua.create_table_with_capacity(0, map.len())?;
                for (key, item) in map {
                    table.raw_set(key.as_str(), Self::push_json(lua, item)?)?;
                }
                LuaValue::Table(table)
            }
        })
    }
}

/// The template `print`: tab-separated, nil prints nothing, no newline.
pub fn append_print(output: &mut String, args: &MultiValue) {
    for (i, value) in args.iter().enumerate() {
        if i > 0 {
            output.push('\t');
        }
        match value {
            LuaValue::Nil => {}
            LuaValue::Boolean(b) => output.push_str(if *b { "true" } else { "false" }),
            LuaValue::String(s) => output.push_str(&s.to_string_lossy()),
            LuaValue::Integer(n) => output.push_str(&n.to_string()),
            LuaValue::Number(n) => output.push_str(&format_number(*n)),
            _ => {}
        }
    }
}

/// Formats a float like Lua 5.4's `%.14g`.
pub fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.is_finite() && n.abs() < 1e15 {
        format!("{n:.1}")
    } else {
        format!("{n}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn converts_shorthands() {
        assert_eq!(convert_template("% for i, 3 do\n<a>${i}</a>\n% end"), "<%for i=1,3 do%><a><%=i%></a>\n<% end%>");
    }

    #[test]
    fn renders_model_and_escape() {
        let xul = SPTemplate::render(
            "<vbox>\n% for i, #model.items do\n<label>${escape(model.items[i])}</label>\n% end\n</vbox>",
            "GET",
            "bungalow:start",
            "",
            &json!({}),
            &json!({ "items": ["a&b", "c"] }),
            false,
        )
        .unwrap();
        assert_eq!(xul, "<vbox>\n<label>a&amp;b</label>\n<label>c</label>\n</vbox>");
    }

    #[test]
    fn model_globals() {
        let xul =
            SPTemplate::render("${name} ${uri}", "GET", "x:y", "", &json!({}), &json!({ "name": "N" }), true).unwrap();
        assert_eq!(xul, "N x:y");
    }

    #[test]
    fn errors_are_reported() {
        assert!(SPTemplate::render("% if then", "GET", "", "", &json!({}), &json!({}), false).is_err());
    }
}
