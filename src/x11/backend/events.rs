//! События X11.

use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::xproto::ConnectionExt;

use super::{Event, X11Backend};

impl X11Backend {
    pub fn subscribe_root_motion(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.root_motion_subscribed { return Ok(()); }
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
        if !self.root_motion_subscribed { return Ok(()); }
        let root = self.root_window();
        self.conn.change_window_attributes(
            root,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::NO_EVENT),
        )?;
        self.conn.flush()?;
        self.root_motion_subscribed = false;
        Ok(())
    }

    pub fn root_motion_subscribed(&self) -> bool { self.root_motion_subscribed }

    pub fn next_event(&mut self) -> Result<Option<Event>, Box<dyn std::error::Error>> {
        let ev = match self.conn.poll_for_event()? {
            Some(ev) => ev,
            None => return Ok(None),
        };
        let out = match ev {
            x11rb::protocol::Event::ButtonPress(e) => Event::ButtonPress {
                x: e.root_x as i32, y: e.root_y as i32,
                button: e.detail, window: e.event,
            },
            x11rb::protocol::Event::ButtonRelease(e) => Event::ButtonRelease {
                x: e.root_x as i32, y: e.root_y as i32,
                button: e.detail, window: e.event,
            },
            x11rb::protocol::Event::MotionNotify(e) => Event::MotionNotify {
                x: e.root_x as i32, y: e.root_y as i32, window: e.event,
            },
            x11rb::protocol::Event::Expose(e) => Event::Expose { window: e.window },
            x11rb::protocol::Event::LeaveNotify(e) => Event::LeaveWindow { window: e.event },
            x11rb::protocol::Event::PropertyNotify(e) => Event::PropertyNotify {
                atom: e.atom, window: e.window,
            },
            x11rb::protocol::Event::ClientMessage(e) => {
                let data = e.data.as_data32();
                let is_xdnd = match self.dnd.ensure_atoms(&self.conn) {
                    Ok(atoms) => atoms.is_enter(e.type_) || atoms.is_position(e.type_)
                        || atoms.is_drop(e.type_) || atoms.is_leave(e.type_),
                    Err(_) => false,
                };
                if !is_xdnd {
                    Event::Other
                } else {
                    match self.dnd.handle_client_message(&self.conn, e.type_, data)? {
                        Some(r) if r.redraw => return Ok(Some(Event::DndRedraw)),
                        _ => Event::Other,
                    }
                }
            }
            x11rb::protocol::Event::SelectionNotify(e) => {
                let is_xdnd = match self.dnd.ensure_atoms(&self.conn) {
                    Ok(atoms) => e.selection == atoms.selection,
                    Err(_) => false,
                };
                if !is_xdnd {
                    Event::Other
                } else {
                    match self.dnd.handle_selection_notify(
                        &self.conn, e.selection, e.target, e.property, e.requestor,
                    )? {
                        Some(r) if r.redraw => return Ok(Some(Event::DndRedraw)),
                        _ => Event::Other,
                    }
                }
            }
            _ => Event::Other,
        };
        Ok(Some(out))
    }
}