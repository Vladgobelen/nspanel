//! Примитивы для декларативного рендера. Пока не используются напрямую
//! (`buf:*` работает через `PixelBuffer`), но оставлены как задел на будущее:
//! виджет (Lua или Rust) сможет вернуть список `DrawCommand`, а рендер
//! их исполнит через `execute_commands`.
//!
//! `#[allow(dead_code)]` — API готов, но пока не востребован.

use crate::core::{Color, Rect};
use crate::render::{Font, PixelBuffer};

/// Примитивы, которые могут использовать и Rust-модули, и (позже) Lua-модули.
/// Набор фиксирован: пользователь комбинирует их, но не добавляет новые.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum DrawCommand {
    Rect {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        color: u32,
    },
    Text {
        x: i32,
        y: i32,
        text: String,
        color: u32,
        size: f32,
    },
    Line {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        color: u32,
    },
}

/// Исполнить команды на буфере. Шрифт нужен для `Text`.
#[allow(dead_code)]
pub fn execute_commands(buf: &mut PixelBuffer, cmds: &[DrawCommand], font: &Font) {
    for cmd in cmds {
        match cmd {
            DrawCommand::Rect { x, y, w, h, color } => {
                buf.fill_rect(Rect::new(*x, *y, *w, *h), Color(*color));
            }
            DrawCommand::Text { x, y, text, color, size } => {
                buf.draw_text(*x, *y, text, Color(*color), *size, font);
            }
            DrawCommand::Line { x1, y1, x2, y2, color } => {
                buf.draw_line(*x1, *y1, *x2, *y2, Color(*color));
            }
        }
    }
}