//! Регистрация глобалов в Lua: `now_hm`, `now_hms`, `Color`, `Rect`, `json`.
//! Это "статические" инструменты — доступны всегда, независимо от
//! `buf`, `font`, `socket` (их регистрирует `mod.rs`).

use mlua::{Lua, Table};

use crate::core::Color;
use crate::widgets::util::{now_hm, now_hms};

use super::json;

pub fn install_api(lua: &Lua, globals: &Table) -> mlua::Result<()> {
    // now_hm() -> "14:35"
    globals.set("now_hm", lua.create_function(|_, ()| Ok(now_hm()))?)?;
    // now_hms() -> "14:35:07"
    globals.set("now_hms", lua.create_function(|_, ()| Ok(now_hms()))?)?;

    // Color.rgb(r, g, b) -> u32 (0xAARRGGBB)
    let color_t = lua.create_table()?;
    color_t.set(
        "rgb",
        lua.create_function(|_, (r, g, b): (u8, u8, u8)| Ok(Color::rgb(r, g, b).0))?,
    )?;
    globals.set("Color", color_t)?;

    // Rect.new(x, y, w, h) -> { x, y, w, h }
    let rect_t = lua.create_table()?;
    rect_t.set(
        "new",
        lua.create_function(|lua, (x, y, w, h): (i32, i32, i32, i32)| {
            let t = lua.create_table()?;
            t.set("x", x)?;
            t.set("y", y)?;
            t.set("w", w)?;
            t.set("h", h)?;
            Ok(t)
        })?,
    )?;
    globals.set("Rect", rect_t)?;

    // json.decode / json.encode
    json::install(lua, globals)?;

    Ok(())
}