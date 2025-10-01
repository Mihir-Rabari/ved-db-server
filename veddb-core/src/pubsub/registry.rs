//! Topic registry for managing topics in shared memory

use super::topic::Topic;
use super::{PubSubConfig, PubSubError};
use crate::arena::Arena;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

/// Topic registry entry
#[repr(C)]
#[derive(Debug)]
struct TopicEntry {
    /// Topic name hash for fast lookup
    name_hash: u64,
    /// Offset to topic in shared memory
    topic_offset: u64,
    /// Topic size in bytes
    topic_size: u64,
    /// Entry flags (0 = empty, 1 = active)
    flags: u32,
    /// Reserved
    reserved: u32,
}

impl TopicEntry {
    fn new() -> Self {
        Self {
            name_hash: 0,
            topic_offset: 0,
            topic_size: 0,
            flags: 0,
            reserved: 0,
        }
    }

    fn is_empty(&self) -> bool {
        self.flags == 0
    }

    fn is_active(&self) -> bool {
        self.flags == 1
    }

    fn mark_active(&mut self) {
        self.flags = 1;
    }

    fn mark_empty(&mut self) {
        self.flags = 0;
        self.name_hash = 0;
        self.topic_offset = 0;
        self.topic_size = 0;
    }
}

/// Topic registry in shared memory
#[repr(C)]
pub struct TopicRegistry {
    /// Maximum number of topics
    max_topics: u64,
    /// Current number of active topics
    active_topics: AtomicU64,
    /// Registry lock for modifications
    lock: RwLock<()>,
    /// Statistics
    total_topics_created: AtomicU64,
    total_topics_deleted: AtomicU64,
    // Topic entries follow this structure
}

impl TopicRegistry {
    /// Calculate size needed for topic registry
    pub fn size_for_max_topics(max_topics: usize) -> usize {
        std::mem::size_of::<Self>() + max_topics * std::mem::size_of::<TopicEntry>()
    }

    /// Initialize topic registry in shared memory
    ///
    /// # Safety
    /// - ptr must point to valid memory of sufficient size
    pub unsafe fn init(ptr: *mut u8, max_topics: usize) -> *mut Self {
        let registry = ptr as *mut Self;

        std::ptr::write(
            registry,
            Self {
                max_topics: max_topics as u64,
                active_topics: AtomicU64::new(0),
                lock: RwLock::new(()),
                total_topics_created: AtomicU64::new(0),
                total_topics_deleted: AtomicU64::new(0),
            },
        );

        // Initialize topic entries
        let entries_ptr = ptr.add(std::mem::size_of::<Self>()) as *mut TopicEntry;
        for i in 0..max_topics {
            std::ptr::write(entries_ptr.add(i), TopicEntry::new());
        }

        registry
    }

    /// Get pointer to topic entries
    unsafe fn entries_ptr(&self) -> *mut TopicEntry {
        let self_ptr = self as *const Self as *mut u8;
        self_ptr.add(std::mem::size_of::<Self>()) as *mut TopicEntry
    }

    /// Get topic entry at index
    unsafe fn entry_at(&self, index: usize) -> &mut TopicEntry {
        let entries = self.entries_ptr();
        &mut *entries.add(index)
    }

    /// Hash a topic name
    fn hash_name(&self, name: &str) -> u64 {
        // Simple FNV-1a hash
        let mut hash = 0xcbf29ce484222325u64;
        for byte in name.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        // Ensure hash is never 0 (reserved for empty)
        if hash == 0 {
            hash = 1;
        }
        hash
    }

    /// Find topic entry by name
    fn find_topic_entry(&self, name: &str) -> Option<usize> {
        let name_hash = self.hash_name(name);

        unsafe {
            for i in 0..self.max_topics as usize {
                let entry = &*self.entries_ptr().add(i);
                if entry.is_active() && entry.name_hash == name_hash {
                    // Verify name matches (handle hash collisions)
                    if let Some(topic) = self.get_topic_at_offset(entry.topic_offset) {
                        if topic.desc().name() == name {
                            return Some(i);
                        }
                    }
                }
            }
        }
        None
    }

