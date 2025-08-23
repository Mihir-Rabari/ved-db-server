//! Subscriber management for topics

use super::PubSubError;
use std::sync::atomic::{AtomicU64, Ordering};

/// Subscriber entry
#[repr(C)]
#[derive(Debug)]
pub struct SubscriberEntry {
    /// Subscriber ID (0 = empty slot)
    pub id: u64,
    /// Current read index in topic ring
    pub read_index: AtomicU64,
    /// Subscriber flags
    pub flags: u32,
    /// Reserved for future use
    pub reserved: u32,
}

impl SubscriberEntry {
    pub fn new() -> Self {
        Self {
            id: 0,
            read_index: AtomicU64::new(0),
            flags: 0,
            reserved: 0,
        }
    }
    
    pub fn is_empty(&self) -> bool {
        self.id == 0
    }
}

/// Subscriber list for a topic
#[repr(C)]
pub struct SubscriberList {
    /// Maximum number of subscribers
    max_subscribers: u64,
    /// Current number of active subscribers
    active_count: AtomicU64,
    // Subscriber entries follow this structure
}

impl SubscriberList {
    /// Calculate size needed for subscriber list
    pub fn size_for_max_subscribers(max_subscribers: usize) -> usize {
        std::mem::size_of::<Self>() + max_subscribers * std::mem::size_of::<SubscriberEntry>()
    }
    
    /// Initialize subscriber list in shared memory
    /// 
    /// # Safety
    /// - ptr must point to valid memory of sufficient size
    pub unsafe fn init(ptr: *mut u8, max_subscribers: usize) -> *mut Self {
        let list = ptr as *mut Self;
        
        std::ptr::write(
            list,
            Self {
                max_subscribers: max_subscribers as u64,
                active_count: AtomicU64::new(0),
            }
        );
        
        // Initialize subscriber entries
        let entries_ptr = ptr.add(std::mem::size_of::<Self>()) as *mut SubscriberEntry;
        for i in 0..max_subscribers {
            std::ptr::write(entries_ptr.add(i), SubscriberEntry::new());
        }
        
        list
    }
    
    /// Get pointer to subscriber entries
    unsafe fn entries_ptr(&self) -> *mut SubscriberEntry {
        let self_ptr = self as *const Self as *mut u8;
        self_ptr.add(std::mem::size_of::<Self>()) as *mut SubscriberEntry
    }
    
    /// Get subscriber entry at index
    unsafe fn entry_at(&self, index: usize) -> &mut SubscriberEntry {
        let entries = self.entries_ptr();
        &mut *entries.add(index)
    }
    
    /// Find subscriber entry by ID
    fn find_subscriber(&self, subscriber_id: u64) -> Option<usize> {
        unsafe {
            for i in 0..self.max_subscribers as usize {
                let entry = &*self.entries_ptr().add(i);
                if entry.id == subscriber_id {
                    return Some(i);
                }
            }
        }
        None
    }
    
    /// Find empty slot for new subscriber
    fn find_empty_slot(&self) -> Option<usize> {
        unsafe {
            for i in 0..self.max_subscribers as usize {
                let entry = &*self.entries_ptr().add(i);
                if entry.is_empty() {
                    return Some(i);
                }
            }
        }
        None
    }
    
