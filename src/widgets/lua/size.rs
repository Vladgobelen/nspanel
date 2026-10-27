//! `size:*` — инструмент изменения размера виджета вдоль панели.
//!
//! - `size:set_length(n)` — виджет сообщает панели, что его длина вдоль
//!   панели теперь `n` пикселей. Панель сохраняет это значение (для
//!   `preferred_size`) и помечает layout грязным (`dirty`), чтобы после
//!   события пересчитать позиции соседей.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use mlua::{UserData, UserDataMethods};

pub struct LuaSize {
    pub length: Rc<Cell<Option<u32>>>,
    pub dirty: Arc<AtomicBool>,
    pub widget_id: String,
}

impl UserData for LuaSize {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("set_length", |_, this, n: u32| {
            this.length.set(Some(n));
            this.dirty.store(true, Ordering::Relaxed);
            Ok(())
        });
    }
}