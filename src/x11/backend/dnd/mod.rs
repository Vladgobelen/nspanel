//! XDND (X Drag-and-Drop).

pub mod forward;
pub mod receive;
pub mod uri_list;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    ConnectionExt, EventMask, PropMode, SelectionNotifyEvent, Window,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;

use crate::core::{Rect, WidgetId};
use crate::widgets::lua::dnd_handle::DndCallbacks;

pub const XDND_VERSION: u32 = 5;

pub(crate) const ATOM_XDND_AWARE: &[u8] = b"XdndAware";
pub(crate) const ATOM_XDND_ENTER: &[u8] = b"XdndEnter";
pub(crate) const ATOM_XDND_POSITION: &[u8] = b"XdndPosition";
pub(crate) const ATOM_XDND_STATUS: &[u8] = b"XdndStatus";
pub(crate) const ATOM_XDND_LEAVE: &[u8] = b"XdndLeave";
pub(crate) const ATOM_XDND_DROP: &[u8] = b"XdndDrop";
pub(crate) const ATOM_XDND_FINISHED: &[u8] = b"XdndFinished";
pub(crate) const ATOM_XDND_SELECTION: &[u8] = b"XdndSelection";
pub(crate) const ATOM_XDND_ACTION_COPY: &[u8] = b"XdndActionCopy";
pub(crate) const ATOM_XDND_TYPE_LIST: &[u8] = b"XdndTypeList";
pub(crate) const ATOM_TEXT_URI_LIST: &[u8] = b"text/uri-list";
pub(crate) const ATOM_NSDE_XDND_DATA: &[u8] = b"NSDE_XDND_DATA";

#[derive(Debug, Clone, Copy)]
pub struct DndAtoms {
    pub aware: u32,
    pub enter: u32,
    pub position: u32,
    pub status: u32,
    pub leave: u32,
    pub drop: u32,
    pub finished: u32,
    pub selection: u32,
    pub action_copy: u32,
    pub type_list: u32,
    pub text_uri_list: u32,
    pub data_property: u32,
}

impl DndAtoms {
    fn intern(conn: &RustConnection) -> Result<Self, Box<dyn std::error::Error>> {
        let i = |name: &[u8]| -> Result<u32, Box<dyn std::error::Error>> {
            Ok(conn.intern_atom(false, name)?.reply()?.atom)
        };
        Ok(Self {
            aware: i(ATOM_XDND_AWARE)?,
            enter: i(ATOM_XDND_ENTER)?,
            position: i(ATOM_XDND_POSITION)?,
            status: i(ATOM_XDND_STATUS)?,
            leave: i(ATOM_XDND_LEAVE)?,
            drop: i(ATOM_XDND_DROP)?,
            finished: i(ATOM_XDND_FINISHED)?,
            selection: i(ATOM_XDND_SELECTION)?,
            action_copy: i(ATOM_XDND_ACTION_COPY)?,
            type_list: i(ATOM_XDND_TYPE_LIST)?,
            text_uri_list: i(ATOM_TEXT_URI_LIST)?,
            data_property: i(ATOM_NSDE_XDND_DATA)?,
        })
    }

