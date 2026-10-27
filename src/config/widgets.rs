//! Конфиг виджетов: папка `widgets/<id>/` рядом с `config.lua`.
//!
//! В каждой папке:
//! - `config.lua` (обязательно) — таблица: `type`, `side`, `priority`,
//!   `enabled`, `tooltip_enabled`, плюс любые параметры виджета
//!   (`format`, `text`, `font_size`, ...).
//! - `widget.lua` (опционально) — код Lua-виджета: `render`, `on_click`,
//!   `on_hover`, `tooltip`, `update`, `on_socket_ready`, `menu_items`.
//!   Если есть — виджет грузится как `LuaWidget`. Если нет — пропуск
//!   (Rust-виджеты в панели не поддерживаются).
//!
//! `side`/`priority` можно сохранять обратно в `config.lua` (merge),
//! не затирая остальные поля (format, text, ...).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use mlua::{Lua, Table};

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
/// `type_name` и `params` пока не читаются в `factory.rs` (Rust-виджетов
/// нет, Lua-виджеты читают параметры сами из `config.lua`), но оставлены
/// для будущего: Rust-виджеты (`type = "clock"` и т.п.) и передача
/// параметров в Lua через `state`.
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
    /// Дефолт `true`. Настройка модуля — в его `config.lua`.
    pub tooltip_enabled: bool,
    /// Сырая таблица параметров из config.lua (всё, кроме
    /// type/side/priority/enabled/tooltip_enabled).
    pub params: HashMap<String, ParamValue>,
}

/// Значения параметров виджета, прочитанные из `config.lua`.
/// Ограниченный набор типов — то, что реально нужно виджетам.
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
    pub dir: PathBuf,
    /// Все виджеты, найденные в `dir`.
    pub entries: Vec<WidgetEntry>,
    /// Только `side/priority` по id (для `Panel::apply_prefs`).
    pub prefs: HashMap<String, WidgetPrefs>,
}

impl WidgetsConfig {
    /// Загрузить все виджеты из `dir`. Если папки нет — создать.
    pub fn load(dir: PathBuf) -> Result<Self, std::io::Error> {
        std::fs::create_dir_all(&dir)?;
        let mut entries = Vec::new();
        let mut prefs = HashMap::new();

        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let id = match path.file_name().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };

            let config_path = path.join("config.lua");
            if !config_path.exists() {
                eprintln!(
                    "[config] widget '{}' has no config.lua, skipping",
                    id
                );
                continue;
            }

            let widget_path = {
                let p = path.join("widget.lua");
                if p.exists() { Some(p) } else { None }
            };

