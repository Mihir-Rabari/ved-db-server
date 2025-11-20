//! Redis-compatible data structures for the cache layer

use crate::document::Value;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Cached value with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedValue {
    /// The actual value
    pub value: CacheData,
    /// Time-to-live expiration timestamp
    pub expires_at: Option<DateTime<Utc>>,
    /// Last access timestamp (for LRU)
    pub last_accessed: DateTime<Utc>,
    /// Access count (for LFU)
    pub access_count: u64,
    /// Size in bytes (approximate)
    pub size_bytes: usize,
}

impl CachedValue {
    /// Create a new cached value
    pub fn new(value: CacheData) -> Self {
        let size_bytes = value.size_bytes();
        Self {
            value,
            expires_at: None,
            last_accessed: Utc::now(),
            access_count: 0,
            size_bytes,
        }
    }

    /// Create a cached value with TTL
    pub fn with_ttl(value: CacheData, ttl_seconds: u64) -> Self {
        let size_bytes = value.size_bytes();
        let expires_at = Utc::now() + chrono::Duration::seconds(ttl_seconds as i64);
        Self {
            value,
            expires_at: Some(expires_at),
            last_accessed: Utc::now(),
            access_count: 0,
            size_bytes,
        }
    }

    /// Check if the value has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    /// Update access metadata
    pub fn mark_accessed(&mut self) {
        self.last_accessed = Utc::now();
        self.access_count += 1;
    }

    /// Update TTL
    pub fn update_ttl(&mut self, ttl_seconds: u64) {
        self.expires_at = Some(Utc::now() + chrono::Duration::seconds(ttl_seconds as i64));
    }

    /// Remove TTL (make persistent)
    pub fn persist(&mut self) {
        self.expires_at = None;
    }
}

/// Cache data types (Redis-compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheData {
    /// Simple key-value (String)
    String(Value),
    /// List (ordered collection)
    List(CacheList),
    /// Set (unordered unique collection)
    Set(CacheSet),
    /// Sorted Set (ordered by score)
    SortedSet(CacheSortedSet),
    /// Hash (field-value map)
    Hash(CacheHash),
}

impl CacheData {
    /// Calculate approximate size in bytes
    pub fn size_bytes(&self) -> usize {
        match self {
            CacheData::String(v) => v.size_bytes(),
            CacheData::List(l) => l.size_bytes(),
            CacheData::Set(s) => s.size_bytes(),
            CacheData::SortedSet(ss) => ss.size_bytes(),
            CacheData::Hash(h) => h.size_bytes(),
        }
    }
}

/// List data structure (Redis LIST)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheList {
    items: VecDeque<Value>,
}

impl CacheList {
    /// Create a new empty list
    pub fn new() -> Self {
        Self {
            items: VecDeque::new(),
        }
    }

    /// Push value to the left (head)
    pub fn lpush(&mut self, value: Value) {
        self.items.push_front(value);
    }

    /// Push value to the right (tail)
    pub fn rpush(&mut self, value: Value) {
        self.items.push_back(value);
    }

    /// Pop value from the left (head)
    pub fn lpop(&mut self) -> Option<Value> {
        self.items.pop_front()
    }

    /// Pop value from the right (tail)
    pub fn rpop(&mut self) -> Option<Value> {
        self.items.pop_back()
    }

    /// Get value at index
    pub fn lindex(&self, index: i64) -> Option<&Value> {
        let idx = if index < 0 {
            let len = self.items.len() as i64;
            (len + index) as usize
        } else {
            index as usize
        };
        self.items.get(idx)
    }

    /// Get list length
    pub fn llen(&self) -> usize {
        self.items.len()
    }

    /// Get range of values
    pub fn lrange(&self, start: i64, stop: i64) -> Vec<Value> {
        let len = self.items.len() as i64;
        let start_idx = if start < 0 { (len + start).max(0) } else { start.min(len) } as usize;
        let stop_idx = if stop < 0 { (len + stop + 1).max(0) } else { (stop + 1).min(len) } as usize;
        
        self.items.range(start_idx..stop_idx).cloned().collect()
    }

    /// Calculate size in bytes
    pub fn size_bytes(&self) -> usize {
        self.items.iter().map(|v| v.size_bytes()).sum::<usize>() + 16
    }
}

impl Default for CacheList {
    fn default() -> Self {
        Self::new()
    }
}

