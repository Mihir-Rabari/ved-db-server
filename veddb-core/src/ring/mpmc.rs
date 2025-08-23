//! Multi Producer Multi Consumer ring buffer using Vyukov algorithm
//! 
//! This is a lock-free MPMC queue based on Dmitry Vyukov's bounded MPMC queue.
//! Each slot has a sequence number that coordinates access between multiple
//! producers and consumers without locks.

use super::{AlignedAtomicU64, Slot};
use std::sync::atomic::{AtomicU64, Ordering};

/// MPMC slot with sequence number for coordination
#[repr(C)]
struct MpmcSlot {
    /// Sequence number for coordination
    sequence: AtomicU64,
    /// The actual data slot
    data: Slot,
}

impl MpmcSlot {
    fn new(seq: u64) -> Self {
        Self {
            sequence: AtomicU64::new(seq),
            data: Slot::empty(),
        }
    }
}

/// MPMC Ring buffer layout in shared memory
/// 
/// Memory layout:
/// - head: u64 (producer index, cache-line aligned)
/// - tail: u64 (consumer index, cache-line aligned)
/// - capacity: u64
/// - mask: u64
/// - slots: [MpmcSlot; capacity]
#[repr(C)]
pub struct MpmcRing {
    /// Head index for producers (cache-line aligned)
    head: AlignedAtomicU64,
    /// Tail index for consumers (cache-line aligned)  
    tail: AlignedAtomicU64,
    /// Ring capacity (power of 2)
    capacity: u64,
    /// Mask for fast modulo (capacity - 1)
    mask: u64,
    // Slots array follows this struct in memory
}

impl MpmcRing {
    /// Calculate total size needed for ring with given capacity
    pub fn size_for_capacity(capacity: u64) -> usize {
        assert!(capacity.is_power_of_two(), "Capacity must be power of 2");
        std::mem::size_of::<Self>() + (capacity as usize * std::mem::size_of::<MpmcSlot>())
    }
    
