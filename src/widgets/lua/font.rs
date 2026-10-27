//! `font:*` — работа со шрифтом из Lua.
//!
//! - `font:text_width(text, size)` — ширина текста в пикселях
//!   при заданном размере шрифта. Для центрирования букв в иконках.

use std::sync::Arc;

use mlua::{UserData, UserDataMethods};

use crate::render::Font;

pub struct LuaFont {
    pub font: Arc<Font>,
}

impl UserData for LuaFont {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("text_width", |_, this, (text, size): (String, f32)| {
            Ok(this.font.text_width(&text, size))
        });
    }
}