//! Cache eviction policies

use crate::cache::data_structures::CachedValue;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Cache eviction policy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvictionPolicy {
    /// Least Recently Used
    LRU,
    /// Least Frequently Used
    LFU,
    /// Time-To-Live based (evict expired first)
    TTL,
    /// No eviction (fail when full)
    NoEviction,
}

impl EvictionPolicy {
    /// Calculate eviction score for a cached value
    /// Lower score = higher priority for eviction
    pub fn calculate_score(&self, value: &CachedValue, now: DateTime<Utc>) -> f64 {
        match self {
            EvictionPolicy::LRU => {
                // Score based on last access time (older = lower score)
                let age = now.signed_duration_since(value.last_accessed);
                age.num_seconds() as f64
            }
            EvictionPolicy::LFU => {
                // Score based on access count (less frequent = lower score)
                value.access_count as f64
            }
            EvictionPolicy::TTL => {
                // Score based on expiration time (sooner expiration = lower score)
                if let Some(expires_at) = value.expires_at {
                    let ttl = expires_at.signed_duration_since(now);
                    ttl.num_seconds() as f64
                } else {
                    // No TTL = highest priority to keep
                    f64::MAX
                }
            }
            EvictionPolicy::NoEviction => {
                // All items have equal priority (no eviction)
                0.0
            }
        }
    }

    /// Select keys to evict from a set of candidates
    pub fn select_victims<'a>(
        &self,
        candidates: Vec<(&'a String, &'a CachedValue)>,
        count: usize,
    ) -> Vec<String> {
        if *self == EvictionPolicy::NoEviction {
            return Vec::new();
        }

        let now = Utc::now();
        let mut scored: Vec<_> = candidates
            .into_iter()
            .map(|(key, value)| {
                let score = self.calculate_score(value, now);
                (key.clone(), score)
            })
            .collect();

        // Sort by score
        // For LRU: higher score = older = higher priority to evict (sort descending)
        // For LFU: lower score = less frequent = higher priority to evict (sort ascending)
        // For TTL: lower score = sooner expiration = higher priority to evict (sort ascending)
        match self {
            EvictionPolicy::LRU => {
                // Sort descending (highest scores/oldest first)
                scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            }
            EvictionPolicy::LFU | EvictionPolicy::TTL => {
                // Sort ascending (lowest scores first)
                scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            }
            EvictionPolicy::NoEviction => {}
        }

        // Take the requested number of victims
        scored.into_iter()
            .take(count)
            .map(|(key, _)| key)
            .collect()
    }
}

/// Eviction statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvictionStats {
    /// Total number of evictions
    pub total_evictions: u64,
    /// Evictions by policy
    pub evictions_by_policy: u64,
    /// Evictions by TTL expiration
    pub evictions_by_ttl: u64,
    /// Total bytes evicted
    pub bytes_evicted: u64,
}

impl EvictionStats {
    /// Record an eviction
    pub fn record_eviction(&mut self, size_bytes: usize, by_ttl: bool) {
        self.total_evictions += 1;
        self.bytes_evicted += size_bytes as u64;
        
        if by_ttl {
            self.evictions_by_ttl += 1;
        } else {
            self.evictions_by_policy += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::data_structures::CacheData;
    use crate::document::Value;

    fn create_test_value(access_count: u64, seconds_ago: i64) -> CachedValue {
        let data = CacheData::String(Value::String("test".to_string()));
        let mut cached = CachedValue::new(data);
        cached.access_count = access_count;
        cached.last_accessed = Utc::now() - chrono::Duration::seconds(seconds_ago);
        cached
    }

    #[test]
    fn test_lru_scoring() {
        let policy = EvictionPolicy::LRU;
        let now = Utc::now();
        
        let old_value = create_test_value(10, 100);
        let new_value = create_test_value(1, 10);
        
        let old_score = policy.calculate_score(&old_value, now);
        let new_score = policy.calculate_score(&new_value, now);
        
        // Older value should have higher score (higher eviction priority when sorted ascending)
        assert!(old_score > new_score);
    }

    #[test]
    fn test_lfu_scoring() {
        let policy = EvictionPolicy::LFU;
        let now = Utc::now();
        
        let frequent = create_test_value(100, 10);
        let infrequent = create_test_value(5, 10);
        
        let frequent_score = policy.calculate_score(&frequent, now);
        let infrequent_score = policy.calculate_score(&infrequent, now);
        
        // Less frequent value should have lower score (higher eviction priority)
        assert!(infrequent_score < frequent_score);
    }

    #[test]
    fn test_ttl_scoring() {
        let policy = EvictionPolicy::TTL;
        let now = Utc::now();
        
        let data = CacheData::String(Value::String("test".to_string()));
        let soon_expire = CachedValue::with_ttl(data.clone(), 10);
        let later_expire = CachedValue::with_ttl(data, 100);
        
        let soon_score = policy.calculate_score(&soon_expire, now);
        let later_score = policy.calculate_score(&later_expire, now);
        
        // Sooner expiration should have lower score (higher eviction priority)
        assert!(soon_score < later_score);
    }

    #[test]
    fn test_select_victims_lru() {
        let policy = EvictionPolicy::LRU;
        
        let key1 = "key1".to_string();
        let key2 = "key2".to_string();
        let key3 = "key3".to_string();
        
        let val1 = create_test_value(10, 100); // Oldest
        let val2 = create_test_value(10, 50);
        let val3 = create_test_value(10, 10);  // Newest
        
        let candidates = vec![
            (&key1, &val1),
            (&key2, &val2),
            (&key3, &val3),
        ];
        
        let victims = policy.select_victims(candidates, 2);
        assert_eq!(victims.len(), 2);
        assert_eq!(victims[0], "key1"); // Oldest should be first
        assert_eq!(victims[1], "key2"); // Second oldest
    }

    #[test]
    fn test_select_victims_lfu() {
        let policy = EvictionPolicy::LFU;
        
        let key1 = "key1".to_string();
        let key2 = "key2".to_string();
        let key3 = "key3".to_string();
        
        let val1 = create_test_value(5, 10);   // Least frequent
        let val2 = create_test_value(50, 10);
        let val3 = create_test_value(100, 10); // Most frequent
        
        let candidates = vec![
            (&key1, &val1),
            (&key2, &val2),
            (&key3, &val3),
        ];
        
        let victims = policy.select_victims(candidates, 1);
        assert_eq!(victims.len(), 1);
        assert_eq!(victims[0], "key1"); // Least frequent
    }

    #[test]
    fn test_no_eviction_policy() {
        let policy = EvictionPolicy::NoEviction;
        
        let key1 = "key1".to_string();
        let val1 = create_test_value(10, 100);
        
        let candidates = vec![(&key1, &val1)];
        let victims = policy.select_victims(candidates, 10);
        
        assert_eq!(victims.len(), 0); // No eviction
    }

    #[test]
    fn test_eviction_stats() {
        let mut stats = EvictionStats::default();
        
        assert_eq!(stats.total_evictions, 0);
        
        stats.record_eviction(100, false);
        assert_eq!(stats.total_evictions, 1);
        assert_eq!(stats.evictions_by_policy, 1);
        assert_eq!(stats.bytes_evicted, 100);
        
        stats.record_eviction(200, true);
        assert_eq!(stats.total_evictions, 2);
        assert_eq!(stats.evictions_by_ttl, 1);
        assert_eq!(stats.bytes_evicted, 300);
    }
}
