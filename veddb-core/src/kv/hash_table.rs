//! Hash table implementation using open addressing
//!
//! Provides a lock-free hash table with linear probing for collision resolution.
//! Supports versioned entries for CAS operations and tombstone deletion.

use super::KvError;
use crate::arena::Arena;
use std::sync::atomic::{AtomicU64, Ordering};

/// Hash table entry
#[repr(C)]
#[derive(Debug)]
struct Entry {
    /// Hash of the key (0 = empty, 1 = tombstone)
    hash: AtomicU64,
    /// Version for CAS operations
    version: AtomicU64,
    /// Key length
    key_len: u32,
    /// Value length
    val_len: u32,
    /// Offset to key+value data in arena
    data_offset: u64,
}

impl Entry {
    const EMPTY_HASH: u64 = 0;
    const TOMBSTONE_HASH: u64 = 1;

    fn new() -> Self {
        Self {
            hash: AtomicU64::new(Self::EMPTY_HASH),
            version: AtomicU64::new(0),
            key_len: 0,
            val_len: 0,
            data_offset: 0,
        }
    }

    fn is_empty(&self) -> bool {
        self.hash.load(Ordering::Acquire) == Self::EMPTY_HASH
    }

    fn is_tombstone(&self) -> bool {
        self.hash.load(Ordering::Acquire) == Self::TOMBSTONE_HASH
    }

    fn is_available(&self) -> bool {
        let hash = self.hash.load(Ordering::Acquire);
        hash == Self::EMPTY_HASH || hash == Self::TOMBSTONE_HASH
    }

    fn mark_tombstone(&self) {
        self.hash.store(Self::TOMBSTONE_HASH, Ordering::Release);
    }
}

/// Hash table with open addressing
#[repr(C)]
pub struct HashTable {
    /// Capacity (power of 2)
    capacity: u64,
    /// Mask for fast modulo
    mask: u64,
    /// Number of entries
    size: AtomicU64,
    /// Arena for key/value storage
    arena: *mut Arena,
    // Entries follow this structure
}

impl HashTable {
    /// Calculate size needed for hash table with given capacity
    pub fn size_for_capacity(capacity: usize) -> usize {
        assert!(capacity.is_power_of_two(), "Capacity must be power of 2");
        std::mem::size_of::<Self>() + capacity * std::mem::size_of::<Entry>()
    }

    /// Initialize hash table in shared memory
    ///
    /// # Safety
    /// - ptr must point to valid memory of sufficient size
    /// - arena must be valid and initialized
    pub unsafe fn init(ptr: *mut u8, capacity: usize, arena: *mut Arena) -> *mut Self {
        assert!(capacity.is_power_of_two());

        let table = ptr as *mut Self;

        std::ptr::write(
            table,
            Self {
                capacity: capacity as u64,
                mask: (capacity - 1) as u64,
                size: AtomicU64::new(0),
                arena,
            },
        );

        // Initialize entries
        let entries_ptr = ptr.add(std::mem::size_of::<Self>()) as *mut Entry;
        for i in 0..capacity {
            std::ptr::write(entries_ptr.add(i), Entry::new());
        }

        table
    }

    /// Get pointer to entries array
    unsafe fn entries_ptr(&self) -> *mut Entry {
        let self_ptr = self as *const Self as *mut u8;
        self_ptr.add(std::mem::size_of::<Self>()) as *mut Entry
    }

    /// Get entry at given index
    unsafe fn entry_at(&self, index: u64) -> &Entry {
        let entries = self.entries_ptr();
        &*entries.add(index as usize)
    }

    /// Get mutable entry at given index
    unsafe fn entry_at_mut(&mut self, index: u64) -> &mut Entry {
        let entries = self.entries_ptr();
        &mut *entries.add(index as usize)
    }

