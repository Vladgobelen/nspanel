pub mod widgets;

use std::path::{Path, PathBuf};

use mlua::{Lua, Table};

use crate::config::widgets::WidgetsConfig;
use crate::core::module::PanelPosition;

const DEFAULT_CONFIG: &str = include_str!("config/default.lua");

#[derive(Debug, Clone)]
pub struct Config {
    pub panel: PanelConfig,
    pub theme: ThemeConfig,
    #[allow(dead_code)]
    pub modules: Vec<String>,
    pub bindings: BindingsConfig,
    pub menu_items: Vec<MenuItemConfig>,
    pub widgets: WidgetsConfig,
}

#[derive(Debug, Clone)]
pub struct PanelConfig {
    pub height: u16,
    pub position: PanelPosition,
    pub show_borders: bool,
}

#[derive(Debug, Clone)]
pub struct ThemeConfig {
    pub bg: u32,
    pub fg: u32,
    pub font_path: String,
    pub font_size: f32,
}

#[derive(Debug, Clone)]
pub struct BindingsConfig {
    pub right_click_panel: String,
}

#[derive(Debug, Clone)]
pub struct MenuItemConfig {
    pub id: String,
    pub label: String,
    pub action: MenuItemAction,
}

#[derive(Debug, Clone)]
pub enum MenuItemAction {
    Run(String),
    Log(String),
    Quit,
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Lua(mlua::Error),
    Invalid(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "io error: {e}"),
            ConfigError::Lua(e) => write!(f, "lua error: {e}"),
            ConfigError::Invalid(s) => write!(f, "invalid config: {s}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        ConfigError::Io(e)
    }
}

impl From<mlua::Error> for ConfigError {
    fn from(e: mlua::Error) -> Self {
        ConfigError::Lua(e)
    }
}

impl Config {
    pub fn load_default() -> Result<Self, ConfigError> {
        if let Ok(p) = std::env::var("NSPANEL_CONFIG") {
            let path = PathBuf::from(p);
            if path.exists() {
                return Self::load(&path);
            }
            return Err(ConfigError::Invalid(format!(
                "NSPANEL_CONFIG points to non-existent file: {}",
                path.display()
            )));
        }

        let user_path = Self::user_config_path()?;
        if !user_path.exists() {
            Self::create_user_config(&user_path)?;
        }
        Self::load(&user_path)
    }

    pub fn user_config_path() -> Result<PathBuf, ConfigError> {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            if !xdg.is_empty() {
                return Ok(Path::new(&xdg).join("nspanel/config.lua"));
            }
        }
        if let Some(home) = std::env::var_os("HOME") {
            return Ok(Path::new(&home).join(".config/nspanel/config.lua"));
        }
        Err(ConfigError::Invalid(
            "cannot determine config path: neither XDG_CONFIG_HOME nor HOME is set".into(),
        ))
    }

    fn create_user_config(path: &Path) -> Result<(), ConfigError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, DEFAULT_CONFIG)?;
        eprintln!("[config] created default config at {}", path.display());
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let lua = Lua::new();
        let chunk = lua.load(path).into_function()?;
        let table: Table = chunk.call(())?;
        let mut cfg = Self::from_lua_table(&table)?;

        // Виджеты лежат в `widgets/` рядом с `config.lua`.
        let dir = path
            .parent()
            .map(|p| p.join("widgets"))
            .unwrap_or_else(|| PathBuf::from("widgets"));

        cfg.widgets = WidgetsConfig::load(dir)?;

        Ok(cfg)
    }

    /// Сохранить позицию панели в `config.lua` (merge, не затирая остальное).
    pub fn save_panel_position(&self, position: &str) -> Result<(), ConfigError> {
        let path = Self::user_config_path()?;
        let src = std::fs::read_to_string(&path)?;
        let out = replace_panel_position(&src, position);
        std::fs::write(&path, out)?;
        Ok(())
    }

    fn from_lua_table(root: &Table) -> Result<Self, ConfigError> {
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
}

/// Заменить `position = "..."` в `config.lua` (первое вхождение).
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