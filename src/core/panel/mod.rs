//! Модель панели: виджеты, позиции, состояние drag, флаги lock.
//!
//! `Panel` — модель + фасад: поля, `new`, `add_widget` и однострочные
//! делегаты в подмодули. Вся логика — в подмодулях:
//! - `layout.rs`  — чистая раскладка;
//! - `input.rs`   — попадание, hover, клик, тултип, меню;
//! - `drag.rs`    — всё про `WidgetDragState` (включая `drag_main`,
//!   `current_strip`, `place_widget_at`);
//! - `render.rs`  — отрисовка панели;
//! - `timers.rs`  — update/next_wakeup/socket/needs_relayout.

pub mod drag;
pub mod input;
pub mod layout;
pub mod render;
pub mod timers;

use std::collections::HashMap;
use std::time::Instant;

use crate::config::widgets::WidgetPrefs;
use crate::core::ipc::UnixStreamClient;
use crate::core::module::PanelPosition;
use crate::core::{Action, Rect, Theme, Widget, WidgetId};
use crate::render::SharedBuffer;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Layout {
    Zones {
        left: Vec<WidgetId>,
        center: Vec<WidgetId>,
        right: Vec<WidgetId>,
    },
    Linear,
}

#[derive(Debug, Clone, Copy)]
pub struct DragState {
    pub offset_x: i32,
    pub offset_y: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct WidgetDragState {
    pub original_index: usize,
    pub offset_x: i32,
    pub offset_y: i32,
    pub mouse_x: i32,
    pub mouse_y: i32,
}

pub struct Panel {
    pub widgets: Vec<Box<dyn Widget>>,
    pub positions: Vec<i32>,
    pub next_update: Vec<Option<Instant>>,
    #[allow(dead_code)]
    pub layout: Layout,
    pub area: Rect,
    pub locked: bool,
    pub widgets_locked: bool,
    pub position: PanelPosition,
    pub dragging: Option<DragState>,
    pub widget_drag: Option<WidgetDragState>,
    pub show_borders: bool,
    prefs: HashMap<String, WidgetPrefs>,
}

impl Panel {
    pub fn new(area: Rect, position: PanelPosition) -> Self {
        Self {
            widgets: Vec::new(),
            positions: Vec::new(),
            next_update: Vec::new(),
            layout: Layout::Linear,
            area,
            locked: true,
            widgets_locked: true,
            position,
            dragging: None,
            widget_drag: None,
            show_borders: true,
            prefs: HashMap::new(),
        }
    }

    pub fn add_widget(&mut self, w: Box<dyn Widget>) {
        let thickness = self.panel_thickness();
        let vertical = self.is_vertical();
        let x = self
            .positions
            .last()
            .map(|p| p + layout::last_size(&self.widgets, thickness, vertical))
            .unwrap_or(0);
        self.positions.push(x);
        self.next_update.push(None);
        self.widgets.push(w);
    }

    pub fn widget_ids(&self) -> Vec<WidgetId> {
        self.widgets.iter().map(|w| w.id()).collect()
    }

    pub fn set_socket_for(&mut self, id: &str, socket: UnixStreamClient) {
        timers::set_socket_for(&mut self.widgets, id, socket);
    }

    pub fn notify_socket_ready(&mut self, id: &str) {
        timers::notify_socket_ready(&mut self.widgets, id);
    }

    pub fn any_needs_relayout(&mut self) -> bool {
        timers::any_needs_relayout(&mut self.widgets)
    }

    pub fn reflow(&mut self) {
        let prefs = self.prefs.clone();
        self.apply_prefs(&prefs);
    }

    pub fn update_due(&mut self, now: Instant) -> bool {
        timers::update_due(&mut self.widgets, &mut self.next_update, now)
    }

    pub fn update_all(&mut self) -> bool {
        timers::update_all(&mut self.widgets, &mut self.next_update);
        true
    }

    pub fn next_wakeup(&self) -> Option<Instant> {
        timers::next_wakeup(&self.next_update)
    }

    fn is_vertical(&self) -> bool {
        matches!(self.position, PanelPosition::Left | PanelPosition::Right)
    }

    fn panel_len(&self) -> i32 {
        layout::panel_len(self.area, self.is_vertical())
    }

