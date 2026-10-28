//! Обработка X11-событий панели.
//!
//! Одна ответственность: принять `Event` от `X11Backend`, разложить его
//! по типу и передать в нужный подмодуль с общим контекстом `EventCtx`.
//!
//! Здесь НЕТ:
//! - собственно логики кнопок/движения/экспоуза — она в `button.rs`,
//!   `motion.rs`, `expose.rs`;
//! - исполнения `Action` — это `action.rs`;
//! - poll-цикла и переподключения к nsbridge — это `app/run.rs`.
//!
//! `EventCtx` — единственный «толстый» объект, который видят подмодули.
//! Он собирается один раз в `run.rs` на каждое событие и передаётся
//! по `&mut`, чтобы подмодули могли менять `panel`, `x11`, `position`,
//! `current_tooltip` и т.д.

pub mod action;
pub mod button;
pub mod expose;
pub mod motion;

use crate::config::Config;
use crate::core::module::PanelPosition;
use crate::core::{ModuleRegistry, Panel, Theme};
use crate::render::SharedBuffer;
use crate::x11::{Event, X11Backend};

pub use motion::CurrentTooltip;

/// Общий контекст одного X11-события.
///
/// Собирается в `run.rs` перед вызовом `handle_x11_event`. Все поля —
/// ссылки на долгоживущие объекты `run()`, поэтому `EventCtx` дешёвый
/// и живёт только на время обработки одного события.
pub struct EventCtx<'a> {
    pub x11: &'a mut X11Backend,
    pub panel: &'a mut Panel,
    pub registry: &'a mut ModuleRegistry,
    pub config: &'a Config,
    pub theme: &'a Theme,
    pub panel_buf: &'a mut SharedBuffer,
    /// Текущая позиция панели. Меняется при перетаскивании панели
    /// за пределы экрана (`ButtonRelease` → `nearest_edge`).
    pub position: &'a mut PanelPosition,
    pub screen_w: i32,
    pub screen_h: i32,
    pub current_tooltip: &'a mut Option<CurrentTooltip>,
}

impl EventCtx<'_> {
    /// Толщина панели в пикселях (высота для top/bottom, ширина для
    /// left/right). Одна и та же для всех веток — вынесено сюда, чтобы
    /// не дублировать `config.panel.height as i32` в каждом подмодуле.
    pub fn thickness(&self) -> i32 {
        self.config.panel.height as i32
    }
}

/// Точка входа: разложить `Event` по типу и вызвать нужный обработчик.
///
/// `Other` и `PropertyNotify` панель игнорирует: `PropertyNotify` — это
/// забота nsbridge (он следит за `_NET_CLIENT_LIST` и т.п.), а панель
/// получает уже готовый JSON через сокет.
///
/// `DndRedraw` — no-op на Фазе 2. В Фазе 5 будет вызывать `redraw.rs`
/// (render + put_image), чтобы подсветить иконку под курсором с файлом.
pub fn handle_x11_event(ctx: &mut EventCtx, ev: Event) -> Result<(), Box<dyn std::error::Error>> {
    match ev {
        Event::ButtonPress {
            x,
            y,
            button,
            window,
        } => button::handle_press(ctx, x, y, button, window),

        Event::ButtonRelease { x, y, button, .. } => {
            button::handle_release(ctx, x, y, button)
        }

        Event::MotionNotify { x, y, window } => motion::handle_motion(ctx, x, y, window),

        Event::LeaveWindow { window } => expose::handle_leave(ctx, window),

        Event::Expose { window } => expose::handle_expose(ctx, window),

        Event::DndRedraw => Ok(()),

        Event::PropertyNotify { .. } | Event::Other => Ok(()),
    }
}