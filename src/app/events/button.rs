//! Обработка нажатий и отпусканий кнопок мыши на панели.
//!
//! Одна ответственность: `ButtonPress` и `ButtonRelease`. Правая кнопка —
//! контекстное меню (через `ModuleRegistry`), левая — drag виджета, drag
//! панели целиком или клик по виджету. Отпускание левой кнопки фиксирует
//! drag (виджета или панели) и сохраняет prefs/позицию в конфиг.
//!
//! Здесь НЕТ:
//! - движения мыши (это `motion.rs`);
//! - экспоуза и ухода курсора (это `expose.rs`);
//! - исполнения `Action` (это `action.rs`);
//! - знания о том, как устроены модули (это `core/module*`).

use crate::core::module::{ModuleAction, OpenContext};
use crate::core::panel::{DragState, WidgetDragState};
use crate::core::Action;
use crate::render::PixelBuffer;

use super::action::handle_action;
use super::EventCtx;
use crate::app::geometry::{nearest_edge, panel_buf_size, panel_rect_for, position_str};

pub fn handle_press(
    ctx: &mut EventCtx,
    x: i32,
    y: i32,
    button: u8,
    window: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    if ctx.panel.dragging.is_some() || ctx.panel.widget_drag.is_some() {
        return Ok(());
    }
    if ctx.current_tooltip.is_some() {
        ctx.x11.tooltip_hide()?;
        *ctx.current_tooltip = None;
    }

    let menu_open = ctx
        .registry
        .get("menu")
        .map(|m| m.is_open())
        .unwrap_or(false);

    if menu_open {
        if button == 1 {
            let action = ctx.registry.handle_click_any(ctx.x11, x, y)?;
            if let Some(act) = action {
                handle_menu_action(ctx, act)?;
            }
        } else if button == 3 {
            ctx.registry
                .close(&ctx.config.bindings.right_click_panel, ctx.x11)?;
        }
    } else if window == ctx.x11.panel_window {
        if button == 3 {
            open_context_menu(ctx, x, y)?;
        } else if button == 1 {
            handle_left_click(ctx, x, y)?;
        }
    }

    Ok(())
}

pub fn handle_release(
    ctx: &mut EventCtx,
    x: i32,
    y: i32,
    button: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    if button != 1 {
        return Ok(());
    }

    if ctx.panel.widget_drag.is_some() {
        finish_widget_drag(ctx, x, y)?;
    } else if ctx.panel.dragging.is_some() {
        finish_panel_drag(ctx)?;
    }

    Ok(())
}

fn handle_menu_action(
    ctx: &mut EventCtx,
    act: ModuleAction,
) -> Result<(), Box<dyn std::error::Error>> {
    match act {
        ModuleAction::Close => {
            ctx.registry
                .close(&ctx.config.bindings.right_click_panel, ctx.x11)?;
        }
        ModuleAction::RunCommand(cmd) => {
            handle_action(Action::RunCommand(cmd));
            ctx.registry
                .close(&ctx.config.bindings.right_click_panel, ctx.x11)?;
        }
        ModuleAction::TogglePanelLock => {
            ctx.registry
                .close(&ctx.config.bindings.right_click_panel, ctx.x11)?;
            ctx.panel.locked = !ctx.panel.locked;
        }
        ModuleAction::ToggleWidgetsLock => {
            ctx.registry
                .close(&ctx.config.bindings.right_click_panel, ctx.x11)?;
            ctx.panel.widgets_locked = !ctx.panel.widgets_locked;
        }
        ModuleAction::Custom { widget_id, msg } => {
            ctx.registry
                .close(&ctx.config.bindings.right_click_panel, ctx.x11)?;
            if ctx.panel.on_custom_action(&widget_id, &msg) {
                let reflowed = crate::app::poll::relayout_and_render(
                    ctx.panel,
                    ctx.x11,
                    ctx.panel_buf,
                    ctx.theme,
                );
                if !reflowed {
                    ctx.panel.render(ctx.panel_buf, ctx.theme);
                    ctx.x11.put_image(ctx.panel_buf)?;
                }
            } else {
                let _ = crate::app::poll::relayout_and_render(
                    ctx.panel,
                    ctx.x11,
                    ctx.panel_buf,
                    ctx.theme,
                );
            }
        }
        ModuleAction::None => {}
    }
    Ok(())
}

fn open_context_menu(
    ctx: &mut EventCtx,
    x: i32,
    y: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let widget_id = ctx.panel.hit_test(x, y);
    let widget_menu_items = match &widget_id {
        Some(id) => ctx.panel.widget_menu_items(id, x, y),
        None => Vec::new(),
    };

    let open_ctx = OpenContext {
        x,
        y,
        screen_w: ctx.screen_w,
        screen_h: ctx.screen_h,
        panel_locked: ctx.panel.locked,
        widgets_locked: ctx.panel.widgets_locked,
        widget_id: widget_id.map(|s| s.clone()),
        widget_menu_items,
    };
    ctx.registry.open(
        &ctx.config.bindings.right_click_panel,
        ctx.x11,
        open_ctx,
    )?;
    ctx.registry.render_open(ctx.x11)?;
    Ok(())
}

