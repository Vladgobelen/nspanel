//! Чистая раскладка виджетов вдоль панели.
//!
//! Одна ответственность: по списку виджетов, их `prefs` (side/priority)
//! и длине панели посчитать позиции вдоль панели. И обратно: по текущим
//! позициям вывести `prefs` (side/priority).
//!
//! Здесь НЕТ:
//! - `Panel` как типа (только `&[Box<dyn Widget>]` и `&HashMap<...>`);
//! - X11, рендера, событий, drag'а, таймеров;
//! - побочных эффектов: все функции чистые, результат — `Vec<i32>`
//!   или `HashMap<String, WidgetPrefs>`.
//!
//! `apply_prefs` и `prefs_for_current` симметричны: первый `prefs →
//! positions`, второй `positions → prefs`. Общая логика «левая/правая
//! сторона по центру панели» вынесена в `split_by_side`, чтобы они
//! не разъехались.

use std::collections::HashMap;

use crate::config::widgets::{Side, WidgetPrefs};
use crate::core::widget::Widget;
use crate::core::Rect;

/// Длина панели вдоль её главной оси: `area.w` для горизонтальной,
/// `area.h` для вертикальной.
pub fn panel_len(area: Rect, vertical: bool) -> i32 {
    if vertical { area.h } else { area.w }
}

/// Толщина панели поперёк главной оси: `area.h` для горизонтальной,
/// `area.w` для вертикальной.
pub fn panel_thickness(area: Rect, vertical: bool) -> u32 {
    if vertical {
        area.w as u32
    } else {
        area.h as u32
    }
}

/// Размер виджета вдоль главной оси панели.
pub fn widget_size(
    w: &dyn Widget,
    panel_thickness: u32,
    vertical: bool,
) -> i32 {
    let (pw, ph) = w.preferred_size(panel_thickness, vertical);
    if vertical { ph as i32 } else { pw as i32 }
}

/// Размер последнего виджета вдоль главной оси (для `add_widget`,
/// который ставит новый виджет сразу за последним).
pub fn last_size(
    widgets: &[Box<dyn Widget>],
    panel_thickness: u32,
    vertical: bool,
) -> i32 {
    widgets
        .last()
        .map(|w| widget_size(w.as_ref(), panel_thickness, vertical))
        .unwrap_or(0)
}

/// Прямоугольник виджета в локальных координатах панели.
///
/// Для горизонтальной панели `main_pos` — это `x`, `y = 0`;
/// для вертикальной — `y`, `x = 0`.
pub fn widget_rect(
    main_pos: i32,
    pw: u32,
    ph: u32,
    vertical: bool,
) -> Rect {
    if vertical {
        Rect::new(0, main_pos, pw as i32, ph as i32)
    } else {
        Rect::new(main_pos, 0, pw as i32, ph as i32)
    }
}

/// Раскладка: `prefs` (side/priority) → позиции вдоль панели.
///
/// Левые виджеты идут от `0` вправо по возрастанию `priority`,
/// правые — от `panel_len` влево по возрастанию `priority`.
/// Виджеты без `prefs` считаются `Side::Left, priority=u32::MAX`
/// (уезжают в конец левой группы).
///
/// Возвращает `Vec<i32>` той же длины, что `widgets` (индекс = индекс
/// виджета в `widgets`).
pub fn apply_prefs(
    widgets: &[Box<dyn Widget>],
    prefs: &HashMap<String, WidgetPrefs>,
    panel_len: i32,
    panel_thickness: u32,
    vertical: bool,
) -> Vec<i32> {
    if widgets.is_empty() {
        return Vec::new();
    }

    let (left, right) = split_by_side(widgets, prefs);

    let mut positions = vec![0i32; widgets.len()];
    let mut cursor = 0i32;
    for &i in &left {
        let size = widget_size(widgets[i].as_ref(), panel_thickness, vertical);
        positions[i] = cursor;
        cursor += size;
    }
    let mut cursor = panel_len;
    for &i in &right {
        let size = widget_size(widgets[i].as_ref(), panel_thickness, vertical);
        cursor -= size;
        positions[i] = cursor;
    }
    positions
}

