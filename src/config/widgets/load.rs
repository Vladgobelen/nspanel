//! Загрузка виджетов: чтение папки `widgets/` и парсинг `config.lua`
//! каждого виджета в `WidgetEntry`.
//!
//! Одна ответственность: файловая система → `Vec<WidgetEntry>`.
//! Сохранением занимается `save.rs`.
//!
//! Если папки нет — создаётся. Виджет без `config.lua` пропускается
//! (с предупреждением в stderr). Ошибка парсинга одного виджета не
//! роняет загрузку остальных.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use mlua::{Lua, Table};

use super::{ParamValue, Side, WidgetEntry, WidgetPrefs, WidgetsConfig};

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

            match load_one(&id, &config_path, widget_path) {
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
}

/// Прочитать один `config.lua` и собрать `WidgetEntry`.
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
    let enabled: bool = t.get("enabled").unwrap_or(true);
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