/// Set data structure (Redis SET)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheSet {
    items: BTreeSet<String>, // Using String for simplicity
}

impl CacheSet {
    /// Create a new empty set
    pub fn new() -> Self {
        Self {
            items: BTreeSet::new(),
        }
    }

    /// Add member to set
    pub fn sadd(&mut self, member: String) -> bool {
        self.items.insert(member)
    }

    /// Remove member from set
    pub fn srem(&mut self, member: &str) -> bool {
        self.items.remove(member)
    }

    /// Check if member exists
    pub fn sismember(&self, member: &str) -> bool {
        self.items.contains(member)
    }

    /// Get all members
    pub fn smembers(&self) -> Vec<String> {
        self.items.iter().cloned().collect()
    }

    /// Get set cardinality (size)
    pub fn scard(&self) -> usize {
        self.items.len()
    }

    /// Calculate size in bytes
    pub fn size_bytes(&self) -> usize {
        self.items.iter().map(|s| s.len()).sum::<usize>() + 16
    }
}

impl Default for CacheSet {
    fn default() -> Self {
        Self::new()
    }
}

/// Sorted Set data structure (Redis ZSET)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheSortedSet {
    items: BTreeMap<String, f64>, // member -> score
}

impl CacheSortedSet {
    /// Create a new empty sorted set
    pub fn new() -> Self {
        Self {
            items: BTreeMap::new(),
        }
    }

    /// Add member with score
    pub fn zadd(&mut self, member: String, score: f64) -> bool {
        self.items.insert(member, score).is_none()
    }

    /// Remove member
    pub fn zrem(&mut self, member: &str) -> bool {
        self.items.remove(member).is_some()
    }

    /// Get score of member
    pub fn zscore(&self, member: &str) -> Option<f64> {
        self.items.get(member).copied()
    }

    /// Get cardinality (size)
    pub fn zcard(&self) -> usize {
        self.items.len()
    }

    /// Get range by rank (sorted by score)
    pub fn zrange(&self, start: i64, stop: i64) -> Vec<(String, f64)> {
        let mut sorted: Vec<_> = self.items.iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let len = sorted.len() as i64;
        let start_idx = if start < 0 { (len + start).max(0) } else { start.min(len) } as usize;
        let stop_idx = if stop < 0 { (len + stop + 1).max(0) } else { (stop + 1).min(len) } as usize;

        sorted[start_idx..stop_idx].to_vec()
    }

    /// Get range by score
    pub fn zrangebyscore(&self, min: f64, max: f64) -> Vec<(String, f64)> {
        let mut result: Vec<_> = self.items.iter()
            .filter(|(_, score)| **score >= min && **score <= max)
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        result.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        result
    }

    /// Calculate size in bytes
    pub fn size_bytes(&self) -> usize {
        self.items.iter().map(|(k, _)| k.len() + 8).sum::<usize>() + 16
    }
}

impl Default for CacheSortedSet {
    fn default() -> Self {
        Self::new()
    }
}

/// Hash data structure (Redis HASH)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheHash {
    fields: BTreeMap<String, Value>,
}

impl CacheHash {
    /// Create a new empty hash
    pub fn new() -> Self {
        Self {
            fields: BTreeMap::new(),
        }
    }

    /// Set field value
    pub fn hset(&mut self, field: String, value: Value) -> bool {
        self.fields.insert(field, value).is_none()
    }

    /// Get field value
    pub fn hget(&self, field: &str) -> Option<&Value> {
        self.fields.get(field)
    }

    /// Delete field
    pub fn hdel(&mut self, field: &str) -> bool {
        self.fields.remove(field).is_some()
    }

    /// Check if field exists
    pub fn hexists(&self, field: &str) -> bool {
        self.fields.contains_key(field)
    }

    /// Get all fields
    pub fn hkeys(&self) -> Vec<String> {
        self.fields.keys().cloned().collect()
    }

    /// Get all values
    pub fn hvals(&self) -> Vec<Value> {
        self.fields.values().cloned().collect()
    }

    /// Get all field-value pairs
    pub fn hgetall(&self) -> BTreeMap<String, Value> {
        self.fields.clone()
    }

    /// Get number of fields
    pub fn hlen(&self) -> usize {
        self.fields.len()
    }

