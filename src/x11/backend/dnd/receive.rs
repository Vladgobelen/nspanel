//! Приём XDND от source'а (файлового менеджера).

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageData, ClientMessageEvent, ConnectionExt, EventMask, Window,
};
use x11rb::rust_connection::RustConnection;

use super::uri_list;
use super::{Dnd, DndAtoms, DndResult, DndState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DndEvent {
    Enter { source: Window, version: u32, types: Vec<u32> },
    Position { source: Window, x: i32, y: i32, time: u32 },
    Drop { source: Window, time: u32 },
    Leave { source: Window },
}

pub fn parse_client_message(
    atoms: &DndAtoms,
    msg_type: u32,
    data: [u32; 5],
) -> Option<DndEvent> {
    let source = data[0];
    if atoms.is_enter(msg_type) {
        let version = (data[1] >> 24) & 0xFF;
        let mut types = Vec::new();
        for &t in &data[2..5] {
            if t != 0 { types.push(t); }
        }
        return Some(DndEvent::Enter { source, version, types });
    }
    if atoms.is_position(msg_type) {
        let packed = data[2];
        let x = ((packed >> 16) & 0xFFFF) as i32;
        let y = (packed & 0xFFFF) as i32;
        return Some(DndEvent::Position { source, x, y, time: data[3] });
    }
    if atoms.is_drop(msg_type) {
        return Some(DndEvent::Drop { source, time: data[2] });
    }
    if atoms.is_leave(msg_type) {
        return Some(DndEvent::Leave { source });
    }
    None
}

pub fn apply(
    conn: &RustConnection,
    dnd: &Dnd,
    atoms: &DndAtoms,
    ev: DndEvent,
) -> Result<DndResult, Box<dyn std::error::Error>> {
    match ev {
        DndEvent::Enter { source, version: _, types } => {
            dnd.set_state(DndState::Entered { source });
            let accepts = types.contains(&atoms.text_uri_list) || types.is_empty();
            let target = dnd.panel_window().unwrap_or(source);
            send_status(conn, atoms, source, target, accepts)?;
            Ok(DndResult::NONE)
        }
        DndEvent::Position { source, x, y, time } => {
            dnd.set_cursor(x, y);
            dnd.set_last_position_time(time);
            let target = dnd.panel_window().unwrap_or(source);
            send_status(conn, atoms, source, target, true)?;

            if let Some((wx, wy, cb)) = dnd.callbacks_at_cursor() {
                if let Some(f) = cb.on_hover.borrow().as_ref() {
                    if let Ok(mlua::Value::Boolean(true)) = f.call::<mlua::Value>((wx, wy, true)) {
                        return Ok(DndResult::REDRAW);
                    }
                }
            }
            Ok(DndResult::NONE)
        }
        DndEvent::Drop { source, time } => {
            dnd.set_state(DndState::Dropped { source, time });
            let target = dnd.panel_window().unwrap_or(source);
            send_status(conn, atoms, source, target, true)?;

            let panel_window = match dnd.panel_window() {
                Some(w) => w,
                None => return Ok(DndResult::REDRAW),
            };

            let last_pos_time = dnd.last_position_time();
            let conv_time = if last_pos_time != 0 { last_pos_time } else { x11rb::CURRENT_TIME };
            let _ = conn.convert_selection(
                panel_window, atoms.selection, atoms.text_uri_list, atoms.data_property, conv_time,
            );
            conn.flush()?;
            Ok(DndResult::REDRAW)
        }
        DndEvent::Leave { source: _ } => {
            dnd.set_state(DndState::Idle);
            if let Some((wx, wy, cb)) = dnd.callbacks_at_cursor() {
                if let Some(f) = cb.on_hover.borrow().as_ref() {
                    let _ = f.call::<mlua::Value>((wx, wy, false));
                }
            }
            Ok(DndResult::REDRAW)
        }
    }
}

fn send_status(
    conn: &RustConnection,
    atoms: &DndAtoms,
    source: Window,
    target_window: Window,
    accept: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let flags = if accept { 1u32 } else { 0u32 };
    let ev = ClientMessageEvent::new(
        32, source, atoms.status,
        ClientMessageData::from([target_window, flags, 0, 0, atoms.action_copy]),
    );
    conn.send_event(false, source, EventMask::NO_EVENT, ev)?;
    conn.flush()?;
    Ok(())
}

fn send_finished(
    conn: &RustConnection,
    atoms: &DndAtoms,
    source: Window,
    success: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let ev = ClientMessageEvent::new(
        32, source, atoms.finished,
        ClientMessageData::from([source, if success { 1 } else { 0 }, atoms.action_copy, 0, 0]),
    );
    conn.send_event(false, source, EventMask::NO_EVENT, ev)?;
    conn.flush()?;
    Ok(())
}

pub fn handle_selection_notify(
    conn: &RustConnection,
    dnd: &Dnd,
    atoms: &DndAtoms,
    target: u32,
    property: u32,
    _requestor: Window,
) -> Result<DndResult, Box<dyn std::error::Error>> {
    if target != atoms.text_uri_list || property != atoms.data_property {
        return Ok(DndResult::NONE);
    }

    let panel_window = match dnd.panel_window() {
        Some(w) => w,
        None => return Ok(DndResult::NONE),
    };

    let reply = conn.get_property(
        true, panel_window, atoms.data_property, AtomEnum::ANY, 0, 64 * 1024,
    )?.reply()?;

    if reply.value.is_empty() {
        if let DndState::Dropped { source, .. } = dnd.state() {
            let _ = send_finished(conn, atoms, source, false);
        }
        dnd.set_state(DndState::Idle);
        return Ok(DndResult::REDRAW);
    }

    let paths = uri_list::parse_uri_list(&reply.value);

    let mut target_exe: String = String::new();
    if let Some((wx, wy, cb)) = dnd.callbacks_at_cursor() {
        if let Some(f) = cb.on_drop.borrow().as_ref() {
            if let Ok(mlua::Value::String(s)) = f.call::<mlua::Value>((wx, wy)) {
                target_exe = s.to_string_lossy().to_string();
            }
        }
    }

    if !paths.is_empty() {
        let exe = if target_exe.is_empty() {
            "xdg-open".to_string()
        } else {
            target_exe
        };
        for p in &paths {
            let url = format!("file://{}", p.display());
            let mut cmd = std::process::Command::new(&exe);
            cmd.current_dir("/");
            cmd.arg(&url);
            let _ = cmd.spawn();
        }
    }

    if let DndState::Dropped { source, .. } = dnd.state() {
        let _ = send_finished(conn, atoms, source, !paths.is_empty());
    }
    dnd.set_state(DndState::Idle);
    Ok(DndResult::REDRAW)
}