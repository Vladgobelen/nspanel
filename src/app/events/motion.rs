//! Движение мыши: drag виджета/панели, hover по панели, тултипы.
//!
//! Одна ответственность: `MotionNotify`. Если идёт drag — обновляем
//! координаты и перерисовываем. Иначе — либо меню (motion идёт в
//! `ModuleRegistry`), либо hover по панели, либо уход курсора с панели
//! (сброс hover + скрытие тултипа).
//!
//! Здесь НЕТ:
//! - нажатий/отпусканий (это `button.rs`);
//! - экспоуза и `LeaveWindow` (это `expose.rs`);
//! - исполнения `Action` (это `action.rs`).

use super::EventCtx;

/// Текущий показанный тултип: строки + точка, у которой он показан.
///
/// Хранится в `run.rs` между событиями, чтобы не пересоздавать popup
/// тултипа на каждый `MotionNotify`. Сравнение `lines` даёт «тот же
/// тултип или новый?»: если строки те же — `tooltip_show` не зовём.
pub struct CurrentTooltip {
    pub lines: Vec<String>,
    pub x: i32,
    pub y: i32,
}

impl CurrentTooltip {
    /// Совпадают ли строки с текущим тултипом. Координаты не сравниваем:
    /// тултип «прилипает» к первой точке, где он появился, и не должен
    /// дёргаться за курсором на каждом пикселе.
    pub fn matches(&self, lines: &[String]) -> bool {
        self.lines == lines
    }
}

/// Движение мыши.
///
/// Приоритет:
/// 1. Если идёт drag виджета — обновить `mouse_x/mouse_y`, перерисовать.
/// 2. Если идёт drag панели — сдвинуть окно панели.
/// 3. Если открыто меню — motion уходит в `ModuleRegistry` (подсветка
///    пункта меню через `copy_area`), панель не трогаем.
/// 4. Если курсор над окном панели — hover виджета + тултип.
/// 5. Если курсор над областью панели (но событие пришло с root) —
///    то же самое (нужно для случая, когда панель уже под курсором,
///    но `MotionNotify` пришёл с root после `subscribe_root_motion`).
/// 6. Иначе — курсор ушёл с панели: сбросить hover, скрыть тултип,
///    отписаться от root motion.
pub fn handle_motion(
    ctx: &mut EventCtx,
    x: i32,
    y: i32,
    window: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Drag виджета.
    if let Some(mut drag) = ctx.panel.widget_drag {
        drag.mouse_x = x;
        drag.mouse_y = y;
        ctx.panel.widget_drag = Some(drag);
        ctx.panel.render(ctx.panel_buf, ctx.theme);
        ctx.x11.put_image(ctx.panel_buf)?;
        return Ok(());
    }

    // 2. Drag панели.
    if let Some(drag) = ctx.panel.dragging {
        ctx.panel.area.x = x - drag.offset_x;
        ctx.panel.area.y = y - drag.offset_y;
        ctx.x11.move_panel(ctx.panel.area.x, ctx.panel.area.y)?;
        return Ok(());
    }

    // 3. Открытое меню.
    let menu_open = ctx
        .registry
        .get("menu")
        .map(|m| m.is_open())
        .unwrap_or(false);
    if menu_open {
        let _ = ctx.registry.handle_motion_any(ctx.x11, x, y)?;
        return Ok(());
    }

    // 4. Курсор над окном панели.
    if window == ctx.x11.panel_window {
        if !ctx.x11.root_motion_subscribed() {
            ctx.x11.subscribe_root_motion()?;
        }
        let changed = ctx.panel.on_hover_at(x, y);
        if changed {
            ctx.panel.render(ctx.panel_buf, ctx.theme);
            ctx.x11.put_image(ctx.panel_buf)?;
            update_tooltip(ctx, x, y)?;
        }
        return Ok(());
    }

    // 5. Курсор над областью панели (событие с root).
    let over_panel = ctx.panel.area.contains(x, y);
    if over_panel {
        if !ctx.x11.root_motion_subscribed() {
            ctx.x11.subscribe_root_motion()?;
        }
        let changed = ctx.panel.on_hover_at(x, y);
        if changed {
            ctx.panel.render(ctx.panel_buf, ctx.theme);
            ctx.x11.put_image(ctx.panel_buf)?;
            update_tooltip(ctx, x, y)?;
        }
        return Ok(());
    }

    // 6. Курсор ушёл с панели.
    let changed = ctx.panel.clear_hover();
    if changed {
        ctx.panel.render(ctx.panel_buf, ctx.theme);
        ctx.x11.put_image(ctx.panel_buf)?;
        ctx.x11.unsubscribe_root_motion()?;
    }
    if ctx.current_tooltip.is_some() {
        ctx.x11.tooltip_hide()?;
        *ctx.current_tooltip = None;
    }
    Ok(())
}

/// Обновить тултип в точке `(x, y)`.
///
/// Сравниваем строки с текущим тултипом:
/// - `None, None` — ничего.
/// - `None, Some` — скрыть.
/// - `Some, None` — показать.
/// - `Some, Some` и строки совпадают — ничего (тултип не дёргается).
/// - `Some, Some` и строки разные — обновить (показать новые).
fn update_tooltip(
    ctx: &mut EventCtx,
    x: i32,
    y: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let lines = ctx.panel.tooltip_at(x, y);
    match (lines, ctx.current_tooltip.as_ref()) {
        (None, None) => Ok(()),
        (None, Some(_)) => {
            ctx.x11.tooltip_hide()?;
            *ctx.current_tooltip = None;
            Ok(())
        }
        (Some(lines), None) => {
            ctx.x11.tooltip_show(&lines, ctx.theme, x, y)?;
            *ctx.current_tooltip = Some(CurrentTooltip { lines, x, y });
            Ok(())
        }
        (Some(lines), Some(cur)) => {
            if cur.matches(&lines) {
                Ok(())
            } else {
                ctx.x11.tooltip_show(&lines, ctx.theme, x, y)?;
                *ctx.current_tooltip = Some(CurrentTooltip { lines, x, y });
                Ok(())
            }
        }
    }
}