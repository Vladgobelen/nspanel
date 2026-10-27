use crate::config::{Config, MenuItemAction, MenuItemConfig};
use crate::core::menu::{Menu, MenuItem};
use crate::core::module::{
    Module, ModuleAction, ModuleError, ModuleHost, OpenContext, PixmapId, PopupId,
};
use crate::core::{Action, Theme};
use crate::render::PixelBuffer;

const MENU_WIDTH: i32 = 200;
const MENU_ITEM_HEIGHT: i32 = 28;

pub struct MenuModule {
    menu: Option<Menu>,
    popup: Option<PopupId>,
    pixmap_normal: Option<PixmapId>,
    pixmap_hover: Option<PixmapId>,
    buf_normal: Option<crate::render::SharedBuffer>,
    items: Vec<MenuItemConfig>,
    theme: Theme,
    /// ID виджета, для которого открыто меню (если ПКМ по виджету).
    /// Используется, чтобы вернуть `ModuleAction::Custom { widget_id, msg }`.
    widget_id: Option<String>,
}

impl MenuModule {
    pub fn new(config: &Config, theme: Theme) -> Self {
        Self {
            menu: None,
            popup: None,
            pixmap_normal: None,
            pixmap_hover: None,
            buf_normal: None,
            items: config.menu_items.clone(),
            theme,
            widget_id: None,
        }
    }

    fn build_menu_items(&self, ctx: &OpenContext) -> Vec<MenuItem> {
        let mut items: Vec<MenuItem> = Vec::new();

        if !ctx.widget_menu_items.is_empty() {
            for (i, (label, action)) in ctx.widget_menu_items.iter().enumerate() {
                items.push(MenuItem::new(
                    format!("widget_{i}"),
                    label.clone(),
                    action.clone(),
                ));
            }
            items.push(MenuItem::separator());
        }

        for cfg in &self.items {
            let action = match &cfg.action {
                MenuItemAction::Run(cmd) => Action::RunCommand(cmd.clone()),
                MenuItemAction::Log(msg) => Action::Log(msg.clone()),
                MenuItemAction::Quit => Action::Log("quit".into()),
            };
            items.push(MenuItem::new(cfg.id.clone(), cfg.label.clone(), action));
        }

        let panel_lock_label = if ctx.panel_locked {
            "Разблокировать панель"
        } else {
            "Заблокировать панель"
        };
        items.push(MenuItem::new(
            "panel_lock",
            panel_lock_label,
            Action::Log("__toggle_panel_lock__".into()),
        ));

        let widgets_lock_label = if ctx.widgets_locked {
            "Разблокировать виджеты"
        } else {
            "Заблокировать виджеты"
        };
        items.push(MenuItem::new(
            "widgets_lock",
            widgets_lock_label,
            Action::Log("__toggle_widgets_lock__".into()),
        ));

        items
    }

    fn destroy_pixmaps(&mut self, host: &mut dyn ModuleHost) {
        if let Some(pm) = self.pixmap_normal.take() {
            let _ = host.pixmap_destroy(pm);
        }
        if let Some(pm) = self.pixmap_hover.take() {
            let _ = host.pixmap_destroy(pm);
        }
    }
}

impl Module for MenuModule {
    fn id(&self) -> &str {
        "menu"
    }

    fn is_open(&self) -> bool {
        self.menu.is_some()
    }

    fn popup_id(&self) -> Option<PopupId> {
        self.popup
    }

    fn popup_size(&self) -> (u16, u16) {
        match &self.menu {
            Some(m) => (m.area.w as u16, m.area.h as u16),
            None => (1, 1),
        }
    }

