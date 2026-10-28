//! Конфигурация панели.
//!
//! Разбито по файлам:
//! - `parse.rs` — `from_lua_table`: `mlua::Table` → `Config` (все поля,
//!   включая `menu_items`). Чистая функция, без файловой системы.
//! - `save.rs` — `save_panel_position` + `replace_panel_position`:
//!   запись позиции панели обратно в `config.lua` merge'ом.
//! - `mod.rs` — структуры (`Config`, `PanelConfig`, ...), `ConfigError`,
//!   загрузка файла (`load`, `load_default`, `user_config_path`,
//!   `create_user_config`).
//!
//! `widgets/` (конфиг виджетов) — отдельный подмодуль `widgets.rs`,
//! он не менялся.

pub mod parse;
pub mod save;
pub mod widgets;

use std::path::{Path, PathBuf};

use mlua::{Lua, Table};

use crate::config::widgets::WidgetsConfig;
use crate::core::module::PanelPosition;

const DEFAULT_CONFIG: &str = include_str!("../config/default.lua");

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
        let mut cfg = parse::from_lua_table(&table)?;

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
        save::save_panel_position(position)
    }
}