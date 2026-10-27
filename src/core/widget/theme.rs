use std::sync::Arc;

use crate::config::ThemeConfig;
use crate::core::Color;
use crate::render::Font;

/// Тема панели: цвета фона/текста, шрифт и его размер.
/// Клонируется в каждый виджет при рендере.
#[derive(Clone)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub font: Arc<Font>,
    pub font_size: f32,
}

impl Theme {
    pub fn from_config(cfg: &ThemeConfig, font: Arc<Font>) -> Self {
        Self {
            bg: Color(cfg.bg),
            fg: Color(cfg.fg),
            font,
            font_size: cfg.font_size,
        }
    }
}