            match Self::load_one(&id, &config_path, widget_path) {
                Ok(e) => {
                    prefs.insert(id.clone(), e.prefs);
                    entries.push(e);
                }
                Err(e) => {
                    eprintln!("[config] failed to load widget '{}': {}", id, e);
                }
            }
        }

        // Стабильный порядок для детерминизма (по id).
        entries.sort_by(|a, b| a.id.cmp(&b.id));

        Ok(Self { dir, entries, prefs })
    }

    fn load_one(
        id: &str,
        config_path: &Path,
        widget_path: Option<PathBuf>,
    ) -> Result<WidgetEntry, mlua::Error> {
        let lua = Lua::new();
        let chunk = lua.load(config_path).into_function()?;
        let t: Table = chunk.call(())?;

        let type_name: Option<String> = t.get("type").ok();
        let side_s: String = t.get("side").unwrap_or_else(|_| "left".into());
        let side = match side_s.as_str() {
            "right" => Side::Right,
            _ => Side::Left,
        };
        let priority: u32 = t.get("priority").unwrap_or(u32::MAX);
        let prefs = WidgetPrefs { side, priority };
        // enabled по умолчанию true — если в config.lua не указано, виджет включён.
        let enabled: bool = t.get("enabled").unwrap_or(true);
        // tooltip_enabled по умолчанию true — если не указано, тултип разрешён.
        let tooltip_enabled: bool = t.get("tooltip_enabled").unwrap_or(true);

        // Параметры: все ключи, кроме служебных.
        let mut params: HashMap<String, ParamValue> = HashMap::new();
        for pair in t.pairs::<String, mlua::Value>() {
            let (k, v) = pair?;
            if k == "type"
                || k == "side"
                || k == "priority"
                || k == "enabled"
                || k == "tooltip_enabled"
            {
                continue;
            }
            let pv = match v {
                mlua::Value::Integer(i) => Some(ParamValue::Int(i)),
                mlua::Value::Number(n) => Some(ParamValue::Float(n)),
                mlua::Value::String(s) => {
                    Some(ParamValue::Str(s.to_str().map(|x| x.to_string()).unwrap_or_default()))
                }
                mlua::Value::Boolean(b) => Some(ParamValue::Bool(b)),
                _ => None,
            };
            if let Some(pv) = pv {
                params.insert(k, pv);
            }
        }

        Ok(WidgetEntry {
            id: id.to_string(),
            config_path: config_path.to_path_buf(),
            widget_path,
            type_name,
            prefs,
            enabled,
            tooltip_enabled,
            params,
        })
    }

    pub fn get(&self, id: &str) -> Option<&WidgetEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Сохранить `side/priority` для всех entries в их `config.lua` (merge,
    /// не затирая остальные поля). Вызывается после drag.
    ///
    /// `prefs` — актуальные prefs (из `Panel::prefs_for_current`), а не
    /// `self.entries[].prefs` (те — снимок на момент загрузки).
    pub fn save_all(
        &self,
        prefs: &HashMap<String, WidgetPrefs>,
    ) -> Result<(), std::io::Error> {
        for e in &self.entries {
            let p = match prefs.get(&e.id) {
                Some(p) => *p,
                // Виджет не двигали — не трогаем файл.
                None => continue,
            };
            let side = match p.side {
                Side::Left => "left",
                Side::Right => "right",
            };
            let src = std::fs::read_to_string(&e.config_path)?;
            let mut out = replace_key(&src, "side", &format!("\"{}\"", side));
            out = replace_key(&out, "priority", &p.priority.to_string());
            std::fs::write(&e.config_path, out)?;
        }
        Ok(())
    }

    /// Сохранить `side/priority` для одного id.
    pub fn save_one(
        &self,
        id: &str,
        prefs: &HashMap<String, WidgetPrefs>,
    ) -> Result<(), std::io::Error> {
        let e = match self.get(id) {
            Some(e) => e,
            None => return Ok(()),
        };
        let p = match prefs.get(id) {
            Some(p) => *p,
            None => return Ok(()),
        };
        let side = match p.side {
            Side::Left => "left",
            Side::Right => "right",
        };
        let src = std::fs::read_to_string(&e.config_path)?;
        let mut out = replace_key(&src, "side", &format!("\"{}\"", side));
        out = replace_key(&out, "priority", &p.priority.to_string());
        std::fs::write(&e.config_path, out)?;
        Ok(())
    }
}

/// Заменить значение `key = ...` в тексте Lua-файла. Если ключа нет —
/// вставить перед последней `}`.
fn replace_key(src: &str, key: &str, value: &str) -> String {
    let needle = format!("{} =", key);
    if let Some(pos) = src.find(&needle) {
        // Найти конец строки после `=`.
        let rest = &src[pos + needle.len()..];
        let end = rest.find(|c: char| c == ',' || c == '\n' || c == '}').unwrap_or(rest.len());
        let value_start = pos + needle.len();
        let value_end = value_start + end;
        // Пропустить пробелы после `=`.
        let mut vs = value_start;
        while vs < value_end && src.as_bytes()[vs].is_ascii_whitespace() {
            vs += 1;
        }
        let mut result = String::new();
        result.push_str(&src[..value_start]);
        result.push(' ');
        result.push_str(value);
        result.push_str(&src[value_end..]);
        result
    } else {
        // Вставить перед последней `}`.
        if let Some(pos) = src.rfind('}') {
            let mut result = String::new();
            result.push_str(&src[..pos]);
            result.push_str(&format!("    {} = {},\n", key, value));
            result.push_str(&src[pos..]);
            result
        } else {
            src.to_string()
        }
    }
}