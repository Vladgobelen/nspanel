//! Стартовое подключение к nsbridge.
//!
//! Одна ответственность: при старте панели попробовать подключиться
//! к nsbridge и вычислить `reconnect_at` (когда пробовать снова, если
//! мост недоступен).
//!
//! Здесь НЕТ:
//! - переподключения в рантайме (это `app/bridge.rs::try_reconnect`);
//! - `poll`-цикла (это `run/loop.rs`);
//! - знания о том, что шлёт nsbridge (это виджеты).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::runtime::Runtime;
use crate::app::bridge::recreate_bridge_connections;
use crate::core::ipc::UnixStreamClient;

/// Задержка перед повторной попыткой подключиться к nsbridge.
/// Используется и на старте, и в рантайме (`run/loop.rs`).
pub const RECONNECT_DELAY: Duration = Duration::from_secs(1);

/// Стартовое подключение: попытаться подключиться ко всем виджетам,
/// вернуть `reconnect_at` (момент следующей попытки или `None`, если
/// подключились).
pub fn initial_connect(
    rt: &mut Runtime,
    sockets: &mut HashMap<String, UnixStreamClient>,
) -> Option<Instant> {
    let connected = recreate_bridge_connections(
        &mut rt.panel,
        &mut rt.x11,
        sockets,
        &rt.bridge_path,
        &mut rt.panel_buf,
        &rt.theme,
    );

    if connected {
        None
    } else {
        Some(Instant::now() + RECONNECT_DELAY)
    }
}