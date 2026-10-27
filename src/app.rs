use crate::config::Config;
use crate::core::ipc::SocketClient;
use crate::core::module::{ModuleAction, PanelPosition};
use crate::core::modules::MenuModule;
use crate::core::panel::{DragState, WidgetDragState};
use crate::core::{
    Action, ModuleRegistry, OpenContext, Panel, Rect, Theme,
};
use crate::render::{Font, PixelBuffer};
use crate::widgets::factory;
use crate::x11::{Event, X11Backend};
use std::collections::HashMap;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::time::{Duration, Instant};

use nix::poll::{poll, PollFd, PollFlags, PollTimeout};

const RECONNECT_DELAY: Duration = Duration::from_secs(1);

fn panel_rect_for(position: PanelPosition, thickness: i32, sw: i32, sh: i32) -> Rect {
    match position {
        PanelPosition::Top => Rect::new(0, 0, sw, thickness),
        PanelPosition::Bottom => Rect::new(0, sh - thickness, sw, thickness),
        PanelPosition::Left => Rect::new(0, 0, thickness, sh),
        PanelPosition::Right => Rect::new(sw - thickness, 0, thickness, sh),
    }
}

fn panel_buf_size(position: PanelPosition, thickness: u16, sw: u16, sh: u16) -> (u32, u32) {
    match position {
        PanelPosition::Top | PanelPosition::Bottom => (sw as u32, thickness as u32),
        PanelPosition::Left | PanelPosition::Right => (thickness as u32, sh as u32),
    }
}

fn nearest_edge(area: Rect, sw: i32, sh: i32) -> PanelPosition {
    let cx = area.x + area.w / 2;
    let cy = area.y + area.h / 2;
    let d_top = cy;
    let d_bottom = sh - cy;
    let d_left = cx;
    let d_right = sw - cx;
    let min = d_top.min(d_bottom).min(d_left).min(d_right);
    if min == d_top { PanelPosition::Top }
    else if min == d_bottom { PanelPosition::Bottom }
    else if min == d_left { PanelPosition::Left }
    else { PanelPosition::Right }
}

fn position_str(p: PanelPosition) -> &'static str {
    match p {
        PanelPosition::Top => "top",
        PanelPosition::Bottom => "bottom",
        PanelPosition::Left => "left",
        PanelPosition::Right => "right",
    }
}

fn socket_path() -> std::path::PathBuf {
    let uid = nix::unistd::getuid();
    std::path::PathBuf::from(format!("/run/user/{}/nsbridge.sock", uid))
}

fn recreate_bridge_connections(
    panel: &mut Panel,
    x11: &mut X11Backend,
    sockets: &mut HashMap<String, SocketClient>,
    bridge_path: &std::path::Path,
    panel_buf: &mut crate::render::SharedBuffer,
    theme: &Theme,
) -> bool {
    eprintln!("[nspanel] recreating bridge connections...");
    let _ = x11.unsubscribe_root_motion();
    sockets.clear();

    let mut any_connected = false;
    for id in panel.widget_ids() {
        if let Ok(client) = SocketClient::connect(bridge_path) {
            let clone = match client.try_clone() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[nspanel] widget '{}' try_clone failed: {e}", id);
                    continue;
                }
            };
            sockets.insert(id.clone(), client);
            panel.set_socket_for(&id, clone);
            any_connected = true;

            if let Some(sock) = sockets.get_mut(&id) {
                let msg = r#"{"type":"request"}"#;
                let len = msg.len() as u32;
                let mut frame = Vec::with_capacity(4 + msg.len());
                frame.extend_from_slice(&len.to_be_bytes());
                frame.extend_from_slice(msg.as_bytes());
                let _ = sock.send(&frame);
            }
        }
    }

    if any_connected {
        eprintln!("[nspanel] bridge recreated: {}/{} connected", sockets.len(), panel.widget_ids().len());
    } else {
        eprintln!("[nspanel] bridge recreate failed: no connections (nsbridge down?)");
    }

    panel.render(panel_buf, theme);
    let _ = x11.put_image(panel_buf);

    any_connected
}

struct CurrentTooltip {
    lines: Vec<String>,
    x: i32,
    y: i32,
}

impl CurrentTooltip {
    fn matches(&self, lines: &[String]) -> bool {
        self.lines == lines
    }
}

