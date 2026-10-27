use crate::core::{Action, Rect};
use crate::render::SharedBuffer;

/// Идентификатор модуля. Хранится в реестре как String.
pub type ModuleId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PopupId(pub u32);

/// Идентификатор pixmap (область памяти на X-сервере).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PixmapId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelPosition {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub struct OpenContext {
    pub x: i32,
    pub y: i32,
    pub screen_w: i32,
    pub screen_h: i32,
    pub panel_locked: bool,
    pub widgets_locked: bool,
    pub widget_id: Option<String>,
    pub widget_menu_items: Vec<(String, Action)>,
}

#[derive(Debug, Clone)]
pub enum ModuleAction {
    None,
    Close,
    RunCommand(String),
    TogglePanelLock,
    ToggleWidgetsLock,
    /// Пользовательское действие, которое модуль возвращает из меню.
    /// Панель пробрасывает его в виджет через `Widget::on_custom_action`.
    /// `widget_id` — id виджета, для которого было открыто меню.
    Custom { widget_id: String, msg: String },
}

#[derive(Debug)]
pub enum ModuleError {
    Host(String),
    Internal(String),
}

impl std::fmt::Display for ModuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModuleError::Host(s) => write!(f, "host error: {s}"),
            ModuleError::Internal(s) => write!(f, "internal error: {s}"),
        }
    }
}

impl std::error::Error for ModuleError {}

pub trait ModuleHost {
    fn popup_create(&mut self, w: u16, h: u16) -> Result<PopupId, ModuleError>;
    fn popup_show(&mut self, id: PopupId, x: i32, y: i32, w: u16, h: u16) -> Result<(), ModuleError>;
    fn popup_hide(&mut self, id: PopupId) -> Result<(), ModuleError>;
    /// Отправить весь буфер в окно.
    fn popup_put_image(&mut self, id: PopupId, buf: &SharedBuffer) -> Result<(), ModuleError>;

    /// Создать pixmap из буфера (область памяти на X-сервере).
    fn pixmap_create(&mut self, buf: &SharedBuffer) -> Result<PixmapId, ModuleError>;
    /// Удалить pixmap.
    fn pixmap_destroy(&mut self, id: PixmapId) -> Result<(), ModuleError>;

    /// Залить прямоугольник в окне указанным цветом (0xAARRGGBB).
    fn fill_rect(&mut self, popup: PopupId, rect: Rect, color: u32) -> Result<(), ModuleError>;

    /// Скопировать область из pixmap в окно.
    fn copy_area(
        &mut self,
        popup: PopupId,
        pixmap: PixmapId,
        src_x: i32,
        src_y: i32,
        w: u16,
        h: u16,
        dst_x: i32,
        dst_y: i32,
    ) -> Result<(), ModuleError>;

    fn grab_pointer(&mut self) -> Result<(), ModuleError>;
    fn ungrab_pointer(&mut self) -> Result<(), ModuleError>;
}

pub trait Module {
    fn id(&self) -> &str;
    fn is_open(&self) -> bool;
    fn popup_id(&self) -> Option<PopupId>;
    fn popup_size(&self) -> (u16, u16);
    fn open(&mut self, host: &mut dyn ModuleHost, ctx: OpenContext) -> Result<(), ModuleError>;
    fn close(&mut self, host: &mut dyn ModuleHost) -> Result<(), ModuleError>;
    fn handle_click(
        &mut self,
        host: &mut dyn ModuleHost,
        x: i32,
        y: i32,
    ) -> Result<ModuleAction, ModuleError>;
    fn handle_motion(
        &mut self,
        _host: &mut dyn ModuleHost,
        _x: i32,
        _y: i32,
    ) -> Result<bool, ModuleError> {
        Ok(false)
    }
    /// Готовый буфер (для первоначальной отправки или для отладки).
    /// Если модуль использует pixmap+copy_area, этот метод может возвращать None.
    fn buffer(&self) -> Option<&SharedBuffer>;
}