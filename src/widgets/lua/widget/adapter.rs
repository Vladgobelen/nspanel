//! `impl Widget for LuaWidget`: трансляция Rust-трейта `Widget`
//! в Lua-таблицу `M` (`render`, `on_click`, `on_hover`, `tooltip`,
//! `menu_items`, `on_custom_action`, `update`, `set_socket`,
//! `on_socket_ready`).
//!
//! Одна ответственность: соответствие «метод трейта `Widget`» ↔
//! «функция в `widget.lua`». Каждый метод:
//! 1. подкладывает в глобалы актуальные `size`/`socket` (и `buf`/`font`
//!    для `render`);
//! 2. зовёт `M.<name>` (если функции нет — возвращает дефолт);
//! 3. конвертирует Lua-результат в Rust-тип.
//!
//! Здесь НЕТ:
//! - `struct LuaWidget` и `load` (это `mod.rs`);
//! - `LuaImage` (это `image.rs`).

use std::sync::atomic::Ordering;
use std::time::Duration;

use mlua::{Function, Table, Value};

use crate::core::{Action, Rect, SizeMode, Theme, Widget, WidgetId};
use crate::core::ipc::UnixStreamClient;
use crate::render::SharedBuffer;

use super::super::helpers::{lua_value_to_action, rect_to_table};
use super::LuaWidget;

impl Widget for LuaWidget {
    fn id(&self) -> WidgetId {
        self.id.clone()
    }

    fn size_mode(&self) -> SizeMode {
        self.sync_length_from_api();
        let margin: u32 = self.api.get("margin").unwrap_or(1);
        match self.length.get() {
            Some(length) => SizeMode::Fixed { length, margin },
            None => {
                let mode: String = self
                    .api
                    .get("size_mode")
                    .unwrap_or_else(|_| "stretch".into());
                if mode == "fixed" {
                    let length: u32 = self.api.get("length").unwrap_or(80);
                    SizeMode::Fixed { length, margin }
                } else {
                    let ratio: f32 = self.api.get("ratio").unwrap_or(2.0);
                    SizeMode::Stretch { margin, ratio }
                }
            }
        }
    }

    fn preferred_size(&self, panel_thickness: u32, vertical: bool) -> (u32, u32) {
        self.size_mode().preferred_size(panel_thickness, vertical)
    }

    fn needs_relayout(&mut self) -> bool {
        let dirty = self.relayout_dirty.swap(false, Ordering::Relaxed);
        if dirty {
            self.sync_length_from_api();
        }
        dirty
    }

    fn render(&self, buf: &SharedBuffer, area: Rect, theme: &Theme) {
        self.install_buf(buf, theme);
        self.install_size();
        self.install_socket();

        let lua = &self.lua;
        if let Ok(theme_t) = lua.create_table() {
            let _ = theme_t.set("bg", theme.bg.0);
            let _ = theme_t.set("fg", theme.fg.0);
            let _ = theme_t.set("font_size", theme.font_size);

            if let Ok(render_fn) = self.api.get::<Function>("render") {
                if let Ok(area_t) = rect_to_table(lua, area) {
                    if let Err(e) = render_fn.call::<()>((&self.state, &area_t, &theme_t)) {
                        eprintln!("[lua] render error: {e}");
                    }
                }
            }
        }
    }

    fn on_click(&mut self, x: i32, y: i32) -> Action {
        self.install_size();
        self.install_socket();
        let f = match self.api.get::<Function>("on_click") {
            Ok(f) => f,
            Err(_) => return Action::None,
        };
        match f.call::<Value>((&self.state, x, y)) {
            Ok(v) => lua_value_to_action(&v),
            Err(e) => {
                eprintln!("[lua] on_click error: {e}");
                Action::None
            }
        }
    }

