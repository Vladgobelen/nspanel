use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct Font {
    inner: fontdue::Font,
    /// Кэш растеризованных глифов: (символ, размер в битах f32) -> (метрики, coverage).
    cache: Mutex<HashMap<(char, u32), (fontdue::Metrics, Vec<u8>)>>,
}

impl Font {
    pub fn load(path: &str) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
        let data = std::fs::read(path)
            .map_err(|e| format!("failed to read font '{path}': {e}"))?;
        let inner = fontdue::Font::from_bytes(data, fontdue::FontSettings::default())
            .map_err(|e| format!("failed to parse font '{path}': {e}"))?;
        Ok(Arc::new(Self {
            inner,
            cache: Mutex::new(HashMap::new()),
        }))
    }

    /// Растеризовать символ (с кэшем).
    pub fn rasterize(&self, ch: char, size: f32) -> (fontdue::Metrics, Vec<u8>) {
        let key = (ch, size.to_bits());
        {
            let cache = self.cache.lock().unwrap();
            if let Some((m, c)) = cache.get(&key) {
                return (m.clone(), c.clone());
            }
        }
        let (m, c) = self.inner.rasterize(ch, size);
        let mut cache = self.cache.lock().unwrap();
        cache.insert(key, (m.clone(), c.clone()));
        (m, c)
    }

    pub fn text_width(&self, text: &str, size: f32) -> f32 {
        let mut w = 0.0;
        for ch in text.chars() {
            let (m, _) = self.rasterize(ch, size);
            w += m.advance_width;
        }
        w
    }

    #[allow(dead_code)]
    pub fn line_height(&self, size: f32) -> f32 {
        let (m, _) = self.rasterize('M', size);
        m.height as f32 + m.ymin as f32
    }
}