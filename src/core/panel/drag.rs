//! Перетаскивание виджета вдоль панели.
//!
//! Одна ответственность: всё, что знает про `WidgetDragState` и
//! координаты мыши при drag'е:
//! - `strip_coord` — координата полосы вставки (с прижатием к краям);
//! - `drop_position_for` — позиция и strip для drop;
//! - `insert_index` / `insert_widget` — куда и как вставить виджет;
//! - `drag_main` — координата `main` перетаскиваемого виджета;
//! - `current_strip` — полоса вставки для текущего drag;
//! - `place_widget_at` — вся транзакция «вынуть, пересчитать, вставить».
//!
//! Здесь НЕТ:
//! - раскладки как таковой (это `layout.rs`);
//! - рендера призрака (это `render.rs`, сюда приходит готовый `strip`);
//! - hover/кликов (это `input.rs`);
//! - X11 и событий.
//!
//! `WidgetDragState` живёт в `panel/mod.rs`, но **все вычисления**
//! (`mouse - area - offset`, выбор между `positions[i]` и `mouse`) —
//! здесь. `Panel` только передаёт `self.widget_drag` и поля.

use std::collections::HashMap;
use std::time::Instant;

use crate::config::widgets::WidgetPrefs;
use crate::core::panel::WidgetDragState;
use crate::core::{Rect, Widget, WidgetId};

use super::layout;

/// Координата «полосы вставки» с учётом краёв панели.
///
/// Если `coord` ближе к левому краю, чем к первому виджету — вернуть `0`
/// (вставка в самое начало). Если ближе к правому краю, чем к концу
/// последнего виджета — вернуть `panel_len` (вставка в самый конец).
/// Иначе — `coord` как есть.
pub fn strip_coord(
    coord: i32,
    panel_len: i32,
    vertical: bool,
    layout_items: &[(WidgetId, Rect, i32)],
    drag_from: Option<usize>,
) -> i32 {
    let others: Vec<(i32, i32)> = layout_items
        .iter()
        .enumerate()
        .filter(|(i, _)| Some(*i) != drag_from)
        .map(|(_, (_, r, _))| {
            if vertical {
                (r.y, r.y + r.h)
            } else {
                (r.x, r.x + r.w)
            }
        })
        .collect();

    if others.is_empty() {
        return coord;
    }

    let first_start = others[0].0;
    let last_end = others.last().unwrap().1;

    let dist_left = coord.abs();
    let dist_first = (coord - first_start).abs();
    if dist_left < dist_first {
        return 0;
    }
    let dist_right = (panel_len - coord).abs();
    let dist_last = (coord - last_end).abs();
    if dist_right < dist_last {
        return panel_len;
    }

    coord
}

/// Позиция вставки и `strip` для drop.
///
/// `pos` — координата вставки с прижатием к краям (не выходит за
/// `[0, panel_len - size]`). `strip` — «полоса вставки» без прижатия,
/// нужна для отрисовки линии.
pub fn drop_position_for(
    coord_main: i32,
    size: i32,
    panel_len: i32,
    vertical: bool,
    layout_items: &[(WidgetId, Rect, i32)],
    drag_from: Option<usize>,
) -> (i32, i32) {
    let strip = strip_coord(coord_main, panel_len, vertical, layout_items, drag_from);
    let pos = if strip == 0 {
        0
    } else if strip >= panel_len {
        (panel_len - size).max(0)
    } else {
        strip
    };
    (pos, strip)
}

/// Индекс, куда вставить перетаскиваемый виджет при `strip`.
///
/// `positions` — текущие позиции **без** перетаскиваемого виджета
/// (его уже вынули). `sizes` — размеры виджетов без него же.
/// `to_left` — вставляем в левую половину панели.
pub fn insert_index(
    strip: i32,
    to_left: bool,
    panel_len: i32,
    positions: &[i32],
    sizes: &[i32],
) -> usize {
    let mid = panel_len / 2;

    let mut left_count = 0;
    for i in 0..positions.len() {
        if positions[i] + sizes[i] / 2 < mid {
            left_count += 1;
        } else {
            break;
        }
    }

    if to_left {
        let mut at = left_count;
        for k in 0..left_count {
            if positions[k] + sizes[k] / 2 > strip {
                at = k;
                break;
            }
        }
        at
    } else {
        let mut at = positions.len();
        for k in left_count..positions.len() {
            if positions[k] + sizes[k] / 2 > strip {
                at = k;
                break;
            }
        }
        at
    }
}

