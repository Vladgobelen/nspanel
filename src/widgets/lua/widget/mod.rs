//! `LuaWidget`: модель Lua-виджета.
//!
//! Здесь только «что у виджета есть» и «как он создаётся»:
//! - `struct LuaWidget` (Lua-контекст, `api`-таблица, `state`, сокет,
//!   `image_loader`, длина, флаг relayout);
//! - `LuaWidget::load` (создать `Lua`, выполнить `widget.lua`, получить
//!   `api`-таблицу, создать `state`, положить `img` в глобалы);
//! - `install_*` (подложить `buf`/`font`/`size`/`socket` в глобалы перед
//!   вызовом Lua-функции);
//! - `sync_length_from_api` (взять `length` из `api` один раз).
//!
//! Здесь НЕТ:
//! - `impl Widget` — это `adapter.rs` (трансляция Lua ↔ Rust);
//! - `LuaImage` — это `image.rs` (глобал `img` для Lua-скриптов).

pub mod adapter;
pub mod image;

use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use mlua::{Lua, Table, Value};

use crate::core::ipc::UnixStreamClient;
use crate::core::Theme;
use crate::img::ImageLoader;
use crate::render::SharedBuffer;

use self::image::LuaImage;

use super::api::install_api;
use super::buf::LuaBuf;
use super::font::LuaFont;
use super::size::LuaSize;
use super::socket::LuaSocket;

pub struct LuaWidget {
    pub(super) id: String,
    pub(super) lua: Lua,
    pub(super) api: Table,
    pub(super) state: Table,
    pub(super) socket: Option<UnixStreamClient>,
    pub(super) tooltip_enabled: bool,
    pub(super) image_loader: Arc<ImageLoader>,
    pub(super) length: Rc<Cell<Option<u32>>>,
    pub(super) relayout_dirty: Arc<AtomicBool>,
}

impl LuaWidget {
    pub fn load(
        id: impl Into<String>,
        path: &Path,
        tooltip_enabled: bool,
    ) -> Result<Self, mlua::Error> {
        let id = id.into();
        let lua = Lua::new();

        let globals = lua.globals();
        install_api(&lua, &globals)?;

        let chunk = lua.load(path).into_function()?;
        let api: Table = chunk.call(())?;

        let state = lua.create_table()?;

        let image_loader = Arc::new(ImageLoader::new());
        let _ = globals.set(
            "img",
            LuaImage {
                loader: image_loader.clone(),
            },
        );

        Ok(Self {
            id,
            lua,
            api,
            state,
            socket: None,
            tooltip_enabled,
            image_loader,
            length: Rc::new(Cell::new(None)),
            relayout_dirty: Arc::new(AtomicBool::new(false)),
        })
    }

    pub(super) fn install_buf(&self, buf: &SharedBuffer, theme: &Theme) {
        let _ = self.lua.globals().set(
            "buf",
            LuaBuf {
                buf: buf.clone(),
                font: theme.font.clone(),
            },
        );
        let _ = self.lua.globals().set(
            "font",
            LuaFont {
                font: theme.font.clone(),
            },
        );
    }

    pub(super) fn install_size(&self) {
        let _ = self.lua.globals().set(
            "size",
            LuaSize {
                length: self.length.clone(),
                dirty: self.relayout_dirty.clone(),
                widget_id: self.id.clone(),
            },
        );
    }

    pub(super) fn install_socket(&self) {
        if let Some(client) = &self.socket {
            if let Ok(clone) = client.try_clone() {
                let _ = self.lua.globals().set(
                    "socket",
                    LuaSocket {
                        client: clone,
                        pending: Vec::new(),
                    },
                );
            }
        } else {
            let _ = self.lua.globals().set("socket", Value::Nil);
        }
    }

    pub(super) fn sync_length_from_api(&self) {
        if self.length.get().is_some() {
            return;
        }
        if let Ok(v) = self.api.get::<u32>("length") {
            self.length.set(Some(v));
        }
    }
}