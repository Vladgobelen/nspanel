//! Pixmap'ы: создание из пиксельного буфера и удаление.
//!
//! Одна ответственность: `Pixmap` на X-сервере. Модуль один раз
//! заливает нормальное и hover-состояние меню, а потом на каждом
//! движении мыши делает `copy_area` нужного прямоугольника в popup —
//! это дешевле, чем перерисовывать меню целиком.
//!
//! Здесь НЕТ:
//! - popup-окон (это `windows.rs`);
//! - рисования в popup (это `draw.rs`);
//! - `impl ModuleHost` (это `mod.rs`).

use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::xproto::ConnectionExt;

use crate::core::module::{ModuleError, PixmapId};
use crate::render::SharedBuffer;

use super::super::X11Backend;

/// Создать pixmap размером с буфер и залить в него пиксели.
pub(super) fn create(
    b: &mut X11Backend,
    buf: &SharedBuffer,
) -> Result<PixmapId, ModuleError> {
    let screen = &b.conn.setup().roots[b.screen_num];
    let root = screen.root;

    let buf = buf.lock().unwrap();
    let w = buf.width as u16;
    let h = buf.height as u16;
    let data: &[u8] = unsafe {
        std::slice::from_raw_parts(buf.data.as_ptr() as *const u8, buf.data.len() * 4)
    };

    let pixmap: Pixmap = b
        .conn
        .generate_id()
        .map_err(|e| ModuleError::Host(e.to_string()))?;

    b.conn
        .create_pixmap(24, pixmap, root, w, h)
        .map_err(|e| ModuleError::Host(e.to_string()))?;

    b.conn
        .put_image(
            ImageFormat::Z_PIXMAP,
            pixmap,
            b.gc,
            w,
            h,
            0,
            0,
            0,
            24,
            data,
        )
        .map_err(|e| ModuleError::Host(e.to_string()))?;

    b.conn.flush().map_err(|e| ModuleError::Host(e.to_string()))?;

    let id = PixmapId(b.next_pixmap_id);
    b.next_pixmap_id += 1;
    b.pixmaps.insert(id, pixmap);
    Ok(id)
}

/// Удалить pixmap по id.
pub(super) fn destroy(b: &mut X11Backend, id: PixmapId) -> Result<(), ModuleError> {
    let pixmap = *b
        .pixmaps
        .get(&id)
        .ok_or_else(|| ModuleError::Host(format!("pixmap {id:?} not found")))?;
    b.conn
        .free_pixmap(pixmap)
        .map_err(|e| ModuleError::Host(e.to_string()))?;
    b.conn.flush().map_err(|e| ModuleError::Host(e.to_string()))?;
    b.pixmaps.remove(&id);
    Ok(())
}