    fn on_hover(&mut self, x: i32, y: i32) -> bool {
        let f = match self.api.get::<Function>("on_hover") {
            Ok(f) => f,
            Err(_) => return false,
        };
        match f.call::<Value>((&self.state, x, y)) {
            Ok(Value::Boolean(b)) => b,
            Ok(_) => false,
            Err(e) => {
                eprintln!("[lua] on_hover error: {e}");
                false
            }
        }
    }

    fn tooltip(&mut self, x: i32, y: i32) -> Option<Vec<String>> {
        if !self.tooltip_enabled {
            return None;
        }
        let f = match self.api.get::<Function>("tooltip") {
            Ok(f) => f,
            Err(_) => return None,
        };
        let v = match f.call::<Value>((&self.state, x, y)) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[lua] tooltip error: {e}");
                return None;
            }
        };
        let t = match v {
            Value::Table(t) => t,
            _ => return None,
        };
        let mut lines = Vec::new();
        for item in t.sequence_values::<String>() {
            match item {
                Ok(s) => lines.push(s),
                Err(_) => break,
            }
        }
        if lines.is_empty() {
            None
        } else {
            Some(lines)
        }
    }

    fn menu_items(&self, x: i32, y: i32) -> Vec<(String, Action)> {
        let f = match self.api.get::<Function>("menu_items") {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };
        let v: Value = match f.call((&self.state, x, y)) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[lua] menu_items error: {e}");
                return Vec::new();
            }
        };
        let t = match v {
            Value::Table(t) => t,
            _ => return Vec::new(),
        };
        let mut out = Vec::new();
        for pair in t.sequence_values::<Table>() {
            let item = match pair {
                Ok(i) => i,
                Err(_) => continue,
            };
            let label: String = item.get("label").unwrap_or_default();
            if label.is_empty() {
                continue;
            }
            let action = match item.get::<Value>("action") {
                Ok(v) => lua_value_to_action(&v),
                Err(_) => Action::None,
            };
            out.push((label, action));
        }
        out
    }

    fn on_custom_action(&mut self, msg: &str) -> bool {
        self.install_size();
        let f = match self.api.get::<Function>("on_custom_action") {
            Ok(f) => f,
            Err(_) => return false,
        };
        match f.call::<Value>((&self.state, msg)) {
            Ok(Value::Boolean(b)) => b,
            Ok(_) => false,
            Err(e) => {
                eprintln!("[lua] on_custom_action error: {e}");
                false
            }
        }
    }

    fn update(&mut self) -> Option<Duration> {
        self.install_size();
        let f = match self.api.get::<Function>("update") {
            Ok(f) => f,
            Err(_) => return None,
        };
        match f.call::<Value>((&self.state,)) {
            Ok(Value::Integer(ms)) => {
                if ms <= 0 { None } else { Some(Duration::from_millis(ms as u64)) }
            }
            Ok(Value::Number(n)) => {
                if n <= 0.0 { None } else { Some(Duration::from_millis(n as u64)) }
            }
            Ok(_) => None,
            Err(e) => {
                eprintln!("[lua] update error: {e}");
                None
            }
        }
    }

    fn set_socket(&mut self, socket: UnixStreamClient) {
        self.socket = Some(socket);
        self.install_socket();
    }

    fn on_socket_ready(&mut self) {
        self.install_size();
        self.install_socket();
        if let Ok(f) = self.api.get::<Function>("on_socket_ready") {
            if let Err(e) = f.call::<()>((&self.state,)) {
                eprintln!("[lua] on_socket_ready error: {e}");
            }
        } else if let Some(client) = &mut self.socket {
            // Старый fallback: если у виджета нет `M.on_socket_ready`,
            // просто вычитываем сокет, чтобы не забивать буфер.
            // `read_into` + `parse_frames` — единственный способ сделать
            // это без `read_available` (её больше нет).
            let mut temp = [0u8; 4096];
            loop {
                match client.read_into(&mut temp) {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => break,
                }
            }
        }
    }
}