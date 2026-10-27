use std::collections::HashMap;
use std::os::fd::AsRawFd;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::xproto::ConnectionExt;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;

use crate::core::module::{ModuleError, ModuleHost, PanelPosition, PixmapId, PopupId};
use crate::core::{Color, Rect};
use crate::render::{PixelBuffer, SharedBuffer};

#[allow(dead_code)]
pub enum Event {
    ButtonPress {
        x: i32,
        y: i32,
        button: u8,
        window: Window,
    },
    ButtonRelease {
        x: i32,
        y: i32,
        button: u8,
        window: Window,
    },
    MotionNotify { x: i32, y: i32, window: Window },
    Expose { window: Window },
    LeaveWindow { window: Window },
    PropertyNotify { atom: u32, window: Window },
    Other,
}

pub struct X11Backend {
    conn: RustConnection,
    screen_num: usize,
    pub panel_window: Window,
    gc: Gcontext,
    popups: HashMap<PopupId, Window>,
    pixmaps: HashMap<PixmapId, Pixmap>,
    next_popup_id: u32,
    next_pixmap_id: u32,
    grabbed: bool,
    root_motion_subscribed: bool,
    tooltip_window: Option<Window>,
    tooltip_pixmap: Option<Pixmap>,
    tooltip_pixmap_size: (u16, u16),
    tooltip_gc: Option<Gcontext>,
    tooltip_visible: bool,
}

impl X11Backend {
    pub fn new(panel_height: u16) -> Result<Self, Box<dyn std::error::Error>> {
        let (conn, screen_num) = x11rb::connect(None)?;
        let screen = &conn.setup().roots[screen_num];
        let root = screen.root;

        let width = screen.width_in_pixels as u16;
        let height = panel_height;

        let panel_window: Window = conn.generate_id()?;
        let gc: Gcontext = conn.generate_id()?;

        conn.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            panel_window,
            root,
            0,
            0,
            width,
            height,
            0,
            WindowClass::INPUT_OUTPUT,
            x11rb::COPY_FROM_PARENT,
            &CreateWindowAux::new()
                .background_pixel(0x00202020)
                .event_mask(
                    EventMask::EXPOSURE
                        | EventMask::BUTTON_PRESS
                        | EventMask::BUTTON_RELEASE
                        | EventMask::POINTER_MOTION
                        | EventMask::LEAVE_WINDOW,
                ),
        )?;

        conn.create_gc(gc, panel_window, &CreateGCAux::new())?;

        let atom_type = conn
            .intern_atom(false, b"_NET_WM_WINDOW_TYPE")?
            .reply()?
            .atom;
        let atom_dock = conn
            .intern_atom(false, b"_NET_WM_WINDOW_TYPE_DOCK")?
            .reply()?
            .atom;
        conn.change_property32(
            PropMode::REPLACE,
            panel_window,
            atom_type,
            AtomEnum::ATOM,
            &[atom_dock],
        )?;

        conn.map_window(panel_window)?;
        conn.flush()?;