fn timeout_until(deadline: Instant) -> PollTimeout {
    let now = Instant::now();
    if deadline <= now {
        return PollTimeout::from(0u16);
    }
    let d = deadline - now;
    let ms = d.as_millis();
    if ms > i32::MAX as u128 {
        PollTimeout::try_from(i32::MAX).unwrap()
    } else {
        PollTimeout::try_from(d).unwrap_or(PollTimeout::NONE)
    }
}

fn relayout_and_render(
    panel: &mut Panel,
    x11: &mut X11Backend,
    panel_buf: &mut crate::render::SharedBuffer,
    theme: &Theme,
) -> bool {
    if !panel.any_needs_relayout() {
        return false;
    }
    panel.reflow();
    panel.render(panel_buf, theme);
    let _ = x11.put_image(panel_buf);
    true
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load_default()?;
    let font = Font::load(&config.theme.font_path)?;
    let theme = Theme::from_config(&config.theme, font.clone());

    let mut x11 = X11Backend::new(config.panel.height)?;
    let (sw, sh) = x11.screen_size();
    let sw_i = sw as i32;
    let sh_i = sh as i32;

    let thickness = config.panel.height as i32;
    let mut position = config.panel.position;

    let mut panel = Panel::new(panel_rect_for(position, thickness, sw_i, sh_i), position);
    panel.locked = true;
    panel.widgets_locked = true;
    panel.show_borders = config.panel.show_borders;

    for w in factory::create_all(&config) {
        panel.add_widget(w);
    }

    panel.apply_prefs(&config.widgets.prefs);

    let (bw, bh) = panel_buf_size(position, config.panel.height, sw, sh);
    x11.resize_panel(bw as u16, bh as u16)?;
    x11.move_panel(panel.area.x, panel.area.y)?;

    match position {
        PanelPosition::Top | PanelPosition::Bottom => {
            x11.update_strut(position, config.panel.height as u32, sw as u32)?;
        }
        PanelPosition::Left | PanelPosition::Right => {
            x11.update_strut(position, config.panel.height as u32, sh as u32)?;
        }
    }

    let mut registry = ModuleRegistry::new();
    registry.register(Box::new(MenuModule::new(&config, theme.clone())));

    let mut panel_buf = PixelBuffer::new_shared(bw, bh);
    let bridge_path = socket_path();
    let mut sockets: HashMap<String, SocketClient> = HashMap::new();

    let connected = recreate_bridge_connections(
        &mut panel,
        &mut x11,
        &mut sockets,
        &bridge_path,
        &mut panel_buf,
        &theme,
    );

    let mut reconnect_at: Option<Instant> = if connected {
        None
    } else {
        Some(Instant::now() + RECONNECT_DELAY)
    };

    let mut current_tooltip: Option<CurrentTooltip> = None;

    let _ = panel.update_all();
    let _ = panel.any_needs_relayout();
    panel.render(&panel_buf, &theme);
    x11.put_image(&panel_buf)?;

    loop {
        let x11_fd = x11.as_raw_fd();
        let mut fds: Vec<PollFd> = Vec::new();
        fds.push(PollFd::new(unsafe { BorrowedFd::borrow_raw(x11_fd) }, PollFlags::POLLIN));

        let socket_ids: Vec<String> = sockets.keys().cloned().collect();
        for id in &socket_ids {
            if let Some(sock) = sockets.get(id) {
                fds.push(PollFd::new(unsafe { BorrowedFd::borrow_raw(sock.as_raw_fd()) }, PollFlags::POLLIN));
            }
        }

        let mut deadline: Option<Instant> = panel.next_wakeup();
        if let Some(t) = reconnect_at {
            deadline = Some(match deadline {
                Some(d) => d.min(t),
                None => t,
            });
        }
        let poll_timeout: PollTimeout = match deadline {
            None => PollTimeout::NONE,
            Some(t) => timeout_until(t),
        };

        match poll(&mut fds, poll_timeout) {
            Ok(0) => {}
            Ok(_) => {
                if let Some(revents) = fds[0].revents() {
                    if revents.contains(PollFlags::POLLIN) {
                        loop {
                            match x11.next_event()? {
                                Some(ev) => {
                                    handle_x11_event(
                                        &mut x11,
                                        &mut panel,
                                        &mut registry,
                                        &config,
                                        &theme,
                                        &mut panel_buf,
                                        ev,
                                        &mut position,
                                        sw_i,
                                        sh_i,
                                        &mut current_tooltip,
                                    )?;
                                }
                                None => break,
                            }
                        }
                    }
                }

                let mut need_recreate = false;
                for (i, id) in socket_ids.iter().enumerate() {
                    let fd_idx = i + 1;
                    if fd_idx < fds.len() {
                        if let Some(revents) = fds[fd_idx].revents() {
                            if revents.contains(PollFlags::POLLIN) {
                                let eof = if let Some(sock) = sockets.get_mut(id) {
                                    match sock.peek() { Ok(0) => true, Ok(_) => false, Err(_) => true }
                                } else { true };

                                if eof {
                                    eprintln!("[nspanel] widget '{}' socket closed", id);
                                    need_recreate = true;
                                    break;
                                } else {
                                    panel.notify_socket_ready(id);
                                    let reflowed = relayout_and_render(
                                        &mut panel, &mut x11, &mut panel_buf, &theme,
                                    );
                                    if !reflowed {
                                        panel.render(&panel_buf, &theme);
                                        x11.put_image(&panel_buf)?;
                                    }
                                }
                            }
                        }
                    }
                }

                if need_recreate {
                    if current_tooltip.is_some() {
                        let _ = x11.tooltip_hide();
                        current_tooltip = None;
                    }
                    let ok = recreate_bridge_connections(&mut panel, &mut x11, &mut sockets, &bridge_path, &mut panel_buf, &theme);
                    reconnect_at = if ok { None } else { Some(Instant::now() + RECONNECT_DELAY) };
                }
            }
            Err(nix::errno::Errno::EINTR) => continue,
            Err(e) => return Err(format!("poll failed: {e}").into()),
        }

        if let Some(t) = reconnect_at {
            if Instant::now() >= t {
                let ok = recreate_bridge_connections(&mut panel, &mut x11, &mut sockets, &bridge_path, &mut panel_buf, &theme);
                reconnect_at = if ok { None } else { Some(Instant::now() + RECONNECT_DELAY) };
            }
        } else if sockets.is_empty() {
            reconnect_at = Some(Instant::now() + RECONNECT_DELAY);
        }

        if panel.update_due(Instant::now()) {
            let reflowed = relayout_and_render(
                &mut panel, &mut x11, &mut panel_buf, &theme,
            );
            if !reflowed {
                panel.render(&panel_buf, &theme);
                x11.put_image(&panel_buf)?;
            }
        }
    }
}

