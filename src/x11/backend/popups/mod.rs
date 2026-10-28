//! Popup'ы модулей: окна, pixmap'ы, рисование.

mod draw;
mod pixmaps;
mod windows;

use crate::core::module::{ModuleError, ModuleHost, PixmapId, PopupId};
use crate::render::SharedBuffer;

use super::X11Backend;

impl ModuleHost for X11Backend {
    fn popup_create(&mut self, w: u16, h: u16) -> Result<PopupId, ModuleError> {
        windows::create(self, w, h)
    }

    fn popup_show(
        &mut self,
        id: PopupId,
        x: i32,
        y: i32,
        w: u16,
        h: u16,
    ) -> Result<(), ModuleError> {
        windows::show(self, id, x, y, w, h)
    }

    fn popup_hide(&mut self, id: PopupId) -> Result<(), ModuleError> {
        windows::hide(self, id)
    }

    fn popup_put_image(
        &mut self,
        id: PopupId,
        buf: &SharedBuffer,
    ) -> Result<(), ModuleError> {
        windows::put_image(self, id, buf)
    }

    fn pixmap_create(&mut self, buf: &SharedBuffer) -> Result<PixmapId, ModuleError> {
        pixmaps::create(self, buf)
    }

    fn pixmap_destroy(&mut self, id: PixmapId) -> Result<(), ModuleError> {
        pixmaps::destroy(self, id)
    }

    fn copy_area(
        &mut self,
        popup: PopupId,
        pixmap: PixmapId,
        src_x: i32,
        src_y: i32,
        w: u16,
        h: u16,
        dst_x: i32,
        dst_y: i32,
    ) -> Result<(), ModuleError> {
        draw::copy_area(self, popup, pixmap, src_x, src_y, w, h, dst_x, dst_y)
    }

    fn grab_pointer(&mut self) -> Result<(), ModuleError> {
        X11Backend::grab_pointer(self).map_err(|e| ModuleError::Host(e.to_string()))
    }

    fn ungrab_pointer(&mut self) -> Result<(), ModuleError> {
        X11Backend::ungrab_pointer(self).map_err(|e| ModuleError::Host(e.to_string()))
    }
}