//! `buf:*` — рисование в буфер из Lua.
//!
//! - `buf:fill_rect(area, color)` — залить прямоугольник.
//! - `buf:draw_text(x, y, text, color, size)` — текст по координатам.
//! - `buf:text_centered(area, text, color, size)` — текст по центру (масштабируется).
//! - `buf:draw_line(x1, y1, x2, y2, color)` — линия.
//! - `buf:draw_pixels(x, y, w, h, pixels)` — нарисовать массив ARGB-пикселей.

use std::sync::Arc;

use mlua::{Table, UserData, UserDataMethods};

use crate::core::Color;
use crate::render::{Font, SharedBuffer};
use crate::widgets::util::text_centered;

use super::helpers::table_to_rect;

pub struct LuaBuf {
    pub buf: SharedBuffer,
    pub font: Arc<Font>,
}

impl UserData for LuaBuf {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("fill_rect", |_, this, (area_t, color_u32): (Table, u32)| {
            let r = table_to_rect(&area_t);
            let mut b = this.buf.lock().unwrap();
            b.fill_rect(r, Color(color_u32));
            Ok(())
        });

        methods.add_method(
            "draw_text",
            |_, this, (x, y, text, color_u32, size): (i32, i32, String, u32, f32)| {
                let mut b = this.buf.lock().unwrap();
                b.draw_text(x, y, &text, Color(color_u32), size, &this.font);
                Ok(())
            },
        );

        methods.add_method(
            "text_centered",
            |_, this, (area_t, text, color_u32, size): (Table, String, u32, f32)| {
                let r = table_to_rect(&area_t);
                text_centered(&this.buf, r, &text, Color(color_u32), size, &this.font);
                Ok(())
            },
        );

        methods.add_method(
            "draw_line",
            |_, this, (x1, y1, x2, y2, color_u32): (i32, i32, i32, i32, u32)| {
                let mut b = this.buf.lock().unwrap();
                b.draw_line(x1, y1, x2, y2, Color(color_u32));
                Ok(())
            },
        );

        methods.add_method(
            "draw_pixels",
            |_, this, (x, y, w, h, pixels_t): (i32, i32, i32, i32, Table)| {
                // Собираем таблицу в Vec<u32> (в порядке построчно).
                let mut pixels = Vec::with_capacity((w * h) as usize);
                for i in 1..=w * h {
                    if let Ok(v) = pixels_t.get::<u32>(i) {
                        pixels.push(v);
                    } else {
                        return Ok(()); // таблица короче, чем нужно
                    }
                }
                let mut b = this.buf.lock().unwrap();
                b.draw_argb_pixels(x, y, w, h, &pixels);
                Ok(())
            },
        );
    }
}