fn handle_x11_event(
    x11: &mut X11Backend,
    panel: &mut Panel,
    registry: &mut ModuleRegistry,
    config: &Config,
    theme: &Theme,
    panel_buf: &mut crate::render::SharedBuffer,
    ev: Event,
    position: &mut PanelPosition,
    sw_i: i32,
    sh_i: i32,
    current_tooltip: &mut Option<CurrentTooltip>,
) -> Result<(), Box<dyn std::error::Error>> {
    let thickness = config.panel.height as i32;

    match ev {
        Event::ButtonPress { x, y, button, window } => {
            if panel.dragging.is_some() || panel.widget_drag.is_some() {
                return Ok(());
            }
            if current_tooltip.is_some() {
                x11.tooltip_hide()?;
                *current_tooltip = None;
            }
            let menu_open = registry.get("menu").map(|m| m.is_open()).unwrap_or(false);

            if menu_open {
                if button == 1 {
                    let action = registry.handle_click_any(x11, x, y)?;
                    if let Some(act) = action {
                        match act {
                            ModuleAction::Close => { registry.close(&config.bindings.right_click_panel, x11)?; }
                            ModuleAction::RunCommand(cmd) => {
                                handle_action(Action::RunCommand(cmd));
                                registry.close(&config.bindings.right_click_panel, x11)?;
                            }
                            ModuleAction::TogglePanelLock => {
                                registry.close(&config.bindings.right_click_panel, x11)?;
                                panel.locked = !panel.locked;
                            }
                            ModuleAction::ToggleWidgetsLock => {
                                registry.close(&config.bindings.right_click_panel, x11)?;
                                panel.widgets_locked = !panel.widgets_locked;
                            }
                            ModuleAction::Custom { widget_id, msg } => {
                                registry.close(&config.bindings.right_click_panel, x11)?;
                                if panel.on_custom_action(&widget_id, &msg) {
                                    let reflowed = relayout_and_render(
                                        panel, x11, panel_buf, theme,
                                    );
                                    if !reflowed {
                                        panel.render(panel_buf, theme);
                                        x11.put_image(panel_buf)?;
                                    }
                                } else {
                                    let _ = relayout_and_render(panel, x11, panel_buf, theme);
                                }
                            }
                            ModuleAction::None => {}
                        }
                    }
                } else if button == 3 {
                    registry.close(&config.bindings.right_click_panel, x11)?;
                }
            } else if window == x11.panel_window {
                if button == 3 {
                    let widget_id = panel.hit_test(x, y);

                    let widget_menu_items = match &widget_id {
                        Some(id) => panel.widget_menu_items(id, x, y),
                        None => Vec::new(),
                    };

                    let ctx = OpenContext {
                        x, y,
                        screen_w: sw_i,
                        screen_h: sh_i,
                        panel_locked: panel.locked,
                        widgets_locked: panel.widgets_locked,
                        widget_id: widget_id.map(|s| s.clone()),
                        widget_menu_items,
                    };
                    registry.open(&config.bindings.right_click_panel, x11, ctx)?;
                    registry.render_open(x11)?;
                } else if button == 1 {
                    let widget_idx = panel.widget_index_at(x, y);

                    if !panel.widgets_locked && widget_idx.is_some() {
                        let idx = widget_idx.unwrap();
                        let layout = panel.layout_widgets();
                        let r = layout[idx].1;
                        let abs_x = panel.area.x + r.x;
                        let abs_y = panel.area.y + r.y;
                        let offset_x = x - abs_x;
                        let offset_y = y - abs_y;

                        panel.widget_drag = Some(WidgetDragState {
                            original_index: idx,
                            offset_x,
                            offset_y,
                            mouse_x: x,
                            mouse_y: y,
                        });
                        x11.grab_pointer()?;
                    } else if !panel.locked {
                        panel.dragging = Some(DragState {
                            offset_x: x - panel.area.x,
                            offset_y: y - panel.area.y,
                        });
                        x11.clear_strut()?;
                        x11.grab_pointer()?;
                    } else if let Some(action) = panel.on_click_at(x, y) {
                        handle_action(action);
                    }
                }
            }
        }
        Event::MotionNotify { x, y, window } => {
            if let Some(mut drag) = panel.widget_drag {
                drag.mouse_x = x;
                drag.mouse_y = y;
                panel.widget_drag = Some(drag);
                panel.render(panel_buf, theme);
                x11.put_image(panel_buf)?;
                return Ok(());
            }

            if let Some(drag) = panel.dragging {
                panel.area.x = x - drag.offset_x;
                panel.area.y = y - drag.offset_y;
                x11.move_panel(panel.area.x, panel.area.y)?;
                return Ok(());
            }

            let menu_open = registry.get("menu").map(|m| m.is_open()).unwrap_or(false);
            if menu_open {
                let _ = registry.handle_motion_any(x11, x, y)?;
                return Ok(());
            }

            if window == x11.panel_window {
                if !x11.root_motion_subscribed() {
                    x11.subscribe_root_motion()?;
                }
                let changed = panel.on_hover_at(x, y);
                if changed {
                    panel.render(panel_buf, theme);
                    x11.put_image(panel_buf)?;
                    update_tooltip(x11, panel, theme, x, y, current_tooltip)?;
                }
                return Ok(());
            }

            let over_panel = panel.area.contains(x, y);
            if over_panel {
                if !x11.root_motion_subscribed() {
                    x11.subscribe_root_motion()?;
                }
                let changed = panel.on_hover_at(x, y);
                if changed {
                    panel.render(panel_buf, theme);
                    x11.put_image(panel_buf)?;
                    update_tooltip(x11, panel, theme, x, y, current_tooltip)?;
                }
            } else {
                let changed = panel.clear_hover();
                if changed {
                    panel.render(panel_buf, theme);
                    x11.put_image(panel_buf)?;
                    x11.unsubscribe_root_motion()?;
                }
                if current_tooltip.is_some() {
                    x11.tooltip_hide()?;
                    *current_tooltip = None;
                }
            }
        }
        Event::ButtonRelease { x, y, button, .. } => {
            if button == 1 {
                if panel.widget_drag.is_some() {
                    let vertical = matches!(panel.position, PanelPosition::Left | PanelPosition::Right);
                    let coord_main = if vertical { y - panel.area.y } else { x - panel.area.x };
                    let from = panel.widget_drag.unwrap().original_index;
                    let (_pos, strip) = panel.drop_position_for(coord_main, from);
                    let drag = panel.widget_drag.take().unwrap();
                    panel.place_widget_at(drag.original_index, strip);
                    panel.render(panel_buf, theme);
                    x11.put_image(panel_buf)?;
                    x11.ungrab_pointer()?;
                    let current = panel.prefs_for_current();
                    if let Err(e) = config.widgets.save_all(&current) {
                        eprintln!("[config] failed to save widget prefs: {e}");
                    }
                } else if panel.dragging.is_some() {
                    let edge = nearest_edge(panel.area, sw_i, sh_i);
                    *position = edge;
                    panel.position = edge;
                    panel.area = panel_rect_for(edge, thickness, sw_i, sh_i);
                    panel.dragging = None;
                    x11.ungrab_pointer()?;
                    let (bw, bh) = panel_buf_size(edge, config.panel.height, sw_i as u16, sh_i as u16);
                    x11.resize_panel(bw as u16, bh as u16)?;
                    x11.move_panel(panel.area.x, panel.area.y)?;
                    match edge {
                        PanelPosition::Top | PanelPosition::Bottom => {
                            x11.update_strut(edge, config.panel.height as u32, sw_i as u32)?;
                        }
                        PanelPosition::Left | PanelPosition::Right => {
                            x11.update_strut(edge, config.panel.height as u32, sh_i as u32)?;
                        }
                    }
                    let current = panel.prefs_for_current();
                    panel.apply_prefs(&current);
                    if let Err(e) = config.save_panel_position(position_str(edge)) {
                        eprintln!("[config] failed to save panel position: {e}");
                    }
                    let new_buf = PixelBuffer::new_shared(bw, bh);
                    panel.render(&new_buf, theme);
                    x11.put_image(&new_buf)?;
                    *panel_buf = new_buf;
                    if current_tooltip.is_some() {
                        x11.tooltip_hide()?;
                        *current_tooltip = None;
                    }
                }
            }
        }
        Event::LeaveWindow { window } => {
            if window == x11.panel_window {
                if panel.clear_hover() {
                    panel.render(panel_buf, theme);
                    x11.put_image(panel_buf)?;
                    x11.unsubscribe_root_motion()?;
                }
                if current_tooltip.is_some() {
                    x11.tooltip_hide()?;
                    *current_tooltip = None;
                }
            }
        }
        Event::Expose { window } => {
            if panel.dragging.is_some() {
                return Ok(());
            }
            if window == x11.panel_window {
                panel.render(panel_buf, theme);
                x11.put_image(panel_buf)?;
            } else if x11.popup_id_for_window(window).is_some() {
                registry.render_open(x11)?;
            } else if x11.is_tooltip_window(window) {
                if let Some(tt) = current_tooltip.as_ref() {
                    x11.tooltip_show(&tt.lines, theme, tt.x, tt.y)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn update_tooltip(
    x11: &mut X11Backend,
    panel: &mut Panel,
    theme: &Theme,
    x: i32,
    y: i32,
    current_tooltip: &mut Option<CurrentTooltip>,
) -> Result<(), Box<dyn std::error::Error>> {
    let lines = panel.tooltip_at(x, y);
    match (lines, current_tooltip.as_ref()) {
        (None, None) => Ok(()),
        (None, Some(_)) => {
            x11.tooltip_hide()?;
            *current_tooltip = None;
            Ok(())
        }
        (Some(lines), None) => {
            x11.tooltip_show(&lines, theme, x, y)?;
            *current_tooltip = Some(CurrentTooltip { lines, x, y });
            Ok(())
        }
        (Some(lines), Some(cur)) => {
            if cur.matches(&lines) {
                Ok(())
            } else {
                x11.tooltip_show(&lines, theme, x, y)?;
                *current_tooltip = Some(CurrentTooltip { lines, x, y });
                Ok(())
            }
        }
    }
}

fn handle_action(action: Action) {
    match action {
        Action::None => {}
        Action::Log(msg) => eprintln!("[action] {msg}"),
        Action::RunCommand(cmd) => {
            eprintln!("[action] run: {cmd}");
            let _ = std::process::Command::new("sh").arg("-c").arg(&cmd).spawn();
        }
        Action::Custom(_) => {}
    }
}