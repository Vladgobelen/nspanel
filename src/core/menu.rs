use crate::core::{Action, Color, Rect, Theme};
use crate::render::PixelBuffer;

#[derive(Debug, Clone)]
pub struct MenuItem {
    #[allow(dead_code)]
    pub id: String,
    pub label: String,
    pub action: Action,
    pub is_separator: bool,
}

impl MenuItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>, action: Action) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            action,
            is_separator: false,
        }
    }

    pub fn separator() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            action: Action::None,
            is_separator: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Menu {
    pub items: Vec<MenuItem>,
    pub selected: Option<usize>,
    pub area: Rect,
    pub item_height: i32,
    #[allow(dead_code)]
    pub width: i32,
}

impl Menu {
    pub fn new(x: i32, y: i32, items: Vec<MenuItem>) -> Self {
        let item_height = 28;
        let width = 200;
        let height = item_height * items.len() as i32;
        Self {
            items,
            selected: None,
            area: Rect::new(x, y, width, height),
            item_height,
            width,
        }
    }

    pub fn local_item_rect(&self, index: usize) -> Rect {
        Rect::new(
            0,
            (index as i32) * self.item_height,
            self.area.w,
            self.item_height,
        )
    }

    pub fn hit_test(&self, x: i32, y: i32) -> Option<usize> {
        if !self.area.contains(x, y) {
            return None;
        }
        let local_y = y - self.area.y;
        let idx = (local_y / self.item_height) as usize;
        if idx < self.items.len() {
            if self.items[idx].is_separator {
                None
            } else {
                Some(idx)
            }
        } else {
            None
        }
    }

    fn draw_background(&self, buf: &mut PixelBuffer) {
        let menu_bg = Color::rgb(0x30, 0x30, 0x30);
        let menu_border = Color::rgb(0x50, 0x50, 0x50);

        let w = self.area.w;
        let h = self.area.h;

        buf.fill_rect(Rect::new(0, 0, w, h), menu_bg);
        buf.fill_rect(Rect::new(0, 0, w, 1), menu_border);
        buf.fill_rect(Rect::new(0, h - 1, w, 1), menu_border);
        buf.fill_rect(Rect::new(0, 0, 1, h), menu_border);
        buf.fill_rect(Rect::new(w - 1, 0, 1, h), menu_border);
    }

    /// Смешанный (непрозрачный) цвет подсветки: имитирует полупрозрачный
    /// синий (0x50,0x70,0x90) с альфой 0x64 поверх фона меню (0x30,0x30,0x30).
    /// Композитор не нужен, альфа-канал не нужен, copy_area работает как есть.
    fn hover_color() -> Color {
        const BG_R: u32 = 0x30;
        const BG_G: u32 = 0x30;
        const BG_B: u32 = 0x30;
        const FG_R: u32 = 0x50;
        const FG_G: u32 = 0x70;
        const FG_B: u32 = 0x90;
        const A: u32 = 0x64; // 100 из 255
        let r = (FG_R * A + BG_R * (255 - A)) / 255;
        let g = (FG_G * A + BG_G * (255 - A)) / 255;
        let b = (FG_B * A + BG_B * (255 - A)) / 255;
        Color::rgb(r as u8, g as u8, b as u8)
    }

    fn draw_items(
        &self,
        buf: &mut PixelBuffer,
        theme: &Theme,
        selected: Option<usize>,
        hover_all: bool,
    ) {
        let menu_border = Color::rgb(0x50, 0x50, 0x50);
        let item_hover = Self::hover_color();

        for (i, item) in self.items.iter().enumerate() {
            let r = self.local_item_rect(i);
            if item.is_separator {
                let line_y = r.y + r.h / 2;
                buf.draw_line(r.x + 4, line_y, r.x + r.w - 4, line_y, menu_border);
                continue;
            }
            let hovered = if hover_all {
                true
            } else {
                Some(i) == selected
            };
            if hovered {
                buf.fill_rect(Rect::new(r.x + 1, r.y + 1, r.w - 2, r.h - 2), item_hover);
            }
            let text_y = r.y + (r.h - theme.font_size as i32) / 2;
            buf.draw_text(
                r.x + 8,
                text_y,
                &item.label,
                theme.fg,
                theme.font_size,
                &theme.font,
            );
        }
    }

    /// Полный рендер в заданном состоянии подсветки.
    /// `selected = Some(i)` — подсвечен только пункт i.
    /// `selected = None` — подсветки нет.
    pub fn render_full(&self, buf: &mut PixelBuffer, theme: &Theme, selected: Option<usize>) {
        self.draw_background(buf);
        self.draw_items(buf, theme, selected, false);
    }

    /// Рендер, в котором подсвечены ВСЕ не-разделители одновременно.
    /// Используется как источник для быстрого copy_area при hover.
    pub fn render_full_all_hover(&self, buf: &mut PixelBuffer, theme: &Theme) {
        self.draw_background(buf);
        self.draw_items(buf, theme, None, true);
    }
}