    /// Add a new subscriber
    pub fn add_subscriber(&mut self, subscriber_id: u64) -> Result<(), PubSubError> {
        if subscriber_id == 0 {
            return Err(PubSubError::InvalidSubscriber);
        }
        
        // Check if subscriber already exists
        if self.find_subscriber(subscriber_id).is_some() {
            return Ok(()); // Already subscribed
        }
        
        // Find empty slot
        let slot = self.find_empty_slot()
            .ok_or(PubSubError::TooManySubscribers)?;
        
        unsafe {
            let entry = self.entry_at(slot);
            entry.id = subscriber_id;
            entry.read_index.store(0, Ordering::Relaxed);
            entry.flags = 0;
        }
        
        self.active_count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    
    /// Remove a subscriber
    pub fn remove_subscriber(&mut self, subscriber_id: u64) -> Result<bool, PubSubError> {
        let slot = match self.find_subscriber(subscriber_id) {
            Some(slot) => slot,
            None => return Ok(false), // Not found
        };
        
        unsafe {
            let entry = self.entry_at(slot);
            entry.id = 0;
            entry.read_index.store(0, Ordering::Relaxed);
            entry.flags = 0;
        }
        
        self.active_count.fetch_sub(1, Ordering::Relaxed);
        Ok(true)
    }
    
    /// Get subscriber's current read index
    pub fn get_read_index(&self, subscriber_id: u64) -> Result<u64, PubSubError> {
        let slot = self.find_subscriber(subscriber_id)
            .ok_or(PubSubError::InvalidSubscriber)?;
        
        unsafe {
            let entry = &*self.entries_ptr().add(slot);
            Ok(entry.read_index.load(Ordering::Acquire))
        }
    }
    
    /// Advance subscriber's read index
    pub fn advance_read_index(&mut self, subscriber_id: u64) -> Result<u64, PubSubError> {
        let slot = self.find_subscriber(subscriber_id)
            .ok_or(PubSubError::InvalidSubscriber)?;
        
        unsafe {
            let entry = self.entry_at(slot);
            let new_index = entry.read_index.fetch_add(1, Ordering::Relaxed) + 1;
            Ok(new_index)
        }
    }
    
    /// Set subscriber's read index to specific value
    pub fn set_read_index(&mut self, subscriber_id: u64, index: u64) -> Result<(), PubSubError> {
        let slot = self.find_subscriber(subscriber_id)
            .ok_or(PubSubError::InvalidSubscriber)?;
        
        unsafe {
            let entry = self.entry_at(slot);
            entry.read_index.store(index, Ordering::Release);
        }
        
        Ok(())
    }
    
    /// Get minimum read index across all subscribers (for GC)
    pub fn min_read_index(&self) -> u64 {
        let mut min_index = u64::MAX;
        
        unsafe {
            for i in 0..self.max_subscribers as usize {
                let entry = &*self.entries_ptr().add(i);
                if !entry.is_empty() {
                    let index = entry.read_index.load(Ordering::Acquire);
                    min_index = min_index.min(index);
                }
            }
        }
        
        if min_index == u64::MAX {
            0 // No subscribers
        } else {
            min_index
        }
    }
    
    /// Get current subscriber count
    pub fn count(&self) -> u64 {
        self.active_count.load(Ordering::Acquire)
    }
    
    /// Get maximum subscribers
    pub fn max_count(&self) -> u64 {
        self.max_subscribers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscriber_list() {
        let size = SubscriberList::size_for_max_subscribers(10);
        let mut memory = vec![0u8; size];
        
        let list = unsafe {
            SubscriberList::init(memory.as_mut_ptr(), 10)
        };
        
        let list_ref = unsafe { &mut *list };
        
        // Add subscribers
        assert!(list_ref.add_subscriber(1).is_ok());
        assert!(list_ref.add_subscriber(2).is_ok());
        assert_eq!(list_ref.count(), 2);
        
        // Check read indices
        assert_eq!(list_ref.get_read_index(1).unwrap(), 0);
        assert_eq!(list_ref.get_read_index(2).unwrap(), 0);
        
        // Advance read index
        assert_eq!(list_ref.advance_read_index(1).unwrap(), 1);
        assert_eq!(list_ref.get_read_index(1).unwrap(), 1);
        
        // Remove subscriber
        assert!(list_ref.remove_subscriber(1).unwrap());
        assert_eq!(list_ref.count(), 1);
        assert!(list_ref.get_read_index(1).is_err());
        
        // Min read index
        list_ref.set_read_index(2, 5).unwrap();
        assert_eq!(list_ref.min_read_index(), 5);
    }
    
    #[test]
    fn test_subscriber_list_capacity() {
        let size = SubscriberList::size_for_max_subscribers(2);
        let mut memory = vec![0u8; size];
        
        let list = unsafe {
            SubscriberList::init(memory.as_mut_ptr(), 2)
        };
        
        let list_ref = unsafe { &mut *list };
        
        // Fill to capacity
        assert!(list_ref.add_subscriber(1).is_ok());
        assert!(list_ref.add_subscriber(2).is_ok());
        
        // Should fail when full
        assert_eq!(list_ref.add_subscriber(3), Err(PubSubError::TooManySubscribers));
        
        // Should succeed after removal
        list_ref.remove_subscriber(1).unwrap();
        assert!(list_ref.add_subscriber(3).is_ok());
    }
}
