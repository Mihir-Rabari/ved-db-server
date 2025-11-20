//! Main cache layer implementation

use crate::cache::data_structures::{CacheData, CachedValue};
use crate::cache::eviction::{EvictionPolicy, EvictionStats};
use crate::cache::ttl_manager::TtlManager;
use crate::document::Value;
use anyhow::Result;
use dashmap::DashMap;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

/// Cache layer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Maximum cache size in bytes
    pub max_size_bytes: usize,
    /// Eviction policy
    pub eviction_policy: EvictionPolicy,
    /// Default TTL in seconds (None = no default TTL)
    pub default_ttl: Option<u64>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_size_bytes: 100 * 1024 * 1024, // 100 MB
            eviction_policy: EvictionPolicy::LRU,
            default_ttl: None,
        }
    }
}

/// Cache layer using DashMap for concurrent access
pub struct CacheLayer {
    /// Cache storage (key -> value)
    cache: Arc<DashMap<String, CachedValue>>,
    /// TTL manager for expiration tracking
    ttl_manager: Arc<TtlManager>,
    /// Configuration
    config: CacheConfig,
    /// Current cache size in bytes
    current_size: AtomicUsize,
    /// Cache statistics
    stats: CacheStats,
}

impl CacheLayer {
    /// Create a new cache layer
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
            ttl_manager: Arc::new(TtlManager::new()),
            config,
            current_size: AtomicUsize::new(0),
            stats: CacheStats::default(),
        }
    }

    /// Create a cache layer with default configuration
    pub fn with_defaults() -> Self {
        Self::new(CacheConfig::default())
    }

    /// Get a value from cache
    pub fn get(&self, key: &str) -> Option<CacheData> {
        // Clean up expired keys first
        self.evict_expired();

        if let Some(mut entry) = self.cache.get_mut(key) {
            if entry.is_expired() {
                drop(entry);
                self.remove(key);
                self.stats.record_miss();
                return None;
            }

            entry.mark_accessed();
            self.stats.record_hit();
            Some(entry.value.clone())
        } else {
            self.stats.record_miss();
            None
        }
    }

    /// Set a value in cache
    pub fn set(&self, key: String, value: CacheData) -> Result<()> {
        self.set_with_ttl(key, value, self.config.default_ttl)
    }

    /// Set a value with specific TTL
    pub fn set_with_ttl(&self, key: String, value: CacheData, ttl: Option<u64>) -> Result<()> {
        let cached_value = if let Some(ttl_seconds) = ttl {
            let cached = CachedValue::with_ttl(value, ttl_seconds);
            self.ttl_manager.add(key.clone(), cached.expires_at.unwrap());
            cached
        } else {
            CachedValue::new(value)
        };

        let size = cached_value.size_bytes;

        // Check if we need to evict
        self.ensure_space(size)?;

        // Remove old value if exists
        if let Some(old) = self.cache.get(&key) {
            self.current_size.fetch_sub(old.size_bytes, Ordering::Relaxed);
        }

        // Insert new value
        self.cache.insert(key, cached_value);
        self.current_size.fetch_add(size, Ordering::Relaxed);
        self.stats.record_set();

        Ok(())
    }

    /// Remove a value from cache
    pub fn remove(&self, key: &str) -> bool {
        if let Some((_, value)) = self.cache.remove(key) {
            self.current_size.fetch_sub(value.size_bytes, Ordering::Relaxed);
            self.ttl_manager.remove(key);
            true
        } else {
            false
        }
    }

    /// Check if a key exists
    pub fn exists(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Update TTL for a key
    pub fn expire(&self, key: &str, ttl_seconds: u64) -> bool {
        if let Some(mut entry) = self.cache.get_mut(key) {
            entry.update_ttl(ttl_seconds);
            self.ttl_manager.update(key.to_string(), entry.expires_at.unwrap());
            true
        } else {
            false
        }
    }

    /// Remove TTL from a key (make it persistent)
    pub fn persist(&self, key: &str) -> bool {
        if let Some(mut entry) = self.cache.get_mut(key) {
            entry.persist();
            self.ttl_manager.remove(key);
            true
        } else {
            false
        }
    }

    /// Get TTL for a key (in seconds)
    pub fn ttl(&self, key: &str) -> Option<i64> {
        if let Some(entry) = self.cache.get(key) {
            if let Some(expires_at) = entry.expires_at {
                let now = chrono::Utc::now();
                let ttl = expires_at.signed_duration_since(now);
                Some(ttl.num_seconds())
            } else {
                Some(-1) // No expiration
            }
        } else {
            None // Key doesn't exist
        }
    }

    /// Clear all cache entries
    pub fn clear(&self) {
        self.cache.clear();
        self.ttl_manager.clear();
        self.current_size.store(0, Ordering::Relaxed);
    }

    /// Get cache size in bytes
    pub fn size_bytes(&self) -> usize {
        self.current_size.load(Ordering::Relaxed)
    }

    /// Get number of keys in cache
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Get cache statistics
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Evict expired keys
    fn evict_expired(&self) {
        let expired_keys = self.ttl_manager.get_expired_keys();
        for key in expired_keys {
            if let Some((_, value)) = self.cache.remove(&key) {
                self.current_size.fetch_sub(value.size_bytes, Ordering::Relaxed);
                self.stats.eviction_stats.lock().record_eviction(value.size_bytes, true);
            }
        }
    }

    /// Ensure there's enough space for a new value
    fn ensure_space(&self, required_size: usize) -> Result<()> {
        let current = self.current_size.load(Ordering::Relaxed);
        let max_size = self.config.max_size_bytes;

        if current + required_size <= max_size {
            return Ok(());
        }

        // Need to evict
        if self.config.eviction_policy == EvictionPolicy::NoEviction {
            anyhow::bail!("Cache is full and eviction is disabled");
        }

        // Calculate how much to evict (evict 20% more than needed)
        let target_size = (current + required_size) - max_size;
        let evict_size = (target_size as f64 * 1.2) as usize;

        self.evict_by_policy(evict_size)?;

        Ok(())
    }

    /// Evict entries based on policy
    fn evict_by_policy(&self, target_bytes: usize) -> Result<()> {
        // Collect candidates
        let candidates: Vec<_> = self.cache.iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();

        if candidates.is_empty() {
            return Ok(());
        }

        // Select victims
        let candidate_refs: Vec<_> = candidates.iter()
            .map(|(k, v)| (k, v))
            .collect();

        let victims = self.config.eviction_policy.select_victims(
            candidate_refs,
            candidates.len(), // Consider all candidates
        );

        // Evict until we've freed enough space
        let mut evicted_bytes = 0;
        for key in victims {
            if evicted_bytes >= target_bytes {
                break;
            }

            if let Some((_, value)) = self.cache.remove(&key) {
                evicted_bytes += value.size_bytes;
                self.current_size.fetch_sub(value.size_bytes, Ordering::Relaxed);
                self.ttl_manager.remove(&key);
                self.stats.eviction_stats.lock().record_eviction(value.size_bytes, false);
            }
        }

        Ok(())
    }

    /// Evict a percentage of cache entries (for manual cache management)
    pub fn evict_percentage(&self, percentage: f64) -> Result<usize> {
        let percentage = percentage.clamp(0.0, 1.0);
        let current = self.current_size.load(Ordering::Relaxed);
        let target_bytes = (current as f64 * percentage) as usize;

        let before_count = self.len();
        self.evict_by_policy(target_bytes)?;
        let after_count = self.len();

        Ok(before_count - after_count)
    }
}

