use std::collections::HashMap;

use crate::core::module::{
    Module, ModuleAction, ModuleError, ModuleHost, ModuleId, OpenContext, PopupId,
};

pub struct ModuleRegistry {
    modules: HashMap<ModuleId, Box<dyn Module>>,
    order: Vec<ModuleId>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            order: Vec::new(),
        }
    }

    pub fn register(&mut self, module: Box<dyn Module>) {
        let id = module.id().to_string();
        if !self.modules.contains_key(&id) {
            self.order.push(id.clone());
        }
        self.modules.insert(id, module);
    }

    #[allow(dead_code)]
    pub fn get(&self, id: &str) -> Option<&dyn Module> {
        self.modules.get(id).map(|m| m.as_ref())
    }

    pub fn open(
        &mut self,
        id: &str,
        host: &mut dyn ModuleHost,
        ctx: OpenContext,
    ) -> Result<(), ModuleError> {
        let m = self
            .modules
            .get_mut(id)
            .ok_or_else(|| ModuleError::Internal(format!("module '{id}' not registered")))?;
        m.open(host, ctx)
    }

    pub fn close(&mut self, id: &str, host: &mut dyn ModuleHost) -> Result<(), ModuleError> {
        let m = self
            .modules
            .get_mut(id)
            .ok_or_else(|| ModuleError::Internal(format!("module '{id}' not registered")))?;
        m.close(host)
    }

    #[allow(dead_code)]
    pub fn close_all(&mut self, host: &mut dyn ModuleHost) {
        for id in self.order.clone() {
            if let Some(m) = self.modules.get_mut(&id) {
                if m.is_open() {
                    let _ = m.close(host);
                }
            }
        }
    }

    pub fn handle_popup_click(
        &mut self,
        host: &mut dyn ModuleHost,
        popup: PopupId,
        x: i32,
        y: i32,
    ) -> Result<Option<ModuleAction>, ModuleError> {
        for id in self.order.clone() {
            let action = {
                let m = match self.modules.get_mut(&id) {
                    Some(m) => m,
                    None => continue,
                };
                if !m.is_open() || m.popup_id() != Some(popup) {
                    continue;
                }
                m.handle_click(host, x, y)?
            };
            match action {
                ModuleAction::None => {}
                other => return Ok(Some(other)),
            }
        }
        Ok(None)
    }

    pub fn handle_click_any(
        &mut self,
        host: &mut dyn ModuleHost,
        x: i32,
        y: i32,
    ) -> Result<Option<ModuleAction>, ModuleError> {
        for id in self.order.clone() {
            let action = {
                let m = match self.modules.get_mut(&id) {
                    Some(m) => m,
                    None => continue,
                };
                if !m.is_open() {
                    continue;
                }
                m.handle_click(host, x, y)?
            };
            match action {
                ModuleAction::None => {}
                other => return Ok(Some(other)),
            }
        }
        Ok(None)
    }

    pub fn handle_motion_any(
        &mut self,
        host: &mut dyn ModuleHost,
        x: i32,
        y: i32,
    ) -> Result<bool, ModuleError> {
        for id in self.order.clone() {
            let changed = {
                let m = match self.modules.get_mut(&id) {
                    Some(m) => m,
                    None => continue,
                };
                if !m.is_open() {
                    continue;
                }
                m.handle_motion(host, x, y)?
            };
            if changed {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Отправить полный буфер открытых модулей в их popup-окна.
    /// Используется только для редких событий (Expose). В hot-path
    /// (движение мыши) не вызывается — модуль сам делает copy_area.
    pub fn render_open(&mut self, host: &mut dyn ModuleHost) -> Result<(), ModuleError> {
        let open_ids: Vec<ModuleId> = self
            .order
            .iter()
            .filter(|id| self.modules.get(*id).map(|m| m.is_open()).unwrap_or(false))
            .cloned()
            .collect();

        for id in open_ids {
            let m = match self.modules.get(&id) {
                Some(m) => m,
                None => continue,
            };
            let popup = match m.popup_id() {
                Some(p) => p,
                None => continue,
            };
            let buf = match m.buffer() {
                Some(b) => b,
                None => continue,
            };
            host.popup_put_image(popup, buf)?;
        }
        Ok(())
    }
}

impl Default for ModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}