//! Общие утилиты для Lua-виджетов:
//! - `table_to_rect` — Lua-таблица `{x,y,w,h}` → `Rect`.
//! - `rect_to_table` — `Rect` → Lua-таблица `{x,y,w,h}`.
//! - `lua_value_to_action` — Lua-таблица `{type="log"|"run"|"custom", ...}` → `Action`.

use mlua::{Lua, Table, Value};

use crate::core::{Action, Rect};

pub fn table_to_rect(t: &Table) -> Rect {
    Rect::new(
        t.get("x").unwrap_or(0),
        t.get("y").unwrap_or(0),
        t.get("w").unwrap_or(0),
        t.get("h").unwrap_or(0),
    )
}

pub fn rect_to_table(lua: &Lua, r: Rect) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("x", r.x)?;
    t.set("y", r.y)?;
    t.set("w", r.w)?;
    t.set("h", r.h)?;
    Ok(t)
}

pub fn lua_value_to_action(v: &Value) -> Action {
    let t = match v {
        Value::Table(t) => t,
        _ => return Action::None,
    };
    let kind: String = t.get("type").unwrap_or_default();
    match kind.as_str() {
        "log" => Action::Log(t.get::<String>("msg").unwrap_or_default()),
        "run" => Action::RunCommand(t.get::<String>("cmd").unwrap_or_default()),
        "custom" => Action::Custom(t.get::<String>("msg").unwrap_or_default()),
        _ => Action::None,
    }
}