    /// Hash a key
    fn hash_key(&self, key: &[u8]) -> u64 {
        // Ensure hash is never 0 or 1 (reserved values)
        let mut hash = 0xcbf29ce484222325u64;
        for &byte in key {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }

        if hash <= Entry::TOMBSTONE_HASH {
            hash = 2;
        }

        hash
    }

    /// Find entry for key, returns (entry_index, is_match)
    fn find_entry(&mut self, key: &[u8], hash: u64) -> (u64, bool) {
        let mut index = hash & self.mask;

        loop {
            unsafe {
                let entry = self.entry_at(index);
                let entry_hash = entry.hash.load(Ordering::Acquire);

                if entry_hash == Entry::EMPTY_HASH {
                    // Found empty slot
                    return (index, false);
                } else if entry_hash == hash {
                    // Potential match, check key
                    if self.keys_equal(entry, key) {
                        return (index, true);
                    }
                }

                // Continue probing
                index = (index + 1) & self.mask;
            }
        }
    }

    /// Check if entry's key matches the given key
    unsafe fn keys_equal(&self, entry: &Entry, key: &[u8]) -> bool {
        if entry.key_len as usize != key.len() {
            return false;
        }

        let arena = &*self.arena;
        let key_ptr = arena.offset_to_ptr(entry.data_offset);
        let stored_key = std::slice::from_raw_parts(key_ptr, entry.key_len as usize);

        stored_key == key
    }

    /// Get key and value from entry
    unsafe fn get_key_value(&self, entry: &Entry) -> (Vec<u8>, Vec<u8>) {
        let arena = &*self.arena;
        let data_ptr = arena.offset_to_ptr(entry.data_offset);

        let key = std::slice::from_raw_parts(data_ptr, entry.key_len as usize).to_vec();
        let value_ptr = data_ptr.add(entry.key_len as usize);
        let value = std::slice::from_raw_parts(value_ptr, entry.val_len as usize).to_vec();

        (key, value)
    }

    /// Insert or update a key-value pair
    /// Returns true if this was a new insertion
    pub fn insert(&mut self, key: &[u8], value: &[u8]) -> Result<bool, KvError> {
        if key.len() > u32::MAX as usize || value.len() > u32::MAX as usize {
            return Err(KvError::KeyTooLarge);
        }

        let hash = self.hash_key(key);
        let (index, is_match) = self.find_entry(key, hash);

        unsafe {
            // Copy the raw arena pointer locally to avoid borrowing self while entry is mutably borrowed
            let arena_ptr = self.arena;

            if is_match {
                // Phase 1: read current entry fields without performing arena ops
                let (old_offset, old_size);
                {
                    let entry = self.entry_at_mut(index);
                    old_offset = entry.data_offset;
                    old_size = entry.key_len as usize + entry.val_len as usize;
                } // entry mutable borrow ends

                let new_size = key.len() + value.len();
                let new_offset = if new_size > old_size {
                    let arena = &*arena_ptr;
                    arena.free(old_offset, old_size);
                    let off = arena.allocate(new_size, 8);
                    if off == 0 {
                        return Err(KvError::OutOfMemory);
                    }
                    off
                } else {
                    old_offset
                };

                // Copy key and value into arena
                {
                    let arena = &*arena_ptr;
                    let data_ptr = arena.offset_to_ptr(new_offset);
                    std::ptr::copy_nonoverlapping(key.as_ptr(), data_ptr, key.len());
                    std::ptr::copy_nonoverlapping(
                        value.as_ptr(),
                        data_ptr.add(key.len()),
                        value.len(),
                    );
                }

                // Phase 2: write updated entry fields
                {
                    let entry = self.entry_at_mut(index);
                    entry.data_offset = new_offset;
                    entry.key_len = key.len() as u32;
                    entry.val_len = value.len() as u32;
                }

                Ok(false)
            } else {
                // Allocate and write, then set entry
                let data_size = key.len() + value.len();
                let new_offset = {
                    let arena = &*arena_ptr;
                    let off = arena.allocate(data_size, 8);
                    if off == 0 {
                        return Err(KvError::OutOfMemory);
                    }
                    // Copy key and value now
                    let data_ptr = arena.offset_to_ptr(off);
                    std::ptr::copy_nonoverlapping(key.as_ptr(), data_ptr, key.len());
                    std::ptr::copy_nonoverlapping(
                        value.as_ptr(),
                        data_ptr.add(key.len()),
                        value.len(),
                    );
                    off
                };

                // Update entry after arena operations
                {
                    let entry = self.entry_at_mut(index);
                    entry.hash.store(hash, Ordering::Release);
                    entry.key_len = key.len() as u32;
                    entry.val_len = value.len() as u32;
                    entry.data_offset = new_offset;
                    // Initialize version to 1 on first insert so that first CAS expects version 1
                    entry.version.store(1, Ordering::Release);
                }

                // Increment size if this is a new entry (not a tombstone)
                self.size.fetch_add(1, Ordering::Release);
                Ok(true)
            }
        }
    }

