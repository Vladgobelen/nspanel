//! Разбор `text/uri-list` (RFC 2483) для XDND.
//!
//! Одна ответственность: байты ↔ `Vec<PathBuf>`.
//! - `parse_uri_list` — то, что приходит от source'а (файлового менеджера).
//! - `build_uri_list` — то, что панель отдаёт target'у (приложению) при forward.
//!
//! Здесь НЕТ:
//! - X11, D-Bus, сокетов;
//! - знания о том, что за файлы и куда их бросают.
//!
//! Формат: строки, разделитель `\r\n` (допускается одиночный `\n`),
//! `#` в начале строки — комментарий, пустые строки — пропускаются.
//! Поддерживаются только `file://` URI. Остальные схемы (`http://`, ...)
//! пропускаются: для drop на приложение нужны именно файлы.
//! Percent-decoding: `%20` → пробел, `%XX` → байт.

use std::path::PathBuf;

/// Разобрать `text/uri-list` в список путей.
///
/// Правила:
/// - `\r\n` и `\n` — разделители строк;
/// - `#` в начале строки (после trim) — комментарий, пропускается;
/// - пустые строки — пропускаются;
/// - `file://...` → путь (percent-decode);
/// - не-`file://` схемы — пропускаются;
/// - невалидный percent-encoding — строка пропускается.
pub fn parse_uri_list(bytes: &[u8]) -> Vec<PathBuf> {
    let text = String::from_utf8_lossy(bytes);
    let mut out = Vec::new();

    for raw_line in text.split('\n') {
        let line = raw_line.trim_end_matches('\r').trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(path) = uri_to_path(line) {
            out.push(path);
        }
    }

    out
}

/// Собрать `text/uri-list` из путей (для forward target'у).
///
/// Каждый путь → `file://<percent-encoded path>`, разделитель `\r\n`.
pub fn build_uri_list(paths: &[PathBuf]) -> Vec<u8> {
    let mut out = String::new();
    for p in paths {
        out.push_str("file://");
        out.push_str(&percent_encode_path(p));
        out.push_str("\r\n");
    }
    out.into_bytes()
}

/// `file://...` → `PathBuf`. Только `file://`. Невалидное — `None`.
fn uri_to_path(uri: &str) -> Option<PathBuf> {
    // Разрешаем "file:///path" и "file:/path" (второй — редко, но бывает).
    let rest = uri.strip_prefix("file://")
        .or_else(|| uri.strip_prefix("file:/"))?;

    // Осталось: "/path/to/file" (для file:///) или "host/path" (для file://host/).
    // file://host/... — для drop на приложение не поддерживаем (пропускаем),
    // кроме пустого host, т.е. "file:///path".
    if !rest.starts_with('/') {
        return None;
    }

    let decoded = percent_decode(rest)?;
    Some(PathBuf::from(decoded))
}

/// Percent-decoding: `%XX` → байт. Невалидный `%` — `None`.
fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return None;
            }
            let hi = hex_val(bytes[i + 1])?;
            let lo = hex_val(bytes[i + 2])?;
            out.push((hi << 4) | lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Percent-encoding пути для `file://` URI.
///
/// Кодируются только байты, недопустимые в URI. `/` остаётся как есть
/// (это разделитель пути, не часть имени файла).
fn percent_encode_path(p: &PathBuf) -> String {
    let s = p.to_string_lossy();
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0xF) as usize] as char);
        }
    }
    out
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

const HEX: &[u8; 16] = b"0123456789ABCDEF";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_file_uri() {
        let v = parse_uri_list(b"file:///home/user/a.txt\r\n");
        assert_eq!(v, vec![PathBuf::from("/home/user/a.txt")]);
    }

    #[test]
    fn parse_percent_encoded_space() {
        let v = parse_uri_list(b"file:///home/user/a%20b.txt\r\n");
        assert_eq!(v, vec![PathBuf::from("/home/user/a b.txt")]);
    }

    #[test]
    fn parse_multiple_with_comments_and_blank() {
        let src = b"# comment\r\nfile:///a\r\n\r\nfile:///b\r\n";
        let v = parse_uri_list(src);
        assert_eq!(v, vec![PathBuf::from("/a"), PathBuf::from("/b")]);
    }

    #[test]
    fn parse_lf_only() {
        let v = parse_uri_list(b"file:///a\nfile:///b\n");
        assert_eq!(v, vec![PathBuf::from("/a"), PathBuf::from("/b")]);
    }

    #[test]
    fn parse_skips_non_file_scheme() {
        let v = parse_uri_list(b"http://example.com/x\r\nfile:///a\r\n");
        assert_eq!(v, vec![PathBuf::from("/a")]);
    }

    #[test]
    fn parse_skips_file_with_host() {
        // file://host/path — не поддерживаем.
        let v = parse_uri_list(b"file://somehost/path\r\n");
        assert!(v.is_empty());
    }

    #[test]
    fn parse_rejects_bad_percent() {
        let v = parse_uri_list(b"file:///a%ZZ\r\n");
        assert!(v.is_empty());
    }

    #[test]
    fn parse_empty() {
        assert!(parse_uri_list(b"").is_empty());
        assert!(parse_uri_list(b"\r\n\r\n").is_empty());
    }

    #[test]
    fn build_simple() {
        let out = build_uri_list(&[PathBuf::from("/home/user/a.txt")]);
        assert_eq!(out, b"file:///home/user/a.txt\r\n");
    }

    #[test]
    fn build_encodes_space() {
        let out = build_uri_list(&[PathBuf::from("/home/user/a b.txt")]);
        assert_eq!(out, b"file:///home/user/a%20b.txt\r\n");
    }

    #[test]
    fn roundtrip() {
        let paths = vec![
            PathBuf::from("/home/user/a.txt"),
            PathBuf::from("/tmp/a b"),
        ];
        let bytes = build_uri_list(&paths);
        let back = parse_uri_list(&bytes);
        assert_eq!(back, paths);
    }
}