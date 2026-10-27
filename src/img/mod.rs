//! Инструмент для виджетов: загрузить готовый PNG (из кеша nsbridge)
//! и отресайзить до нужного размера. Поиск/нормализация иконок —
//! в nsbridge, панель получает готовый `icon_path` (64×64 PNG).
//!
//! Кеш: ключ включает mtime файла. Если nsbridge перезаписал icon.png
//! (иконка поменялась), mtime меняется → кеш промахивается → грузим заново.

pub mod cache;
pub mod resize;

use std::io::BufReader;
use std::path::Path;
use std::time::SystemTime;

use cache::IconCache;

/// Глобальный загрузчик иконок (для Lua-виджетов, `img:load`).
pub struct ImageLoader {
    cache: IconCache,
}

impl ImageLoader {
    pub fn new() -> Self {
        Self {
            cache: IconCache::new(),
        }
    }

    /// Загрузить PNG по пути и отресайзить до `size`.
    /// Возвращает массив ARGB u32 (для `buf:draw_pixels`).
    pub fn load(&self, path: &Path, size: u32, filter: resize::Filter) -> Option<Vec<u32>> {
        let mtime = file_mtime(path)?;
        let key = (path.to_path_buf(), size, filter, mtime);
        if let Some(pixels) = self.cache.get(&key) {
            return Some(pixels);
        }

        let decoded = decode_png(path)?;
        let out = resize::resize(
            &decoded.rgba,
            decoded.width,
            decoded.height,
            size as usize,
            size as usize,
            filter,
        );

        self.cache.insert(key, out.clone());
        Some(out)
    }

    #[allow(dead_code)]
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    #[allow(dead_code)]
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }
}

impl Default for ImageLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// mtime файла в наносекундах от UNIX_EPOCH (0, если не удалось).
/// 0 как «нет mtime» — кеш всё равно будет промахиваться при каждом
/// обращении, но лучше, чем вечный старый пиксель.
fn file_mtime(path: &Path) -> Option<u128> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    let dur = mtime.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    Some(dur.as_nanos())
}

struct DecodedPng {
    rgba: Vec<u8>,
    width: usize,
    height: usize,
}

/// Прочитать PNG (любой формат: RGB, RGBA, Grayscale, Palette) → RGBA8.
fn decode_png(path: &Path) -> Option<DecodedPng> {
    let file = std::fs::File::open(path).ok()?;
    let reader = BufReader::new(file);
    let decoder = png::Decoder::new(reader);
    let mut reader = decoder.read_info().ok()?;

    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let raw = &buf[..info.buffer_size()];
    let (w, h) = (info.width as usize, info.height as usize);

    let rgba = match info.color_type {
        png::ColorType::Rgba => raw.to_vec(),

        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity(w * h * 4);
            for chunk in raw.chunks(3) {
                out.extend_from_slice(chunk);
                out.push(255);
            }
            out
        }

        png::ColorType::Grayscale => {
            let mut out = Vec::with_capacity(w * h * 4);
            for &g in raw {
                out.extend_from_slice(&[g, g, g, 255]);
            }
            out
        }

        png::ColorType::GrayscaleAlpha => {
            let mut out = Vec::with_capacity(w * h * 4);
            for chunk in raw.chunks(2) {
                let (g, a) = (chunk[0], chunk[1]);
                out.extend_from_slice(&[g, g, g, a]);
            }
            out
        }

        png::ColorType::Indexed => {
            let palette = reader.info().palette.as_ref()?;
            let trns = reader.info().trns.clone();

            let mut out = Vec::with_capacity(w * h * 4);
            for &idx in raw {
                let i = (idx as usize) * 3;
                if i + 2 < palette.len() {
                    let r = palette[i];
                    let g = palette[i + 1];
                    let b = palette[i + 2];
                    let a = match &trns {
                        Some(t) => *t.get(idx as usize).unwrap_or(&255),
                        None => 255,
                    };
                    out.extend_from_slice(&[r, g, b, a]);
                } else {
                    out.extend_from_slice(&[0, 0, 0, 255]);
                }
            }
            out
        }
    };

    Some(DecodedPng {
        rgba,
        width: w,
        height: h,
    })
}