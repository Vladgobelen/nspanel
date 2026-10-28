//! Конфиг виджетов: папка `widgets/<id>/` рядом с `config.lua`.
//!
//! В каждой папке:
//! - `config.lua` (обязательно) — таблица: `type`, `side`, `priority`,
//!   `enabled`, `tooltip_enabled`, плюс любые параметры виджета
//!   (`format`, `text`, `font_size`, ...).
//! - `widget.lua` (опционально) — код Lua-виджета.
//!
//! Разбито по файлам:
//! - `load.rs` — `WidgetsConfig::load`, `load_one`: чтение папки и парсинг
//!   `config.lua` в `WidgetEntry`.
//! - `save.rs` — `save_all`, `save_one`, `replace_key`: запись
//!   `side`/`priority` обратно в `config.lua` merge'ом (не затирая
//!   остальные поля).
//! - `mod.rs` — структуры (`WidgetsConfig`, `WidgetEntry`, `WidgetPrefs`,
//!   `Side`, `ParamValue`).
//!
//! `side`/`priority` сохраняются после drag виджета (см.
//! `app/events/button.rs::finish_widget_drag`).

pub mod load;
pub mod save;

use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy)]
pub struct WidgetPrefs {
    pub side: Side,
    pub priority: u32,
}

/// Один виджет из папки `widgets/<id>/`.
///
/// `type_name` и `params` пока не читаются в `factory.rs` (Lua-виджеты
/// читают параметры сами из `config.lua`), но оставлены для будущего.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct WidgetEntry {
    /// id = имя папки (например, "clock", "my_weather").
    pub id: String,
    /// Путь к `config.lua` (обязательно).
    pub config_path: PathBuf,
    /// Путь к `widget.lua` (опционально).
    pub widget_path: Option<PathBuf>,
    /// `type` из config.lua (например, "lua", "clock", "label").
    pub type_name: Option<String>,
    /// `side`/`priority` из config.lua (по умолчанию left/MAX).
    pub prefs: WidgetPrefs,
    /// Включён ли виджет. Если `false` — `factory` его пропускает.
    pub enabled: bool,
    /// Показывать ли тултип. Если `false` — панель не вызывает
    /// `Widget::tooltip` у этого виджета (даже если `M.tooltip` есть).
    pub tooltip_enabled: bool,
    /// Сырая таблица параметров из config.lua (всё, кроме служебных).
    pub params: HashMap<String, ParamValue>,
}

/// Значения параметров виджета, прочитанные из `config.lua`.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum ParamValue {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
}

#[derive(Debug, Clone, Default)]
pub struct WidgetsConfig {
    /// Папка `widgets/` рядом с `config.lua`.
    #[allow(dead_code)]
    pub dir: PathBuf,
    /// Все виджеты, найденные в `dir`.
    pub entries: Vec<WidgetEntry>,
    /// Только `side/priority` по id (для `Panel::apply_prefs`).
    pub prefs: HashMap<String, WidgetPrefs>,
}