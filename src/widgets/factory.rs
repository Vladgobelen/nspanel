//! Фабрика виджетов. Панель — безликий отрисовщик: она не знает
//! конкретных виджетов. Все виджеты — Lua (`~/.config/nspanel/widgets/<id>/widget.lua`),
//! которые используют универсальные инструменты панели:
//! - `buf:*` (рисование: fill_rect, draw_text, draw_line, text_centered),
//! - `socket:*` (чтение/запись в сокет моста: nsbridge, ...),
//! - `json:*` (decode/encode для обмена с мостом),
//! - `font:text_width` (центрирование),
//! - `dnd:*` (XDND: update/set_on_hover/set_on_drop),
//! - `now_hm`, `now_hms`, `Color.rgb`, `Rect.new`.
//!
//! Логика:
//! - `enabled = false` → пропуск.
//! - есть `widget.lua` → `LuaWidget`.
//! - нет `widget.lua` → пропуск (Rust-виджетов нет).
//!
//! `tooltip_enabled` из `config.lua` модуля передаётся в `LuaWidget::load`.
//! Если `false` — `LuaWidget::tooltip` возвращает `None`, панель не
//! показывает тултип для этого виджета (даже если `M.tooltip` определён).
//!
//! `CreatedWidgets` — виджеты + их `DndCallbacks`. `Runtime::setup`
//! передаёт callbacks в `X11Backend::dnd`, чтобы при drop панель знала,
//! какой виджет под курсором (Фаза 5).

use std::collections::HashMap;

use crate::config::Config;
use crate::core::{Widget, WidgetId};
use crate::widgets::lua::dnd_handle::DndCallbacks;
use crate::widgets::lua::LuaWidget;

/// Результат `create_all`: виджеты и их DND-callbacks.
pub struct CreatedWidgets {
    pub widgets: Vec<Box<dyn Widget>>,
    pub dnd_callbacks: HashMap<WidgetId, DndCallbacks>,
}

/// Создать все виджеты из `config.widgets.entries`.
pub fn create_all(config: &Config) -> CreatedWidgets {
    let mut widgets: Vec<Box<dyn Widget>> = Vec::new();
    let mut dnd_callbacks: HashMap<WidgetId, DndCallbacks> = HashMap::new();

    for entry in &config.widgets.entries {
        if !entry.enabled {
            eprintln!("[widgets] '{}' disabled in config, skipping", entry.id);
            continue;
        }

        match &entry.widget_path {
            Some(widget_path) => {
                match LuaWidget::load(
                    entry.id.clone(),
                    widget_path,
                    entry.tooltip_enabled,
                ) {
                    Ok(w) => {
                        eprintln!(
                            "[widgets] loaded Lua widget '{}' from {} (tooltip_enabled={})",
                            entry.id,
                            widget_path.display(),
                            entry.tooltip_enabled
                        );
                        dnd_callbacks.insert(entry.id.clone(), w.dnd_cb());
                        widgets.push(Box::new(w));
                    }
                    Err(e) => {
                        eprintln!("[widgets] failed to load Lua widget '{}': {}", entry.id, e);
                    }
                }
            }
            None => {
                eprintln!(
                    "[widgets] widget '{}' has no widget.lua (Rust widgets are not supported), skipping",
                    entry.id
                );
            }
        }
    }

    CreatedWidgets { widgets, dnd_callbacks }
}