    fn open(&mut self, host: &mut dyn ModuleHost, ctx: OpenContext) -> Result<(), ModuleError> {
        self.widget_id = ctx.widget_id.clone();
        let items = self.build_menu_items(&ctx);
        let menu_h = MENU_ITEM_HEIGHT * items.len() as i32;

        let mut mx = ctx.x;
        let mut my = ctx.y;
        if mx + MENU_WIDTH > ctx.screen_w {
            mx = ctx.screen_w - MENU_WIDTH;
        }
        if my + menu_h > ctx.screen_h {
            my = ctx.screen_h - menu_h;
        }
        if mx < 0 {
            mx = 0;
        }
        if my < 0 {
            my = 0;
        }

        let menu = Menu::new(mx, my, items);

        let popup = match self.popup {
            Some(p) => p,
            None => {
                let p = host
                    .popup_create(menu.area.w as u16, menu.area.h as u16)
                    .map_err(|e| ModuleError::Host(e.to_string()))?;
                self.popup = Some(p);
                p
            }
        };

        host.popup_show(
            popup,
            menu.area.x,
            menu.area.y,
            menu.area.w as u16,
            menu.area.h as u16,
        )
        .map_err(|e| ModuleError::Host(e.to_string()))?;

        host.grab_pointer()
            .map_err(|e| ModuleError::Host(e.to_string()))?;

        let w = menu.area.w as u32;
        let h = menu.area.h as u32;

        self.destroy_pixmaps(host);

        let buf_normal = PixelBuffer::new_shared(w, h);
        {
            let mut b = buf_normal.lock().unwrap();
            menu.render_full(&mut b, &self.theme, None);
        }
        let pixmap_normal = host
            .pixmap_create(&buf_normal)
            .map_err(|e| ModuleError::Host(e.to_string()))?;

        let buf_hover = PixelBuffer::new_shared(w, h);
        {
            let mut b = buf_hover.lock().unwrap();
            menu.render_full_all_hover(&mut b, &self.theme);
        }
        let pixmap_hover = host
            .pixmap_create(&buf_hover)
            .map_err(|e| ModuleError::Host(e.to_string()))?;

        host.popup_put_image(popup, &buf_normal)
            .map_err(|e| ModuleError::Host(e.to_string()))?;

        self.pixmap_normal = Some(pixmap_normal);
        self.pixmap_hover = Some(pixmap_hover);
        self.buf_normal = Some(buf_normal);
        self.menu = Some(menu);
        Ok(())
    }

    fn close(&mut self, host: &mut dyn ModuleHost) -> Result<(), ModuleError> {
        if let Some(p) = self.popup {
            host.popup_hide(p)
                .map_err(|e| ModuleError::Host(e.to_string()))?;
        }
        self.destroy_pixmaps(host);
        host.ungrab_pointer()
            .map_err(|e| ModuleError::Host(e.to_string()))?;
        self.menu = None;
        self.buf_normal = None;
        self.widget_id = None;
        Ok(())
    }

    fn handle_click(
        &mut self,
        _host: &mut dyn ModuleHost,
        x: i32,
        y: i32,
    ) -> Result<ModuleAction, ModuleError> {
        let menu = match &self.menu {
            Some(m) => m,
            None => return Ok(ModuleAction::None),
        };
        if let Some(idx) = menu.hit_test(x, y) {
            let action = menu.items[idx].action.clone();
            return Ok(match action {
                Action::None => ModuleAction::Close,
                Action::Log(msg) if msg == "__toggle_panel_lock__" => {
                    ModuleAction::TogglePanelLock
                }
                Action::Log(msg) if msg == "__toggle_widgets_lock__" => {
                    ModuleAction::ToggleWidgetsLock
                }
                Action::Log(msg) => {
                    eprintln!("[menu] {msg}");
                    ModuleAction::Close
                }
                Action::RunCommand(cmd) => ModuleAction::RunCommand(cmd),
                Action::Custom(msg) => {
                    if let Some(wid) = &self.widget_id {
                        ModuleAction::Custom {
                            widget_id: wid.clone(),
                            msg,
                        }
                    } else {
                        ModuleAction::Close
                    }
                }
            });
        }
        Ok(ModuleAction::Close)
    }

    fn handle_motion(
        &mut self,
        host: &mut dyn ModuleHost,
        x: i32,
        y: i32,
    ) -> Result<bool, ModuleError> {
        let menu = match &mut self.menu {
            Some(m) => m,
            None => return Ok(false),
        };
        let new_selected = menu.hit_test(x, y);
        if menu.selected == new_selected {
            return Ok(false);
        }

        let popup = match self.popup {
            Some(p) => p,
            None => return Ok(false),
        };
        let pm_normal = match self.pixmap_normal {
            Some(p) => p,
            None => return Ok(false),
        };
        let pm_hover = match self.pixmap_hover {
            Some(p) => p,
            None => return Ok(false),
        };

        let old = menu.selected;
        menu.selected = new_selected;

        if let Some(o) = old {
            let r = menu.local_item_rect(o);
            host.copy_area(
                popup,
                pm_normal,
                r.x,
                r.y,
                r.w as u16,
                r.h as u16,
                r.x,
                r.y,
            )?;
        }
        if let Some(n) = new_selected {
            let r = menu.local_item_rect(n);
            host.copy_area(
                popup,
                pm_hover,
                r.x,
                r.y,
                r.w as u16,
                r.h as u16,
                r.x,
                r.y,
            )?;
        }

        Ok(true)
    }

    fn buffer(&self) -> Option<&crate::render::SharedBuffer> {
        self.buf_normal.as_ref()
    }
}