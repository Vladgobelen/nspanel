//! `Runtime` — всё, что живёт через весь `poll`-цикл.
//!
//! Одна ответственность: собрать объекты панели в один контейнер и
//! один раз выполнить установку (конфиг, шрифт, тема, X11, панель,
//! виджеты, окно, модули, буфер, стартовая отрисовка).
//!
//! Здесь НЕТ:
//! - `poll`-цикла (это `run/loop.rs`);
//! - подключения к nsbridge (это `run/connect.rs` + `app/bridge.rs`);
//! - обработки событий (это `app/events/*`);
//! - геометрии (это `app/geometry.rs`);
//! - методов `render`/`handle_event` — `Runtime` не «божественный
//!   объект», а контейнер данных. Логика — в подмодулях.

use std::path::PathBuf;

use crate::app::bridge::socket_path;
use crate::app::geometry::{panel_buf_size, panel_rect_for};
use crate::config::Config;
use crate::core::module::PanelPosition;
use crate::core::modules::MenuModule;
use crate::core::{ModuleRegistry, Panel, Theme};
use crate::render::{Font, PixelBuffer, SharedBuffer};
use crate::widgets::factory;
use crate::x11::X11Backend;

/// Всё, что живёт через весь `poll`-цикл.
///
/// Поля публичные: `run/loop.rs` читает и мутирует их напрямую.
/// Никаких геттеров/сеттеров — `Runtime` не инкапсулирует поведение,
/// он контейнер.
pub struct Runtime {
    pub x11: X11Backend,
    pub panel: Panel,
    pub registry: ModuleRegistry,
    pub config: Config,
    pub theme: Theme,
    pub panel_buf: SharedBuffer,
    pub position: PanelPosition,
    pub screen_w: i32,
    pub screen_h: i32,
    pub bridge_path: PathBuf,
}

impl Runtime {
    /// Выполнить установку панели. Шаги:
    /// 1. `Config::load_default`, `Font::load`, `Theme::from_config`.
    /// 2. `X11Backend::new`, `Panel::new` с геометрией из `app::geometry`.
    /// 3. Создать виджеты (`factory::create_all`) и применить prefs.
    /// 4. `resize_panel` / `move_panel` / `update_strut` под позицию.
    /// 5. `ModuleRegistry` + `MenuModule`.
    /// 6. `PixelBuffer` нужного размера + стартовая отрисовка.
    ///
    /// Подключение к nsbridge — **не здесь** (это `run/connect.rs`,
    /// потому что оно возвращает `reconnect_at`, который нужен циклу).
    pub fn setup() -> Result<Self, Box<dyn std::error::Error>> {
        // ---- 1. Конфиг, шрифт, тема ----
        let config = Config::load_default()?;
        let font = Font::load(&config.theme.font_path)?;
        let theme = Theme::from_config(&config.theme, font.clone());

        // ---- 2. X11 + панель ----
        let mut x11 = X11Backend::new(config.panel.height)?;
        let (sw, sh) = x11.screen_size();
        let sw_i = sw as i32;
        let sh_i = sh as i32;

        let thickness = config.panel.height as i32;
        let position = config.panel.position;

        let mut panel = Panel::new(panel_rect_for(position, thickness, sw_i, sh_i), position);
        panel.locked = true;
        panel.widgets_locked = true;
        panel.show_borders = config.panel.show_borders;

        // ---- 3. Виджеты ----
        for w in factory::create_all(&config) {
            panel.add_widget(w);
        }
        panel.apply_prefs(&config.widgets.prefs);

        // ---- 4. Окно панели под позицию ----
        let (bw, bh) = panel_buf_size(position, config.panel.height, sw, sh);
        x11.resize_panel(bw as u16, bh as u16)?;
        x11.move_panel(panel.area.x, panel.area.y)?;
        match position {
            PanelPosition::Top | PanelPosition::Bottom => {
                x11.update_strut(position, config.panel.height as u32, sw as u32)?;
            }
            PanelPosition::Left | PanelPosition::Right => {
                x11.update_strut(position, config.panel.height as u32, sh as u32)?;
            }
        }

        // ---- 5. Модули ----
        let mut registry = ModuleRegistry::new();
        registry.register(Box::new(MenuModule::new(&config, theme.clone())));

        // ---- 6. Буфер + стартовая отрисовка ----
        let panel_buf = PixelBuffer::new_shared(bw, bh);
        let _ = panel.update_all();
        let _ = panel.any_needs_relayout();
        panel.render(&panel_buf, &theme);
        x11.put_image(&panel_buf)?;

        Ok(Self {
            x11,
            panel,
            registry,
            config,
            theme,
            panel_buf,
            position,
            screen_w: sw_i,
            screen_h: sh_i,
            bridge_path: socket_path(),
        })
    }
}