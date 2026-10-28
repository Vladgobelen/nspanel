//! `Runtime` — всё, что живёт через весь `poll`-цикл.

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
        let created = factory::create_all(&config);
        for w in created.widgets {
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

        // ---- 7. DND: callbacks виджетов + область панели ----
        let pw = x11.panel_window;
        x11.dnd_mut().register(created.dnd_callbacks);
        x11.dnd_mut().resize(panel.area);
        x11.dnd_mut().set_panel_window(pw);

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