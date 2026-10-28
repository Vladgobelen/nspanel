//! Транспорт: неблокирующий Unix-сокет-клиент.
//!
//! Одна ответственность: «есть соединение, умею читать/писать сырые байты,
//! проверять живость (`peek`), клонировать, отдавать fd для `poll`».
//!
//! Здесь НЕТ:
//! - фрейминга (это `frame.rs`);
//! - знания о том, что внутри payload (это виджеты и `nsbridge`);
//! - `poll`-цикла (это `app/run.rs`).

use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::path::Path;

use nix::sys::socket::{recv, MsgFlags};

/// Неблокирующий Unix-сокет-клиент.
///
/// Все операции чтения возвращают `WouldBlock`, если данных нет —
/// вызывающий (`app/run.rs`) сам решает, когда звать, по результату `poll`.
pub struct UnixStreamClient {
    stream: UnixStream,
}

impl UnixStreamClient {
    /// Подключиться к сокету. Сразу переводит в неблокирующий режим.
    pub fn connect<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let stream = UnixStream::connect(path)?;
        stream.set_nonblocking(true)?;
        Ok(Self { stream })
    }

    /// Клонировать сокет (через `UnixStream::try_clone`). Оба fd указывают
    /// на одно соединение; буферы у клиентов независимые (у транспорта
    /// буфера нет вообще — его ведёт `frame::parse_frames` на стороне
    /// вызывающего).
    pub fn try_clone(&self) -> io::Result<Self> {
        let stream = self.stream.try_clone()?;
        Ok(Self { stream })
    }

    /// Проверить, жив ли сокет, без удаления данных (`MSG_PEEK`).
    ///
    /// - `Ok(0)` — EOF (сокет закрыт, nsbridge умер).
    /// - `Ok(n)` — есть данные (n > 0).
    /// - `Err(WouldBlock)` — нет данных (жив).
    /// - `Err(e)` — ошибка (закрыт).
    ///
    /// Нужен `nix::recv` с `MSG_PEEK`: `std::os::unix::net::UnixStream`
    /// не даёт peek напрямую.
    pub fn peek(&mut self) -> io::Result<usize> {
        let mut buf = [0u8; 1];
        match recv(self.stream.as_raw_fd(), &mut buf, MsgFlags::MSG_PEEK) {
            Ok(n) => Ok(n),
            Err(nix::errno::Errno::EAGAIN) => {
                Err(io::Error::from(io::ErrorKind::WouldBlock))
            }
            Err(e) => Err(io::Error::from_raw_os_error(e as i32)),
        }
    }

    /// Прочитать доступные байты в `buf`, вернуть сколько прочитано.
    ///
    /// Один `read` (не цикл): вызывающий сам решает, читать ли до
    /// `WouldBlock`. Возвращает `Ok(0)` на EOF.
    pub fn read_into(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf)
    }

    /// Записать все байты. `WouldBlock` при неблокирующем сокете
    /// пробрасывается как есть — вызывающий решает, повторять ли.
    pub fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        self.stream.write_all(data)?;
        self.stream.flush()?;
        Ok(())
    }

    /// fd для `poll` в `app/run.rs`.
    pub fn as_raw_fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }
}

impl AsRawFd for UnixStreamClient {
    fn as_raw_fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }
}