    /// Get value for a key
    pub fn get(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        let hash = self.hash_key(key);
        let (index, is_match) = self.find_entry(key, hash);

        if is_match {
            unsafe {
                let entry = self.entry_at(index);
                let (_, value) = self.get_key_value(entry);
                Some(value)
            }
        } else {
            None
        }
    }

    /// Remove a key, returns the size of removed data
    pub fn remove(&mut self, key: &[u8]) -> Option<usize> {
        let hash = self.hash_key(key);
        let (index, is_match) = self.find_entry(key, hash);

        if is_match {
            unsafe {
                let arena_ptr = self.arena;
                // Phase 1: read and mark entry, capture values, then end borrow
                let (offset, size);
                {
                    let entry = self.entry_at_mut(index);
                    if entry.is_tombstone() {
                        return None;
                    }
                    size = (entry.key_len + entry.val_len) as usize;
                    offset = entry.data_offset;
                    entry.mark_tombstone();
                    entry.key_len = 0;
                    entry.val_len = 0;
                    entry.data_offset = 0;
                }
                // Phase 2: perform arena free without overlapping borrow
                {
                    let arena = &*arena_ptr;
                    arena.free(offset, size);
                }
                // Decrement size
                self.size.fetch_sub(1, Ordering::Release);
                Some(size)
            }
        } else {
            None
        }
    }

    /// Compare-and-swap operation
    pub fn cas(
        &mut self,
        key: &[u8],
        expected_version: u64,
        new_value: &[u8],
    ) -> Result<u64, KvError> {
        let hash = self.hash_key(key);
        let (index, is_match) = self.find_entry(key, hash);

        if !is_match {
            return Err(KvError::NotFound);
        }

        unsafe {
            let arena_ptr = self.arena;
            // Phase 1: read required entry state
            let (key_len, old_offset, old_size, current_version);
            {
                let entry = self.entry_at_mut(index);
                current_version = entry.version.load(Ordering::Acquire);
                if current_version != expected_version {
                    return Err(KvError::VersionMismatch);
                }
                key_len = entry.key_len as usize;
                old_offset = entry.data_offset;
                old_size = entry.key_len as usize + entry.val_len as usize;
            }

            let new_size = key_len + new_value.len();
            let data_offset = if new_size > old_size {
                let arena = &*arena_ptr;
                arena.free(old_offset, old_size);
                let off = arena.allocate(new_size, 8);
                if off == 0 {
                    return Err(KvError::OutOfMemory);
                }
                off
            } else {
                old_offset
            };

            // Copy key and new value
            {
                let arena = &*arena_ptr;
                let data_ptr = arena.offset_to_ptr(data_offset);
                std::ptr::copy_nonoverlapping(key.as_ptr(), data_ptr, key_len);
                std::ptr::copy_nonoverlapping(
                    new_value.as_ptr(),
                    data_ptr.add(key_len),
                    new_value.len(),
                );
            }

            // Phase 2: update entry fields and version
            let new_version = current_version.wrapping_add(1);
            {
                let entry = self.entry_at_mut(index);
                entry.data_offset = data_offset;
                entry.val_len = new_value.len() as u32;
                entry.version.store(new_version, Ordering::Release);
            }
            Ok(new_version)
        }
    }