/// Cache statistics
#[derive(Debug)]
pub struct CacheStats {
    /// Total cache hits
    hits: AtomicU64,
    /// Total cache misses
    misses: AtomicU64,
    /// Total set operations
    sets: AtomicU64,
    /// Eviction statistics
    eviction_stats: Mutex<EvictionStats>,
}

impl Default for CacheStats {
    fn default() -> Self {
        Self {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            sets: AtomicU64::new(0),
            eviction_stats: Mutex::new(EvictionStats::default()),
        }
    }
}

impl CacheStats {
    /// Record a cache hit
    fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache miss
    fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a set operation
    fn record_set(&self) {
        self.sets.fetch_add(1, Ordering::Relaxed);
    }

    /// Get total hits
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// Get total misses
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    /// Get total sets
    pub fn sets(&self) -> u64 {
        self.sets.load(Ordering::Relaxed)
    }

    /// Calculate hit rate
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits();
        let total = hits + self.misses();
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    /// Get eviction statistics
    pub fn eviction_stats(&self) -> EvictionStats {
        self.eviction_stats.lock().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_layer_basic_operations() {
        let cache = CacheLayer::with_defaults();

        let key = "test_key".to_string();
        let value = CacheData::String(Value::String("test_value".to_string()));

        // Set and get
        cache.set(key.clone(), value.clone()).unwrap();
        assert!(cache.exists(&key));

        let retrieved = cache.get(&key).unwrap();
        match retrieved {
            CacheData::String(v) => assert_eq!(v.as_str(), Some("test_value")),
            _ => panic!("Wrong type"),
        }

        // Remove
        assert!(cache.remove(&key));
        assert!(!cache.exists(&key));
    }

    #[test]
    fn test_cache_layer_ttl() {
        let cache = CacheLayer::with_defaults();

        let key = "ttl_key".to_string();
        let value = CacheData::String(Value::String("test".to_string()));

        // Set with 2 second TTL
        cache.set_with_ttl(key.clone(), value, Some(2)).unwrap();
        assert!(cache.exists(&key));

        // Check TTL (should be close to 2 seconds)
        let ttl = cache.ttl(&key).unwrap();
        assert!(ttl > 0 && ttl <= 2);

        // Wait for expiration
        std::thread::sleep(std::time::Duration::from_millis(2100));

        // Should be expired
        assert!(!cache.exists(&key));
    }

    #[test]
    fn test_cache_layer_persist() {
        let cache = CacheLayer::with_defaults();

        let key = "persist_key".to_string();
        let value = CacheData::String(Value::String("test".to_string()));

        // Set with TTL
        cache.set_with_ttl(key.clone(), value, Some(10)).unwrap();
        assert!(cache.ttl(&key).unwrap() > 0);

        // Persist (remove TTL)
        cache.persist(&key);
        assert_eq!(cache.ttl(&key).unwrap(), -1);
    }

    #[test]
    fn test_cache_layer_expire() {
        let cache = CacheLayer::with_defaults();

        let key = "expire_key".to_string();
        let value = CacheData::String(Value::String("test".to_string()));

        // Set without TTL
        cache.set(key.clone(), value).unwrap();
        assert_eq!(cache.ttl(&key).unwrap(), -1);

        // Add expiration
        cache.expire(&key, 10);
        assert!(cache.ttl(&key).unwrap() > 0);
    }

    #[test]
    fn test_cache_layer_size_tracking() {
        let cache = CacheLayer::with_defaults();

        assert_eq!(cache.size_bytes(), 0);
        assert_eq!(cache.len(), 0);

        let value = CacheData::String(Value::String("test".to_string()));
        cache.set("key1".to_string(), value.clone()).unwrap();

        assert!(cache.size_bytes() > 0);
        assert_eq!(cache.len(), 1);

        cache.set("key2".to_string(), value).unwrap();
        assert_eq!(cache.len(), 2);

        cache.clear();
        assert_eq!(cache.size_bytes(), 0);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_cache_layer_stats() {
        let cache = CacheLayer::with_defaults();

        let value = CacheData::String(Value::String("test".to_string()));
        cache.set("key1".to_string(), value).unwrap();

        // Hit
        cache.get("key1");
        assert_eq!(cache.stats().hits(), 1);

        // Miss
        cache.get("nonexistent");
        assert_eq!(cache.stats().misses(), 1);

        // Hit rate
        assert_eq!(cache.stats().hit_rate(), 0.5);
    }

    #[test]
    fn test_cache_layer_eviction() {
        let config = CacheConfig {
            max_size_bytes: 1000, // Small cache
            eviction_policy: EvictionPolicy::LRU,
            default_ttl: None,
        };
        let cache = CacheLayer::new(config);

        // Fill cache
        for i in 0..10 {
            let key = format!("key{}", i);
            let value = CacheData::String(Value::String("x".repeat(200)));
            cache.set(key, value).unwrap();
        }

        // Cache should have evicted some entries
        assert!(cache.len() < 10);
    }

    #[test]
    fn test_cache_layer_evict_percentage() {
        let cache = CacheLayer::with_defaults();

        // Add some entries
        for i in 0..10 {
            let key = format!("key{}", i);
            let value = CacheData::String(Value::String("test".to_string()));
            cache.set(key, value).unwrap();
        }

        let initial_count = cache.len();
        assert_eq!(initial_count, 10);

        // Evict 50%
        let evicted = cache.evict_percentage(0.5).unwrap();
        assert!(evicted > 0);
        assert!(cache.len() < initial_count);
    }

    #[test]
    fn test_no_eviction_policy() {
        let config = CacheConfig {
            max_size_bytes: 100,
            eviction_policy: EvictionPolicy::NoEviction,
            default_ttl: None,
        };
        let cache = CacheLayer::new(config);

        let value = CacheData::String(Value::String("x".repeat(50)));
        
        // First set should work
        cache.set("key1".to_string(), value.clone()).unwrap();

        // Second set should work
        cache.set("key2".to_string(), value.clone()).unwrap();

        // Third set should fail (cache full, no eviction)
        let result = cache.set("key3".to_string(), value);
        assert!(result.is_err());
    }
}
