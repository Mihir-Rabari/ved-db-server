//! Arena allocator for shared memory
//! 
//! Provides bump allocation with offset-based pointers for shared memory usage.
//! Supports size classes and basic free list management for better memory utilization.

use std::sync::atomic::{AtomicU64, Ordering};
use std::mem;

/// Size classes for allocation (powers of 2)
const SIZE_CLASSES: &[usize] = &[
    8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768
];

/// Free list node stored inline in freed memory
#[repr(C)]
struct FreeNode {
    next: u64, // Offset to next free node (0 = end of list)
    size: u32, // Size of this free block
    _padding: u32,
}

/// Arena allocator header in shared memory
#[repr(C)]
pub struct Arena {
    /// Current allocation offset (bump pointer)
    current: AtomicU64,
    /// Total arena size
    size: u64,
    /// Start offset of allocatable space (after this header)
    start_offset: u64,
    /// Free lists for each size class (offsets to first free node)
    free_lists: [AtomicU64; SIZE_CLASSES.len()],
    /// Statistics
    allocated_bytes: AtomicU64,
    allocation_count: AtomicU64,
    free_count: AtomicU64,
}

impl Arena {
    /// Calculate size needed for arena header
    pub const fn header_size() -> usize {
        mem::size_of::<Self>()
    }
    
    /// Initialize a new arena in the given memory location
    /// 
    /// # Safety
    /// - ptr must point to valid memory of at least `size` bytes
    /// - Memory should be zero-initialized
    /// - Only one thread should call this function
    pub unsafe fn init(ptr: *mut u8, size: usize) -> *mut Self {
        let arena = ptr as *mut Self;
        let start_offset = Self::header_size() as u64;
        
        std::ptr::write(
            arena,
            Self {
                current: AtomicU64::new(start_offset),
                size: size as u64,
                start_offset,
                free_lists: [const { AtomicU64::new(0) }; SIZE_CLASSES.len()],
                allocated_bytes: AtomicU64::new(0),
                allocation_count: AtomicU64::new(0),
                free_count: AtomicU64::new(0),
            }
        );
        
        arena
    }
    
    /// Get the base pointer of the arena
    fn base_ptr(&self) -> *mut u8 {
        self as *const Self as *mut u8
    }
    
    /// Convert offset to pointer
    /// 
    /// # Safety
    /// Offset must be valid within this arena
    pub unsafe fn offset_to_ptr(&self, offset: u64) -> *mut u8 {
        self.base_ptr().add(offset as usize)
    }
    
    /// Convert pointer to offset
    /// 
    /// # Safety
    /// Pointer must be within this arena
    unsafe fn ptr_to_offset(&self, ptr: *mut u8) -> u64 {
        (ptr as usize - self.base_ptr() as usize) as u64
    }
    
    /// Public wrapper for offset_to_ptr
    /// 
    /// # Safety
    /// Offset must be valid within this arena
    pub unsafe fn offset_to_ptr_pub(&self, offset: u64) -> *mut u8 {
        self.offset_to_ptr(offset)
    }
    
    /// Find the appropriate size class for the given size
    fn size_class_for(size: usize) -> Option<usize> {
        SIZE_CLASSES.iter().position(|&class_size| size <= class_size)
    }
    
    /// Get the size for a given size class index
    fn size_for_class(class_idx: usize) -> usize {
        SIZE_CLASSES[class_idx]
    }
    
    /// Allocate memory with the given size and alignment
    /// 
    /// Returns offset to allocated memory, or 0 if allocation failed
    pub fn allocate(&self, size: usize, align: usize) -> u64 {
        if size == 0 {
            return 0;
        }
        
        let aligned_size = (size + align - 1) & !(align - 1);
        
        // Try to allocate from free list first
        if let Some(offset) = self.allocate_from_free_list(aligned_size) {
            self.allocation_count.fetch_add(1, Ordering::Relaxed);
            self.allocated_bytes.fetch_add(aligned_size as u64, Ordering::Relaxed);
            return offset;
        }
        
        // Fall back to bump allocation
        self.bump_allocate(aligned_size, align)
    }
    
    /// Try to allocate from appropriate free list
    fn allocate_from_free_list(&self, size: usize) -> Option<u64> {
        let class_idx = Self::size_class_for(size)?;
        let class_size = Self::size_for_class(class_idx);
        
        // Only use free list if the size matches exactly or is close
        if size > class_size || size < class_size / 2 {
            return None;
        }
        
        // Try to pop from free list
        loop {
            let head_offset = self.free_lists[class_idx].load(Ordering::Acquire);
            if head_offset == 0 {
                return None; // Free list is empty
            }
            
            unsafe {
                let head_ptr = self.offset_to_ptr(head_offset) as *mut FreeNode;
                let head_node = &*head_ptr;
                let next_offset = head_node.next;
                
                // Try to update head to next node
                match self.free_lists[class_idx].compare_exchange_weak(
                    head_offset,
                    next_offset,
                    Ordering::Release,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return Some(head_offset),
                    Err(_) => continue, // Retry
                }
            }
        }
    }
    