/// Обратная операция: `positions` → `prefs` (side/priority).
///
/// Виджеты делятся на левые/правые по центру панели (`mid = panel_len/2`).
/// Внутри левой группы `priority` — это порядок по возрастанию `pos`,
/// внутри правой — по убыванию `pos` (чтобы `priority=0` был ближе
/// к своему краю).
pub fn prefs_for_current(
    widgets: &[Box<dyn Widget>],
    positions: &[i32],
    panel_len: i32,
    panel_thickness: u32,
    vertical: bool,
) -> HashMap<String, WidgetPrefs> {
    let mut out: HashMap<String, WidgetPrefs> = HashMap::new();
    if widgets.is_empty() {
        return out;
    }

    let mid = panel_len / 2;
    let mut left: Vec<(usize, i32)> = Vec::new();
    let mut right: Vec<(usize, i32)> = Vec::new();
    for (i, w) in widgets.iter().enumerate() {
        let pos = positions[i];
        let size = widget_size(w.as_ref(), panel_thickness, vertical);
        let center = pos + size / 2;
        if center < mid {
            left.push((i, pos));
        } else {
            right.push((i, pos));
        }
    }

    left.sort_by_key(|(_, p)| *p);
    for (prio, (i, _)) in left.iter().enumerate() {
        out.insert(
            widgets[*i].id(),
            WidgetPrefs { side: Side::Left, priority: prio as u32 },
        );
    }

    right.sort_by_key(|(_, p)| -*p);
    for (prio, (i, _)) in right.iter().enumerate() {
        out.insert(
            widgets[*i].id(),
            WidgetPrefs { side: Side::Right, priority: prio as u32 },
        );
    }
    out
}

/// Разбить виджеты на левую и правую группы по `prefs`.
///
/// Левая группа — по возрастанию `priority`, правая — по возрастанию
/// `priority`. Виджеты без `prefs` идут в левую группу с `u32::MAX`.
/// Возвращает индексы в `widgets` (не id!).
pub fn split_by_side(
    widgets: &[Box<dyn Widget>],
    prefs: &HashMap<String, WidgetPrefs>,
) -> (Vec<usize>, Vec<usize>) {
    let mut left: Vec<(usize, u32)> = Vec::new();
    let mut right: Vec<(usize, u32)> = Vec::new();
    for (i, w) in widgets.iter().enumerate() {
        let id = w.id();
        let p = prefs.get(&id).copied().unwrap_or(WidgetPrefs {
            side: Side::Left,
            priority: u32::MAX,
        });
        match p.side {
            Side::Left => left.push((i, p.priority)),
            Side::Right => right.push((i, p.priority)),
        }
    }
    left.sort_by_key(|(_, p)| *p);
    right.sort_by_key(|(_, p)| *p);
    (
        left.into_iter().map(|(i, _)| i).collect(),
        right.into_iter().map(|(i, _)| i).collect(),
    )
}

/// Локальная раскладка: `Vec<(WidgetId, Rect, main_pos)>` для всех виджетов.
///
/// `positions` — те же позиции, что в `Panel::positions`. Если виджет
/// сейчас тащат (`drag_from = Some(i)`), его `main_pos` берётся из
/// `drag_main` (координата мыши минус offset), а не из `positions[i]`.
///
/// Эта функция чистая: не трогает `Panel`, не читает `widget_drag`,
/// не рисует. Всё, что нужно для drag, передаётся аргументами.
pub fn layout_widgets(
    widgets: &[Box<dyn Widget>],
    positions: &[i32],
    panel_thickness: u32,
    vertical: bool,
    drag_from: Option<usize>,
    drag_main: i32,
) -> Vec<(String, Rect, i32)> {
    let mut out = Vec::with_capacity(widgets.len());
    for (i, w) in widgets.iter().enumerate() {
        let (pw, ph) = w.preferred_size(panel_thickness, vertical);
        let main_pos = if Some(i) == drag_from {
            drag_main
        } else {
            positions[i]
        };
        let rect = widget_rect(main_pos, pw, ph, vertical);
        out.push((w.id(), rect, main_pos));
    }
    out
}

/// Пересчитать `positions` после изменения размеров виджетов.
///
/// В отличие от `apply_prefs`, не использует `prefs`: сохраняет текущее
/// разделение на левые/правые (по центру панели) и переукладывает
/// каждую группу от своего края, устраняя дыры и наложения, которые
/// могли появиться после смены `preferred_size`.
pub fn relayout(
    widgets: &[Box<dyn Widget>],
    positions: &[i32],
    panel_len: i32,
    panel_thickness: u32,
    vertical: bool,
) -> Vec<i32> {
    let mid = panel_len / 2;
    let mut left: Vec<usize> = Vec::new();
    let mut right: Vec<usize> = Vec::new();
    for i in 0..widgets.len() {
        let pos = positions[i];
        let size = widget_size(widgets[i].as_ref(), panel_thickness, vertical);
        let center = pos + size / 2;
        if center < mid {
            left.push(i);
        } else {
            right.push(i);
        }
    }

    let mut out = positions.to_vec();
    let mut cursor = 0i32;
    for &i in &left {
        out[i] = cursor;
        cursor += widget_size(widgets[i].as_ref(), panel_thickness, vertical);
    }
    let mut cursor = panel_len;
    for &i in right.iter().rev() {
        cursor -= widget_size(widgets[i].as_ref(), panel_thickness, vertical);
        out[i] = cursor;
    }
    out
}