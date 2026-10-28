//! Исполнение `Action`, который вернул виджет или пункт меню.
//!
//! Одна ответственность: превратить `Action` в побочный эффект
//! (лог, запуск команды). Всё, что «делает» панель по клику, — здесь.
//!
//! Здесь НЕТ:
//! - решения, какой `Action` вернуть (это `Widget::on_click` /
//!   `Module::handle_click`);
//! - рисования (это `Panel` / `ModuleRegistry`);
//! - знаний о виджетах и модулях (только `Action`).
//!
//! `Action::Custom` и `Action::None` — no-op: `Custom` обрабатывается
//! виджетом через `Widget::on_custom_action` (это делает `button.rs`,
//! а не этот файл), а `None` — явное «ничего не делать».

use crate::core::Action;

/// Исполнить `Action`.
///
/// - `None` — ничего.
/// - `Log(msg)` — `eprintln!` в stderr (отладка).
/// - `RunCommand(cmd)` — `sh -c <cmd>` в фоне (не ждём завершения).
/// - `Custom(_)` — ничего: кастомные действия виджета обрабатываются
///   через `Widget::on_custom_action`, а не здесь.
pub fn handle_action(action: Action) {
    match action {
        Action::None => {}
        Action::Log(msg) => eprintln!("[action] {msg}"),
        Action::RunCommand(cmd) => {
            eprintln!("[action] run: {cmd}");
            let _ = std::process::Command::new("sh").arg("-c").arg(&cmd).spawn();
        }
        Action::Custom(_) => {}
    }
}