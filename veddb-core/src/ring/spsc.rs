//! Single Producer Single Consumer ring buffer
//!
//! Lock-free SPSC ring optimized for shared memory usage.
//! Uses separate cache-line aligned producer/consumer indices to avoid false sharing.

use super::{AlignedAtomicU64, Slot};
use std::sync::atomic::Ordering;

/// SPSC Ring buffer layout in shared memory
///
/// Memory layout:
/// - producer_index: u64 (cache-line aligned)
/// - consumer_index: u64 (cache-line aligned)
/// - capacity: u64
/// - slots: [Slot; capacity]
#[repr(C)]
pub struct SpscRing {
    /// Producer index (written by producer, read by consumer)
    producer_index: AlignedAtomicU64,
    /// Consumer index (written by consumer, read by producer)  
    consumer_index: AlignedAtomicU64,
    /// Ring capacity (power of 2)
    capacity: u64,
    /// Mask for fast modulo (capacity - 1)
    mask: u64,
    // Slots array follows this struct in memory
    // Use offset to access: slots_offset = size_of::<SpscRing>()
}

impl SpscRing {
    /// Calculate total size needed for ring with given capacity
    pub fn size_for_capacity(capacity: u64) -> usize {
        assert!(capacity.is_power_of_two(), "Capacity must be power of 2");
        std::mem::size_of::<Self>() + (capacity as usize * std::mem::size_of::<Slot>())
    }

    /// Initialize a new SPSC ring in the given memory location
    ///
    /// # Safety
    /// - ptr must point to valid memory of at least size_for_capacity(capacity) bytes
    /// - Memory must be zero-initialized
    /// - Only one thread should call this function
    pub unsafe fn init(ptr: *mut u8, capacity: u64) -> *mut Self {
        assert!(capacity.is_power_of_two(), "Capacity must be power of 2");

        let ring = ptr as *mut Self;

        // Initialize the ring structure
        std::ptr::write(
            ring,
            Self {
                producer_index: AlignedAtomicU64::new(0),
                consumer_index: AlignedAtomicU64::new(0),
                capacity,
                mask: capacity - 1,
            },
        );

        // Zero-initialize slots
        let slots_ptr = ptr.add(std::mem::size_of::<Self>()) as *mut Slot;
        for i in 0..capacity {
            std::ptr::write(slots_ptr.add(i as usize), Slot::empty());
        }

        ring
    }

    /// Get pointer to slots array
    unsafe fn slots_ptr(&self) -> *mut Slot {
        let self_ptr = self as *const Self as *mut u8;
        self_ptr.add(std::mem::size_of::<Self>()) as *mut Slot
    }

    /// Get slot at given index
    unsafe fn slot_at(&self, index: u64) -> *mut Slot {
        let slot_index = index & self.mask;
        self.slots_ptr().add(slot_index as usize)
    }

    /// Producer: Try to push a slot into the ring
    ///
    /// Returns true if successful, false if ring is full
    pub fn try_push(&self, slot: Slot) -> bool {
        let producer_idx = self.producer_index.load(Ordering::Relaxed);
        let consumer_idx = self.consumer_index.load(Ordering::Acquire);

        // Check if ring is full
        if producer_idx - consumer_idx >= self.capacity {
            return false;
        }

        unsafe {
            // Write slot data
            let slot_ptr = self.slot_at(producer_idx);
            std::ptr::write(slot_ptr, slot);

            // Advance producer index with release ordering
            // This ensures slot write happens-before index update
            self.producer_index
                .store(producer_idx + 1, Ordering::Release);
        }

        true
    }

    /// Producer: Push slot, spinning until space is available
    pub fn push(&self, slot: Slot) {
        while !self.try_push(slot) {
            std::hint::spin_loop();
        }
    }

    /// Consumer: Try to pop a slot from the ring
    ///
    /// Returns Some(slot) if successful, None if ring is empty
    pub fn try_pop(&self) -> Option<Slot> {
        let consumer_idx = self.consumer_index.load(Ordering::Relaxed);
        let producer_idx = self.producer_index.load(Ordering::Acquire);

        // Check if ring is empty
        if consumer_idx >= producer_idx {
            return None;
        }

        unsafe {
            // Read slot data
            let slot_ptr = self.slot_at(consumer_idx);
            let slot = std::ptr::read(slot_ptr);

            // Clear the slot
            std::ptr::write(slot_ptr, Slot::empty());

            // Advance consumer index with release ordering
            self.consumer_index
                .store(consumer_idx + 1, Ordering::Release);

            Some(slot)
        }
    }

