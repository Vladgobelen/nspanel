//! `json:*` — парсинг/сериализация JSON для обмена с мостом.
//!
//! - `json.decode(str)` — JSON-строка → Lua-таблица/значение.
//! - `json.encode(value)` — Lua-значение → JSON-строка.
//!
//! Мост (nsbridge) шлёт JSON, виджет парсит его через `json.decode`
//! в `M.on_socket_ready`. Обратно (команда `activate`) — `json.encode`.

use mlua::{Lua, LuaString, Table, Value};

pub fn install(lua: &Lua, globals: &Table) -> mlua::Result<()> {
    let json_t = lua.create_table()?;

    json_t.set(
        "decode",
        lua.create_function(|lua, s: LuaString| {
            let val: serde_json::Value =
                serde_json::from_slice(&s.as_bytes()).map_err(mlua::Error::external)?;
            json_to_lua(lua, &val)
        })?,
    )?;

    json_t.set(
        "encode",
        lua.create_function(|_, val: Value| {
            let json_val = lua_to_json(&val);
            serde_json::to_string(&json_val).map_err(mlua::Error::external)
        })?,
    )?;

    globals.set("json", json_t)?;
    Ok(())
}

/// `serde_json::Value` → Lua-значение.
fn json_to_lua(lua: &Lua, val: &serde_json::Value) -> mlua::Result<Value> {
    Ok(match val {
        serde_json::Value::Null => Value::Nil,
        serde_json::Value::Bool(b) => Value::Boolean(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                Value::Number(f)
            } else {
                Value::Nil
            }
        }
        serde_json::Value::String(s) => Value::String(lua.create_string(s)?),
        serde_json::Value::Array(arr) => {
            let t = lua.create_table()?;
            for (i, v) in arr.iter().enumerate() {
                t.set(i + 1, json_to_lua(lua, v)?)?;
            }
            Value::Table(t)
        }
        serde_json::Value::Object(obj) => {
            let t = lua.create_table()?;
            for (k, v) in obj {
                t.set(k.clone(), json_to_lua(lua, v)?)?;
            }
            Value::Table(t)
        }
    })
}

/// Lua-значение → `serde_json::Value`.
fn lua_to_json(val: &Value) -> serde_json::Value {
    match val {
        Value::Nil => serde_json::Value::Null,
        Value::Boolean(b) => serde_json::Value::Bool(*b),
        Value::Integer(i) => serde_json::Value::Number((*i).into()),
        Value::Number(n) => serde_json::Number::from_f64(*n)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::String(s) => serde_json::Value::String(s.to_string_lossy().to_string()),
        Value::Table(t) => {
            let len = t.raw_len();
            let is_array = len > 0
                && t.clone()
                    .pairs::<Value, Value>()
                    .filter_map(|p| p.ok())
                    .all(|(k, _)| matches!(k, Value::Integer(_)));
            if is_array {
                let mut arr = Vec::new();
                for v in t.clone().sequence_values::<Value>().flatten() {
                    arr.push(lua_to_json(&v));
                }
                serde_json::Value::Array(arr)
            } else {
                let mut obj = serde_json::Map::new();
                for pair in t.clone().pairs::<String, Value>() {
                    if let Ok((k, v)) = pair {
                        obj.insert(k, lua_to_json(&v));
                    }
                }
                serde_json::Value::Object(obj)
            }
        }
        _ => serde_json::Value::Null,
    }
}