fn handle_left_click(
    ctx: &mut EventCtx,
    x: i32,
    y: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let widget_idx = ctx.panel.widget_index_at(x, y);

    if !ctx.panel.widgets_locked && widget_idx.is_some() {
        begin_widget_drag(ctx, widget_idx.unwrap(), x, y)?;
    } else if !ctx.panel.locked {
        begin_panel_drag(ctx, x, y)?;
    } else if let Some(action) = ctx.panel.on_click_at(x, y) {
        handle_action(action);
    }
    Ok(())
}

fn begin_widget_drag(
    ctx: &mut EventCtx,
    idx: usize,
    x: i32,
    y: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let layout = ctx.panel.layout_widgets();
    let r = layout[idx].1;
    let abs_x = ctx.panel.area.x + r.x;
    let abs_y = ctx.panel.area.y + r.y;

    ctx.panel.widget_drag = Some(WidgetDragState {
        original_index: idx,
        offset_x: x - abs_x,
        offset_y: y - abs_y,
        mouse_x: x,
        mouse_y: y,
    });
    ctx.x11.grab_pointer()?;
    Ok(())
}

fn begin_panel_drag(ctx: &mut EventCtx, x: i32, y: i32) -> Result<(), Box<dyn std::error::Error>> {
    ctx.panel.dragging = Some(DragState {
        offset_x: x - ctx.panel.area.x,
        offset_y: y - ctx.panel.area.y,
    });
    ctx.x11.clear_strut()?;
    ctx.x11.grab_pointer()?;
    Ok(())
}

fn finish_widget_drag(
    ctx: &mut EventCtx,
    x: i32,
    y: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::core::module::PanelPosition;

    let vertical = matches!(
        ctx.panel.position,
        PanelPosition::Left | PanelPosition::Right
    );
    let coord_main = if vertical {
        y - ctx.panel.area.y
    } else {
        x - ctx.panel.area.x
    };
    let from = ctx.panel.widget_drag.unwrap().original_index;
    let (_pos, strip) = ctx.panel.drop_position_for(coord_main, from);

    let drag = ctx.panel.widget_drag.take().unwrap();
    ctx.panel.place_widget_at(drag.original_index, strip);
    ctx.panel.render(ctx.panel_buf, ctx.theme);
    ctx.x11.put_image(ctx.panel_buf)?;
    ctx.x11.ungrab_pointer()?;

    let current = ctx.panel.prefs_for_current();
    if let Err(e) = ctx.config.widgets.save_all(&current) {
        eprintln!("[config] failed to save widget prefs: {e}");
    }
    Ok(())
}

fn finish_panel_drag(ctx: &mut EventCtx) -> Result<(), Box<dyn std::error::Error>> {
    let edge = nearest_edge(ctx.panel.area, ctx.screen_w, ctx.screen_h);
    *ctx.position = edge;
    ctx.panel.position = edge;
    ctx.panel.area = panel_rect_for(edge, ctx.thickness(), ctx.screen_w, ctx.screen_h);
    ctx.panel.dragging = None;
    ctx.x11.ungrab_pointer()?;

    let (bw, bh) = panel_buf_size(
        edge,
        ctx.config.panel.height,
        ctx.screen_w as u16,
        ctx.screen_h as u16,
    );
    ctx.x11.resize_panel(bw as u16, bh as u16)?;
    ctx.x11.move_panel(ctx.panel.area.x, ctx.panel.area.y)?;

    match edge {
        crate::core::module::PanelPosition::Top
        | crate::core::module::PanelPosition::Bottom => {
            ctx.x11
                .update_strut(edge, ctx.config.panel.height as u32, ctx.screen_w as u32)?;
        }
        crate::core::module::PanelPosition::Left
        | crate::core::module::PanelPosition::Right => {
            ctx.x11
                .update_strut(edge, ctx.config.panel.height as u32, ctx.screen_h as u32)?;
        }
    }

    let current = ctx.panel.prefs_for_current();
    ctx.panel.apply_prefs(&current);
    if let Err(e) = ctx.config.save_panel_position(position_str(edge)) {
        eprintln!("[config] failed to save panel position: {e}");
    }

    let new_buf = PixelBuffer::new_shared(bw, bh);
    ctx.panel.render(&new_buf, ctx.theme);
    ctx.x11.put_image(&new_buf)?;
    *ctx.panel_buf = new_buf;

    if ctx.current_tooltip.is_some() {
        ctx.x11.tooltip_hide()?;
        *ctx.current_tooltip = None;
    }
    Ok(())
}