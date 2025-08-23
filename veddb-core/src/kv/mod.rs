//! Key-Value store implementation with sharding
//! 
//! Provides a sharded hash table for high-performance concurrent access.
//! Each shard is independently lockable to reduce contention.

pub mod shard;
pub mod hash_table;

pub use shard::*;
pub use hash_table::*;

use crate::arena::Arena;
use std::sync::atomic::{AtomicU64, Ordering};

/// KV store configuration
#[derive(Debug, Clone)]
pub struct KvConfig {
    pub num_shards: usize,
    pub initial_capacity_per_shard: usize,
    pub max_key_size: usize,
    pub max_value_size: usize,
}

impl Default for KvConfig {
    fn default() -> Self {
        Self {
            num_shards: 16,
            initial_capacity_per_shard: 1024,
            max_key_size: 1024,
            max_value_size: 1024 * 1024, // 1MB
        }
    }
}

/// Main KV store with multiple shards
#[repr(C)]
pub struct KvStore {
    /// Number of shards (power of 2)
    num_shards: u64,
    /// Mask for shard selection (num_shards - 1)
    shard_mask: u64,
    /// Global statistics
    total_operations: AtomicU64,
    total_keys: AtomicU64,
    // Shards follow this structure in memory
}

impl KvStore {
    /// Calculate size needed for KV store with given config
    pub fn size_for_config(config: &KvConfig) -> usize {
        assert!(config.num_shards.is_power_of_two(), "Number of shards must be power of 2");
        
        std::mem::size_of::<Self>() + 
        config.num_shards * KvShard::size_for_capacity(config.initial_capacity_per_shard)
    }
    
    /// Initialize KV store in shared memory
    /// 
    /// # Safety
    /// - ptr must point to valid memory of sufficient size
    /// - arena must be valid and initialized
    pub unsafe fn init(
        ptr: *mut u8, 
        config: &KvConfig,
        arena: *mut Arena
    ) -> *mut Self {
        assert!(config.num_shards.is_power_of_two());
        
        let kv_store = ptr as *mut Self;
        
        std::ptr::write(
            kv_store,
            Self {
                num_shards: config.num_shards as u64,
                shard_mask: (config.num_shards - 1) as u64,
                total_operations: AtomicU64::new(0),
                total_keys: AtomicU64::new(0),
            }
        );
        
        // Initialize shards
        let mut shard_ptr = ptr.add(std::mem::size_of::<Self>());
        for _ in 0..config.num_shards {
            KvShard::init(shard_ptr, config.initial_capacity_per_shard, arena);
            shard_ptr = shard_ptr.add(KvShard::size_for_capacity(config.initial_capacity_per_shard));
        }
        
        kv_store
    }
    
    /// Get shard for a given key
    fn get_shard(&self, key: &[u8]) -> &KvShard {
        let hash = self.hash_key(key);
        let shard_idx = hash & self.shard_mask;
        
        unsafe {
            let shard_ptr = self.shard_ptr(shard_idx as usize);
            &*shard_ptr
        }
    }
    
    /// Get mutable shard for a given key
    fn get_shard_mut(&self, key: &[u8]) -> &mut KvShard {
        let hash = self.hash_key(key);
        let shard_idx = hash & self.shard_mask;
        
        unsafe {
            let shard_ptr = self.shard_ptr(shard_idx as usize);
            &mut *shard_ptr
        }
    }
    
    /// Get pointer to shard at given index
    unsafe fn shard_ptr(&self, index: usize) -> *mut KvShard {
        let self_ptr = self as *const Self as *mut u8;
        let shard_size = KvShard::size_for_capacity(1024); // This should be stored in config
        self_ptr.add(std::mem::size_of::<Self>() + index * shard_size) as *mut KvShard
    }
    
    /// Hash a key using a fast hash function
    fn hash_key(&self, key: &[u8]) -> u64 {
        // Simple FNV-1a hash - in production you might want xxhash or similar
        let mut hash = 0xcbf29ce484222325u64;
        for &byte in key {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }
    
    /// Set a key-value pair
    pub fn set(&self, key: &[u8], value: &[u8]) -> Result<(), KvError> {
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        
        let shard = self.get_shard_mut(key);
        let result = shard.set(key, value);
        
        if result.is_ok() {
            self.total_keys.fetch_add(1, Ordering::Relaxed);
        }
        
        result
    }
    
    /// Get a value by key
    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        
        let shard = self.get_shard(key);
        shard.get(key)
    }
    
    /// Delete a key
    pub fn delete(&self, key: &[u8]) -> bool {
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        
        let shard = self.get_shard_mut(key);
        let deleted = shard.delete(key);
        
        if deleted {
            self.total_keys.fetch_sub(1, Ordering::Relaxed);
        }
        
        deleted
    }
    
    /// Compare-and-swap operation
    pub fn cas(&self, key: &[u8], expected_version: u64, new_value: &[u8]) -> Result<u64, KvError> {
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        
        let shard = self.get_shard_mut(key);
        shard.cas(key, expected_version, new_value)
    }
    
    /// Get statistics
    pub fn stats(&self) -> KvStats {
        let mut total_entries = 0;
        let mut total_memory = 0;
        
        for i in 0..self.num_shards {
            unsafe {
                let shard = &*self.shard_ptr(i as usize);
                let shard_stats = shard.stats();
                total_entries += shard_stats.entry_count;
                total_memory += shard_stats.memory_used;
            }
        }
        
        KvStats {
            total_operations: self.total_operations.load(Ordering::Relaxed),
            total_keys: self.total_keys.load(Ordering::Relaxed),
            total_entries,
            total_memory_used: total_memory,
            num_shards: self.num_shards,
        }
    }
}

/// KV store statistics
#[derive(Debug, Clone)]
pub struct KvStats {
    pub total_operations: u64,
    pub total_keys: u64,
    pub total_entries: u64,
    pub total_memory_used: u64,
    pub num_shards: u64,
}

/// KV operation errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KvError {
    KeyTooLarge,
    ValueTooLarge,
    OutOfMemory,
    VersionMismatch,
    NotFound,
}

impl std::fmt::Display for KvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KvError::KeyTooLarge => write!(f, "Key too large"),
            KvError::ValueTooLarge => write!(f, "Value too large"),
            KvError::OutOfMemory => write!(f, "Out of memory"),
            KvError::VersionMismatch => write!(f, "Version mismatch"),
            KvError::NotFound => write!(f, "Key not found"),
        }
    }
}

impl std::error::Error for KvError {}