    /// Find empty entry slot
    fn find_empty_entry(&self) -> Option<usize> {
        unsafe {
            for i in 0..self.max_topics as usize {
                let entry = &*self.entries_ptr().add(i);
                if entry.is_empty() {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Get topic at given offset
    unsafe fn get_topic_at_offset(&self, offset: u64) -> Option<&Topic> {
        if offset == 0 {
            return None;
        }

        // This is a simplified approach - in practice you'd need proper
        // offset-to-pointer conversion within the shared memory segment
        let topic_ptr = offset as *const Topic;
        Some(&*topic_ptr)
    }

    /// Create a new topic
    pub fn create_topic(
        &self,
        name: &str,
        config: &PubSubConfig,
        arena: *mut Arena,
    ) -> Result<(), PubSubError> {
        if name.len() > config.max_topic_name_len {
            return Err(PubSubError::TopicNameTooLong);
        }

        let _guard = self.lock.write();

        // Check if topic already exists
        if self.find_topic_entry(name).is_some() {
            return Err(PubSubError::TopicExists);
        }

        // Check capacity
        if self.active_topics.load(Ordering::Relaxed) >= self.max_topics {
            return Err(PubSubError::TooManyTopics);
        }

        // Find empty slot
        let entry_index = self.find_empty_entry().ok_or(PubSubError::TooManyTopics)?;

        // Calculate topic size
        let topic_size = Topic::size_for_config(
            config.default_topic_capacity as u64,
            config.max_subscribers_per_topic as u32,
        );

        // Allocate space for topic
        let arena_ref = unsafe { &*arena };
        let topic_offset = arena_ref.allocate(topic_size, 64);
        if topic_offset == 0 {
            return Err(PubSubError::OutOfMemory);
        }

        // Initialize topic
        unsafe {
            let topic_ptr = arena_ref.offset_to_ptr(topic_offset);
            let _topic = Topic::init(
                topic_ptr,
                name,
                config.default_topic_capacity as u64,
                config.max_subscribers_per_topic as u32,
                arena,
            )?;

            // Update registry entry
            let entry = self.entry_at(entry_index);
            entry.name_hash = self.hash_name(name);
            entry.topic_offset = topic_offset;
            entry.topic_size = topic_size as u64;
            entry.mark_active();
        }

        self.active_topics.fetch_add(1, Ordering::Relaxed);
        self.total_topics_created.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Delete a topic
    pub fn delete_topic(&self, name: &str, arena: *mut Arena) -> Result<(), PubSubError> {
        let _guard = self.lock.write();

        let entry_index = self
            .find_topic_entry(name)
            .ok_or(PubSubError::TopicNotFound)?;

        unsafe {
            let entry = self.entry_at(entry_index);
            let topic_offset = entry.topic_offset;
            let topic_size = entry.topic_size;

            // Free topic memory
            let arena_ref = &*arena;
            arena_ref.free(topic_offset, topic_size as usize);

            // Clear registry entry
            entry.mark_empty();
        }

        self.active_topics.fetch_sub(1, Ordering::Relaxed);
        self.total_topics_deleted.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Get topic by name
    pub fn get_topic(&self, name: &str) -> Result<&Topic, PubSubError> {
        let _guard = self.lock.read();

        let entry_index = self
            .find_topic_entry(name)
            .ok_or(PubSubError::TopicNotFound)?;

        unsafe {
            let entry = &*self.entries_ptr().add(entry_index);
            self.get_topic_at_offset(entry.topic_offset)
                .ok_or(PubSubError::TopicNotFound)
        }
    }

    /// List all active topics
    pub fn list_topics(&self) -> Vec<String> {
        let _guard = self.lock.read();
        let mut topics = Vec::new();

        unsafe {
            for i in 0..self.max_topics as usize {
                let entry = &*self.entries_ptr().add(i);
                if entry.is_active() {
                    if let Some(topic) = self.get_topic_at_offset(entry.topic_offset) {
                        topics.push(topic.desc().name().to_string());
                    }
                }
            }
        }

        topics
    }

    /// Get registry statistics
    pub fn stats(&self) -> RegistryStats {
        RegistryStats {
            max_topics: self.max_topics,
            active_topics: self.active_topics.load(Ordering::Relaxed),
            total_topics_created: self.total_topics_created.load(Ordering::Relaxed),
            total_topics_deleted: self.total_topics_deleted.load(Ordering::Relaxed),
        }
    }
}

/// Registry statistics
#[derive(Debug, Clone)]
pub struct RegistryStats {
    pub max_topics: u64,
    pub active_topics: u64,
    pub total_topics_created: u64,
    pub total_topics_deleted: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::ArenaBuffer;

    #[test]
    fn test_topic_registry() {
        let arena_buf = ArenaBuffer::new(65536);
        let arena = arena_buf.arena() as *const Arena as *mut Arena;

        let registry_size = TopicRegistry::size_for_max_topics(10);
        let mut registry_memory = vec![0u8; registry_size];

        let registry = unsafe { TopicRegistry::init(registry_memory.as_mut_ptr(), 10) };

        let registry_ref = unsafe { &*registry };
        let config = PubSubConfig::default();

        // Create topics
        assert!(registry_ref.create_topic("topic1", &config, arena).is_ok());
        assert!(registry_ref.create_topic("topic2", &config, arena).is_ok());

        // Check stats
        let stats = registry_ref.stats();
        assert_eq!(stats.active_topics, 2);
        assert_eq!(stats.total_topics_created, 2);

        // List topics
        let topics = registry_ref.list_topics();
        assert_eq!(topics.len(), 2);
        assert!(topics.contains(&"topic1".to_string()));
        assert!(topics.contains(&"topic2".to_string()));

        // Get topic
        assert!(registry_ref.get_topic("topic1").is_ok());
        assert!(registry_ref.get_topic("nonexistent").is_err());

        // Delete topic
        assert!(registry_ref.delete_topic("topic1", arena).is_ok());
        assert_eq!(registry_ref.stats().active_topics, 1);
        assert!(registry_ref.get_topic("topic1").is_err());

        // Try to create duplicate
        assert_eq!(
            registry_ref.create_topic("topic2", &config, arena),
            Err(PubSubError::TopicExists)
        );
    }
}
