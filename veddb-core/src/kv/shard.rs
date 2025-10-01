//! Individual KV shard implementation
//!
//! Each shard contains a hash table and manages its own locking for thread safety.

use super::{hash_table::HashTable, KvError};
use crate::arena::Arena;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

/// KV shard with its own hash table and lock
#[repr(C)]
pub struct KvShard {
    /// Read-write lock for this shard
    lock: RwLock<()>,
    /// Statistics
    operation_count: AtomicU64,
    entry_count: AtomicU64,
    memory_used: AtomicU64,
    // Hash table follows this structure
}

impl KvShard {
    /// Calculate size needed for shard with given capacity
    pub fn size_for_capacity(capacity: usize) -> usize {
        std::mem::size_of::<Self>() + HashTable::size_for_capacity(capacity)
    }

    /// Initialize shard in shared memory
    ///
    /// # Safety
    /// - ptr must point to valid memory of sufficient size
    /// - arena must be valid and initialized
    pub unsafe fn init(ptr: *mut u8, capacity: usize, arena: *mut Arena) -> *mut Self {
        let shard = ptr as *mut Self;

        std::ptr::write(
            shard,
            Self {
                lock: RwLock::new(()),
                operation_count: AtomicU64::new(0),
                entry_count: AtomicU64::new(0),
                memory_used: AtomicU64::new(0),
            },
        );

        // Initialize hash table
        let hash_table_ptr = ptr.add(std::mem::size_of::<Self>());
        HashTable::init(hash_table_ptr, capacity, arena);

        shard
    }

    /// Get mutable reference to hash table
    fn hash_table(&self) -> &mut HashTable {
        unsafe {
            let self_ptr = self as *const Self as *const u8;
            let hash_table_ptr = self_ptr.add(std::mem::size_of::<Self>()) as *mut u8;
            &mut *(hash_table_ptr as *mut HashTable)
        }
    }

    /// Get mutable reference to hash table (alias of hash_table())
    fn hash_table_mut(&self) -> &mut HashTable {
        self.hash_table()
    }

    /// Set a key-value pair
    pub fn set(&self, key: &[u8], value: &[u8]) -> Result<(), KvError> {
        let _guard = self.lock.write();
        self.operation_count.fetch_add(1, Ordering::Relaxed);

        let hash_table = self.hash_table_mut();
        let was_new = hash_table.insert(key, value)?;

        if was_new {
            self.entry_count.fetch_add(1, Ordering::Relaxed);
        }

        self.memory_used
            .fetch_add((key.len() + value.len()) as u64, Ordering::Relaxed);

        Ok(())
    }

    /// Get a value by key
    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let _guard = self.lock.read();
        self.operation_count.fetch_add(1, Ordering::Relaxed);

        let hash_table = self.hash_table();
        hash_table.get(key)
    }

    /// Delete a key
    pub fn delete(&self, key: &[u8]) -> bool {
        let _guard = self.lock.write();
        self.operation_count.fetch_add(1, Ordering::Relaxed);

        let hash_table = self.hash_table_mut();
        if let Some(old_size) = hash_table.remove(key) {
            self.entry_count.fetch_sub(1, Ordering::Relaxed);
            self.memory_used
                .fetch_sub(old_size as u64, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Compare-and-swap operation
    pub fn cas(&self, key: &[u8], expected_version: u64, new_value: &[u8]) -> Result<u64, KvError> {
        let _guard = self.lock.write();
        self.operation_count.fetch_add(1, Ordering::Relaxed);

        let hash_table = self.hash_table_mut();
        hash_table.cas(key, expected_version, new_value)
    }

    /// Get shard statistics
    pub fn stats(&self) -> ShardStats {
        ShardStats {
            operation_count: self.operation_count.load(Ordering::Relaxed),
            entry_count: self.entry_count.load(Ordering::Relaxed),
            memory_used: self.memory_used.load(Ordering::Relaxed),
        }
    }
}

/// Shard statistics
#[derive(Debug, Clone)]
pub struct ShardStats {
    pub operation_count: u64,
    pub entry_count: u64,
    pub memory_used: u64,
}