    /// Bump allocate from the main arena
    fn bump_allocate(&self, size: usize, align: usize) -> u64 {
        loop {
            let current = self.current.load(Ordering::Relaxed);
            
            // Align the current offset
            let aligned_offset = (current + align as u64 - 1) & !(align as u64 - 1);
            let new_offset = aligned_offset + size as u64;
            
            // Check if we have enough space
            if new_offset > self.size {
                return 0; // Out of memory
            }
            
            // Try to advance the bump pointer
            match self.current.compare_exchange_weak(
                current,
                new_offset,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    self.allocation_count.fetch_add(1, Ordering::Relaxed);
                    self.allocated_bytes.fetch_add(size as u64, Ordering::Relaxed);
                    return aligned_offset;
                }
                Err(_) => continue, // Retry
            }
        }
    }
    
    /// Free memory at the given offset
    /// 
    /// # Safety
    /// - offset must point to the start of a valid allocation
    /// - size must match the original allocation size
    /// - Memory must not be accessed after freeing
    pub unsafe fn free(&self, offset: u64, size: usize) {
        if offset == 0 || size == 0 {
            return;
        }
        
        // Find appropriate size class
        if let Some(class_idx) = Self::size_class_for(size) {
            let class_size = Self::size_for_class(class_idx);
            
            // Only add to free list if size is appropriate
            if size >= class_size / 2 && size <= class_size {
                self.add_to_free_list(offset, class_size, class_idx);
                self.free_count.fetch_add(1, Ordering::Relaxed);
                return;
            }
        }
        
        // For sizes that don't fit size classes, we can't easily reuse the memory
        // In a production system, you might want to implement a more sophisticated
        // free list or compaction strategy
        self.free_count.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Add a freed block to the appropriate free list
    unsafe fn add_to_free_list(&self, offset: u64, size: usize, class_idx: usize) {
        let node_ptr = self.offset_to_ptr(offset) as *mut FreeNode;
        
        loop {
            let head_offset = self.free_lists[class_idx].load(Ordering::Acquire);
            
            // Initialize the free node
            std::ptr::write(
                node_ptr,
                FreeNode {
                    next: head_offset,
                    size: size as u32,
                    _padding: 0,
                }
            );
            
            // Try to update the free list head
            match self.free_lists[class_idx].compare_exchange_weak(
                head_offset,
                offset,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(_) => continue, // Retry
            }
        }
    }
    
    /// Get current allocation statistics
    pub fn stats(&self) -> ArenaStats {
        ArenaStats {
            total_size: self.size,
            allocated_bytes: self.allocated_bytes.load(Ordering::Relaxed),
            current_offset: self.current.load(Ordering::Relaxed),
            allocation_count: self.allocation_count.load(Ordering::Relaxed),
            free_count: self.free_count.load(Ordering::Relaxed),
        }
    }
    
    /// Check if an offset is valid within this arena
    pub fn is_valid_offset(&self, offset: u64) -> bool {
        offset >= self.start_offset && offset < self.size
    }
    
    /// Get remaining space in arena
    pub fn remaining_space(&self) -> u64 {
        let current = self.current.load(Ordering::Relaxed);
        self.size.saturating_sub(current)
    }
}

/// Arena allocation statistics
#[derive(Debug, Clone, Copy)]
pub struct ArenaStats {
    pub total_size: u64,
    pub allocated_bytes: u64,
    pub current_offset: u64,
    pub allocation_count: u64,
    pub free_count: u64,
}

impl ArenaStats {
    pub fn utilization(&self) -> f64 {
        if self.total_size == 0 {
            0.0
        } else {
            self.allocated_bytes as f64 / self.total_size as f64
        }
    }
    
    pub fn fragmentation(&self) -> f64 {
        if self.current_offset == 0 {
            0.0
        } else {
            1.0 - (self.allocated_bytes as f64 / self.current_offset as f64)
        }
    }
}

/// Safe wrapper for arena that manages memory
pub struct ArenaBuffer {
    memory: Vec<u8>,
    arena: *mut Arena,
}

impl ArenaBuffer {
    /// Create a new arena with the given size
    pub fn new(size: usize) -> Self {
        assert!(size >= Arena::header_size(), "Arena size too small");
        
        let mut memory = vec![0u8; size];
        let arena = unsafe {
            Arena::init(memory.as_mut_ptr(), size)
        };
        
        Self { memory, arena }
    }
    
    /// Get reference to the arena
    pub fn arena(&self) -> &Arena {
        unsafe { &*self.arena }
    }
    
