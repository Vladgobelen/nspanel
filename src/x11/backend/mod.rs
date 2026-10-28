//! X11-бэкенд панели.

pub mod events;
pub mod grab;
pub mod panel_window;
pub mod popups;
pub mod tooltip;

use std::collections::HashMap;
use std::os::fd::AsRawFd;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::xproto::ConnectionExt;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;

use crate::core::module::{PixmapId, PopupId};
use crate::core::Theme;

use tooltip::Tooltip;

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
    pub(crate) conn: RustConnection,
    pub(crate) screen_num: usize,
    pub panel_window: Window,
    pub(crate) gc: Gcontext,
    pub(crate) popups: HashMap<PopupId, Window>,
    pub(crate) pixmaps: HashMap<PixmapId, Pixmap>,
    pub(crate) next_popup_id: u32,
    pub(crate) next_pixmap_id: u32,
    pub(crate) grabbed: bool,
    pub(crate) root_motion_subscribed: bool,
    pub(crate) tooltip: Tooltip,
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
            tooltip: Tooltip::new(),
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

    // ---------- tooltip (тонкие обёртки над `Tooltip`) ----------

    pub fn is_tooltip_window(&self, window: Window) -> bool {
        self.tooltip.is_window(window)
    }

    pub fn tooltip_show(
        &mut self,
        lines: &[String],
        theme: &Theme,
        cx: i32,
        cy: i32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = self.root_window();
        let screen_size = self.screen_size();
        self.tooltip
            .show(&self.conn, root, screen_size, lines, theme, cx, cy)
    }

    pub fn tooltip_hide(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.tooltip.hide(&self.conn)
    }

    // ---------- popup lookup ----------

    pub fn popup_id_for_window(&self, window: Window) -> Option<PopupId> {
        self.popups
            .iter()
            .find(|&(_, &w)| w == window)
            .map(|(&id, _)| id)
    }
}