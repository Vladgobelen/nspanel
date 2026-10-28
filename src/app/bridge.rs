//! Соединения панели с nsbridge.
//!
//! Одна ответственность: найти сокет nsbridge, подключить к нему каждый
//! виджет панели, разослать стартовый `{"type":"request"}` и перерисовать
//! панель после подключения.
//!
//! Здесь НЕТ:
//! - poll-цикла и решения «когда переподключаться» (это `app/run.rs`);
//! - обработки X11-событий (это `app/events/*`);
//! - знания о том, что именно шлёт nsbridge (это виджеты и `core/ipc`).
//!
//! Функция `recreate_bridge_connections` идемпотентна: она полностью
//! пересоздаёт набор соединений, не полагаясь на предыдущее состояние.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::core::ipc::{build_frame, UnixStreamClient};
use crate::core::{Panel, Theme};
use crate::render::SharedBuffer;
use crate::x11::X11Backend;

/// Путь к сокету nsbridge для текущего пользователя.
///
/// `/run/user/<uid>/nsbridge.sock`. UID берём через `nix::unistd::getuid`,
/// чтобы не парсить `/proc` и не зависеть от `$XDG_RUNTIME_DIR`.
pub fn socket_path() -> PathBuf {
    let uid = nix::unistd::getuid();
    PathBuf::from(format!("/run/user/{}/nsbridge.sock", uid))
}

/// Пересоздать все соединения с nsbridge и разослать стартовый запрос.
///
/// Шаги:
/// 1. Отписаться от root motion (панель могла быть в состоянии hover —
///    после переподключения hover-состояние неактуально).
/// 2. Очистить старый набор сокетов (мёртвые fd больше не нужны).
/// 3. Для каждого виджета панели попытаться подключиться к nsbridge.
///    Успешные подключения кладутся и в `sockets` (для poll в `run.rs`),
///    и в сам виджет (через `panel.set_socket_for`).
/// 4. Каждому подключённому виджету отправить `{"type":"request"}` —
///    nsbridge ответит снимком групп/трея.
/// 5. Перерисовать панель (виджеты могли сбросить кэш иконок).
///
/// Возвращает `true`, если удалось подключить хотя бы один виджет.
/// `false` означает «nsbridge недоступен» — `run.rs` запланирует ретрай.
pub fn recreate_bridge_connections(
    panel: &mut Panel,
    x11: &mut X11Backend,
    sockets: &mut HashMap<String, UnixStreamClient>,
    bridge_path: &Path,
    panel_buf: &mut SharedBuffer,
    theme: &Theme,
) -> bool {
    eprintln!("[nspanel] recreating bridge connections...");
    let _ = x11.unsubscribe_root_motion();
    sockets.clear();

    let mut any_connected = false;
    for id in panel.widget_ids() {
        let mut client = match UnixStreamClient::connect(bridge_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Клон — для виджета (читает данные сам). Оригинал — в `sockets`
        // (слушает poll в `run.rs`). Оба fd смотрят в одно соединение.
        let clone = match client.try_clone() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[nspanel] widget '{}' try_clone failed: {e}", id);
                continue;
            }
        };

        panel.set_socket_for(&id, clone);

        // Стартовый запрос — фрейм собираем через `build_frame`.
        let frame = match build_frame(br#"{"type":"request"}"#) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("[nspanel] widget '{}' build_frame failed: {e}", id);
                continue;
            }
        };
        if let Err(e) = client.write_all(&frame) {
            eprintln!("[nspanel] widget '{}' send request failed: {e}", id);
            continue;
        }

        sockets.insert(id, client);
        any_connected = true;
    }

    if any_connected {
        eprintln!(
            "[nspanel] bridge recreated: {}/{} connected",
            sockets.len(),
            panel.widget_ids().len()
        );
    } else {
        eprintln!("[nspanel] bridge recreate failed: no connections (nsbridge down?)");
    }

    panel.render(panel_buf, theme);
    let _ = x11.put_image(panel_buf);

    any_connected
}