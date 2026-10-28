//! Рендер панели в буфер: фон, рамки виджетов, призрак перетаскиваемого
//! виджета и полоса вставки.
//!
//! Одна ответственность: «как панель выглядит». Не трогает модель
//! (`Panel` только читается), не трогает X11, не трогает события.
//!
//! Здесь НЕТ:
//! - раскладки (это `layout.rs`);
//! - drag-математики (это `drag.rs`, сюда приходит готовый `strip`);
//! - ввода (это `input.rs`).

use crate::core::panel::WidgetDragState;
use crate::core::{Color, Rect, Theme, Widget, WidgetId};
use crate::render::SharedBuffer;

/// Отрисовать панель целиком.
///
/// `drag` — текущий drag виджета (если есть), `strip` — заранее
/// посчитанная полоса вставки (если есть drag).
#[allow(clippy::too_many_arguments)]
pub fn render(
    buf: &SharedBuffer,
    area: Rect,
    vertical: bool,
    panel_thickness: u32,
    show_borders: bool,
    widgets: &[Box<dyn Widget>],
    layout_items: &[(WidgetId, Rect, i32)],
    drag: Option<WidgetDragState>,
    drag_strip: Option<i32>,
    theme: &Theme,
) {
    {
        let mut b = buf.lock().unwrap();
        b.fill_rect(Rect::new(0, 0, area.w, area.h), theme.bg);
    }

    let drag_from: Option<usize> = drag.map(|d| d.original_index);

    for (i, w) in widgets.iter().enumerate() {
        if Some(i) == drag_from {
            continue;
        }
        let r = layout_items[i].1;

        if show_borders {
            let mut b = buf.lock().unwrap();
            let border = Color::rgb(0x40, 0x40, 0x40);
            b.fill_rect(Rect::new(r.x, r.y, r.w, 1), border);
            b.fill_rect(Rect::new(r.x, r.y + r.h - 1, r.w, 1), border);
            b.fill_rect(Rect::new(r.x, r.y, 1, r.h), border);
            b.fill_rect(Rect::new(r.x + r.w - 1, r.y, 1, r.h), border);
        }

        w.render(buf, r, theme);
    }

    if let Some(d) = drag {
        let w = &widgets[d.original_index];
        let (pw, ph) = w.preferred_size(panel_thickness, vertical);
        let lx = d.mouse_x - area.x - d.offset_x;
        let ly = d.mouse_y - area.y - d.offset_y;
        let r = Rect::new(lx, ly, pw as i32, ph as i32);
        {
            let mut b = buf.lock().unwrap();
            b.fill_rect(r, Color::rgb(0x40, 0x60, 0x80));
        }
        w.render(buf, r, theme);

        if let Some(strip) = drag_strip {
            let color = Color::rgb(0x70, 0xa0, 0xc0);
            let mut b = buf.lock().unwrap();
            if vertical {
                b.fill_rect(Rect::new(0, strip - 1, area.w, 2), color);
            } else {
                b.fill_rect(Rect::new(strip - 1, 0, 2, area.h), color);
            }
        }
    }
}