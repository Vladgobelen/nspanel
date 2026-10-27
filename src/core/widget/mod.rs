//! Контракт виджета панели.
//!
//! Виджет — безликий для панели объект: панель не знает, что он рисует
//! и как реагирует на события. Она вызывает методы трейта `Widget`,
//! а виджет сам решает, что делать.
//!
//! Разбито по файлам:
//! - `id.rs`       — `WidgetId` (алиас на `String`).
//! - `action.rs`   — `Action` (что сделать по клику: лог, команда).
//! - `size.rs`     — `SizeMode` (Fixed / Stretch) + расчёт `preferred_size`.
//! - `theme.rs`    — `Theme` (цвета + шрифт) + `from_config`.
//! - `behavior.rs` — трейт `Widget` (жизненный цикл виджета).
//!
//! Тултип — метод `tooltip` в трейте `Widget` (возвращает `Option<Vec<String>>`),
//! не отдельный тип: панель сама решает, как и где его рисовать.

pub mod action;
pub mod behavior;
pub mod id;
pub mod size;
pub mod theme;

pub use action::Action;
pub use behavior::Widget;
pub use id::WidgetId;
pub use size::SizeMode;
pub use theme::Theme;