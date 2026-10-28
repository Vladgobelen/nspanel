//! Запись позиции панели обратно в `config.lua` (merge, без затирания
//! остальных полей).
//!
//! Одна ответственность: найти `position = "..."` в тексте Lua-файла
//! и заменить значение, не трогая всё остальное. Функция намеренно
//! работает со строкой, а не с `mlua`: пересохранение через `mlua`
//! потеряло бы комментарии и форматирование.

use crate::config::{Config, ConfigError};

/// Сохранить позицию панели в `config.lua` (merge).
///
/// Читает текущий `config.lua` (`Config::user_config_path`), заменяет
/// первое вхождение `position = "..."`, пишет обратно.
pub fn save_panel_position(position: &str) -> Result<(), ConfigError> {
    let path = Config::user_config_path()?;
    let src = std::fs::read_to_string(&path)?;
    let out = replace_panel_position(&src, position);
    std::fs::write(&path, out)?;
    Ok(())
}

/// Заменить `position = "..."` в `config.lua` (первое вхождение).
///
/// Ищет `position =`, затем первую пару `"..."` после него, и заменяет
/// содержимое кавычек на `new_position`. Если `position =` не найден —
/// возвращает строку как есть (нечего менять).
fn replace_panel_position(src: &str, new_position: &str) -> String {
    let needle = "position =";
    if let Some(pos) = src.find(needle) {
        let rest = &src[pos + needle.len()..];
        // Найти первую `"` и вторую `"`.
        if let Some(q1) = rest.find('"') {
            if let Some(q2_rel) = rest[q1 + 1..].find('"') {
                let q2 = q1 + 1 + q2_rel;
                let value_start = pos + needle.len() + q1;
                let value_end = pos + needle.len() + q2 + 1;
                let mut result = String::new();
                result.push_str(&src[..value_start]);
                result.push('"');
                result.push_str(new_position);
                result.push('"');
                result.push_str(&src[value_end..]);
                return result;
            }
        }
    }
    src.to_string()
}