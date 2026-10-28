//! Экспоуз и уход курсора: перерисовка после перекрытия окна и сброс
//! hover/тултипа при `LeaveWindow`.
//!
//! Одна ответственность: `Expose` и `LeaveWindow`. `Expose` — «окно
//! перерисовалось, отдай актуальный буфер» (панель / popup меню /
//! окно тултипа). `LeaveWindow` — «курсор ушёл с окна панели, сбрось
//! hover и скрой тултип».
//!
//! Здесь НЕТ:
//! - движения мыши (это `motion.rs`);
//! - нажатий/отпусканий (это `button.rs`);
//! - решений о том, ЧТО рисовать (это `Panel` / `ModuleRegistry` /
//!   `X11Backend::tooltip_show`).

use super::EventCtx;

/// `Expose`: окно перерисовалось.
///
/// Варианты:
/// - идёт drag панели — не перерисовываем (drag сам рисует на каждом
///   `MotionNotify`, а лишний `render + put_image` только мерцает);
/// - окно панели — `render + put_image` из `panel`;
/// - окно popup'а открытого модуля — `registry.render_open`;
/// - окно тултипа — заново `tooltip_show` по сохранённым строкам
///   (`current_tooltip`), потому что `Expose` мог прийти после
///   `unmap/map` или перекрытия, и содержимое окна потеряно.
///
/// Ветка «окно тултипа» нужна, потому что тултип — отдельное X-окно
/// с `background_pixmap = NONE`: после перекрытия X-сервер сам его
/// не восстановит, мы обязаны перерисовать.
pub fn handle_expose(
    ctx: &mut EventCtx,
    window: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    if ctx.panel.dragging.is_some() {
        return Ok(());
    }

    if window == ctx.x11.panel_window {
        ctx.panel.render(ctx.panel_buf, ctx.theme);
        ctx.x11.put_image(ctx.panel_buf)?;
    } else if ctx.x11.popup_id_for_window(window).is_some() {
        ctx.registry.render_open(ctx.x11)?;
    } else if ctx.x11.is_tooltip_window(window) {
        if let Some(tt) = ctx.current_tooltip.as_ref() {
            ctx.x11.tooltip_show(&tt.lines, ctx.theme, tt.x, tt.y)?;
        }
    }
    Ok(())
}

/// `LeaveWindow`: курсор ушёл с окна панели.
///
/// Сбрасываем hover у всех виджетов (каждый виджет сам решает, что
/// значит «hover сброшен» — `on_hover(-1, -1)`), отписываемся от root
/// motion и скрываем тултип.
///
/// `window != panel_window` игнорируем: `LeaveWindow` от popup'ов и
/// тултипа нам не интересен — их жизнью управляют `ModuleRegistry`
/// и `tooltip_hide`.
pub fn handle_leave(
    ctx: &mut EventCtx,
    window: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    if window != ctx.x11.panel_window {
        return Ok(());
    }

    if ctx.panel.clear_hover() {
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