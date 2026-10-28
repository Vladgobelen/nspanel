//! Глобал `img` для Lua-скриптов: загрузка PNG с ресайзом.
//!
//! Одна ответственность: `LuaImage` — `UserData`-обёртка над
//! `ImageLoader`, которую `LuaWidget::load` кладёт в глобалы.
//! Из Lua: `img:load(path, size, "bilinear")` → таблица ARGB-пикселей
//! (или `nil`, если файл не прочитался).
//!
//! Здесь НЕТ:
//! - `LuaWidget` (это `mod.rs`);
//! - `impl Widget` (это `adapter.rs`).

use std::path::Path;
use std::sync::Arc;

use mlua::{UserData, UserDataMethods, Value};

use crate::img::{resize::Filter, ImageLoader};

pub(super) struct LuaImage {
    pub(super) loader: Arc<ImageLoader>,
}

impl UserData for LuaImage {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method(
            "load",
            |lua, this, (path, size, filter): (String, u32, Option<String>)| {
                let path = Path::new(&path);
                let filter = filter
                    .as_deref()
                    .map(Filter::from_str)
                    .unwrap_or(Filter::Bilinear);

                match this.loader.load(path, size, filter) {
                    Some(pixels) => {
                        let t = lua.create_table()?;
                        for (i, &px) in pixels.iter().enumerate() {
                            t.set(i + 1, px)?;
                        }
                        Ok(Value::Table(t))
                    }
                    None => Ok(Value::Nil),
                }
            },
        );
    }
}