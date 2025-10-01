//! Pub/Sub system with topic-based message delivery
//!
//! Provides high-performance topic-based messaging using MPMC rings
//! with per-subscriber read indices for efficient delivery.

pub mod registry;
pub mod subscriber;
pub mod topic;

pub use registry::*;
pub use subscriber::*;
pub use topic::*;

/// Pub/Sub configuration
#[derive(Debug, Clone)]
pub struct PubSubConfig {
    pub max_topics: usize,
    pub default_topic_capacity: usize,
    pub max_subscribers_per_topic: usize,
    pub max_topic_name_len: usize,
    pub message_retention_policy: RetentionPolicy,
}

impl Default for PubSubConfig {
    fn default() -> Self {
        Self {
            max_topics: 1024,
            default_topic_capacity: 4096,
            max_subscribers_per_topic: 1000,
            max_topic_name_len: 256,
            message_retention_policy: RetentionPolicy::DropOldest,
        }
    }
}

/// Message retention policy when topic ring is full
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetentionPolicy {
    /// Drop oldest messages to make room for new ones
    DropOldest,
    /// Drop newest messages when full
    DropNewest,
    /// Block publisher until space is available
    Block,
}

/// Pub/Sub statistics
#[derive(Debug, Clone)]
pub struct PubSubStats {
    pub total_topics: u64,
    pub total_subscribers: u64,
    pub total_messages_published: u64,
    pub total_messages_delivered: u64,
    pub total_messages_dropped: u64,
}

/// Pub/Sub errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PubSubError {
    TopicNotFound,
    TopicExists,
    TooManyTopics,
    TooManySubscribers,
    TopicNameTooLong,
    OutOfMemory,
    RingFull,
    InvalidSubscriber,
}

impl std::fmt::Display for PubSubError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PubSubError::TopicNotFound => write!(f, "Topic not found"),
            PubSubError::TopicExists => write!(f, "Topic already exists"),
            PubSubError::TooManyTopics => write!(f, "Too many topics"),
            PubSubError::TooManySubscribers => write!(f, "Too many subscribers"),
            PubSubError::TopicNameTooLong => write!(f, "Topic name too long"),
            PubSubError::OutOfMemory => write!(f, "Out of memory"),
            PubSubError::RingFull => write!(f, "Topic ring buffer full"),
            PubSubError::InvalidSubscriber => write!(f, "Invalid subscriber"),
        }
    }
}

impl std::error::Error for PubSubError {}