    /// Get current size
    pub fn size(&self) -> u64 {
        self.size.load(Ordering::Acquire)
    }

    /// Get capacity
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Get load factor
    pub fn load_factor(&self) -> f64 {
        self.size() as f64 / self.capacity as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::ArenaBuffer;

    #[test]
    fn test_hash_table_basic_operations() {
        let arena_buf = ArenaBuffer::new(8192);
        let arena = arena_buf.arena() as *const Arena as *mut Arena;

        let table_size = HashTable::size_for_capacity(16);
        let mut table_memory = vec![0u8; table_size];

        let table = unsafe { HashTable::init(table_memory.as_mut_ptr(), 16, arena) };

        let table_ref = unsafe { &mut *table };

        // Test insertion
        assert!(table_ref.insert(b"key1", b"value1").unwrap());
        assert_eq!(table_ref.size(), 1);

        // Test retrieval
        assert_eq!(table_ref.get(b"key1").unwrap(), b"value1");
        assert!(table_ref.get(b"nonexistent").is_none());

        // Test update
        assert!(!table_ref.insert(b"key1", b"new_value").unwrap());
        assert_eq!(table_ref.get(b"key1").unwrap(), b"new_value");
        assert_eq!(table_ref.size(), 1);

        // Test deletion
        assert_eq!(table_ref.remove(b"key1").unwrap(), 4 + 9); // key + value length
        assert!(table_ref.get(b"key1").is_none());
        assert_eq!(table_ref.size(), 0);
    }

    #[test]
    fn test_hash_table_cas() {
        let arena_buf = ArenaBuffer::new(8192);
        let arena = arena_buf.arena() as *const Arena as *mut Arena;

        let table_size = HashTable::size_for_capacity(16);
        let mut table_memory = vec![0u8; table_size];

        let table = unsafe { HashTable::init(table_memory.as_mut_ptr(), 16, arena) };

        let table_ref = unsafe { &mut *table };

        // Insert initial value
        table_ref.insert(b"key1", b"value1").unwrap();

        // CAS with wrong version should fail
        assert_eq!(
            table_ref.cas(b"key1", 999, b"new_value"),
            Err(KvError::VersionMismatch)
        );

        // CAS with correct version should succeed
        let new_version = table_ref.cas(b"key1", 1, b"new_value").unwrap();
        assert_eq!(new_version, 2);
        assert_eq!(table_ref.get(b"key1").unwrap(), b"new_value");

        // CAS on non-existent key should fail
        assert_eq!(
            table_ref.cas(b"nonexistent", 1, b"value"),
            Err(KvError::NotFound)
        );
    }

    #[test]
    fn test_hash_table_collisions() {
        let arena_buf = ArenaBuffer::new(8192);
        let arena = arena_buf.arena() as *const Arena as *mut Arena;

        let table_size = HashTable::size_for_capacity(4); // Small table to force collisions
        let mut table_memory = vec![0u8; table_size];

        let table = unsafe { HashTable::init(table_memory.as_mut_ptr(), 4, arena) };

        let table_ref = unsafe { &mut *table };

        // Insert multiple keys that may collide
        for i in 0..8 {
            let key = format!("key{}", i);
            let value = format!("value{}", i);
            table_ref.insert(key.as_bytes(), value.as_bytes()).unwrap();
        }

        // Verify all keys can be retrieved
        for i in 0..8 {
            let key = format!("key{}", i);
            let expected_value = format!("value{}", i);
            assert_eq!(
                table_ref.get(key.as_bytes()).unwrap(),
                expected_value.as_bytes()
            );
        }
    }
}