/// Вставить виджет в позицию `at` и пересчитать раскладку.
#[allow(clippy::too_many_arguments)]
pub fn insert_widget(
    widgets: &mut Vec<Box<dyn Widget>>,
    positions: &mut Vec<i32>,
    next_update: &mut Vec<Option<Instant>>,
    at: usize,
    w: Box<dyn Widget>,
    nu: Option<Instant>,
    temp_pos: i32,
    panel_len: i32,
    panel_thickness: u32,
    vertical: bool,
) {
    widgets.insert(at, w);
    positions.insert(at, temp_pos);
    next_update.insert(at, nu);
    let new_positions = layout::relayout(widgets, positions, panel_len, panel_thickness, vertical);
    *positions = new_positions;
}

/// Координата `main` (x для горизонтали, y для вертикали) для
/// перетаскиваемого виджета: `mouse - area - offset`.
///
/// Это единственное место, где `mod.rs`-поля `area`/`mouse`/`offset`
/// превращаются в координату вдоль панели.
pub fn drag_main(drag: WidgetDragState, area: Rect, vertical: bool) -> i32 {
    if vertical {
        drag.mouse_y - area.y - drag.offset_y
    } else {
        drag.mouse_x - area.x - drag.offset_x
    }
}

/// Координата «полосы вставки» для текущего drag.
///
/// `drag.original_index` исключается из `layout_items` (виджет «висит»
/// на курсоре, его старая позиция не должна влиять на полосу).
pub fn current_strip(
    drag: WidgetDragState,
    area: Rect,
    panel_len: i32,
    vertical: bool,
    layout_items: &[(WidgetId, Rect, i32)],
) -> i32 {
    let coord_main = if vertical {
        drag.mouse_y - area.y
    } else {
        drag.mouse_x - area.x
    };
    strip_coord(
        coord_main,
        panel_len,
        vertical,
        layout_items,
        Some(drag.original_index),
    )
}

/// Вся транзакция «вынуть виджет, пересчитать, вставить, вернуть prefs».
///
/// Возвращает новые prefs (`layout::prefs_for_current` после вставки).
/// Если `index` вне диапазона — возвращает пустой `HashMap` (как
/// `Panel::place_widget_at` раньше делал `return` без изменений).
#[allow(clippy::too_many_arguments)]
pub fn place_widget_at(
    widgets: &mut Vec<Box<dyn Widget>>,
    positions: &mut Vec<i32>,
    next_update: &mut Vec<Option<Instant>>,
    index: usize,
    strip: i32,
    panel_len: i32,
    panel_thickness: u32,
    vertical: bool,
) -> HashMap<String, WidgetPrefs> {
    if index >= widgets.len() {
        return HashMap::new();
    }

    let w = widgets.remove(index);
    let _ = positions.remove(index);
    let nu = next_update.remove(index);

    *positions = layout::relayout(widgets, positions, panel_len, panel_thickness, vertical);

    let sizes: Vec<i32> = (0..widgets.len())
        .map(|i| layout::widget_size(widgets[i].as_ref(), panel_thickness, vertical))
        .collect();
    let to_left = strip <= panel_len / 2;
    let insert_at = insert_index(strip, to_left, panel_len, positions, &sizes);
    let temp_pos = if to_left { 0 } else { panel_len };

    insert_widget(
        widgets,
        positions,
        next_update,
        insert_at,
        w,
        nu,
        temp_pos,
        panel_len,
        panel_thickness,
        vertical,
    );

    layout::prefs_for_current(widgets, positions, panel_len, panel_thickness, vertical)
}