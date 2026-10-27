//! Утилиты для Lua-виджетов. Прячут `chrono` и ручное центрирование текста.
//! Это часть Lua-API панели: `LuaWidget` регистрирует `now_hm`, `now_hms`
//! как глобалы, а `text_centered` вызывается из `buf:text_centered`.

use chrono::Local;

use crate::core::{Color, Rect};
use crate::render::{Font, SharedBuffer};

pub fn now_hm() -> String {
    Local::now().format("%H:%M").to_string()
}

pub fn now_hms() -> String {
    Local::now().format("%H:%M:%S").to_string()
}

pub fn text_centered(
    buf: &SharedBuffer,
    area: Rect,
    text: &str,
    color: Color,
    _size: f32,
    font: &Font,
) {
    let padding = 2;
    let max_w = (area.w - 2 * padding).max(1) as f32;
    let max_h = (area.h - 2 * padding).max(1) as f32;

    let mut lo = 1.0f32;
    let mut hi = 256.0f32;
    let mut best = 1.0f32;
    for _ in 0..20 {
        let mid = (lo + hi) / 2.0;
        let (w, h) = measure(text, mid, font);
        if w <= max_w && h <= max_h {
            best = mid;
            lo = mid;
        } else {
            hi = mid;
        }
    }

    let (tw, th) = measure(text, best, font);
    let tx = area.x + (area.w - tw as i32) / 2;
    let ty = area.y + (area.h - th as i32) / 2;

    let mut b = buf.lock().unwrap();
    b.draw_text(tx, ty, text, color, best, font);
}

fn measure(text: &str, size: f32, font: &Font) -> (f32, f32) {
    let w = font.text_width(text, size);
    let lh = font.line_height(size);
    (w, lh)
}