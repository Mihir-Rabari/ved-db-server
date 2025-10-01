//! Topic implementation with MPMC ring and subscriber management

use super::{subscriber::SubscriberList, PubSubError, RetentionPolicy};
use crate::arena::Arena;
use crate::ring::{MpmcRing, Slot};
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

/// Topic descriptor in shared memory
#[repr(C)]
pub struct TopicDesc {
    /// Topic name (fixed size for simplicity)
    pub name: [u8; 256],
    /// Actual name length
    pub name_len: u32,
    /// Topic flags
    pub flags: u32,
    /// Ring capacity
    pub capacity: u64,
    /// Ring offset in shared memory
    pub ring_offset: u64,
    /// Subscriber list offset
    pub subscribers_offset: u64,
    /// Maximum subscribers
    pub max_subscribers: u32,
    /// Current subscriber count
    pub subscriber_count: AtomicU64,
    /// Message statistics
    pub messages_published: AtomicU64,
    pub messages_dropped: AtomicU64,
    /// Retention policy
    pub retention_policy: RetentionPolicy,
    /// Lock for topic operations
    lock: RwLock<()>,
}

impl TopicDesc {
    pub fn new(name: &str, capacity: u64, max_subscribers: u32) -> Result<Self, PubSubError> {
        if name.len() > 255 {
            return Err(PubSubError::TopicNameTooLong);
        }

        let mut name_bytes = [0u8; 256];
        name_bytes[..name.len()].copy_from_slice(name.as_bytes());

        Ok(Self {
            name: name_bytes,
            name_len: name.len() as u32,
            flags: 0,
            capacity,
            ring_offset: 0,
            subscribers_offset: 0,
            max_subscribers,
            subscriber_count: AtomicU64::new(0),
            messages_published: AtomicU64::new(0),
            messages_dropped: AtomicU64::new(0),
            retention_policy: RetentionPolicy::DropOldest,
            lock: RwLock::new(()),
        })
    }

    pub fn name(&self) -> &str {
        std::str::from_utf8(&self.name[..self.name_len as usize]).unwrap_or("")
    }

    pub fn is_initialized(&self) -> bool {
        self.ring_offset != 0 && self.subscribers_offset != 0
    }
}

/// Topic with MPMC ring and subscriber management
pub struct Topic {
    desc: *mut TopicDesc,
    arena: *mut Arena,
}

impl Topic {
    /// Calculate size needed for topic with given config
    pub fn size_for_config(capacity: u64, max_subscribers: u32) -> usize {
        std::mem::size_of::<TopicDesc>()
            + MpmcRing::size_for_capacity(capacity)
            + SubscriberList::size_for_max_subscribers(max_subscribers as usize)
    }

    /// Initialize topic in shared memory
    ///
    /// # Safety
    /// - ptr must point to valid memory of sufficient size
    /// - arena must be valid and initialized
    pub unsafe fn init(
        ptr: *mut u8,
        name: &str,
        capacity: u64,
        max_subscribers: u32,
        arena: *mut Arena,
    ) -> Result<*mut Self, PubSubError> {
        let topic_desc = TopicDesc::new(name, capacity, max_subscribers)?;
        let desc_ptr = ptr as *mut TopicDesc;
        std::ptr::write(desc_ptr, topic_desc);

        // Initialize MPMC ring
        let ring_ptr = ptr.add(std::mem::size_of::<TopicDesc>());
        MpmcRing::init(ring_ptr, capacity);
        (*desc_ptr).ring_offset = (ring_ptr as usize - ptr as usize) as u64;

        // Initialize subscriber list
        let subscribers_ptr = ring_ptr.add(MpmcRing::size_for_capacity(capacity));
        SubscriberList::init(subscribers_ptr, max_subscribers as usize);
        (*desc_ptr).subscribers_offset = (subscribers_ptr as usize - ptr as usize) as u64;

        // Create topic wrapper
        let topic = Box::new(Topic {
            desc: desc_ptr,
            arena,
        });

        Ok(Box::into_raw(topic))
    }

    /// Get topic descriptor
    pub fn desc(&self) -> &TopicDesc {
        unsafe { &*self.desc }
    }

    /// Get mutable topic descriptor
    pub fn desc_mut(&self) -> &mut TopicDesc {
        unsafe { &mut *self.desc }
    }

    /// Get MPMC ring
    pub fn ring(&self) -> &MpmcRing {
        unsafe {
            let desc = &*self.desc;
            let base_ptr = self.desc as *const u8;
            let ring_ptr = base_ptr.add(desc.ring_offset as usize);
            &*(ring_ptr as *const MpmcRing)
        }
    }

    /// Get subscriber list
    pub fn subscribers(&self) -> &SubscriberList {
        unsafe {
            let desc = &*self.desc;
            let base_ptr = self.desc as *const u8;
            let subs_ptr = base_ptr.add(desc.subscribers_offset as usize);
            &*(subs_ptr as *const SubscriberList)
        }
    }

    /// Get mutable subscriber list
    pub fn subscribers_mut(&self) -> &mut SubscriberList {
        unsafe {
            let desc = &*self.desc;
            let base_ptr = self.desc as *const u8;
            let subs_ptr = base_ptr.add(desc.subscribers_offset as usize);
            &mut *(subs_ptr as *mut SubscriberList)
        }
    }

