//! Фреймовый протокол: 4 байта длины (u32 big-endian) + payload.
//!
//! Одна ответственность: разрезать поток байт на фреймы (`parse_frames`)
//! и собрать фрейм из payload (`build_frame`). Чистые функции — сокета
//! не касаются, тестируются в изоляции.
//!
//! Здесь НЕТ:
//! - транспорта (это `socket.rs`);
//! - знания о том, что внутри payload (это виджеты и `nsbridge`);
//! - `poll`-цикла (это `app/run.rs`).
//!
//! ## Как пользоваться
//!
//! Вызывающий держит у себя `Vec<u8>` — «неразобранный хвост»:
//!
//! ```ignore
//! // чтение:
//! let mut buf = [0u8; 4096];
//! let n = sock.read_into(&mut buf)?;
//! if n == 0 { /* EOF */ }
//! pending.extend_from_slice(&buf[..n]);
//! let frames = frame::parse_frames(&mut pending);
//! for payload in frames { /* ... */ }
//!
//! // запись:
//! let f = frame::build_frame(b"{\"type\":\"request\"}")?;
//! sock.write_all(&f)?;
//! ```
//!
//! Так транспорт не знает про фреймы, а фрейминг не знает про сокет.

use std::io;

/// Максимальный размер payload. Защита от злонамеренного/багованного
/// peer'а: без лимита `len as usize` может заставить аллоцировать гигабайты.
/// Значение совпадает с `nsbridge/src/protocol.rs::MAX_FRAME`.
pub const MAX_FRAME: usize = 16 * 1024 * 1024;

/// Разобрать накопленный буфер на готовые payload'ы.
///
/// Возвращает **только полностью собранные** фреймы; неполный хвост
/// (заголовок без payload, payload не до конца) остаётся в `buf`.
/// Из `buf` удаляются только разобранные байты — порядок сохраняется.
///
/// Ошибки:
/// - `InvalidData` — если заявленная длина больше `MAX_FRAME`.
///   В этом случае буфер **не** трогается (вызывающий решает, что делать:
///   оборвать соединение, залогировать, ...).
pub fn parse_frames(buf: &mut Vec<u8>) -> io::Result<Vec<Vec<u8>>> {
    let mut frames = Vec::new();
    let mut offset = 0;

    while buf.len() - offset >= 4 {
        let len = u32::from_be_bytes([
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
        ]) as usize;

        if len > MAX_FRAME {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("frame too large: {len} > {MAX_FRAME}"),
            ));
        }

        if buf.len() - offset - 4 < len {
            break; // ждём ещё данные
        }

        let payload = buf[offset + 4..offset + 4 + len].to_vec();
        frames.push(payload);
        offset += 4 + len;
    }

    if offset > 0 {
        buf.drain(..offset);
    }

    Ok(frames)
}

/// Собрать фрейм: 4 байта длины + payload.
///
/// Ошибка `InvalidInput`, если payload больше `MAX_FRAME` — тот же лимит,
/// что и на чтении (симметрия).
pub fn build_frame(payload: &[u8]) -> io::Result<Vec<u8>> {
    if payload.len() > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("payload too large: {} > {MAX_FRAME}", payload.len()),
        ));
    }
    let mut out = Vec::with_capacity(4 + payload.len());
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_parse_roundtrip() {
        let f = build_frame(b"hello").unwrap();
        assert_eq!(f.len(), 4 + 5);
        let mut buf = f;
        let frames = parse_frames(&mut buf).unwrap();
        assert_eq!(frames, vec![b"hello".to_vec()]);
        assert!(buf.is_empty());
    }

    #[test]
    fn parse_partial_leaves_tail() {
        let mut buf = build_frame(b"hello").unwrap();
        buf.truncate(6); // заголовок + 2 байта payload
        let frames = parse_frames(&mut buf).unwrap();
        assert!(frames.is_empty());
        assert_eq!(buf.len(), 6); // ничего не потеряли
    }

    #[test]
    fn parse_two_frames_at_once() {
        let mut buf = build_frame(b"a").unwrap();
        buf.extend_from_slice(&build_frame(b"bc").unwrap());
        let frames = parse_frames(&mut buf).unwrap();
        assert_eq!(frames, vec![b"a".to_vec(), b"bc".to_vec()]);
        assert!(buf.is_empty());
    }

    #[test]
    fn parse_empty_payload() {
        let mut buf = build_frame(b"").unwrap();
        let frames = parse_frames(&mut buf).unwrap();
        assert_eq!(frames, vec![Vec::<u8>::new()]);
        assert!(buf.is_empty());
    }

    #[test]
    fn parse_rejects_oversized_header() {
        let mut buf = vec![0u8; 4];
        buf.copy_from_slice(&((MAX_FRAME + 1) as u32).to_be_bytes());
        let err = parse_frames(&mut buf).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert_eq!(buf.len(), 4); // буфер не тронут
    }

    #[test]
    fn build_rejects_oversized_payload() {
        let payload = vec![0u8; MAX_FRAME + 1];
        let err = build_frame(&payload).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }
}