//! Popup-окна модулей: создание, показ, скрытие, вывод буфера.

use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::xproto::ConnectionExt;
use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;

use crate::core::module::{ModuleError, PopupId};
use crate::render::SharedBuffer;

use super::super::X11Backend;

pub(super) fn create(b: &mut X11Backend, w: u16, h: u16) -> Result<PopupId, ModuleError> {
    let screen = &b.conn.setup().roots[b.screen_num];
    let root = screen.root;

    let window: Window = b
        .conn
        .generate_id()
        .map_err(|e| ModuleError::Host(e.to_string()))?;

    b.conn
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

    let atom_type = b
        .conn
        .intern_atom(false, b"_NET_WM_WINDOW_TYPE")
        .map_err(|e| ModuleError::Host(e.to_string()))?
        .reply()
        .map_err(|e| ModuleError::Host(e.to_string()))?
        .atom;
    let atom_popup = b
        .conn
        .intern_atom(false, b"_NET_WM_WINDOW_TYPE_POPUP_MENU")
        .map_err(|e| ModuleError::Host(e.to_string()))?
        .reply()
        .map_err(|e| ModuleError::Host(e.to_string()))?
        .atom;
    b.conn
        .change_property32(
            PropMode::REPLACE,
            window,
            atom_type,
            AtomEnum::ATOM,
            &[atom_popup],
        )
        .map_err(|e| ModuleError::Host(e.to_string()))?;

    b.conn.flush().map_err(|e| ModuleError::Host(e.to_string()))?;

    let id = PopupId(b.next_popup_id);
    b.next_popup_id += 1;
    b.popups.insert(id, window);
    Ok(id)
}

pub(super) fn show(
    b: &mut X11Backend,
    id: PopupId,
    x: i32,
    y: i32,
    w: u16,
    h: u16,
) -> Result<(), ModuleError> {
    let window = *b
        .popups
        .get(&id)
        .ok_or_else(|| ModuleError::Host(format!("popup {id:?} not found")))?;
    b.conn
        .configure_window(
            window,
            &ConfigureWindowAux::new()
                .x(x)
                .y(y)
                .width(w as u32)
                .height(h as u32),
        )
        .map_err(|e| ModuleError::Host(e.to_string()))?;
    b.conn
        .map_window(window)
        .map_err(|e| ModuleError::Host(e.to_string()))?;
    b.conn.flush().map_err(|e| ModuleError::Host(e.to_string()))?;
    Ok(())
}

pub(super) fn hide(b: &mut X11Backend, id: PopupId) -> Result<(), ModuleError> {
    let window = *b
        .popups
        .get(&id)
        .ok_or_else(|| ModuleError::Host(format!("popup {id:?} not found")))?;
    b.conn
        .unmap_window(window)
        .map_err(|e| ModuleError::Host(e.to_string()))?;
    b.conn.flush().map_err(|e| ModuleError::Host(e.to_string()))?;
    Ok(())
}

pub(super) fn put_image(
    b: &mut X11Backend,
    id: PopupId,
    buf: &SharedBuffer,
) -> Result<(), ModuleError> {
    let window = *b
        .popups
        .get(&id)
        .ok_or_else(|| ModuleError::Host(format!("popup {id:?} not found")))?;
    b.put_image_to(window, buf)
        .map_err(|e| ModuleError::Host(e.to_string()))
}