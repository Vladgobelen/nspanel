//! Окно тултипа.

use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::xproto::ConnectionExt;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;

use crate::core::{Color, Rect, Theme};
use crate::render::PixelBuffer;

pub struct Tooltip {
    window: Option<Window>,
    gc: Option<Gcontext>,
    visible: bool,
}

impl Tooltip {
    pub fn new() -> Self {
        Self {
            window: None,
            gc: None,
            visible: false,
        }
    }

    pub fn is_window(&self, window: Window) -> bool {
        self.window == Some(window)
    }

    fn ensure(
        &mut self,
        conn: &RustConnection,
        root: Window,
    ) -> Result<(Window, Gcontext), Box<dyn std::error::Error>> {
        if let (Some(w), Some(gc)) = (self.window, self.gc) {
            return Ok((w, gc));
        }

        let window: Window = conn.generate_id()?;
        let gc: Gcontext = conn.generate_id()?;

        conn.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window,
            root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            x11rb::COPY_FROM_PARENT,
            &CreateWindowAux::new()
                .background_pixmap(x11rb::NONE)
                .override_redirect(1)
                .event_mask(EventMask::EXPOSURE),
        )?;
        conn.create_gc(gc, window, &CreateGCAux::new())?;

        let atom_type = conn
            .intern_atom(false, b"_NET_WM_WINDOW_TYPE")?
            .reply()?
            .atom;
        let atom_tooltip = conn
            .intern_atom(false, b"_NET_WM_WINDOW_TYPE_TOOLTIP")?
            .reply()?
            .atom;
        conn.change_property32(
            PropMode::REPLACE,
            window,
            atom_type,
            AtomEnum::ATOM,
            &[atom_tooltip],
        )?;

        conn.flush()?;
        self.window = Some(window);
        self.gc = Some(gc);
        Ok((window, gc))
    }

    pub fn show(
        &mut self,
        conn: &RustConnection,
        root: Window,
        screen_size: (u16, u16),
        lines: &[String],
        theme: &Theme,
        cx: i32,
        cy: i32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if lines.is_empty() {
            return self.hide(conn);
        }

        let (window, gc) = self.ensure(conn, root)?;

        let padding = 6i32;
        let line_gap = 2i32;
        let font_size = theme.font_size;
        let line_h = font_size as i32 + line_gap;

        let mut max_w = 0.0f32;
        for s in lines {
            let w = theme.font.text_width(s, font_size);
            if w > max_w {
                max_w = w;
            }
        }
        let w = max_w.ceil() as i32 + 2 * padding;
        let h = lines.len() as i32 * line_h - line_gap + 2 * padding;
        let w = w.max(1) as u16;
        let h = h.max(1) as u16;

        let (sw, sh) = screen_size;
        let mut px = cx + 16;
        let mut py = cy + 16;
        if px + w as i32 > sw as i32 {
            px = sw as i32 - w as i32;
        }
        if py + h as i32 > sh as i32 {
            py = cy - h as i32 - 4;
        }
        if px < 0 {
            px = 0;
        }
        if py < 0 {
            py = 0;
        }

        conn.configure_window(
            window,
            &ConfigureWindowAux::new()
                .x(px)
                .y(py)
                .width(w as u32)
                .height(h as u32),
        )?;

        let mut buf = PixelBuffer::new(w as u32, h as u32);
        let bg = Color::rgb(0x2a, 0x2a, 0x2a);
        let border = Color::rgb(0x50, 0x50, 0x50);
        buf.fill_rect(Rect::new(0, 0, w as i32, h as i32), bg);
        buf.fill_rect(Rect::new(0, 0, w as i32, 1), border);
        buf.fill_rect(Rect::new(0, h as i32 - 1, w as i32, 1), border);
        buf.fill_rect(Rect::new(0, 0, 1, h as i32), border);
        buf.fill_rect(Rect::new(w as i32 - 1, 0, 1, h as i32), border);

        let mut ty = padding;
        for s in lines {
            buf.draw_text(padding, ty, s, theme.fg, font_size, &theme.font);
            ty += line_h;
        }

        let data: &[u8] = unsafe {
            std::slice::from_raw_parts(buf.data.as_ptr() as *const u8, buf.data.len() * 4)
        };
        conn.put_image(
            ImageFormat::Z_PIXMAP,
            window,
            gc,
            w,
            h,
            0,
            0,
            0,
            24,
            data,
        )?;
        conn.flush()?;

        if !self.visible {
            conn.map_window(window)?;
            self.visible = true;
        }
        conn.flush()?;
        Ok(())
    }

    pub fn hide(&mut self, conn: &RustConnection) -> Result<(), Box<dyn std::error::Error>> {
        if !self.visible {
            return Ok(());
        }
        if let Some(window) = self.window {
            conn.unmap_window(window)?;
            conn.flush()?;
        }
        self.visible = false;
        Ok(())
    }
}

impl Default for Tooltip {
    fn default() -> Self {
        Self::new()
    }
}