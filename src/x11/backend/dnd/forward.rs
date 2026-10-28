//! Пересылка XDND target'у (приложению).
//!
//! Панель — source для target'а (Firefox). Отправляем `XdndEnter`,
//! `XdndPosition`, `XdndDrop`. Target отвечает `XdndStatus` (accept/reject).
//! Ответ на `SelectionRequest` (text/uri-list) — Шаг 3.2/3.3.

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    ClientMessageData, ClientMessageEvent, ConnectionExt, EventMask, Window,
};
use x11rb::rust_connection::RustConnection;

use super::{DndAtoms, XDND_VERSION};

/// Отправить `XdndEnter` target'у.
///
/// `source` = панель (кто «тащит»), `target` = окно Firefox.
/// `types` = [text/uri-list] (минимум).
pub fn send_enter(
    conn: &RustConnection,
    atoms: &DndAtoms,
    target: Window,
    source: Window,
    types: &[u32],
) -> Result<(), Box<dyn std::error::Error>> {
    // data[0] = source, data[1] = version<<24 | 0 (нет TypeList),
    // data[2..5] = до 3 типов.
    let mut t = [0u32; 3];
    for (i, &ty) in types.iter().take(3).enumerate() {
        t[i] = ty;
    }
    let ev = ClientMessageEvent::new(
        32,
        target,
        atoms.enter,
        ClientMessageData::from([
            source,
            (XDND_VERSION << 24) & 0xFF000000,
            t[0],
            t[1],
            t[2],
        ]),
    );
    conn.send_event(false, target, EventMask::NO_EVENT, ev)?;
    conn.flush()?;
    eprintln!(
        "[forward] XdndEnter -> target=0x{:x} source=0x{:x} types={:?}",
        target, source, types
    );
    Ok(())
}

/// Отправить `XdndPosition` target'у.
pub fn send_position(
    conn: &RustConnection,
    atoms: &DndAtoms,
    target: Window,
    source: Window,
    x: i32,
    y: i32,
    time: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let packed = (((x as u32) & 0xFFFF) << 16) | ((y as u32) & 0xFFFF);
    let ev = ClientMessageEvent::new(
        32,
        target,
        atoms.position,
        ClientMessageData::from([source, 0, packed, time, atoms.action_copy]),
    );
    conn.send_event(false, target, EventMask::NO_EVENT, ev)?;
    conn.flush()?;
    eprintln!(
        "[forward] XdndPosition -> target=0x{:x} x={} y={} time={}",
        target, x, y, time
    );
    Ok(())
}

/// Отправить `XdndDrop` target'у.
pub fn send_drop(
    conn: &RustConnection,
    atoms: &DndAtoms,
    target: Window,
    source: Window,
    time: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let ev = ClientMessageEvent::new(
        32,
        target,
        atoms.drop,
        ClientMessageData::from([source, 0, time, 0, 0]),
    );
    conn.send_event(false, target, EventMask::NO_EVENT, ev)?;
    conn.flush()?;
    eprintln!("[forward] XdndDrop -> target=0x{:x} time={}", target, time);
    Ok(())
}