    fn panel_thickness(&self) -> u32 {
        layout::panel_thickness(self.area, self.is_vertical())
    }

    fn widget_size(&self, idx: usize) -> i32 {
        layout::widget_size(
            self.widgets[idx].as_ref(),
            self.panel_thickness(),
            self.is_vertical(),
        )
    }

    pub fn layout_widgets(&self) -> Vec<(WidgetId, Rect, i32)> {
        let vertical = self.is_vertical();
        let drag_from = self.widget_drag.map(|d| d.original_index);
        let drag_main = self
            .widget_drag
            .map(|d| drag::drag_main(d, self.area, vertical))
            .unwrap_or(0);
        layout::layout_widgets(
            &self.widgets,
            &self.positions,
            self.panel_thickness(),
            vertical,
            drag_from,
            drag_main,
        )
    }

    pub fn apply_prefs(&mut self, prefs: &HashMap<String, WidgetPrefs>) {
        self.prefs = prefs.clone();
        self.positions = layout::apply_prefs(
            &self.widgets,
            &self.prefs,
            self.panel_len(),
            self.panel_thickness(),
            self.is_vertical(),
        );
    }

    pub fn prefs_for_current(&self) -> HashMap<String, WidgetPrefs> {
        layout::prefs_for_current(
            &self.widgets,
            &self.positions,
            self.panel_len(),
            self.panel_thickness(),
            self.is_vertical(),
        )
    }

    pub fn on_hover_at(&mut self, x: i32, y: i32) -> bool {
        let layout_items = self.layout_widgets();
        input::on_hover_at(x, y, self.area, &layout_items, &mut self.widgets)
    }

    pub fn clear_hover(&mut self) -> bool {
        input::clear_hover(&mut self.widgets)
    }

    pub fn tooltip_at(&mut self, x: i32, y: i32) -> Option<Vec<String>> {
        let layout_items = self.layout_widgets();
        input::tooltip_at(x, y, self.area, &layout_items, &mut self.widgets)
    }

    pub fn hit_test(&self, x: i32, y: i32) -> Option<WidgetId> {
        let layout_items = self.layout_widgets();
        input::hit_test(x, y, self.area, &layout_items)
    }

    pub fn widget_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout_items = self.layout_widgets();
        input::widget_index_at(x, y, self.area, &layout_items)
    }

    pub fn on_click_at(&mut self, x: i32, y: i32) -> Option<Action> {
        let layout_items = self.layout_widgets();
        input::on_click_at(x, y, self.area, &layout_items, &mut self.widgets)
    }

    pub fn widget_menu_items(&self, id: &str, x: i32, y: i32) -> Vec<(String, Action)> {
        let layout_items = self.layout_widgets();
        input::widget_menu_items(id, x, y, self.area, &layout_items, &self.widgets)
    }

    pub fn on_custom_action(&mut self, id: &str, msg: &str) -> bool {
        input::on_custom_action(id, msg, &mut self.widgets)
    }

    pub fn drop_position_for(&self, coord_main: i32, drag_index: usize) -> (i32, i32) {
        let layout_items = self.layout_widgets();
        let drag_from = self.widget_drag.map(|d| d.original_index);
        drag::drop_position_for(
            coord_main,
            self.widget_size(drag_index),
            self.panel_len(),
            self.is_vertical(),
            &layout_items,
            drag_from,
        )
    }

    pub fn place_widget_at(&mut self, index: usize, strip: i32) {
        let panel_len = self.panel_len();
        let thickness = self.panel_thickness();
        let vertical = self.is_vertical();
        self.prefs = drag::place_widget_at(
            &mut self.widgets,
            &mut self.positions,
            &mut self.next_update,
            index,
            strip,
            panel_len,
            thickness,
            vertical,
        );
    }

    pub fn render(&self, buf: &SharedBuffer, theme: &Theme) {
        let layout_items = self.layout_widgets();
        let drag_strip = self.widget_drag.map(|d| {
            drag::current_strip(d, self.area, self.panel_len(), self.is_vertical(), &layout_items)
        });
        render::render(
            buf,
            self.area,
            self.is_vertical(),
            self.panel_thickness(),
            self.show_borders,
            &self.widgets,
            &layout_items,
            self.widget_drag,
            drag_strip,
            theme,
        );
    }
}