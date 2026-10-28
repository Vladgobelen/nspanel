//! Ввод: попадание курсора, hover, клик, тултип, контекстное меню,
//! кастомные действия виджета.
//!
//! Одна ответственность: «что виджет делает по вводу». Все методы
//! принимают абсолютные координаты и `Panel`-поля как аргументы, чтобы
//! не тащить `&mut Panel` (кроме тех, что реально мутируют виджет).
//!
//! Здесь НЕТ:
//! - раскладки (это `layout.rs`);
//! - drag (это `drag.rs`);
//! - рендера (это `render.rs`);
//! - X11 и событий (это `app/events/*`).

use crate::core::{Action, Rect, Widget, WidgetId};

/// Найти индекс виджета под точкой (в локальных координатах панели).
fn index_at(
    lx: i32,
    ly: i32,
    layout_items: &[(WidgetId, Rect, i32)],
) -> Option<usize> {
    for (i, (_, rect, _)) in layout_items.iter().enumerate() {
        if rect.contains(lx, ly) {
            return Some(i);
        }
    }
    None
}

/// Виджет под абсолютной точкой (если она внутри панели).
pub fn hit_test(
    x: i32,
    y: i32,
    area: Rect,
    layout_items: &[(WidgetId, Rect, i32)],
) -> Option<WidgetId> {
    if !area.contains(x, y) {
        return None;
    }
    let lx = x - area.x;
    let ly = y - area.y;
    index_at(lx, ly, layout_items).map(|i| layout_items[i].0.clone())
}

/// Индекс виджета под абсолютной точкой.
pub fn widget_index_at(
    x: i32,
    y: i32,
    area: Rect,
    layout_items: &[(WidgetId, Rect, i32)],
) -> Option<usize> {
    if !area.contains(x, y) {
        return None;
    }
    let lx = x - area.x;
    let ly = y - area.y;
    index_at(lx, ly, layout_items)
}

/// Hover: передать `on_hover` виджету под курсором.
///
/// Возвращает `true`, если виджет изменился (нужна перерисовка).
pub fn on_hover_at(
    x: i32,
    y: i32,
    area: Rect,
    layout_items: &[(WidgetId, Rect, i32)],
    widgets: &mut [Box<dyn Widget>],
) -> bool {
    if !area.contains(x, y) {
        return false;
    }
    let lx = x - area.x;
    let ly = y - area.y;
    if let Some(idx) = index_at(lx, ly, layout_items) {
        let rect = layout_items[idx].1;
        let wx = lx - rect.x;
        let wy = ly - rect.y;
        return widgets[idx].on_hover(wx, wy);
    }
    false
}

/// Сбросить hover у всех виджетов (`on_hover(-1, -1)`).
pub fn clear_hover(widgets: &mut [Box<dyn Widget>]) -> bool {
    let mut any = false;
    for w in widgets.iter_mut() {
        if w.on_hover(-1, -1) {
            any = true;
        }
    }
    any
}

/// Тултип для точки (если она над виджетом).
pub fn tooltip_at(
    x: i32,
    y: i32,
    area: Rect,
    layout_items: &[(WidgetId, Rect, i32)],
    widgets: &mut [Box<dyn Widget>],
) -> Option<Vec<String>> {
    if !area.contains(x, y) {
        return None;
    }
    let lx = x - area.x;
    let ly = y - area.y;
    if let Some(idx) = index_at(lx, ly, layout_items) {
        let rect = layout_items[idx].1;
        let wx = lx - rect.x;
        let wy = ly - rect.y;
        return widgets[idx].tooltip(wx, wy);
    }
    None
}

/// Клик по точке: `on_click` виджета под курсором.
pub fn on_click_at(
    x: i32,
    y: i32,
    area: Rect,
    layout_items: &[(WidgetId, Rect, i32)],
    widgets: &mut [Box<dyn Widget>],
) -> Option<Action> {
    if !area.contains(x, y) {
        return None;
    }
    let lx = x - area.x;
    let ly = y - area.y;
    if let Some(idx) = index_at(lx, ly, layout_items) {
        let rect = layout_items[idx].1;
        let wx = lx - rect.x;
        let wy = ly - rect.y;
        return Some(widgets[idx].on_click(wx, wy));
    }
    None
}

/// Пункты контекстного меню для виджета `id` в точке `(x, y)`.
///
/// Если `id` не найден — пустой список.
pub fn widget_menu_items(
    id: &str,
    x: i32,
    y: i32,
    area: Rect,
    layout_items: &[(WidgetId, Rect, i32)],
    widgets: &[Box<dyn Widget>],
) -> Vec<(String, Action)> {
    let idx = match layout_items.iter().position(|(wid, _, _)| wid == id) {
        Some(i) => i,
        None => return Vec::new(),
    };
    let r = layout_items[idx].1;
    let wx = x - area.x - r.x;
    let wy = y - area.y - r.y;
    widgets[idx].menu_items(wx, wy)
}

/// Кастомное действие из меню: `on_custom_action` виджета `id`.
pub fn on_custom_action(
    id: &str,
    msg: &str,
    widgets: &mut [Box<dyn Widget>],
) -> bool {
    match widgets.iter().position(|w| w.id() == id) {
        Some(idx) => widgets[idx].on_custom_action(msg),
        None => false,
    }
}