//! Захват указателя (grab/ungrab).
//!
//! Одна ответственность: удержание указателя на время drag'а панели,
//! drag'а виджета или открытого меню.
//!
//! Здесь НЕТ:
//! - окна панели, событий, tooltip, popup'ов, pixmap'ов.

use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::xproto::ConnectionExt;

use super::X11Backend;

impl X11Backend {
    pub fn grab_pointer(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.grabbed {
            return Ok(());
        }
        let root = self.root_window();
        self.conn.grab_pointer(
            false,
            root,
            EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::POINTER_MOTION,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
            x11rb::NONE,
            x11rb::NONE,
            x11rb::CURRENT_TIME,
        )?;
        self.conn.flush()?;
        self.grabbed = true;
        Ok(())
    }

    pub fn ungrab_pointer(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.grabbed {
            return Ok(());
        }
        self.conn.ungrab_pointer(x11rb::CURRENT_TIME)?;
        self.conn.flush()?;
        self.grabbed = false;
        Ok(())
    }
}