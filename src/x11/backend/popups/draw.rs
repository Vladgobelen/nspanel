//! Рисование в popup: копирование области из pixmap.
//!
//! Одна ответственность: `copy_area` — быстрый перенос прямоугольника
//! из pixmap в popup-окно. Используется `MenuModule` для подсветки
//! пункта меню на hover.
//!
//! Здесь НЕТ:
//! - popup-окон (это `windows.rs`);
//! - pixmap'ов (это `pixmaps.rs`);
//! - `impl ModuleHost` (это `mod.rs`).

use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt;

use crate::core::module::{ModuleError, PixmapId, PopupId};

use super::super::X11Backend;

/// Скопировать прямоугольник `(w × h)` из pixmap в popup.
#[allow(clippy::too_many_arguments)]
pub(super) fn copy_area(
    b: &mut X11Backend,
    popup: PopupId,
    pixmap: PixmapId,
    src_x: i32,
    src_y: i32,
    w: u16,
    h: u16,
    dst_x: i32,
    dst_y: i32,
) -> Result<(), ModuleError> {
    let window = *b
        .popups
        .get(&popup)
        .ok_or_else(|| ModuleError::Host(format!("popup {popup:?} not found")))?;
    let pm = *b
        .pixmaps
        .get(&pixmap)
        .ok_or_else(|| ModuleError::Host(format!("pixmap {pixmap:?} not found")))?;

    b.conn
        .copy_area(
            pm,
            window,
            b.gc,
            src_x as i16,
            src_y as i16,
            dst_x as i16,
            dst_y as i16,
            w,
            h,
        )
        .map_err(|e| ModuleError::Host(e.to_string()))?;

    b.conn.flush().map_err(|e| ModuleError::Host(e.to_string()))?;
    Ok(())
}