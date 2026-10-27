use std::collections::HashMap;
use std::time::Instant;

use crate::config::widgets::{Side, WidgetPrefs};
use crate::core::ipc::SocketClient;
use crate::core::module::PanelPosition;
use crate::core::{Action, Color, Rect, Theme, Widget, WidgetId};
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
        let x = self.positions.last().map(|p| p + self.last_size()).unwrap_or(0);
        self.positions.push(x);
        self.next_update.push(None);
        self.widgets.push(w);
    }

    pub fn set_socket_for(&mut self, id: &str, socket: SocketClient) {
        if let Some(idx) = self.find(id) {
            self.widgets[idx].set_socket(socket);
        }
    }

    pub fn notify_socket_ready(&mut self, id: &str) {
        if let Some(idx) = self.find(id) {
            self.widgets[idx].on_socket_ready();
        }
    }

    pub fn widget_ids(&self) -> Vec<WidgetId> {
        self.widgets.iter().map(|w| w.id()).collect()
    }

    pub fn any_needs_relayout(&mut self) -> bool {
        let mut any = false;
        for w in self.widgets.iter_mut() {
            if w.needs_relayout() {
                any = true;
            }
        }
        any
    }

    pub fn reflow(&mut self) {
        let prefs = self.prefs.clone();
        self.apply_prefs(&prefs);
    }

    pub fn on_hover_at(&mut self, x: i32, y: i32) -> bool {
        if !self.area.contains(x, y) {
            return false;
        }
        let lx = x - self.area.x;
        let ly = y - self.area.y;
        let layout = self.layout_widgets();
        for (id, rect, _) in layout.iter() {
            if rect.contains(lx, ly) {
                if let Some(idx) = self.find(id) {
                    let wx = lx - rect.x;
                    let wy = ly - rect.y;
                    return self.widgets[idx].on_hover(wx, wy);
                }
            }
        }
        false
    }

    pub fn clear_hover(&mut self) -> bool {
        let mut any = false;
        for w in self.widgets.iter_mut() {
            if w.on_hover(-1, -1) {
                any = true;
            }
        }
        any
    }

    pub fn tooltip_at(&mut self, x: i32, y: i32) -> Option<Vec<String>> {
        if !self.area.contains(x, y) {
            return None;
        }
        let lx = x - self.area.x;
        let ly = y - self.area.y;
        let layout = self.layout_widgets();
        for (id, rect, _) in layout.iter() {
            if rect.contains(lx, ly) {
                if let Some(idx) = self.find(id) {
                    let wx = lx - rect.x;
                    let wy = ly - rect.y;
                    return self.widgets[idx].tooltip(wx, wy);
                }
            }
        }
        None
    }

    fn panel_thickness(&self) -> u32 {
        if self.is_vertical() {
            self.area.w as u32
        } else {
            self.area.h as u32
        }
    }

    fn last_size(&self) -> i32 {
        self.widgets
            .last()
            .map(|w| {
                let (pw, ph) = w.preferred_size(self.panel_thickness(), self.is_vertical());
                if self.is_vertical() {
                    ph as i32
                } else {
                    pw as i32
                }
            })
            .unwrap_or(0)
    }

    fn find(&self, id: &str) -> Option<usize> {
        self.widgets.iter().position(|w| w.id() == id)
    }

    fn is_vertical(&self) -> bool {
        matches!(self.position, PanelPosition::Left | PanelPosition::Right)
    }

    fn panel_len(&self) -> i32 {
        if self.is_vertical() {
            self.area.h
        } else {
            self.area.w
        }
    }

    fn widget_size(&self, idx: usize) -> i32 {
        let (pw, ph) = self.widgets[idx].preferred_size(self.panel_thickness(), self.is_vertical());
        if self.is_vertical() {
            ph as i32
        } else {
            pw as i32
        }
    }

    pub fn layout_widgets(&self) -> Vec<(WidgetId, Rect, i32)> {
        let vertical = self.is_vertical();
        let drag_from = self.widget_drag.map(|d| d.original_index);
        let mut out = Vec::new();
        for (i, w) in self.widgets.iter().enumerate() {
            let (pw, ph) = w.preferred_size(self.panel_thickness(), vertical);
            let main_pos = if Some(i) == drag_from {
                let d = self.widget_drag.unwrap();
                if vertical {
                    d.mouse_y - self.area.y - d.offset_y
                } else {
                    d.mouse_x - self.area.x - d.offset_x
                }
            } else {
                self.positions[i]
            };
            let rect = if vertical {
                Rect::new(0, main_pos, pw as i32, ph as i32)
            } else {
                Rect::new(main_pos, 0, pw as i32, ph as i32)
            };
            out.push((w.id(), rect, main_pos));
        }
        out
    }

    fn strip_coord(&self, coord: i32) -> i32 {
        let vertical = self.is_vertical();
        let panel_len = self.panel_len();
        let layout = self.layout_widgets();
        let drag_from = self.widget_drag.map(|d| d.original_index);

        let others: Vec<(i32, i32)> = layout
            .iter()
            .enumerate()
            .filter(|(i, _)| Some(*i) != drag_from)
            .map(|(_, (_, r, _))| {
                if vertical {
                    (r.y, r.y + r.h)
                } else {
                    (r.x, r.x + r.w)
                }
            })
            .collect();

        if others.is_empty() {
            return coord;
        }

        let first_start = others[0].0;
        let last_end = others.last().unwrap().1;

        let dist_left = coord.abs();
        let dist_first = (coord - first_start).abs();
        if dist_left < dist_first {
            return 0;
        }
        let dist_right = (panel_len - coord).abs();
        let dist_last = (coord - last_end).abs();
        if dist_right < dist_last {
            return panel_len;
        }

        coord
    }

    pub fn drop_position_for(&self, coord_main: i32, drag_index: usize) -> (i32, i32) {
        let size = self.widget_size(drag_index);
        let panel_len = self.panel_len();
        let strip = self.strip_coord(coord_main);
        let pos = if strip == 0 {
            0
        } else if strip >= panel_len {
            (panel_len - size).max(0)
        } else {
            strip
        };
        (pos, strip)
    }

    pub fn render(&self, buf: &SharedBuffer, theme: &Theme) {
        {
            let mut b = buf.lock().unwrap();
            b.fill_rect(Rect::new(0, 0, self.area.w, self.area.h), theme.bg);
        }

        let drag_from: Option<usize> = self.widget_drag.map(|d| d.original_index);
        let layout = self.layout_widgets();

        for (i, w) in self.widgets.iter().enumerate() {
            if Some(i) == drag_from {
                continue;
            }
            let r = layout[i].1;

            if self.show_borders {
                let mut b = buf.lock().unwrap();
                let border = Color::rgb(0x40, 0x40, 0x40);
                b.fill_rect(Rect::new(r.x, r.y, r.w, 1), border);
                b.fill_rect(Rect::new(r.x, r.y + r.h - 1, r.w, 1), border);
                b.fill_rect(Rect::new(r.x, r.y, 1, r.h), border);
                b.fill_rect(Rect::new(r.x + r.w - 1, r.y, 1, r.h), border);
            }

            w.render(buf, r, theme);
        }

        if let Some(d) = self.widget_drag {
            let w = &self.widgets[d.original_index];
            let (pw, ph) = w.preferred_size(self.panel_thickness(), self.is_vertical());
            let lx = d.mouse_x - self.area.x - d.offset_x;
            let ly = d.mouse_y - self.area.y - d.offset_y;
            let r = Rect::new(lx, ly, pw as i32, ph as i32);
            {
                let mut b = buf.lock().unwrap();
                b.fill_rect(r, Color::rgb(0x40, 0x60, 0x80));
            }
            w.render(buf, r, theme);

            let vertical = self.is_vertical();
            let coord_main = if vertical {
                d.mouse_y - self.area.y
            } else {
                d.mouse_x - self.area.x
            };
            let strip = self.strip_coord(coord_main);
            let color = Color::rgb(0x70, 0xa0, 0xc0);
            {
                let mut b = buf.lock().unwrap();
                if vertical {
                    b.fill_rect(Rect::new(0, strip - 1, self.area.w, 2), color);
                } else {
                    b.fill_rect(Rect::new(strip - 1, 0, 2, self.area.h), color);
                }
            }
        }
    }

    pub fn hit_test(&self, x: i32, y: i32) -> Option<WidgetId> {
        if !self.area.contains(x, y) {
            return None;
        }
        let lx = x - self.area.x;
        let ly = y - self.area.y;
        for (id, rect, _) in self.layout_widgets() {
            if rect.contains(lx, ly) {
                return Some(id);
            }
        }
        None
    }

    pub fn widget_index_at(&self, x: i32, y: i32) -> Option<usize> {
        if !self.area.contains(x, y) {
            return None;
        }
        let lx = x - self.area.x;
        let ly = y - self.area.y;
        for (i, (_, rect, _)) in self.layout_widgets().iter().enumerate() {
            if rect.contains(lx, ly) {
                return Some(i);
            }
        }
        None
    }

    fn relayout(&mut self) {
        let panel_len = self.panel_len();
        let mid = panel_len / 2;

        let mut left: Vec<usize> = Vec::new();
        let mut right: Vec<usize> = Vec::new();
        for i in 0..self.widgets.len() {
            let pos = self.positions[i];
            let size = self.widget_size(i);
            let center = pos + size / 2;
            if center < mid {
                left.push(i);
            } else {
                right.push(i);
            }
        }

        let mut cursor = 0i32;
        for i in &left {
            self.positions[*i] = cursor;
            cursor += self.widget_size(*i);
        }
        let mut cursor = panel_len;
        for i in right.iter().rev() {
            cursor -= self.widget_size(*i);
            self.positions[*i] = cursor;
        }
    }

    pub fn place_widget_at(&mut self, index: usize, strip: i32) {
        if index >= self.widgets.len() {
            return;
        }
        let panel_len = self.panel_len();

        let w = self.widgets.remove(index);
        let _ = self.positions.remove(index);
        let nu = self.next_update.remove(index);
        self.relayout();

        let to_left = strip <= panel_len / 2;
        let mid = panel_len / 2;

        let mut left_count = 0;
        for i in 0..self.widgets.len() {
            let pos = self.positions[i];
            let size = self.widget_size(i);
            if pos + size / 2 < mid {
                left_count += 1;
            } else {
                break;
            }
        }

        let insert_at = if to_left {
            let mut at = left_count;
            for k in 0..left_count {
                let pos = self.positions[k];
                let size = self.widget_size(k);
                if pos + size / 2 > strip {
                    at = k;
                    break;
                }
            }
            at
        } else {
            let mut at = self.widgets.len();
            for k in left_count..self.widgets.len() {
                let pos = self.positions[k];
                let size = self.widget_size(k);
                if pos + size / 2 > strip {
                    at = k;
                    break;
                }
            }
            at
        };

        self.widgets.insert(insert_at, w);
        let temp_pos = if to_left { 0 } else { panel_len };
        self.positions.insert(insert_at, temp_pos);
        self.next_update.insert(insert_at, nu);

        self.relayout();
        self.prefs = self.prefs_for_current();
    }

    pub fn apply_prefs(&mut self, prefs: &HashMap<String, WidgetPrefs>) {
        self.prefs = prefs.clone();
        if self.widgets.is_empty() {
            return;
        }
        let panel_len = self.panel_len();

        let mut left: Vec<(usize, u32)> = Vec::new();
        let mut right: Vec<(usize, u32)> = Vec::new();
        for (i, w) in self.widgets.iter().enumerate() {
            let id = w.id();
            let p = self.prefs.get(&id).copied().unwrap_or(WidgetPrefs {
                side: Side::Left,
                priority: u32::MAX,
            });
            match p.side {
                Side::Left => left.push((i, p.priority)),
                Side::Right => right.push((i, p.priority)),
            }
        }
        left.sort_by_key(|(_, p)| *p);
        right.sort_by_key(|(_, p)| *p);

        let mut positions = vec![0i32; self.widgets.len()];
        let mut cursor = 0i32;
        for (i, _) in &left {
            let size = self.widget_size(*i);
            positions[*i] = cursor;
            cursor += size;
        }
        let mut cursor = panel_len;
        for (i, _) in &right {
            let size = self.widget_size(*i);
            cursor -= size;
            positions[*i] = cursor;
        }
        self.positions = positions;
    }

    pub fn prefs_for_current(&self) -> HashMap<String, WidgetPrefs> {
        let mut out: HashMap<String, WidgetPrefs> = HashMap::new();
        if self.widgets.is_empty() {
            return out;
        }
        let panel_len = self.panel_len();
        let mid = panel_len / 2;

        let mut left: Vec<(usize, i32)> = Vec::new();
        let mut right: Vec<(usize, i32)> = Vec::new();
        for (i, _) in self.widgets.iter().enumerate() {
            let pos = self.positions[i];
            let size = self.widget_size(i);
            let center = pos + size / 2;
            if center < mid {
                left.push((i, pos));
            } else {
                right.push((i, pos));
            }
        }
        left.sort_by_key(|(_, p)| *p);
        for (prio, (i, _)) in left.iter().enumerate() {
            out.insert(
                self.widgets[*i].id(),
                WidgetPrefs { side: Side::Left, priority: prio as u32 },
            );
        }
        right.sort_by_key(|(_, p)| -*p);
        for (prio, (i, _)) in right.iter().enumerate() {
            out.insert(
                self.widgets[*i].id(),
                WidgetPrefs { side: Side::Right, priority: prio as u32 },
            );
        }
        out
    }

    pub fn on_click_at(&mut self, x: i32, y: i32) -> Option<Action> {
        if !self.area.contains(x, y) {
            return None;
        }
        let lx = x - self.area.x;
        let ly = y - self.area.y;
        let layout = self.layout_widgets();
        for (id, rect, _) in layout.iter() {
            if rect.contains(lx, ly) {
                if let Some(idx) = self.find(id) {
                    let wx = lx - rect.x;
                    let wy = ly - rect.y;
                    return Some(self.widgets[idx].on_click(wx, wy));
                }
            }
        }
        None
    }

    pub fn widget_menu_items(&self, id: &str, x: i32, y: i32) -> Vec<(String, Action)> {
        let idx = match self.find(id) {
            Some(i) => i,
            None => return Vec::new(),
        };
        let layout = self.layout_widgets();
        let r = layout[idx].1;
        let wx = x - self.area.x - r.x;
        let wy = y - self.area.y - r.y;
        self.widgets[idx].menu_items(wx, wy)
    }

    pub fn on_custom_action(&mut self, id: &str, msg: &str) -> bool {
        match self.find(id) {
            Some(idx) => self.widgets[idx].on_custom_action(msg),
            None => false,
        }
    }

    pub fn update_due(&mut self, now: Instant) -> bool {
        let mut any = false;
        for (i, w) in self.widgets.iter_mut().enumerate() {
            let due = match self.next_update[i] {
                Some(t) => now >= t,
                None => false,
            };
            if !due {
                continue;
            }
            let next = w.update();
            self.next_update[i] = next.map(|d| now + d);
            any = true;
        }
        any
    }

    pub fn update_all(&mut self) -> bool {
        let now = Instant::now();
        let mut any = false;
        for (i, w) in self.widgets.iter_mut().enumerate() {
            let next = w.update();
            self.next_update[i] = next.map(|d| now + d);
            any = true;
        }
        any
    }

    pub fn next_wakeup(&self) -> Option<Instant> {
        self.next_update.iter().filter_map(|t| *t).min()
    }
}