    /// Consumer: Pop slot, spinning until one is available
    pub fn pop(&self) -> Slot {
        loop {
            if let Some(slot) = self.try_pop() {
                return slot;
            }
            std::hint::spin_loop();
        }
    }

    /// Get current number of items in ring
    pub fn len(&self) -> u64 {
        let producer_idx = self.producer_index.load(Ordering::Acquire);
        let consumer_idx = self.consumer_index.load(Ordering::Acquire);
        producer_idx.saturating_sub(consumer_idx)
    }

    /// Check if ring is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if ring is full
    pub fn is_full(&self) -> bool {
        self.len() >= self.capacity
    }

    /// Get ring capacity
    pub fn capacity(&self) -> u64 {
        self.capacity
    }
}

/// Safe wrapper for SPSC ring that manages memory
pub struct SpscRingBuffer {
    memory: Vec<u8>,
    ring: *mut SpscRing,
}

impl SpscRingBuffer {
    /// Create a new SPSC ring buffer with the given capacity
    pub fn new(capacity: u64) -> Self {
        assert!(capacity.is_power_of_two(), "Capacity must be power of 2");

        let size = SpscRing::size_for_capacity(capacity);
        let mut memory = vec![0u8; size];

        let ring = unsafe { SpscRing::init(memory.as_mut_ptr(), capacity) };

        Self { memory, ring }
    }

    /// Get reference to the ring
    pub fn ring(&self) -> &SpscRing {
        unsafe { &*self.ring }
    }
}

unsafe impl Send for SpscRingBuffer {}
unsafe impl Sync for SpscRingBuffer {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_spsc_basic_operations() {
        let ring_buf = SpscRingBuffer::new(8);
        let ring = ring_buf.ring();

        assert!(ring.is_empty());
        assert!(!ring.is_full());
        assert_eq!(ring.capacity(), 8);

        // Push some slots
        let slot1 = Slot::inline_data(b"hello").unwrap();
        let slot2 = Slot::arena_offset(100, 0x1000);

        assert!(ring.try_push(slot1));
        assert!(ring.try_push(slot2));
        assert_eq!(ring.len(), 2);

        // Pop slots
        let popped1 = ring.try_pop().unwrap();
        let popped2 = ring.try_pop().unwrap();

        assert_eq!(popped1.get_inline_data().unwrap(), b"hello");
        assert_eq!(popped2.get_arena_offset().unwrap(), 0x1000);

        assert!(ring.is_empty());
        assert!(ring.try_pop().is_none());
    }

    #[test]
    fn test_spsc_full_ring() {
        let ring_buf = SpscRingBuffer::new(4);
        let ring = ring_buf.ring();

        let slot = Slot::inline_data(b"test").unwrap();

        // Fill the ring
        for _ in 0..4 {
            assert!(ring.try_push(slot));
        }

        assert!(ring.is_full());
        assert!(!ring.try_push(slot)); // Should fail when full

        // Empty the ring
        for _ in 0..4 {
            assert!(ring.try_pop().is_some());
        }

        assert!(ring.is_empty());
    }

    #[test]
    fn test_spsc_concurrent() {
        let ring_buf = Arc::new(SpscRingBuffer::new(1024));
        let ring_producer = ring_buf.clone();
        let ring_consumer = ring_buf.clone();

        let producer = thread::spawn(move || {
            for i in 0..1000 {
                let data = format!("msg{}", i);
                let slot = if data.len() <= 8 {
                    Slot::inline_data(data.as_bytes()).unwrap()
                } else {
                    Slot::arena_offset(data.len() as u32, i as u64)
                };
                ring_producer.ring().push(slot);
            }
        });

        let consumer = thread::spawn(move || {
            let mut count = 0;
            while count < 1000 {
                if let Some(_slot) = ring_consumer.ring().try_pop() {
                    count += 1;
                } else {
                    thread::yield_now();
                }
            }
            count
        });

        producer.join().unwrap();
        let received = consumer.join().unwrap();

        assert_eq!(received, 1000);
    }
}
