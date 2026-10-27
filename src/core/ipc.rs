//! Базовый примитив для работы с Unix-сокетами.
//! Панель даёт виджетам возможность подключаться к сокетам,
//! но не знает, какие данные по ним передаются.

use std::io::{self, Read};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::path::Path;

use nix::sys::socket::{recv, MsgFlags};

pub struct SocketClient {
    stream: UnixStream,
    buffer: Vec<u8>,
}

impl SocketClient {
    pub fn connect<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let stream = UnixStream::connect(path)?;
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream,
            buffer: Vec::new(),
        })
    }

    /// Клонировать сокет (через `UnixStream::try_clone`). Оба fd указывают
    /// на одно соединение. Буфер клона — пустой (свой).
    pub fn try_clone(&self) -> io::Result<Self> {
        let stream = self.stream.try_clone()?;
        Ok(Self {
            stream,
            buffer: Vec::new(),
        })
    }

    /// Проверить, жив ли сокет, без удаления данных (MSG_PEEK).
    /// - `Ok(0)` — EOF (сокет закрыт, nsbridge умер).
    /// - `Ok(n)` — есть данные.
    /// - `Err(WouldBlock)` — нет данных (жив).
    /// - `Err(e)` — ошибка (закрыт).
    pub fn peek(&mut self) -> io::Result<usize> {
        let mut buf = [0u8; 1];
        match recv(self.stream.as_raw_fd(), &mut buf, MsgFlags::MSG_PEEK) {
            Ok(n) => Ok(n),
            Err(nix::errno::Errno::EAGAIN) => Err(io::Error::from(io::ErrorKind::WouldBlock)),
            Err(e) => Err(io::Error::from_raw_os_error(e as i32)),
        }
    }

    /// Прочитать доступные данные. Возвращает готовые фреймы (если собрались).
    /// Формат фрейма: 4 байта длины + payload.
    pub fn read_available(&mut self) -> io::Result<Vec<Vec<u8>>> {
        let mut temp = [0u8; 4096];
        let mut frames = Vec::new();

        loop {
            match self.stream.read(&mut temp) {
                Ok(0) => break, // EOF
                Ok(n) => self.buffer.extend_from_slice(&temp[..n]),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            }
        }

        // Разбираем буфер на фреймы
        let mut offset = 0;
        while self.buffer.len() - offset >= 4 {
            let len = u32::from_be_bytes([
                self.buffer[offset],
                self.buffer[offset + 1],
                self.buffer[offset + 2],
                self.buffer[offset + 3],
            ]) as usize;

            if self.buffer.len() - offset - 4 < len {
                break; // Ждём ещё данные
            }

            let payload = self.buffer[offset + 4..offset + 4 + len].to_vec();
            frames.push(payload);
            offset += 4 + len;
        }

        if offset > 0 {
            self.buffer.drain(..offset);
        }

        Ok(frames)
    }

    /// Отправить данные (например, команду "activate window").
    pub fn send(&mut self, data: &[u8]) -> io::Result<()> {
        use std::io::Write;
        self.stream.write_all(data)?;
        self.stream.flush()?;
        Ok(())
    }
}

impl AsRawFd for SocketClient {
    fn as_raw_fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }
}