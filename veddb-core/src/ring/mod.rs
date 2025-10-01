//! Ring buffer implementations for VedDB
//!
//! Provides high-performance lock-free ring buffers:
//! - SPSC: Single Producer Single Consumer for client-server communication
//! - MPMC: Multi Producer Multi Consumer using Vyukov algorithm for pub/sub

pub mod mpmc;
pub mod spsc;

pub use mpmc::*;
pub use spsc::*;

/// Common ring buffer traits and utilities
use std::sync::atomic::{AtomicU64, Ordering};

/// Ring buffer slot that can hold either inline data or an offset to arena
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Slot {
    /// Length of the data (0 means empty slot)
    pub len: u32,
    /// Either inline data (if len <= INLINE_SIZE) or offset to arena
    pub data_or_offset: u64,
}

impl Slot {
    pub const INLINE_SIZE: usize = 8;

    pub fn empty() -> Self {
        Self {
            len: 0,
            data_or_offset: 0,
        }
    }

    pub fn inline_data(data: &[u8]) -> Option<Self> {
        if data.len() <= Self::INLINE_SIZE {
            let mut slot = Self {
                len: data.len() as u32,
                data_or_offset: 0,
            };

            // Copy data into the u64 field
            let bytes = slot.data_or_offset.to_le_bytes();
            let mut new_bytes = [0u8; 8];
            new_bytes[..data.len()].copy_from_slice(data);
            slot.data_or_offset = u64::from_le_bytes(new_bytes);

            Some(slot)
        } else {
            None
        }
    }

    pub fn arena_offset(len: u32, offset: u64) -> Self {
        Self {
            len,
            data_or_offset: offset,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn is_inline(&self) -> bool {
        self.len > 0 && (self.len as usize) <= Self::INLINE_SIZE
    }

    pub fn get_inline_data(&self) -> Option<Vec<u8>> {
        if self.is_inline() {
            let bytes = self.data_or_offset.to_le_bytes();
            Some(bytes[..self.len as usize].to_vec())
        } else {
            None
        }
    }

    pub fn get_arena_offset(&self) -> Option<u64> {
        if !self.is_inline() && self.len > 0 {
            Some(self.data_or_offset)
        } else {
            None
        }
    }
}

/// Cache-line aligned atomic counter to avoid false sharing
#[repr(align(64))]
pub struct AlignedAtomicU64(pub AtomicU64);

impl AlignedAtomicU64 {
    pub fn new(val: u64) -> Self {
        Self(AtomicU64::new(val))
    }

    pub fn load(&self, order: Ordering) -> u64 {
        self.0.load(order)
    }

    pub fn store(&self, val: u64, order: Ordering) {
        self.0.store(val, order)
    }

    pub fn fetch_add(&self, val: u64, order: Ordering) -> u64 {
        self.0.fetch_add(val, order)
    }

    pub fn compare_exchange_weak(
        &self,
        current: u64,
        new: u64,
        success: Ordering,
        failure: Ordering,
    ) -> Result<u64, u64> {
        self.0.compare_exchange_weak(current, new, success, failure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slot_inline_data() {
        let data = b"hello";
        let slot = Slot::inline_data(data).unwrap();

        assert!(slot.is_inline());
        assert_eq!(slot.len, 5);
        assert_eq!(slot.get_inline_data().unwrap(), data.to_vec());
    }

    #[test]
    fn test_slot_arena_offset() {
        let slot = Slot::arena_offset(100, 0x1000);

        assert!(!slot.is_inline());
        assert_eq!(slot.len, 100);
        assert_eq!(slot.get_arena_offset().unwrap(), 0x1000);
    }

    #[test]
    fn test_slot_too_large_for_inline() {
        let data = b"this is too long for inline storage";
        assert!(Slot::inline_data(data).is_none());
    }
}
