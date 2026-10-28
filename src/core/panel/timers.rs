//! Таймеры виджетов и рассылка сокетов.
//!
//! Одна ответственность: «когда будить виджет по таймеру» и
//! «доставить сокет/сигнал нужному виджету».
//!
//! Здесь НЕТ:
//! - раскладки, drag, рендера, ввода;
//! - X11 и событий.

use std::time::{Duration, Instant};

use crate::core::ipc::UnixStreamClient;
use crate::core::Widget;

/// Вызвать `update()` у всех виджетов, у которых наступил `deadline`.
/// `next_update[i]` обновляется: `Some(now + d)` или `None`.
pub fn update_due(
    widgets: &mut [Box<dyn Widget>],
    next_update: &mut [Option<Instant>],
    now: Instant,
) -> bool {
    let mut any = false;
    for (i, w) in widgets.iter_mut().enumerate() {
        let due = match next_update[i] {
            Some(t) => now >= t,
            None => false,
        };
        if !due {
            continue;
        }
        let next: Option<Duration> = w.update();
        next_update[i] = next.map(|d| now + d);
        any = true;
    }
    any
}

/// Вызвать `update()` у всех виджетов (старт панели).
pub fn update_all(widgets: &mut [Box<dyn Widget>], next_update: &mut [Option<Instant>]) {
    let now = Instant::now();
    for (i, w) in widgets.iter_mut().enumerate() {
        let next: Option<Duration> = w.update();
        next_update[i] = next.map(|d| now + d);
    }
}

/// Ближайший дедлайн среди всех виджетов.
pub fn next_wakeup(next_update: &[Option<Instant>]) -> Option<Instant> {
    next_update.iter().filter_map(|t| *t).min()
}

/// Спросить все виджеты, не изменился ли их `preferred_size`.
pub fn any_needs_relayout(widgets: &mut [Box<dyn Widget>]) -> bool {
    let mut any = false;
    for w in widgets.iter_mut() {
        if w.needs_relayout() {
            any = true;
        }
    }
    any
}

/// Дать сокет виджету по id.
pub fn set_socket_for(
    widgets: &mut [Box<dyn Widget>],
    id: &str,
    socket: UnixStreamClient,
) {
    if let Some(idx) = widgets.iter().position(|w| w.id() == id) {
        widgets[idx].set_socket(socket);
    }
}

/// Сигнал виджету: «в сокете есть данные».
pub fn notify_socket_ready(widgets: &mut [Box<dyn Widget>], id: &str) {
    if let Some(idx) = widgets.iter().position(|w| w.id() == id) {
        widgets[idx].on_socket_ready();
    }
}