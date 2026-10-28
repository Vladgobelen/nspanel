//! Lua-виджеты: общие инструменты (`api`, `buf`, `font`, `helpers`,
//! `json`, `size`, `socket`) и сам `LuaWidget`.
//!
//! `LuaWidget` разбит по файлам в `widget/`:
//! - `widget/mod.rs`   — `struct LuaWidget` + `load` + `install_*`;
//! - `widget/adapter.rs` — `impl Widget for LuaWidget`;
//! - `widget/image.rs` — `LuaImage` (глобал `img` для Lua).
//!
//! Общие инструменты (не привязаны к конкретному виджету):
//! - `api.rs`     — `now_hm`, `now_hms`, `Color`, `Rect`, `json`;
//! - `buf.rs`     — `buf:*` (рисование);
//! - `font.rs`    — `font:text_width`;
//! - `helpers.rs` — `table_to_rect`, `rect_to_table`, `lua_value_to_action`;
//! - `json.rs`    — `json.decode` / `json.encode`;
//! - `size.rs`    — `size:set_length`;
//! - `socket.rs`  — `socket:read` / `socket:write`.

pub mod api;
pub mod buf;
pub mod font;
pub mod helpers;
pub mod json;
pub mod size;
pub mod socket;
pub mod widget;

pub use widget::LuaWidget;