    pub fn is_enter(&self, t: u32) -> bool { t == self.enter }
    pub fn is_position(&self, t: u32) -> bool { t == self.position }
    pub fn is_drop(&self, t: u32) -> bool { t == self.drop }
    pub fn is_leave(&self, t: u32) -> bool { t == self.leave }
    pub fn is_status(&self, t: u32) -> bool { t == self.status }
    pub fn is_finished(&self, t: u32) -> bool { t == self.finished }
    pub fn is_selection(&self, t: u32) -> bool { t == self.selection }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DndState {
    Idle,
    Entered { source: Window },
    Dropped { source: Window, time: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DndResult {
    pub redraw: bool,
}

impl DndResult {
    pub const NONE: Self = Self { redraw: false };
    pub const REDRAW: Self = Self { redraw: true };
}

pub struct Dnd {
    overlay: Cell<Option<Window>>,
    atoms: Cell<Option<DndAtoms>>,
    state: Cell<DndState>,
    cursor: Cell<(i32, i32)>,
    panel_area: Cell<Option<Rect>>,
    callbacks: RefCell<HashMap<WidgetId, DndCallbacks>>,
    panel_window: Cell<Option<Window>>,
    last_position_time: Cell<u32>,
    last_paths: RefCell<Vec<PathBuf>>,
}

impl Dnd {
    pub fn new() -> Self {
        Self {
            overlay: Cell::new(None),
            atoms: Cell::new(None),
            state: Cell::new(DndState::Idle),
            cursor: Cell::new((0, 0)),
            panel_area: Cell::new(None),
            callbacks: RefCell::new(HashMap::new()),
            panel_window: Cell::new(None),
            last_position_time: Cell::new(0),
            last_paths: RefCell::new(Vec::new()),
        }
    }

    pub fn state(&self) -> DndState { self.state.get() }
    pub fn overlay(&self) -> Option<Window> { self.overlay.get() }
    pub fn cursor(&self) -> (i32, i32) { self.cursor.get() }
    pub fn panel_window(&self) -> Option<Window> { self.panel_window.get() }
    pub fn set_panel_window(&self, win: Window) { self.panel_window.set(Some(win)); }
    pub fn last_position_time(&self) -> u32 { self.last_position_time.get() }
    pub(crate) fn set_last_position_time(&self, t: u32) { self.last_position_time.set(t); }

    pub fn set_last_paths(&self, paths: Vec<PathBuf>) {
        *self.last_paths.borrow_mut() = paths;
    }

    pub fn ensure_atoms(
        &self,
        conn: &RustConnection,
    ) -> Result<DndAtoms, Box<dyn std::error::Error>> {
        if let Some(a) = self.atoms.get() {
            return Ok(a);
        }
        let a = DndAtoms::intern(conn)?;
        self.atoms.set(Some(a));
        Ok(a)
    }

    pub fn resize(&self, area: Rect) { self.panel_area.set(Some(area)); }
    pub fn panel_area(&self) -> Option<Rect> { self.panel_area.get() }

    pub fn register(&self, callbacks: HashMap<WidgetId, DndCallbacks>) {
        *self.callbacks.borrow_mut() = callbacks;
    }

    pub fn callback_for(&self, x: i32, y: i32) -> Option<(WidgetId, DndCallbacks)> {
        let (lx, ly) = match self.panel_area.get() {
            Some(pa) => (x - pa.x, y - pa.y),
            None => (x, y),
        };
        let cb = self.callbacks.borrow();
        for (id, c) in cb.iter() {
            if let Some(area) = c.area.get() {
                if area.contains(lx, ly) {
                    return Some((id.clone(), c.clone()));
                }
            }
        }
        let mut best: Option<(i32, WidgetId, DndCallbacks)> = None;
        for (id, c) in cb.iter() {
            if let Some(area) = c.area.get() {
                if lx >= area.right() && ly >= area.y && ly < area.bottom() {
                    let right = area.right();
                    if best.as_ref().map(|(r, _, _)| right > *r).unwrap_or(true) {
                        best = Some((right, id.clone(), c.clone()));
                    }
                }
            }
        }
        best.map(|(_, id, c)| (id, c))
    }

    pub fn callbacks_at_cursor(&self) -> Option<(i32, i32, DndCallbacks)> {
        let (x, y) = self.cursor.get();
        let pa = self.panel_area.get()?;
        let (lx_panel, ly_panel) = (x - pa.x, y - pa.y);
        let cb = self.callbacks.borrow();

        for (_id, c) in cb.iter() {
            if let Some(area) = c.area.get() {
                if area.contains(lx_panel, ly_panel) {
                    return Some((lx_panel - area.x, ly_panel - area.y, c.clone()));
                }
            }
        }
        let mut best: Option<(i32, i32, i32, DndCallbacks)> = None;
        for (_id, c) in cb.iter() {
            if let Some(area) = c.area.get() {
                if lx_panel >= area.right() && ly_panel >= area.y && ly_panel < area.bottom() {
                    let right = area.right();
                    if best.as_ref().map(|(r, _, _, _)| right > *r).unwrap_or(true) {
                        best = Some((right, lx_panel - area.x, ly_panel - area.y, c.clone()));
                    }
                }
            }
        }
        best.map(|(_, wx, wy, c)| (wx, wy, c))
    }

    pub fn forward_to_target(
        &self,
        conn: &RustConnection,
        atoms: &DndAtoms,
        target: Window,
        paths: &[PathBuf],
        time: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source = match self.overlay.get() {
            Some(w) => w,
            None => return Ok(()),
        };
        let (cx, cy) = self.cursor.get();
        forward::send_enter(conn, atoms, target, source, &[atoms.text_uri_list])?;
        forward::send_position(conn, atoms, target, source, cx, cy, time)?;
        forward::send_drop(conn, atoms, target, source, time)?;
        Ok(())
    }

    pub fn handle_selection_request(
        &self,
        conn: &RustConnection,
        atoms: &DndAtoms,
        requestor: Window,
        selection: u32,
        target: u32,
        property: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let paths = self.last_paths.borrow().clone();
        let payload = uri_list::build_uri_list(&paths);

        if target != atoms.text_uri_list || property == 0 {
            let ev = SelectionNotifyEvent {
                response_type: x11rb::protocol::xproto::SELECTION_NOTIFY_EVENT,
                sequence: 0,
                time: x11rb::CURRENT_TIME,
                requestor,
                selection,
                target,
                property: 0,
            };
            conn.send_event(false, requestor, EventMask::NO_EVENT, ev)?;
            conn.flush()?;
            return Ok(());
        }

        conn.change_property8(
            PropMode::REPLACE,
            requestor,
            property,
            atoms.text_uri_list,
            &payload,
        )?;
        conn.flush()?;

        let ev = SelectionNotifyEvent {
            response_type: x11rb::protocol::xproto::SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: x11rb::CURRENT_TIME,
            requestor,
            selection,
            target,
            property,
        };
        conn.send_event(false, requestor, EventMask::NO_EVENT, ev)?;
        conn.flush()?;
        Ok(())
    }

    pub fn handle_client_message(
        &self,
        conn: &RustConnection,
        msg_type: u32,
        data: [u32; 5],
    ) -> Result<Option<DndResult>, Box<dyn std::error::Error>> {
        let atoms = self.ensure_atoms(conn)?;
        let ev = match receive::parse_client_message(&atoms, msg_type, data) {
            Some(e) => e,
            None => return Ok(None),
        };
        let result = receive::apply(conn, self, &atoms, ev)?;
        Ok(Some(result))
    }

    pub fn handle_selection_notify(
        &self,
        conn: &RustConnection,
        selection: u32,
        target: u32,
        property: u32,
        requestor: Window,
    ) -> Result<Option<DndResult>, Box<dyn std::error::Error>> {
        let atoms = self.ensure_atoms(conn)?;
        if selection != atoms.selection {
            return Ok(None);
        }
        let result = receive::handle_selection_notify(
            conn, self, &atoms, target, property, requestor,
        )?;
        Ok(Some(result))
    }

    pub(crate) fn set_state(&self, s: DndState) { self.state.set(s); }
    pub(crate) fn set_cursor(&self, x: i32, y: i32) { self.cursor.set((x, y)); }
}

impl Default for Dnd {
    fn default() -> Self {
        Self::new()
    }
}