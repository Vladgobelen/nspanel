//! Геометрия панели: где она стоит на экране и какого размера буфер.
//!
//! Одна ответственность: превратить `PanelPosition` + размеры экрана
//! в `Rect` (позиция/размер окна панели) и размер пиксельного буфера.
//!
//! Никаких зависимостей от `Panel`, `X11`, сокетов и событий — только
//! `PanelPosition` и `Rect`. Всё тестируется в изоляции.

use crate::core::module::PanelPosition;
use crate::core::Rect;

/// Прямоугольник панели для заданной позиции и толщины.
///
/// `thickness` — толщина панели (высота для top/bottom, ширина для
/// left/right). `sw`, `sh` — размеры экрана.
pub fn panel_rect_for(
    position: PanelPosition,
    thickness: i32,
    sw: i32,
    sh: i32,
) -> Rect {
    match position {
        PanelPosition::Top => Rect::new(0, 0, sw, thickness),
        PanelPosition::Bottom => Rect::new(0, sh - thickness, sw, thickness),
        PanelPosition::Left => Rect::new(0, 0, thickness, sh),
        PanelPosition::Right => Rect::new(sw - thickness, 0, thickness, sh),
    }
}

/// Размер пиксельного буфера панели (ширина, высота) для позиции.
///
/// Для горизонтальной панели буфер — на всю ширину экрана и толщину
/// панели; для вертикальной — на всю высоту и толщину.
pub fn panel_buf_size(
    position: PanelPosition,
    thickness: u16,
    sw: u16,
    sh: u16,
) -> (u32, u32) {
    match position {
        PanelPosition::Top | PanelPosition::Bottom => (sw as u32, thickness as u32),
        PanelPosition::Left | PanelPosition::Right => (thickness as u32, sh as u32),
    }
}

/// Ближайший край экрана к центру прямоугольника.
///
/// Используется после перетаскивания панели: панель «прилипает» к тому
/// краю, к которому её центр ближе всего.
pub fn nearest_edge(area: Rect, sw: i32, sh: i32) -> PanelPosition {
    let cx = area.x + area.w / 2;
    let cy = area.y + area.h / 2;
    let d_top = cy;
    let d_bottom = sh - cy;
    let d_left = cx;
    let d_right = sw - cx;
    let min = d_top.min(d_bottom).min(d_left).min(d_right);
    if min == d_top {
        PanelPosition::Top
    } else if min == d_bottom {
        PanelPosition::Bottom
    } else if min == d_left {
        PanelPosition::Left
    } else {
        PanelPosition::Right
    }
}

/// Строковое имя позиции для сохранения в `config.lua`.
pub fn position_str(p: PanelPosition) -> &'static str {
    match p {
        PanelPosition::Top => "top",
        PanelPosition::Bottom => "bottom",
        PanelPosition::Left => "left",
        PanelPosition::Right => "right",
    }
}