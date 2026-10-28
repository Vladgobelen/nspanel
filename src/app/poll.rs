//! Вспомогательные функции главного цикла: таймауты для `poll` и
//! перерисовка после relayout.
//!
//! Одна ответственность: превратить дедлайн в `PollTimeout` и выполнить
//! «reflow + render + put_image», если хоть один виджет просит relayout.
//!
//! Здесь НЕТ:
//! - вызова `poll` и разбора `revents` (это `app/run.rs`);
//! - знания о том, какие fd слушать (это `app/run.rs`);
//! - обработки X11-событий (это `app/events/*`);
//! - решений о переподключении к nsbridge (это `app/run.rs` + `app/bridge.rs`).

use std::time::Instant;

use nix::poll::PollTimeout;

use crate::core::{Panel, Theme};
use crate::render::SharedBuffer;
use crate::x11::X11Backend;

/// Дедлайн `Instant` → `PollTimeout` (сколько миллисекунд ещё ждать).
///
/// Если дедлайн уже прошёл — `0` (poll не спит, сразу возвращает управление).
/// Если дедлайн дальше `i32::MAX` мс — клампим до `i32::MAX`: `PollTimeout`
/// в nix не умеет больше, а ждать «бесконечно долго» всё равно нельзя.
pub fn timeout_until(deadline: Instant) -> PollTimeout {
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

/// Пересчитать раскладку и перерисовать панель, если это нужно.
///
/// Шаги:
/// 1. Спросить виджеты, не изменился ли у кого-то `preferred_size`
///    (`panel.any_needs_relayout()`). Это one-shot флаг: виджет сбрасывает
///    его при первом `true`, поэтому повторный вызов вернёт `false`, пока
///    размер снова не изменится.
/// 2. Если никто не просит — вернуть `false` и НЕ трогать X11.
/// 3. Если просит — `panel.reflow()` (пересчитать `positions` соседей по
///    `side`/`priority`), `panel.render(...)`, `x11.put_image(...)`.
///
/// Возвращает `true`, если перерисовка произошла. Вызывающий код (`run.rs`)
/// использует это, чтобы не делать повторный `render + put_image` поверх
/// свежей картинки.
pub fn relayout_and_render(
    panel: &mut Panel,
    x11: &mut X11Backend,
    panel_buf: &mut SharedBuffer,
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