    /// Initialize a new MPMC ring in the given memory location
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
                head: AlignedAtomicU64::new(0),
                tail: AlignedAtomicU64::new(0),
                capacity,
                mask: capacity - 1,
            }
        );
        
        // Initialize slots with sequence numbers
        let slots_ptr = ptr.add(std::mem::size_of::<Self>()) as *mut MpmcSlot;
        for i in 0..capacity {
            std::ptr::write(slots_ptr.add(i as usize), MpmcSlot::new(i));
        }
        
        ring
    }
    
    /// Get pointer to slots array
    unsafe fn slots_ptr(&self) -> *mut MpmcSlot {
        let self_ptr = self as *const Self as *mut u8;
        self_ptr.add(std::mem::size_of::<Self>()) as *mut MpmcSlot
    }
    
    /// Get slot at given index
    unsafe fn slot_at(&self, index: u64) -> *mut MpmcSlot {
        let slot_index = index & self.mask;
        self.slots_ptr().add(slot_index as usize)
    }
    
    /// Get reference to head atomic for external access
    pub fn head(&self) -> &AlignedAtomicU64 {
        &self.head
    }
    
    /// Producer: Try to push a slot into the ring
    /// 
    /// Returns true if successful, false if ring is full or contention
    pub fn try_push(&self, data: Slot) -> bool {
        // Claim a slot by incrementing head
        let head = self.head.fetch_add(1, Ordering::Relaxed);
        let slot_idx = head & self.mask;
        
        unsafe {
            let slot_ptr = self.slot_at(head);
            let slot = &*slot_ptr;
            
            // Wait for the slot to be available for writing
            // The sequence should equal the slot index for an empty slot
            let expected_seq = slot_idx;
            
            // Spin until we can claim this slot
            loop {
                let seq = slot.sequence.load(Ordering::Acquire);
                
                if seq == expected_seq {
                    // Slot is available, try to claim it
                    // We don't need CAS here because each producer gets a unique head value
                    break;
                } else if seq < expected_seq {
                    // Slot is still being written by a previous producer
                    std::hint::spin_loop();
                    continue;
                } else {
                    // Ring is full (seq > expected_seq means slot is ahead)
                    return false;
                }
            }
            
            // Write the data
            std::ptr::write(&mut (*slot_ptr).data, data);
            
            // Release the slot by updating sequence
            // This makes it available for consumers
            slot.sequence.store(expected_seq + 1, Ordering::Release);
        }
        
        true
    }
    
    /// Producer: Push slot, spinning until space is available
    pub fn push(&self, data: Slot) {
        while !self.try_push(data) {
            std::hint::spin_loop();
        }
    }
    
    /// Consumer: Try to pop a slot from the ring
    /// 
    /// Returns Some(slot) if successful, None if ring is empty or contention
    pub fn try_pop(&self) -> Option<Slot> {
        // Claim a slot by incrementing tail
        let tail = self.tail.fetch_add(1, Ordering::Relaxed);
        let slot_idx = tail & self.mask;
        
        unsafe {
            let slot_ptr = self.slot_at(tail);
            let slot = &*slot_ptr;
            
            // Wait for the slot to be available for reading
            // The sequence should be slot_idx + 1 for a slot with data
            let expected_seq = slot_idx + 1;
            
            // Spin until we can read this slot
            loop {
                let seq = slot.sequence.load(Ordering::Acquire);
                
                if seq == expected_seq {
                    // Slot has data, we can read it
                    break;
                } else if seq < expected_seq {
                    // Ring is empty (slot hasn't been written yet)
                    return None;
                } else {
                    // Slot is ahead, shouldn't happen in normal operation
                    std::hint::spin_loop();
                    continue;
                }
            }
            
            // Read the data
            let data = std::ptr::read(&(*slot_ptr).data);
            
            // Mark slot as empty and available for next round
            // Add capacity to wrap around properly
            slot.sequence.store(expected_seq + self.capacity - 1, Ordering::Release);
            
            Some(data)
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
    
    /// Get approximate number of items in ring
    /// Note: This is approximate due to concurrent access
    pub fn len(&self) -> u64 {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        head.saturating_sub(tail)
    }
    
    /// Check if ring appears empty (approximate)
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    
    /// Check if ring appears full (approximate)
    pub fn is_full(&self) -> bool {
        self.len() >= self.capacity
    }
    
    /// Get ring capacity
    pub fn capacity(&self) -> u64 {
        self.capacity
    }
}

/// Safe wrapper for MPMC ring that manages memory
pub struct MpmcRingBuffer {
    memory: Vec<u8>,
    ring: *mut MpmcRing,
}

impl MpmcRingBuffer {
    /// Create a new MPMC ring buffer with the given capacity
    pub fn new(capacity: u64) -> Self {
        assert!(capacity.is_power_of_two(), "Capacity must be power of 2");
        
        let size = MpmcRing::size_for_capacity(capacity);
        let mut memory = vec![0u8; size];
        
        let ring = unsafe {
            MpmcRing::init(memory.as_mut_ptr(), capacity)
        };
        
        Self { memory, ring }
    }
    
    /// Get reference to the ring
    pub fn ring(&self) -> &MpmcRing {
        unsafe { &*self.ring }
    }
}

unsafe impl Send for MpmcRingBuffer {}
unsafe impl Sync for MpmcRingBuffer {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use std::collections::HashSet;

    #[test]
    fn test_mpmc_basic_operations() {
        let ring_buf = MpmcRingBuffer::new(8);
        let ring = ring_buf.ring();
        
        assert!(ring.is_empty());
        assert_eq!(ring.capacity(), 8);
        
        // Push some slots
        let slot1 = Slot::inline_data(b"hello").unwrap();
        let slot2 = Slot::arena_offset(100, 0x1000);
        
        assert!(ring.try_push(slot1));
        assert!(ring.try_push(slot2));
        
        // Pop slots
        let popped1 = ring.try_pop().unwrap();
        let popped2 = ring.try_pop().unwrap();
        
        assert_eq!(popped1.get_inline_data().unwrap(), b"hello");
        assert_eq!(popped2.get_arena_offset().unwrap(), 0x1000);
        
        assert!(ring.try_pop().is_none());
    }
    
    #[test]
    fn test_mpmc_concurrent_single_producer_consumer() {
        let ring_buf = Arc::new(MpmcRingBuffer::new(1024));
        let ring_producer = ring_buf.clone();
        let ring_consumer = ring_buf.clone();
        
        let producer = thread::spawn(move || {
            for i in 0..1000 {
                let slot = Slot::arena_offset(i as u32, i as u64);
                ring_producer.ring().push(slot);
            }
        });
        
        let consumer = thread::spawn(move || {
            let mut received = Vec::new();
            for _ in 0..1000 {
                let slot = ring_consumer.ring().pop();
                if let Some(offset) = slot.get_arena_offset() {
                    received.push(offset);
                }
            }
            received
        });
        
        producer.join().unwrap();
        let received = consumer.join().unwrap();
        
        assert_eq!(received.len(), 1000);
        // Values should be 0..999 but may be out of order
        let received_set: HashSet<_> = received.into_iter().collect();
        let expected_set: HashSet<_> = (0..1000).collect();
        assert_eq!(received_set, expected_set);
    }
    
    #[test]
    fn test_mpmc_multiple_producers() {
        let ring_buf = Arc::new(MpmcRingBuffer::new(1024));
        let num_producers = 4;
        let items_per_producer = 250;
        
        let mut producers = Vec::new();
        for producer_id in 0..num_producers {
            let ring = ring_buf.clone();
            let handle = thread::spawn(move || {
                for i in 0..items_per_producer {
                    let value = producer_id * items_per_producer + i;
                    let slot = Slot::arena_offset(value as u32, value as u64);
                    ring.ring().push(slot);
                }
            });
            producers.push(handle);
        }
        
        let consumer = thread::spawn({
            let ring = ring_buf.clone();
            move || {
                let mut received = Vec::new();
                for _ in 0..(num_producers * items_per_producer) {
                    let slot = ring.ring().pop();
                    if let Some(offset) = slot.get_arena_offset() {
                        received.push(offset);
                    }
                }
                received
            }
        });
        
        for producer in producers {
            producer.join().unwrap();
        }
        
        let received = consumer.join().unwrap();
        assert_eq!(received.len(), (num_producers * items_per_producer) as usize);
        
        // All values should be present
        let received_set: HashSet<_> = received.into_iter().collect();
        let expected_set: HashSet<_> = (0..(num_producers * items_per_producer)).collect();
        assert_eq!(received_set, expected_set);
    }
    
    #[test]
    fn test_mpmc_multiple_consumers() {
        let ring_buf = Arc::new(MpmcRingBuffer::new(1024));
        let num_consumers = 4;
        let total_items = 1000;
        
        let producer = thread::spawn({
            let ring = ring_buf.clone();
            move || {
                for i in 0..total_items {
                    let slot = Slot::arena_offset(i as u32, i as u64);
                    ring.ring().push(slot);
                }
            }
        });
        
        let mut consumers = Vec::new();
        for _ in 0..num_consumers {
            let ring = ring_buf.clone();
            let handle = thread::spawn(move || {
                let mut received = Vec::new();
                loop {
                    if let Some(slot) = ring.ring().try_pop() {
                        if let Some(offset) = slot.get_arena_offset() {
                            received.push(offset);
                        }
                    } else {
                        // Small delay to avoid busy spinning
                        thread::yield_now();
                        // Check if we might be done
                        if received.len() > 0 && ring.ring().is_empty() {
                            break;
                        }
                    }
                }
                received
            });
            consumers.push(handle);
        }
        
        producer.join().unwrap();
        
        let mut all_received = Vec::new();
        for consumer in consumers {
            let mut received = consumer.join().unwrap();
            all_received.append(&mut received);
        }
        
        assert_eq!(all_received.len(), total_items as usize);
        
        // All values should be present exactly once
        let received_set: HashSet<_> = all_received.into_iter().collect();
        let expected_set: HashSet<_> = (0..total_items).collect();
        assert_eq!(received_set, expected_set);
    }
}
