//! Главный `poll`-цикл панели.
//!
//! Одна ответственность: разложить `revents` по готовым модулям:
//! - X11-события → `app/events::handle_x11_event` (собираем `EventCtx`);
//! - сокеты виджетов → `Panel::notify_socket_ready` + `relayout_and_render`;
//! - EOF на сокете → переподключение (`app/bridge::recreate_bridge_connections`);
//! - таймеры виджетов → `Panel::update_due` + `relayout_and_render`.
//!
//! Здесь НЕТ:
//! - установки панели (это `run/runtime.rs`);
//! - стартового подключения (это `run/connect.rs`);
//! - сборки `PollFd` и дедлайнов (это `app/poll.rs`);
//! - обработки конкретных X11-событий (это `app/events/*`);
//! - соединений с nsbridge (это `app/bridge.rs`).

use std::collections::HashMap;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::time::Instant;

use nix::poll::{poll, PollFd, PollFlags, PollTimeout};

use super::connect::{initial_connect, RECONNECT_DELAY};
use super::runtime::Runtime;
use crate::app::bridge::recreate_bridge_connections;
use crate::app::events::handle_x11_event;
use crate::app::events::motion::CurrentTooltip;
use crate::app::events::EventCtx;
use crate::app::poll::{relayout_and_render, timeout_until};
use crate::core::ipc::UnixStreamClient;

/// Точка входа панели.
///
/// `Runtime::setup()` → `initial_connect` → `poll`-цикл.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // ---- 1. Установка ----
    let mut rt = Runtime::setup()?;

    // ---- 2. Стартовое подключение к nsbridge ----
    let mut sockets: HashMap<String, UnixStreamClient> = HashMap::new();
    let mut reconnect_at = initial_connect(&mut rt, &mut sockets);
    let mut current_tooltip: Option<CurrentTooltip> = None;

    // ---- 3. Главный цикл ----
    loop {
        // ---- 3a. Собрать fd для poll ----
        let x11_fd = rt.x11.as_raw_fd();
        let mut fds: Vec<PollFd> = Vec::new();
        fds.push(PollFd::new(
            unsafe { BorrowedFd::borrow_raw(x11_fd) },
            PollFlags::POLLIN,
        ));

        let socket_ids: Vec<String> = sockets.keys().cloned().collect();
        for id in &socket_ids {
            if let Some(sock) = sockets.get(id) {
                fds.push(PollFd::new(
                    unsafe { BorrowedFd::borrow_raw(sock.as_raw_fd()) },
                    PollFlags::POLLIN,
                ));
            }
        }

        // ---- 3b. Дедлайн: таймеры виджетов + реконнект ----
        let mut deadline: Option<Instant> = rt.panel.next_wakeup();
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

        // ---- 3c. poll ----
        match poll(&mut fds, poll_timeout) {
            Ok(0) => {}
            Ok(_) => {
                // ---- X11-события ----
                if let Some(revents) = fds[0].revents() {
                    if revents.contains(PollFlags::POLLIN) {
                        loop {
                            match rt.x11.next_event()? {
                                Some(ev) => {
                                    let mut ctx = EventCtx {
                                        x11: &mut rt.x11,
                                        panel: &mut rt.panel,
                                        registry: &mut rt.registry,
                                        config: &rt.config,
                                        theme: &rt.theme,
                                        panel_buf: &mut rt.panel_buf,
                                        position: &mut rt.position,
                                        screen_w: rt.screen_w,
                                        screen_h: rt.screen_h,
                                        current_tooltip: &mut current_tooltip,
                                    };
                                    handle_x11_event(&mut ctx, ev)?;
                                }
                                None => break,
                            }
                        }
                    }
                }

                // ---- Данные в сокетах виджетов ----
                let mut need_recreate = false;
                for (i, id) in socket_ids.iter().enumerate() {
                    let fd_idx = i + 1;
                    if fd_idx >= fds.len() {
                        continue;
                    }
                    if let Some(revents) = fds[fd_idx].revents() {
                        if revents.contains(PollFlags::POLLIN) {
                            let eof = if let Some(sock) = sockets.get_mut(id) {
                                match sock.peek() {
                                    Ok(0) => true,
                                    Ok(_) => false,
                                    Err(_) => true,
                                }
                            } else {
                                true
                            };

                            if eof {
                                eprintln!("[nspanel] widget '{}' socket closed", id);
                                need_recreate = true;
                                break;
                            }

                            rt.panel.notify_socket_ready(id);
                            let reflowed = relayout_and_render(
                                &mut rt.panel,
                                &mut rt.x11,
                                &mut rt.panel_buf,
                                &rt.theme,
                            );
                            if !reflowed {
                                rt.panel.render(&rt.panel_buf, &rt.theme);
                                rt.x11.put_image(&rt.panel_buf)?;
                            }
                        }
                    }
                }

                if need_recreate {
                    if current_tooltip.is_some() {
                        let _ = rt.x11.tooltip_hide();
                        current_tooltip = None;
                    }
                    let ok = recreate_bridge_connections(
                        &mut rt.panel,
                        &mut rt.x11,
                        &mut sockets,
                        &rt.bridge_path,
                        &mut rt.panel_buf,
                        &rt.theme,
                    );
                    reconnect_at = if ok {
                        None
                    } else {
                        Some(Instant::now() + RECONNECT_DELAY)
                    };
                }
            }
            Err(nix::errno::Errno::EINTR) => continue,
            Err(e) => return Err(format!("poll failed: {e}").into()),
        }

        // ---- 3d. Реконнект по таймауту ----
        if let Some(t) = reconnect_at {
            if Instant::now() >= t {
                let ok = recreate_bridge_connections(
                    &mut rt.panel,
                    &mut rt.x11,
                    &mut sockets,
                    &rt.bridge_path,
                    &mut rt.panel_buf,
                    &rt.theme,
                );
                reconnect_at = if ok {
                    None
                } else {
                    Some(Instant::now() + RECONNECT_DELAY)
                };
            }
        } else if sockets.is_empty() {
            reconnect_at = Some(Instant::now() + RECONNECT_DELAY);
        }

        // ---- 3e. Таймеры виджетов ----
        if rt.panel.update_due(Instant::now()) {
            let reflowed = relayout_and_render(
                &mut rt.panel,
                &mut rt.x11,
                &mut rt.panel_buf,
                &rt.theme,
            );
            if !reflowed {
                rt.panel.render(&rt.panel_buf, &rt.theme);
                rt.x11.put_image(&rt.panel_buf)?;
            }
        }
    }
}