pub mod color;
pub mod geometry;
pub mod ipc;
pub mod menu;
pub mod module;
pub mod module_registry;
pub mod modules;
pub mod panel;
pub mod widget;

pub use color::Color;
pub use geometry::Rect;
pub use module_registry::ModuleRegistry;
pub use panel::Panel;
pub use widget::{Action, SizeMode, Theme, Widget, WidgetId};