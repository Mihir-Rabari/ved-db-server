//! Simple thread-safe KV store using DashMap

use dashmap::DashMap;
use std::sync::Arc;

/// Simple KV store using DashMap for lock-free concurrent access
#[derive(Clone)]
pub struct SimpleKvStore {
    map: Arc<DashMap<Vec<u8>, Vec<u8>>>,
}

impl SimpleKvStore {
    pub fn new() -> Self {
        Self {
            map: Arc::new(DashMap::new()),
        }
    }

    pub fn set(&self, key: &[u8], value: &[u8]) -> Result<(), String> {
        self.map.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.map.get(key).map(|v| v.value().clone())
    }

    pub fn delete(&self, key: &[u8]) -> bool {
        self.map.remove(key).is_some()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn keys(&self) -> Vec<Vec<u8>> {
        self.map.iter().map(|entry| entry.key().clone()).collect()
    }
}
