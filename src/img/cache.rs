use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use super::resize::Filter;

/// In-memory кеш иконок: (путь, размер, фильтр, mtime) → ARGB-пиксели.
/// mtime в ключе — чтобы обновлённый на диске PNG (nsbridge перезаписал
/// icon.png) не отдавал старые пиксели.
pub struct IconCache {
    map: Mutex<HashMap<(PathBuf, u32, Filter, u128), Vec<u32>>>,
}

impl IconCache {
    pub fn new() -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, key: &(PathBuf, u32, Filter, u128)) -> Option<Vec<u32>> {
        let map = self.map.lock().unwrap();
        map.get(key).cloned()
    }

    pub fn insert(&self, key: (PathBuf, u32, Filter, u128), pixels: Vec<u32>) {
        let mut map = self.map.lock().unwrap();
        map.insert(key, pixels);
    }

    #[allow(dead_code)]
    pub fn clear(&self) {
        let mut map = self.map.lock().unwrap();
        map.clear();
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        let map = self.map.lock().unwrap();
        map.len()
    }
}