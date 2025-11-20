//! TTL (Time-To-Live) manager for cache expiration

use chrono::{DateTime, Utc};
use std::collections::BinaryHeap;
use std::cmp::Ordering;
use parking_lot::RwLock;

/// Entry in the expiration heap
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExpirationEntry {
    /// Cache key
    pub key: String,
    /// Expiration timestamp
    pub expires_at: DateTime<Utc>,
}

impl Ord for ExpirationEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap (earliest expiration first)
        other.expires_at.cmp(&self.expires_at)
    }
}

impl PartialOrd for ExpirationEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// TTL manager using a min-heap for efficient expiration tracking
pub struct TtlManager {
    /// Min-heap of expiration entries
    heap: RwLock<BinaryHeap<ExpirationEntry>>,
}

impl TtlManager {
    /// Create a new TTL manager
    pub fn new() -> Self {
        Self {
            heap: RwLock::new(BinaryHeap::new()),
        }
    }

    /// Add a key with expiration time
    pub fn add(&self, key: String, expires_at: DateTime<Utc>) {
        let entry = ExpirationEntry { key, expires_at };
        self.heap.write().push(entry);
    }

    /// Remove a key from expiration tracking
    pub fn remove(&self, key: &str) {
        let mut heap = self.heap.write();
        // Note: This is O(n) but acceptable for our use case
        // In production, consider using a more sophisticated data structure
        let entries: Vec<_> = heap.drain()
            .filter(|entry| entry.key != key)
            .collect();
        *heap = entries.into_iter().collect();
    }

    /// Update expiration time for a key
    pub fn update(&self, key: String, expires_at: DateTime<Utc>) {
        self.remove(&key);
        self.add(key, expires_at);
    }

    /// Get all expired keys up to the current time
    pub fn get_expired_keys(&self) -> Vec<String> {
        let now = Utc::now();
        let mut expired = Vec::new();
        let mut heap = self.heap.write();

        while let Some(entry) = heap.peek() {
            if entry.expires_at <= now {
                expired.push(heap.pop().unwrap().key);
            } else {
                break;
            }
        }

        expired
    }

    /// Get the next expiration time (if any)
    pub fn next_expiration(&self) -> Option<DateTime<Utc>> {
        self.heap.read().peek().map(|entry| entry.expires_at)
    }

    /// Get the number of tracked keys
    pub fn len(&self) -> usize {
        self.heap.read().len()
    }

    /// Check if the manager is empty
    pub fn is_empty(&self) -> bool {
        self.heap.read().is_empty()
    }

    /// Clear all entries
    pub fn clear(&self) {
        self.heap.write().clear();
    }
}

impl Default for TtlManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ttl_manager_add_and_get() {
        let manager = TtlManager::new();
        
        let now = Utc::now();
        let future = now + chrono::Duration::seconds(10);
        
        manager.add("key1".to_string(), future);
        assert_eq!(manager.len(), 1);
        assert!(!manager.is_empty());
    }

    #[test]
    fn test_ttl_manager_expiration() {
        let manager = TtlManager::new();
        
        let past = Utc::now() - chrono::Duration::seconds(1);
        let future = Utc::now() + chrono::Duration::seconds(10);
        
        manager.add("expired".to_string(), past);
        manager.add("valid".to_string(), future);
        
        let expired = manager.get_expired_keys();
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0], "expired");
        
        // Only valid key should remain
        assert_eq!(manager.len(), 1);
    }

    #[test]
    fn test_ttl_manager_remove() {
        let manager = TtlManager::new();
        
        let future = Utc::now() + chrono::Duration::seconds(10);
        manager.add("key1".to_string(), future);
        manager.add("key2".to_string(), future);
        
        assert_eq!(manager.len(), 2);
        
        manager.remove("key1");
        assert_eq!(manager.len(), 1);
    }

    #[test]
    fn test_ttl_manager_update() {
        let manager = TtlManager::new();
        
        let time1 = Utc::now() + chrono::Duration::seconds(5);
        let time2 = Utc::now() + chrono::Duration::seconds(10);
        
        manager.add("key1".to_string(), time1);
        manager.update("key1".to_string(), time2);
        
        // Should still have one entry
        assert_eq!(manager.len(), 1);
    }

    #[test]
    fn test_ttl_manager_next_expiration() {
        let manager = TtlManager::new();
        
        assert!(manager.next_expiration().is_none());
        
        let time1 = Utc::now() + chrono::Duration::seconds(5);
        let time2 = Utc::now() + chrono::Duration::seconds(10);
        
        manager.add("key1".to_string(), time2);
        manager.add("key2".to_string(), time1);
        
        // Should return the earliest expiration
        let next = manager.next_expiration().unwrap();
        assert!(next <= time1 + chrono::Duration::milliseconds(100));
    }

    #[test]
    fn test_ttl_manager_clear() {
        let manager = TtlManager::new();
        
        let future = Utc::now() + chrono::Duration::seconds(10);
        manager.add("key1".to_string(), future);
        manager.add("key2".to_string(), future);
        
        assert_eq!(manager.len(), 2);
        
        manager.clear();
        assert_eq!(manager.len(), 0);
        assert!(manager.is_empty());
    }

    #[test]
    fn test_expiration_entry_ordering() {
        let now = Utc::now();
        let entry1 = ExpirationEntry {
            key: "key1".to_string(),
            expires_at: now + chrono::Duration::seconds(5),
        };
        let entry2 = ExpirationEntry {
            key: "key2".to_string(),
            expires_at: now + chrono::Duration::seconds(10),
        };
        
        // Earlier expiration should be "greater" for min-heap
        assert!(entry1 > entry2);
    }
}
