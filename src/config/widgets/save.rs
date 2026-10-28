//! Сохранение `side`/`priority` виджета обратно в его `config.lua`
//! (merge, без затирания остальных полей).
//!
//! Одна ответственность: `HashMap<String, WidgetPrefs>` → правки
//! в `config.lua` каждого виджета. Загрузкой занимается `load.rs`.
//!
//! `replace_key` намеренно работает со строкой, а не с `mlua`:
//! пересохранение через Lua потеряло бы комментарии и форматирование.

use std::collections::HashMap;

use super::{Side, WidgetPrefs, WidgetsConfig};

impl WidgetsConfig {
    /// Сохранить `side/priority` для всех entries в их `config.lua` (merge).
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
    #[allow(dead_code)]
    pub fn save_one(
        &self,
        id: &str,
        prefs: &HashMap<String, WidgetPrefs>,
    ) -> Result<(), std::io::Error> {
        let e = match self.entries.iter().find(|e| e.id == id) {
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
        let rest = &src[pos + needle.len()..];
        let end = rest
            .find(|c: char| c == ',' || c == '\n' || c == '}')
            .unwrap_or(rest.len());
        let value_start = pos + needle.len();
        let value_end = value_start + end;
        let mut result = String::new();
        result.push_str(&src[..value_start]);
        result.push(' ');
        result.push_str(value);
        result.push_str(&src[value_end..]);
        result
    } else {
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