    /// Allocate memory and return a safe slice
    /// 
    /// The returned slice is valid until the arena is dropped or the memory is freed
    pub fn allocate_slice(&self, size: usize) -> Option<&mut [u8]> {
        let offset = self.arena().allocate(size, 1);
        if offset == 0 {
            return None;
        }
        
        unsafe {
            let ptr = self.arena().offset_to_ptr(offset);
            Some(std::slice::from_raw_parts_mut(ptr, size))
        }
    }
    
    /// Free a previously allocated slice
    /// 
    /// # Safety
    /// The slice must have been allocated by this arena and not previously freed
    pub unsafe fn free_slice(&self, slice: &[u8]) {
        let ptr = slice.as_ptr() as *mut u8;
        let offset = self.arena().ptr_to_offset(ptr);
        self.arena().free(offset, slice.len());
    }
}

unsafe impl Send for ArenaBuffer {}
unsafe impl Sync for ArenaBuffer {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::sync::Arc;

    #[test]
    fn test_arena_basic_allocation() {
        let arena_buf = ArenaBuffer::new(4096);
        let arena = arena_buf.arena();
        
        // Test basic allocation
        let offset1 = arena.allocate(64, 8);
        assert_ne!(offset1, 0);
        assert!(arena.is_valid_offset(offset1));
        
        let offset2 = arena.allocate(128, 8);
        assert_ne!(offset2, 0);
        assert_ne!(offset1, offset2);
        
        let stats = arena.stats();
        assert_eq!(stats.allocation_count, 2);
        assert!(stats.allocated_bytes >= 64 + 128);
    }
    
    #[test]
    fn test_arena_size_classes() {
        let arena_buf = ArenaBuffer::new(8192);
        let arena = arena_buf.arena();
        
        // Allocate and free memory to populate free lists
        let mut offsets = Vec::new();
        for _ in 0..10 {
            let offset = arena.allocate(64, 8);
            offsets.push(offset);
        }
        
        // Free some allocations
        for &offset in &offsets[..5] {
            unsafe { arena.free(offset, 64); }
        }
        
        // Allocate again - should reuse freed memory
        let new_offset = arena.allocate(64, 8);
        assert!(offsets[..5].contains(&new_offset));
        
        let stats = arena.stats();
        assert!(stats.free_count >= 5);
    }
    
    #[test]
    fn test_arena_alignment() {
        let arena_buf = ArenaBuffer::new(4096);
        let arena = arena_buf.arena();
        
        let offset1 = arena.allocate(1, 1);
        let offset2 = arena.allocate(1, 8);
        let offset3 = arena.allocate(1, 16);
        
        assert_eq!(offset2 % 8, 0);
        assert_eq!(offset3 % 16, 0);
    }
    
    #[test]
    fn test_arena_out_of_memory() {
        let arena_buf = ArenaBuffer::new(1024);
        let arena = arena_buf.arena();
        
        // Allocate until we run out of space
        let mut allocations = 0;
        loop {
            let offset = arena.allocate(64, 8);
            if offset == 0 {
                break;
            }
            allocations += 1;
        }
        
        assert!(allocations > 0);
        assert_eq!(arena.remaining_space(), 0);
    }
    
    #[test]
    fn test_arena_concurrent_allocation() {
        let arena_buf = Arc::new(ArenaBuffer::new(65536));
        let num_threads = 4;
        let allocations_per_thread = 100;
        
        let mut handles = Vec::new();
        for _ in 0..num_threads {
            let arena = arena_buf.clone();
            let handle = thread::spawn(move || {
                let mut offsets = Vec::new();
                for _ in 0..allocations_per_thread {
                    let offset = arena.arena().allocate(32, 8);
                    if offset != 0 {
                        offsets.push(offset);
                    }
                }
                offsets
            });
            handles.push(handle);
        }
        
        let mut all_offsets = Vec::new();
        for handle in handles {
            let mut offsets = handle.join().unwrap();
            all_offsets.append(&mut offsets);
        }
        
        // All offsets should be unique
        all_offsets.sort();
        all_offsets.dedup();
        assert_eq!(all_offsets.len(), (num_threads * allocations_per_thread) as usize);
        
        let stats = arena_buf.arena().stats();
        assert_eq!(stats.allocation_count, (num_threads * allocations_per_thread) as u64);
    }
    
    #[test]
    fn test_arena_slice_allocation() {
        let arena_buf = ArenaBuffer::new(4096);
        
        let slice1 = arena_buf.allocate_slice(100).unwrap();
        let slice2 = arena_buf.allocate_slice(200).unwrap();
        
        // Write some data
        slice1.fill(0xAA);
        slice2.fill(0xBB);
        
        // Verify data
        assert!(slice1.iter().all(|&b| b == 0xAA));
        assert!(slice2.iter().all(|&b| b == 0xBB));
        
        // Free one slice
        unsafe { arena_buf.free_slice(slice1); }
        
        let stats = arena_buf.arena().stats();
        assert_eq!(stats.free_count, 1);
    }
}
