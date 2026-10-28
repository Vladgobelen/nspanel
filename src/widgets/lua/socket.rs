//! `socket:*` — обмен с мостом (nsbridge и любой другой Unix-сокет).
//!
//! - `socket:read()` — массив строк (payload фреймов). Формат фрейма:
//!   4 байта длины + payload. `LuaSocket` держит собственный `pending`-
//!   буфер и режет его через `frame::parse_frames`.
//! - `socket:write(bytes)` — отправить байты в сокет.
//!   Обычно: `socket:write(string.pack(">I4", #msg) .. msg)`.
//!
//! Разделение транспорта и фрейминга:
//! - транспорт — `UnixStreamClient` (`read_into` / `write_all`, сырые байты);
//! - фрейминг — `frame::parse_frames` (чистая функция).
//! `LuaSocket` — единственное место в Lua-слое, где они сходятся.

use mlua::{LuaString, UserData, UserDataMethods};

use crate::core::ipc::{frame, UnixStreamClient};

pub struct LuaSocket {
    pub client: UnixStreamClient,
    /// Неразобранный хвост фреймов. Живёт между вызовами `read`.
    /// Транспорт (`UnixStreamClient`) буфера не имеет — буфер ведёт
    /// тот, кто знает про протокол, то есть `LuaSocket`.
    pub pending: Vec<u8>,
}

impl UserData for LuaSocket {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // socket:read() -> { "payload1", "payload2", ... }
        methods.add_method_mut("read", |lua, this, ()| {
            // 1. Вычитать всё доступное из сокета в pending.
            let mut temp = [0u8; 4096];
            loop {
                match this.client.read_into(&mut temp) {
                    Ok(0) => break, // EOF — дальше read смысла нет
                    Ok(n) => this.pending.extend_from_slice(&temp[..n]),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => break,
                }
            }

            // 2. Разрезать pending на готовые фреймы.
            let frames = frame::parse_frames(&mut this.pending).unwrap_or_default();

            // 3. Отдать payload'ы в Lua как массив строк.
            let t = lua.create_table()?;
            for (i, payload) in frames.into_iter().enumerate() {
                let s = String::from_utf8_lossy(&payload).to_string();
                t.set(i + 1, s)?;
            }
            Ok(t)
        });

        // socket:write(bytes) — отправить байты как есть.
        // Фрейминг — на Lua-стороне: `string.pack(">I4", #msg) .. msg`.
        methods.add_method_mut("write", |_, this, data: LuaString| {
            let bytes = data.as_bytes();
            this.client.write_all(&bytes).map_err(mlua::Error::external)?;
            Ok(())
        });
    }
}