    /// Calculate size in bytes
    pub fn size_bytes(&self) -> usize {
        self.fields.iter()
            .map(|(k, v)| k.len() + v.size_bytes())
            .sum::<usize>() + 16
    }
}

impl Default for CacheHash {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_value_expiration() {
        let value = CacheData::String(Value::String("test".to_string()));
        let mut cached = CachedValue::with_ttl(value, 1);
        
        assert!(!cached.is_expired());
        
        // Simulate expiration
        cached.expires_at = Some(Utc::now() - chrono::Duration::seconds(1));
        assert!(cached.is_expired());
    }

    #[test]
    fn test_cached_value_access_tracking() {
        let value = CacheData::String(Value::String("test".to_string()));
        let mut cached = CachedValue::new(value);
        
        assert_eq!(cached.access_count, 0);
        cached.mark_accessed();
        assert_eq!(cached.access_count, 1);
        cached.mark_accessed();
        assert_eq!(cached.access_count, 2);
    }

    #[test]
    fn test_list_operations() {
        let mut list = CacheList::new();
        
        list.rpush(Value::Int32(1));
        list.rpush(Value::Int32(2));
        list.lpush(Value::Int32(0));
        
        assert_eq!(list.llen(), 3);
        assert_eq!(list.lindex(0).unwrap().as_i64(), Some(0));
        assert_eq!(list.lindex(1).unwrap().as_i64(), Some(1));
        assert_eq!(list.lindex(2).unwrap().as_i64(), Some(2));
        
        assert_eq!(list.lpop().unwrap().as_i64(), Some(0));
        assert_eq!(list.rpop().unwrap().as_i64(), Some(2));
        assert_eq!(list.llen(), 1);
    }

    #[test]
    fn test_list_range() {
        let mut list = CacheList::new();
        for i in 0..5 {
            list.rpush(Value::Int32(i));
        }
        
        let range = list.lrange(1, 3);
        assert_eq!(range.len(), 3);
        assert_eq!(range[0].as_i64(), Some(1));
        assert_eq!(range[2].as_i64(), Some(3));
    }

    #[test]
    fn test_set_operations() {
        let mut set = CacheSet::new();
        
        assert!(set.sadd("a".to_string()));
        assert!(set.sadd("b".to_string()));
        assert!(!set.sadd("a".to_string())); // Duplicate
        
        assert_eq!(set.scard(), 2);
        assert!(set.sismember("a"));
        assert!(!set.sismember("c"));
        
        assert!(set.srem("a"));
        assert!(!set.srem("a")); // Already removed
        assert_eq!(set.scard(), 1);
    }

    #[test]
    fn test_sorted_set_operations() {
        let mut zset = CacheSortedSet::new();
        
        zset.zadd("alice".to_string(), 100.0);
        zset.zadd("bob".to_string(), 85.0);
        zset.zadd("charlie".to_string(), 95.0);
        
        assert_eq!(zset.zcard(), 3);
        assert_eq!(zset.zscore("alice"), Some(100.0));
        
        let range = zset.zrange(0, 1);
        assert_eq!(range.len(), 2);
        assert_eq!(range[0].0, "bob"); // Lowest score
        assert_eq!(range[1].0, "charlie");
        
        let by_score = zset.zrangebyscore(90.0, 100.0);
        assert_eq!(by_score.len(), 2);
    }

    #[test]
    fn test_hash_operations() {
        let mut hash = CacheHash::new();
        
        hash.hset("name".to_string(), Value::String("John".to_string()));
        hash.hset("age".to_string(), Value::Int32(30));
        
        assert_eq!(hash.hlen(), 2);
        assert!(hash.hexists("name"));
        assert!(!hash.hexists("email"));
        
        assert_eq!(hash.hget("name").unwrap().as_str(), Some("John"));
        
        let keys = hash.hkeys();
        assert_eq!(keys.len(), 2);
        
        assert!(hash.hdel("age"));
        assert_eq!(hash.hlen(), 1);
    }

    #[test]
    fn test_size_calculations() {
        let list = CacheList::new();
        assert!(list.size_bytes() > 0);
        
        let set = CacheSet::new();
        assert!(set.size_bytes() > 0);
        
        let zset = CacheSortedSet::new();
        assert!(zset.size_bytes() > 0);
        
        let hash = CacheHash::new();
        assert!(hash.size_bytes() > 0);
    }
}
