//! Фабрика виджетов. Панель — безликий отрисовщик: она не знает
//! конкретных виджетов. Все виджеты — Lua (`~/.config/nspanel/widgets/<id>/widget.lua`),
//! которые используют универсальные инструменты панели:
//! - `buf:*` (рисование: fill_rect, draw_text, draw_line, text_centered),
//! - `socket:*` (чтение/запись в сокет моста: nsbridge, ...),
//! - `json:*` (decode/encode для обмена с мостом),
//! - `font:text_width` (центрирование),
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

use crate::config::Config;
use crate::core::Widget;
use crate::widgets::lua::LuaWidget;

/// Создать все виджеты из `config.widgets.entries`.
pub fn create_all(config: &Config) -> Vec<Box<dyn Widget>> {
    let mut out: Vec<Box<dyn Widget>> = Vec::new();

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
                        out.push(Box::new(w));
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

    out
}