    /// Publish a message to the topic
    pub fn publish(&self, message: &[u8]) -> Result<(), PubSubError> {
        let _guard = self.desc().lock.write();
        let arena = unsafe { &*self.arena };

        // Allocate space for message in arena
        let msg_offset = arena.allocate(message.len(), 1);
        if msg_offset == 0 {
            return Err(PubSubError::OutOfMemory);
        }

        // Copy message to arena
        unsafe {
            let msg_ptr = arena.offset_to_ptr(msg_offset);
            std::ptr::copy_nonoverlapping(message.as_ptr(), msg_ptr, message.len());
        }

        // Create slot with arena offset
        let slot = Slot::arena_offset(message.len() as u32, msg_offset);

        // Try to push to ring
        let ring = self.ring();
        if ring.try_push(slot) {
            self.desc_mut()
                .messages_published
                .fetch_add(1, Ordering::Relaxed);
            Ok(())
        } else {
            // Handle backpressure based on retention policy
            match self.desc().retention_policy {
                RetentionPolicy::DropOldest => {
                    // Force push by consuming oldest message first
                    let _ = ring.try_pop();
                    ring.push(slot);
                    self.desc_mut()
                        .messages_published
                        .fetch_add(1, Ordering::Relaxed);
                    self.desc_mut()
                        .messages_dropped
                        .fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }
                RetentionPolicy::DropNewest => {
                    // Drop this message
                    unsafe {
                        arena.free(msg_offset, message.len());
                    }
                    self.desc_mut()
                        .messages_dropped
                        .fetch_add(1, Ordering::Relaxed);
                    Err(PubSubError::RingFull)
                }
                RetentionPolicy::Block => {
                    // Block until space is available
                    ring.push(slot);
                    self.desc_mut()
                        .messages_published
                        .fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }
            }
        }
    }

    /// Subscribe to the topic
    pub fn subscribe(&self, subscriber_id: u64) -> Result<(), PubSubError> {
        let _guard = self.desc().lock.write();

        if self.desc().subscriber_count.load(Ordering::Relaxed)
            >= self.desc().max_subscribers as u64
        {
            return Err(PubSubError::TooManySubscribers);
        }

        let subscribers = self.subscribers_mut();
        subscribers.add_subscriber(subscriber_id)?;

        self.desc_mut()
            .subscriber_count
            .fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Unsubscribe from the topic
    pub fn unsubscribe(&self, subscriber_id: u64) -> Result<(), PubSubError> {
        let _guard = self.desc().lock.write();

        let subscribers = self.subscribers_mut();
        if subscribers.remove_subscriber(subscriber_id)? {
            self.desc_mut()
                .subscriber_count
                .fetch_sub(1, Ordering::Relaxed);
        }

        Ok(())
    }

    /// Get next message for a subscriber
    pub fn get_next_message(&self, subscriber_id: u64) -> Result<Option<Vec<u8>>, PubSubError> {
        let _guard = self.desc().lock.read();
        let arena = unsafe { &*self.arena };

        let subscribers = self.subscribers();
        let read_index = subscribers.get_read_index(subscriber_id)?;

        // Check if there are new messages
        let ring = self.ring();
        let current_head = ring.head().load(Ordering::Acquire);

        if read_index >= current_head {
            return Ok(None); // No new messages
        }

        // Try to read message at subscriber's current position
        // This is a simplified approach - in practice you'd need more sophisticated
        // coordination to ensure messages aren't garbage collected while being read
        if let Some(slot) = ring.try_pop() {
            if let Some(offset) = slot.get_arena_offset() {
                unsafe {
                    let msg_ptr = arena.offset_to_ptr(offset);
                    let message = std::slice::from_raw_parts(msg_ptr, slot.len as usize).to_vec();

                    // Update subscriber's read index
                    let subscribers_mut = self.subscribers_mut();
                    subscribers_mut.advance_read_index(subscriber_id)?;

                    Ok(Some(message))
                }
            } else if let Some(inline_data) = slot.get_inline_data() {
                // Update subscriber's read index
                let subscribers_mut = self.subscribers_mut();
                subscribers_mut.advance_read_index(subscriber_id)?;

                Ok(Some(inline_data.to_vec()))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Get topic statistics
    pub fn stats(&self) -> TopicStats {
        let desc = self.desc();
        TopicStats {
            name: desc.name().to_string(),
            capacity: desc.capacity,
            subscriber_count: desc.subscriber_count.load(Ordering::Relaxed),
            messages_published: desc.messages_published.load(Ordering::Relaxed),
            messages_dropped: desc.messages_dropped.load(Ordering::Relaxed),
            ring_length: self.ring().len(),
        }
    }
}

unsafe impl Send for Topic {}
unsafe impl Sync for Topic {}

/// Topic statistics
#[derive(Debug, Clone)]
pub struct TopicStats {
    pub name: String,
    pub capacity: u64,
    pub subscriber_count: u64,
    pub messages_published: u64,
    pub messages_dropped: u64,
    pub ring_length: u64,
}
