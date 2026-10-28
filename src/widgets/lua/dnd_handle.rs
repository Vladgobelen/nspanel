//! Глобал `dnd` для Lua-виджетов.
//!
//! Одна ответственность: `UserData`-обёртка, через которую Lua-виджет
//! сообщает панели:
//! - `dnd:update(area)` — где сейчас иконки (для hit-test при drop);
//! - `dnd:set_on_hover(fn)` — callback при движении курсора с файлом;
//! - `dnd:set_on_drop(fn)` — callback при drop файла.
//!
//! На Фазе 1 — только хранит callbacks и area. Связи с `X11Backend`
//! нет (Фаза 4).
//!
//! Здесь НЕТ:
//! - X11 (это `x11/backend/dnd/`);
//! - рендера (это `core/panel/render.rs`);
//! - `LuaWidget` (это `widget/mod.rs`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use mlua::{Function, UserData, UserDataMethods};

use crate::core::Rect;

/// Разделяемые callbacks + area. Живёт в `LuaWidget`.
///
/// `Rc<RefCell<...>>` — потому что `mlua::Function` не `Send`/`Sync`,
/// а `Lua` живёт в одном потоке (главный `poll`-цикл).
#[derive(Clone, Default)]
pub struct DndCallbacks {
    pub on_hover: Rc<RefCell<Option<Function>>>,
    pub on_drop: Rc<RefCell<Option<Function>>>,
    pub area: Rc<Cell<Option<Rect>>>,
}

impl DndCallbacks {
    pub fn new() -> Self {
        Self::default()
    }
}

/// `UserData`-обёртка, которую `LuaWidget::install_dnd` кладёт в глобалы
/// как `dnd`.
pub struct DndHandle {
    pub callbacks: DndCallbacks,
}

impl UserData for DndHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // dnd:update(area) — запомнить прямоугольник виджета.
        methods.add_method("update", |_, this, area: mlua::Table| {
            let r = Rect::new(
                area.get("x").unwrap_or(0),
                area.get("y").unwrap_or(0),
                area.get("w").unwrap_or(0),
                area.get("h").unwrap_or(0),
            );
            this.callbacks.area.set(Some(r));
            Ok(())
        });

        // dnd:set_on_hover(fn) — callback(x, y) при движении.
        methods.add_method("set_on_hover", |_, this, f: Option<Function>| {
            *this.callbacks.on_hover.borrow_mut() = f;
            Ok(())
        });

        // dnd:set_on_drop(fn) — callback(x, y, uris) при drop.
        methods.add_method("set_on_drop", |_, this, f: Option<Function>| {
            *this.callbacks.on_drop.borrow_mut() = f;
            Ok(())
        });
    }
}