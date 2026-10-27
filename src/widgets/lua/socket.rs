//! `socket:*` — обмен с мостом (nsbridge и любой другой Unix-сокет).
//!
//! - `socket:read()` — массив строк (payload фреймов). Формат фрейма:
//!   4 байта длины + payload. Панель отдаёт уже разрезанные payload'ы.
//! - `socket:write(bytes)` — отправить байты в сокет.
//!   Обычно: `socket:write(string.pack(">I4", #msg) .. msg)`.

use mlua::{LuaString, UserData, UserDataMethods};

use crate::core::ipc::SocketClient;

pub struct LuaSocket {
    pub client: SocketClient,
}

impl UserData for LuaSocket {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // socket:read() -> { "payload1", "payload2", ... }
        methods.add_method_mut("read", |lua, this, ()| {
            let frames = this.client.read_available().unwrap_or_default();
            let t = lua.create_table()?;
            for (i, frame) in frames.into_iter().enumerate() {
                let s = String::from_utf8_lossy(&frame).to_string();
                t.set(i + 1, s)?;
            }
            Ok(t)
        });

        // socket:write(bytes) — отправить байты.
        methods.add_method_mut("write", |_, this, data: LuaString| {
            let bytes = data.as_bytes();
            this.client.send(&bytes).map_err(mlua::Error::external)?;
            Ok(())
        });
    }
}