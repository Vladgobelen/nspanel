//! Окно панели: размер, позиция, strut, вывод пиксельного буфера.

use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::xproto::ConnectionExt;
use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;

use crate::core::module::PanelPosition;
use crate::render::SharedBuffer;

use super::X11Backend;

impl X11Backend {
    pub fn put_image(&mut self, buf: &SharedBuffer) -> Result<(), Box<dyn std::error::Error>> {
        self.put_image_to(self.panel_window, buf)
    }

    pub(crate) fn put_image_to(
        &mut self,
        window: Window,
        buf: &SharedBuffer,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let b = buf.lock().unwrap();
        let data: &[u8] = unsafe {
            std::slice::from_raw_parts(b.data.as_ptr() as *const u8, b.data.len() * 4)
        };
        self.conn.put_image(
            ImageFormat::Z_PIXMAP,
            window,
            self.gc,
            b.width as u16,
            b.height as u16,
            0,
            0,
            0,
            24,
            data,
        )?;
        self.conn.flush()?;
        Ok(())
    }

    pub fn move_panel(&mut self, x: i32, y: i32) -> Result<(), Box<dyn std::error::Error>> {
        self.conn.configure_window(
            self.panel_window,
            &ConfigureWindowAux::new().x(x).y(y),
        )?;
        self.conn.flush()?;
        Ok(())
    }

    pub fn resize_panel(&mut self, w: u16, h: u16) -> Result<(), Box<dyn std::error::Error>> {
        self.conn.configure_window(
            self.panel_window,
            &ConfigureWindowAux::new().width(w as u32).height(h as u32),
        )?;
        self.conn.flush()?;
        Ok(())
    }

    pub fn update_strut(
        &mut self,
        position: PanelPosition,
        thickness: u32,
        length: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let atom_strut = self
            .conn
            .intern_atom(false, b"_NET_WM_STRUT_PARTIAL")?
            .reply()?
            .atom;

        let strut: [u32; 12] = match position {
            PanelPosition::Top => [0, 0, thickness, 0, 0, 0, 0, 0, 0, length, 0, 0],
            PanelPosition::Bottom => [0, 0, 0, thickness, 0, 0, 0, 0, 0, 0, 0, length],
            PanelPosition::Left => [thickness, 0, 0, 0, 0, length, 0, 0, 0, 0, 0, 0],
            PanelPosition::Right => [0, thickness, 0, 0, 0, 0, 0, length, 0, 0, 0, 0],
        };

        self.conn.change_property32(
            PropMode::REPLACE,
            self.panel_window,
            atom_strut,
            AtomEnum::CARDINAL,
            &strut,
        )?;
        self.conn.flush()?;
        Ok(())
    }

    pub fn clear_strut(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let atom_strut = self
            .conn
            .intern_atom(false, b"_NET_WM_STRUT_PARTIAL")?
            .reply()?
            .atom;
        let strut: [u32; 12] = [0; 12];
        self.conn.change_property32(
            PropMode::REPLACE,
            self.panel_window,
            atom_strut,
            AtomEnum::CARDINAL,
            &strut,
        )?;
        self.conn.flush()?;
        Ok(())
    }
}