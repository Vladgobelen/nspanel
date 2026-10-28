//! Парсинг Lua-таблицы конфига в `Config` (без `widgets`, их грузит
//! `Config::load` отдельно из папки `widgets/`).
//!
//! Одна ответственность: `mlua::Table` → `Config`. Чистая функция,
//! без файловой системы. Ошибки — `ConfigError::Invalid` с указанием,
//! какого поля не хватает или какое значение недопустимо.

use mlua::Table;

use crate::config::{
    BindingsConfig, Config, ConfigError, MenuItemAction, MenuItemConfig, PanelConfig,
    ThemeConfig,
};
use crate::config::widgets::WidgetsConfig;
use crate::core::module::PanelPosition;

pub fn from_lua_table(root: &Table) -> Result<Config, ConfigError> {
    let panel_t: Table = root
        .get("panel")
        .map_err(|_| ConfigError::Invalid("missing 'panel' table".into()))?;
    let height: u16 = panel_t
        .get("height")
        .map_err(|_| ConfigError::Invalid("missing 'panel.height'".into()))?;
    let position_s: String = panel_t
        .get("position")
        .map_err(|_| ConfigError::Invalid("missing 'panel.position'".into()))?;
    let position = match position_s.as_str() {
        "top" => PanelPosition::Top,
        "bottom" => PanelPosition::Bottom,
        "left" => PanelPosition::Left,
        "right" => PanelPosition::Right,
        other => {
            return Err(ConfigError::Invalid(format!(
                "panel.position must be 'top'|'bottom'|'left'|'right', got '{other}'"
            )))
        }
    };
    let show_borders: bool = panel_t.get("show_borders").unwrap_or(true);

    let theme_t: Table = root
        .get("theme")
        .map_err(|_| ConfigError::Invalid("missing 'theme' table".into()))?;
    let bg: u32 = theme_t
        .get("bg")
        .map_err(|_| ConfigError::Invalid("missing 'theme.bg'".into()))?;
    let fg: u32 = theme_t
        .get("fg")
        .map_err(|_| ConfigError::Invalid("missing 'theme.fg'".into()))?;
    let font_path: String = theme_t
        .get("font")
        .map_err(|_| ConfigError::Invalid("missing 'theme.font'".into()))?;
    let font_size: f32 = theme_t
        .get("font_size")
        .map_err(|_| ConfigError::Invalid("missing 'theme.font_size'".into()))?;

    let mut modules = Vec::new();
    if let Ok(mods_t) = root.get::<Table>("modules") {
        for pair in mods_t.sequence_values::<String>() {
            modules.push(pair?);
        }
    }

    let bindings_t: Table = root
        .get("bindings")
        .map_err(|_| ConfigError::Invalid("missing 'bindings' table".into()))?;
    let right_click_panel: String = bindings_t
        .get("right_click_panel")
        .map_err(|_| ConfigError::Invalid("missing 'bindings.right_click_panel'".into()))?;

    let menu_items = parse_menu_items(root)?;

    Ok(Config {
        panel: PanelConfig { height, position, show_borders },
        theme: ThemeConfig {
            bg,
            fg,
            font_path,
            font_size,
        },
        modules,
        bindings: BindingsConfig { right_click_panel },
        menu_items,
        widgets: WidgetsConfig::default(),
    })
}

/// Разобрать `menu_items`: массив таблиц `{id, label, action}`.
/// `action` — таблица `{type="run"|"log"|"quit", ...}`.
fn parse_menu_items(root: &Table) -> Result<Vec<MenuItemConfig>, ConfigError> {
    let mut menu_items = Vec::new();
    let items_t: Table = root
        .get("menu_items")
        .map_err(|_| ConfigError::Invalid("missing 'menu_items' table".into()))?;
    for pair in items_t.sequence_values::<Table>() {
        let item = pair?;
        let id: String = item
            .get("id")
            .map_err(|_| ConfigError::Invalid("menu_items[].id missing".into()))?;
        let label: String = item
            .get("label")
            .map_err(|_| ConfigError::Invalid("menu_items[].label missing".into()))?;
        let action_t: Table = item
            .get("action")
            .map_err(|_| ConfigError::Invalid("menu_items[].action missing".into()))?;
        let kind: String = action_t
            .get("type")
            .map_err(|_| ConfigError::Invalid("menu_items[].action.type missing".into()))?;
        let action = match kind.as_str() {
            "run" => {
                let cmd: String = action_t.get("cmd").map_err(|_| {
                    ConfigError::Invalid("menu_items[].action.cmd missing for type 'run'".into())
                })?;
                MenuItemAction::Run(cmd)
            }
            "log" => {
                let msg: String = action_t.get("msg").map_err(|_| {
                    ConfigError::Invalid("menu_items[].action.msg missing for type 'log'".into())
                })?;
                MenuItemAction::Log(msg)
            }
            "quit" => MenuItemAction::Quit,
            other => {
                return Err(ConfigError::Invalid(format!(
                    "menu_items[].action.type must be 'run'|'log'|'quit', got '{other}'"
                )))
            }
        };
        menu_items.push(MenuItemConfig { id, label, action });
    }
    Ok(menu_items)
}