        Ok(Self {
            conn,
            screen_num,
            panel_window,
            gc,
            popups: HashMap::new(),
            pixmaps: HashMap::new(),
            next_popup_id: 1,
            next_pixmap_id: 1,
            grabbed: false,
            root_motion_subscribed: false,
            tooltip_window: None,
            tooltip_pixmap: None,
            tooltip_pixmap_size: (0, 0),
            tooltip_gc: None,
            tooltip_visible: false,
        })
    }

    pub fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.conn.stream().as_raw_fd()
    }

    pub fn screen_size(&self) -> (u16, u16) {
        let screen = &self.conn.setup().roots[self.screen_num];
        (screen.width_in_pixels, screen.height_in_pixels)
    }

    pub fn root_window(&self) -> Window {
        self.conn.setup().roots[self.screen_num].root
    }

    pub fn subscribe_root_motion(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.root_motion_subscribed {
            return Ok(());
        }
        let root = self.root_window();
        self.conn.change_window_attributes(
            root,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::POINTER_MOTION),
        )?;
        self.conn.flush()?;
        self.root_motion_subscribed = true;
        Ok(())
    }

    pub fn unsubscribe_root_motion(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.root_motion_subscribed {
            return Ok(());
        }
        let root = self.root_window();
        self.conn.change_window_attributes(
            root,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::NO_EVENT),
        )?;
        self.conn.flush()?;
        self.root_motion_subscribed = false;
        Ok(())
    }

    pub fn root_motion_subscribed(&self) -> bool {
        self.root_motion_subscribed
    }

    pub fn next_event(&mut self) -> Result<Option<Event>, Box<dyn std::error::Error>> {
        let ev = match self.conn.poll_for_event()? {
            Some(ev) => ev,
            None => return Ok(None),
        };
        let out = match ev {
            x11rb::protocol::Event::ButtonPress(e) => Event::ButtonPress {
                x: e.root_x as i32,
                y: e.root_y as i32,
                button: e.detail,
                window: e.event,
            },
            x11rb::protocol::Event::ButtonRelease(e) => Event::ButtonRelease {
                x: e.root_x as i32,
                y: e.root_y as i32,
                button: e.detail,
                window: e.event,
            },
            x11rb::protocol::Event::MotionNotify(e) => Event::MotionNotify {
                x: e.root_x as i32,
                y: e.root_y as i32,
                window: e.event,
            },
            x11rb::protocol::Event::Expose(e) => Event::Expose { window: e.window },
            x11rb::protocol::Event::LeaveNotify(e) => Event::LeaveWindow { window: e.event },
            x11rb::protocol::Event::PropertyNotify(e) => Event::PropertyNotify {
                atom: e.atom,
                window: e.window,
            },
            _ => Event::Other,
        };
        Ok(Some(out))
    }

    pub fn put_image(&mut self, buf: &SharedBuffer) -> Result<(), Box<dyn std::error::Error>> {
        self.put_image_to(self.panel_window, buf)
    }

    fn put_image_to(
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

    pub fn popup_id_for_window(&self, window: Window) -> Option<PopupId> {
        self.popups
            .iter()
            .find(|&(_, &w)| w == window)
            .map(|(&id, _)| id)
    }

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

    // ---------- tooltip ----------

    /// Проверить, является ли окно окном тултипа.
    pub fn is_tooltip_window(&self, window: Window) -> bool {
        self.tooltip_window == Some(window)
    }

    /// Создать popup-окно тултипа лениво (один раз).
    fn ensure_tooltip_window(&mut self) -> Result<(Window, Gcontext), Box<dyn std::error::Error>> {
        if let (Some(w), Some(gc)) = (self.tooltip_window, self.tooltip_gc) {
            return Ok((w, gc));
        }
        let root = self.root_window();
        let window: Window = self.conn.generate_id()?;
        let gc: Gcontext = self.conn.generate_id()?;

        self.conn.create_window(
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
                .background_pixmap(x11rb::NONE)  // ВАЖНО: отключаем автоматическую заливку фона
                .override_redirect(1)
                .event_mask(EventMask::EXPOSURE),
        )?;
        self.conn.create_gc(gc, window, &CreateGCAux::new())?;

        let atom_type = self
            .conn
            .intern_atom(false, b"_NET_WM_WINDOW_TYPE")?
            .reply()?
            .atom;
        let atom_tooltip = self
            .conn
            .intern_atom(false, b"_NET_WM_WINDOW_TYPE_TOOLTIP")?
            .reply()?
            .atom;
        self.conn.change_property32(
            PropMode::REPLACE,
            window,
            atom_type,
            AtomEnum::ATOM,
            &[atom_tooltip],
        )?;

        self.conn.flush()?;
        self.tooltip_window = Some(window);
        self.tooltip_gc = Some(gc);
        Ok((window, gc))
    }

    pub fn tooltip_show(
        &mut self,
        lines: &[String],
        theme: &crate::core::Theme,
        cx: i32,
        cy: i32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if lines.is_empty() {
            return self.tooltip_hide();
        }

        let (window, gc) = self.ensure_tooltip_window()?;

        let padding = 6i32;
        let line_gap = 2i32;
        let font_size = theme.font_size;
        let line_h = font_size as i32 + line_gap;

        // Ширина по максимальной строке.
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

        // Позиция: под курсором, справа-снизу. С прижатием к краям.
        let (sw, sh) = self.screen_size();
        let mut px = cx + 16;
        let mut py = cy + 16;
        if px + w as i32 > sw as i32 {
            px = sw as i32 - w as i32;
        }
        if py + h as i32 > sh as i32 {
            py = cy - h as i32 - 4;
        }
        if px < 0 { px = 0; }
        if py < 0 { py = 0; }

        // 1. Настраиваем окно (размер, позиция).
        self.conn.configure_window(
            window,
            &ConfigureWindowAux::new()
                .x(px)
                .y(py)
                .width(w as u32)
                .height(h as u32),
        )?;

        // 2. Рендер в PixelBuffer.
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

        // 3. Копируем данные НАПРЯМУЮ в окно (put_image).
        let data: &[u8] = unsafe {
            std::slice::from_raw_parts(buf.data.as_ptr() as *const u8, buf.data.len() * 4)
        };
        self.conn.put_image(
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

        self.conn.flush()?;

        // 4. Показываем окно (если еще не показано).
        if !self.tooltip_visible {
            self.conn.map_window(window)?;
            self.tooltip_visible = true;
        }
        self.conn.flush()?;
        Ok(())
    }

    /// Скрыть тултип (unmap окна, pixmap не трогаем — переиспользуем).
    pub fn tooltip_hide(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.tooltip_visible {
            return Ok(());
        }
        if let Some(window) = self.tooltip_window {
            self.conn.unmap_window(window)?;
            self.conn.flush()?;
        }
        self.tooltip_visible = false;
        Ok(())
    }
}

impl ModuleHost for X11Backend {
    fn popup_create(&mut self, w: u16, h: u16) -> Result<PopupId, ModuleError> {
        let screen = &self.conn.setup().roots[self.screen_num];
        let root = screen.root;

        let window: Window = self
            .conn
            .generate_id()
            .map_err(|e| ModuleError::Host(e.to_string()))?;

        self.conn
            .create_window(
                x11rb::COPY_DEPTH_FROM_PARENT,
                window,
                root,
                0,
                0,
                w,
                h,
                0,
                WindowClass::INPUT_OUTPUT,
                x11rb::COPY_FROM_PARENT,
                &CreateWindowAux::new()
                    .background_pixel(0x00303030)
                    .override_redirect(1)
                    .event_mask(
                        EventMask::EXPOSURE
                            | EventMask::BUTTON_PRESS
                            | EventMask::BUTTON_RELEASE
                            | EventMask::POINTER_MOTION,
                    ),
            )
            .map_err(|e| ModuleError::Host(e.to_string()))?;

        let atom_type = self
            .conn
            .intern_atom(false, b"_NET_WM_WINDOW_TYPE")
            .map_err(|e| ModuleError::Host(e.to_string()))?
            .reply()
            .map_err(|e| ModuleError::Host(e.to_string()))?
            .atom;
        let atom_popup = self
            .conn
            .intern_atom(false, b"_NET_WM_WINDOW_TYPE_POPUP_MENU")
            .map_err(|e| ModuleError::Host(e.to_string()))?
            .reply()
            .map_err(|e| ModuleError::Host(e.to_string()))?
            .atom;
        self.conn
            .change_property32(
                PropMode::REPLACE,
                window,
                atom_type,
                AtomEnum::ATOM,
                &[atom_popup],
            )
            .map_err(|e| ModuleError::Host(e.to_string()))?;

        self.conn.flush().map_err(|e| ModuleError::Host(e.to_string()))?;

        let id = PopupId(self.next_popup_id);
        self.next_popup_id += 1;
        self.popups.insert(id, window);
        Ok(id)
    }

    fn popup_show(&mut self, id: PopupId, x: i32, y: i32, w: u16, h: u16) -> Result<(), ModuleError> {
        let window = *self
            .popups
            .get(&id)
            .ok_or_else(|| ModuleError::Host(format!("popup {id:?} not found")))?;
        self.conn
            .configure_window(
                window,
                &ConfigureWindowAux::new()
                    .x(x)
                    .y(y)
                    .width(w as u32)
                    .height(h as u32),
            )
            .map_err(|e| ModuleError::Host(e.to_string()))?;
        self.conn
            .map_window(window)
            .map_err(|e| ModuleError::Host(e.to_string()))?;
        self.conn.flush().map_err(|e| ModuleError::Host(e.to_string()))?;
        Ok(())
    }

    fn popup_hide(&mut self, id: PopupId) -> Result<(), ModuleError> {
        let window = *self
            .popups
            .get(&id)
            .ok_or_else(|| ModuleError::Host(format!("popup {id:?} not found")))?;
        self.conn
            .unmap_window(window)
            .map_err(|e| ModuleError::Host(e.to_string()))?;
        self.conn.flush().map_err(|e| ModuleError::Host(e.to_string()))?;
        Ok(())
    }

    fn popup_put_image(&mut self, id: PopupId, buf: &SharedBuffer) -> Result<(), ModuleError> {
        let window = *self
            .popups
            .get(&id)
            .ok_or_else(|| ModuleError::Host(format!("popup {id:?} not found")))?;
        self.put_image_to(window, buf)
            .map_err(|e| ModuleError::Host(e.to_string()))
    }

    fn pixmap_create(&mut self, buf: &SharedBuffer) -> Result<PixmapId, ModuleError> {
        let screen = &self.conn.setup().roots[self.screen_num];
        let root = screen.root;

        let b = buf.lock().unwrap();
        let w = b.width as u16;
        let h = b.height as u16;
        let data: &[u8] = unsafe {
            std::slice::from_raw_parts(b.data.as_ptr() as *const u8, b.data.len() * 4)
        };

        let pixmap: Pixmap = self
            .conn
            .generate_id()
            .map_err(|e| ModuleError::Host(e.to_string()))?;

        self.conn
            .create_pixmap(24, pixmap, root, w, h)
            .map_err(|e| ModuleError::Host(e.to_string()))?;

        self.conn
            .put_image(
                ImageFormat::Z_PIXMAP,
                pixmap,
                self.gc,
                w,
                h,
                0,
                0,
                0,
                24,
                data,
            )
            .map_err(|e| ModuleError::Host(e.to_string()))?;

        self.conn.flush().map_err(|e| ModuleError::Host(e.to_string()))?;

        let id = PixmapId(self.next_pixmap_id);
        self.next_pixmap_id += 1;
        self.pixmaps.insert(id, pixmap);
        Ok(id)
    }

    fn pixmap_destroy(&mut self, id: PixmapId) -> Result<(), ModuleError> {
        let pixmap = *self
            .pixmaps
            .get(&id)
            .ok_or_else(|| ModuleError::Host(format!("pixmap {id:?} not found")))?;
        self.conn
            .free_pixmap(pixmap)
            .map_err(|e| ModuleError::Host(e.to_string()))?;
        self.conn.flush().map_err(|e| ModuleError::Host(e.to_string()))?;
        self.pixmaps.remove(&id);
        Ok(())
    }

    fn fill_rect(&mut self, popup: PopupId, rect: Rect, color: u32) -> Result<(), ModuleError> {
        let window = *self
            .popups
            .get(&popup)
            .ok_or_else(|| ModuleError::Host(format!("popup {popup:?} not found")))?;

        let gc: Gcontext = self
            .conn
            .generate_id()
            .map_err(|e| ModuleError::Host(e.to_string()))?;

        let rgb = color & 0x00FFFFFF;

        self.conn
            .create_gc(
                gc,
                window,
                &CreateGCAux::new()
                    .foreground(rgb)
                    .graphics_exposures(0),
            )
            .map_err(|e| ModuleError::Host(e.to_string()))?;

        self.conn
            .poly_fill_rectangle(
                window,
                gc,
                &[Rectangle {
                    x: rect.x as i16,
                    y: rect.y as i16,
                    width: rect.w.max(0) as u16,
                    height: rect.h.max(0) as u16,
                }],
            )
            .map_err(|e| ModuleError::Host(e.to_string()))?;

        self.conn
            .free_gc(gc)
            .map_err(|e| ModuleError::Host(e.to_string()))?;

        self.conn.flush().map_err(|e| ModuleError::Host(e.to_string()))?;
        Ok(())
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
        let window = *self
            .popups
            .get(&popup)
            .ok_or_else(|| ModuleError::Host(format!("popup {popup:?} not found")))?;
        let pm = *self
            .pixmaps
            .get(&pixmap)
            .ok_or_else(|| ModuleError::Host(format!("pixmap {pixmap:?} not found")))?;

        self.conn
            .copy_area(
                pm,
                window,
                self.gc,
                src_x as i16,
                src_y as i16,
                dst_x as i16,
                dst_y as i16,
                w,
                h,
            )
            .map_err(|e| ModuleError::Host(e.to_string()))?;

        self.conn.flush().map_err(|e| ModuleError::Host(e.to_string()))?;
        Ok(())
    }

    fn grab_pointer(&mut self) -> Result<(), ModuleError> {
        X11Backend::grab_pointer(self).map_err(|e| ModuleError::Host(e.to_string()))
    }

    fn ungrab_pointer(&mut self) -> Result<(), ModuleError> {
        X11Backend::ungrab_pointer(self).map_err(|e| ModuleError::Host(e.to_string()))
    }
}