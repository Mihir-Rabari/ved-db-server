//! Pub/Sub messaging system for VedDB v0.2.0
//!
//! Provides Redis-like publish-subscribe messaging with:
//! - Named channels for exact subscriptions
//! - Pattern-based subscriptions with wildcard matching
//! - Per-subscriber message queues with configurable buffer sizes
//! - Message delivery within 10ms latency target

use dashmap::DashMap;
use regex::Regex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, RwLock};

/// Unique identifier for subscribers
pub type SubscriberId = u64;

/// Message payload with metadata
#[derive(Debug, Clone)]
pub struct Message {
    /// Channel name where message was published
    pub channel: String,
    /// Message payload (up to 1MB per requirement)
    pub payload: Vec<u8>,
    /// Timestamp when message was published
    pub timestamp: u64,
    /// Message ID for deduplication
    pub id: u64,
}

impl Message {
    pub fn new(channel: String, payload: Vec<u8>) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        // Simple message ID generation (in production, use proper ID generation)
        static MESSAGE_COUNTER: AtomicU64 = AtomicU64::new(1);
        let id = MESSAGE_COUNTER.fetch_add(1, Ordering::Relaxed);
        
        Self {
            channel,
            payload,
            timestamp,
            id,
        }
    }
}

/// Subscriber with ring buffer message queue
#[derive(Debug)]
pub struct Subscriber {
    /// Unique subscriber ID
    pub id: SubscriberId,
    /// Message queue with configurable buffer size (default 1000)
    queue: RwLock<VecDeque<Message>>,
    /// Maximum queue size
    max_queue_size: usize,
    /// Channel for async message delivery
    sender: Option<mpsc::UnboundedSender<Message>>,
    /// Statistics
    messages_received: AtomicU64,
    messages_dropped: AtomicU64,
}

impl Subscriber {
    pub fn new(id: SubscriberId, max_queue_size: usize) -> Self {
        Self {
            id,
            queue: RwLock::new(VecDeque::with_capacity(max_queue_size)),
            max_queue_size,
            sender: None,
            messages_received: AtomicU64::new(0),
            messages_dropped: AtomicU64::new(0),
        }
    }

    pub fn with_sender(id: SubscriberId, max_queue_size: usize, sender: mpsc::UnboundedSender<Message>) -> Self {
        Self {
            id,
            queue: RwLock::new(VecDeque::with_capacity(max_queue_size)),
            max_queue_size,
            sender: Some(sender),
            messages_received: AtomicU64::new(0),
            messages_dropped: AtomicU64::new(0),
        }
    }

    /// Add message to subscriber's queue
    pub async fn deliver_message(&self, message: Message) -> Result<(), PubSubError> {
        // Try async delivery first if sender is available
        if let Some(sender) = &self.sender {
            if sender.send(message.clone()).is_ok() {
                self.messages_received.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
        }

        // Fallback to queue-based delivery
        let mut queue = self.queue.write().await;
        
        // Check if queue is full
        if queue.len() >= self.max_queue_size {
            // Drop oldest message to make room (requirement 7.6)
            queue.pop_front();
            self.messages_dropped.fetch_add(1, Ordering::Relaxed);
        }
        
        queue.push_back(message);
        self.messages_received.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Get next message from queue (non-blocking)
    pub async fn get_next_message(&self) -> Option<Message> {
        let mut queue = self.queue.write().await;
        queue.pop_front()
    }

    /// Get queue length
    pub async fn queue_len(&self) -> usize {
        let queue = self.queue.read().await;
        queue.len()
    }

    /// Get statistics
    pub fn stats(&self) -> SubscriberStats {
        SubscriberStats {
            id: self.id,
            messages_received: self.messages_received.load(Ordering::Relaxed),
            messages_dropped: self.messages_dropped.load(Ordering::Relaxed),
        }
    }
}

/// Channel for managing subscribers to a specific topic
#[derive(Debug)]
pub struct Channel {
    /// Channel name
    pub name: String,
    /// Subscribers to this channel
    subscribers: DashMap<SubscriberId, Arc<Subscriber>>,
    /// Channel statistics
    message_count: AtomicU64,
    subscriber_count: AtomicU64,
}

impl Channel {
    pub fn new(name: String) -> Self {
        Self {
            name,
            subscribers: DashMap::new(),
            message_count: AtomicU64::new(0),
            subscriber_count: AtomicU64::new(0),
        }
    }

    /// Add subscriber to channel
    pub fn add_subscriber(&self, subscriber: Arc<Subscriber>) {
        if self.subscribers.insert(subscriber.id, subscriber).is_none() {
            self.subscriber_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Remove subscriber from channel
    pub fn remove_subscriber(&self, subscriber_id: SubscriberId) -> bool {
        if self.subscribers.remove(&subscriber_id).is_some() {
            self.subscriber_count.fetch_sub(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Publish message to all subscribers
    pub async fn publish(&self, payload: Vec<u8>) -> Result<usize, PubSubError> {
        let message = Message::new(self.name.clone(), payload);
        let mut delivered = 0;

        // Deliver to all subscribers
        for subscriber_ref in self.subscribers.iter() {
            let subscriber = subscriber_ref.value();
            if subscriber.deliver_message(message.clone()).await.is_ok() {
                delivered += 1;
            }
        }

        self.message_count.fetch_add(1, Ordering::Relaxed);
        Ok(delivered)
    }

    /// Get channel statistics
    pub fn stats(&self) -> ChannelStats {
        ChannelStats {
            name: self.name.clone(),
            subscriber_count: self.subscriber_count.load(Ordering::Relaxed),
            message_count: self.message_count.load(Ordering::Relaxed),
        }
    }

    /// Get subscriber count
    pub fn subscriber_count(&self) -> u64 {
        self.subscriber_count.load(Ordering::Relaxed)
    }
}

/// Pattern subscription for wildcard matching
#[derive(Debug)]
pub struct PatternSubscription {
    /// Subscriber ID
    pub subscriber_id: SubscriberId,
    /// Compiled regex pattern
    pub pattern: Regex,
    /// Original pattern string
    pub pattern_str: String,
}

impl PatternSubscription {
    pub fn new(subscriber_id: SubscriberId, pattern: &str) -> Result<Self, PubSubError> {
        // Convert Redis-style patterns to regex
        // * matches any sequence of characters
        // ? matches any single character
        let regex_pattern = pattern
            .replace("*", ".*")
            .replace("?", ".");
        
        let regex = Regex::new(&format!("^{}$", regex_pattern))
            .map_err(|_| PubSubError::InvalidPattern)?;

        Ok(Self {
            subscriber_id,
            pattern: regex,
            pattern_str: pattern.to_string(),
        })
    }

    /// Check if channel name matches this pattern
    pub fn matches(&self, channel: &str) -> bool {
        self.pattern.is_match(channel)
    }
}

/// Main Pub/Sub system
#[derive(Debug)]
pub struct PubSubSystem {
    /// Named channels for exact subscriptions
    channels: DashMap<String, Arc<Channel>>,
    /// All subscribers by ID
    subscribers: DashMap<SubscriberId, Arc<Subscriber>>,
    /// Pattern-based subscriptions
    pattern_subscriptions: RwLock<Vec<PatternSubscription>>,
    /// Configuration
    config: PubSubConfig,
    /// Global statistics
    stats: PubSubStats,
}

impl PubSubSystem {
    pub fn new(config: PubSubConfig) -> Self {
        Self {
            channels: DashMap::new(),
            subscribers: DashMap::new(),
            pattern_subscriptions: RwLock::new(Vec::new()),
            config,
            stats: PubSubStats::default(),
        }
    }

    /// Create or get existing channel
    pub fn get_or_create_channel(&self, name: &str) -> Arc<Channel> {
        self.channels
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Channel::new(name.to_string())))
            .clone()
    }

    /// Subscribe to a specific channel
    pub async fn subscribe(&self, subscriber_id: SubscriberId, channel_name: &str) -> Result<(), PubSubError> {
        // Get or create subscriber
        let subscriber = self.get_or_create_subscriber(subscriber_id);
        
        // Get or create channel
        let channel = self.get_or_create_channel(channel_name);
        
        // Add subscriber to channel
        channel.add_subscriber(subscriber);
        
        Ok(())
    }

    /// Unsubscribe from a specific channel
    pub async fn unsubscribe(&self, subscriber_id: SubscriberId, channel_name: &str) -> Result<bool, PubSubError> {
        if let Some(channel_ref) = self.channels.get(channel_name) {
            let channel = channel_ref.value();
            Ok(channel.remove_subscriber(subscriber_id))
        } else {
            Ok(false)
        }
    }

    /// Subscribe to channels matching a pattern
    pub async fn psubscribe(&self, subscriber_id: SubscriberId, pattern: &str) -> Result<(), PubSubError> {
        // Ensure subscriber exists
        let _subscriber = self.get_or_create_subscriber(subscriber_id);
        
        // Create pattern subscription
        let pattern_sub = PatternSubscription::new(subscriber_id, pattern)?;
        
        // Add to pattern subscriptions
        let mut patterns = self.pattern_subscriptions.write().await;
        patterns.push(pattern_sub);
        
        Ok(())
    }

    /// Unsubscribe from pattern
    pub async fn punsubscribe(&self, subscriber_id: SubscriberId, pattern: &str) -> Result<bool, PubSubError> {
        let mut patterns = self.pattern_subscriptions.write().await;
        let initial_len = patterns.len();
        
        patterns.retain(|p| !(p.subscriber_id == subscriber_id && p.pattern_str == pattern));
        
        Ok(patterns.len() < initial_len)
    }

    /// Publish message to channel and matching patterns
    pub async fn publish(&self, channel_name: &str, payload: Vec<u8>) -> Result<usize, PubSubError> {
        if payload.len() > self.config.max_message_size {
            return Err(PubSubError::MessageTooLarge);
        }

        let mut total_delivered = 0;

        // Publish to exact channel subscribers
        if let Some(channel_ref) = self.channels.get(channel_name) {
            let channel = channel_ref.value();
            total_delivered += channel.publish(payload.clone()).await?;
        }

        // Publish to pattern subscribers
        let patterns = self.pattern_subscriptions.read().await;
        let message = Message::new(channel_name.to_string(), payload);
        
        for pattern_sub in patterns.iter() {
            if pattern_sub.matches(channel_name) {
                if let Some(subscriber_ref) = self.subscribers.get(&pattern_sub.subscriber_id) {
                    let subscriber = subscriber_ref.value();
                    if subscriber.deliver_message(message.clone()).await.is_ok() {
                        total_delivered += 1;
                    }
                }
            }
        }

        // Update global statistics
        self.stats.total_messages_published.fetch_add(1, Ordering::Relaxed);
        self.stats.total_messages_delivered.fetch_add(total_delivered as u64, Ordering::Relaxed);

        Ok(total_delivered)
    }

    /// Get or create subscriber
    fn get_or_create_subscriber(&self, subscriber_id: SubscriberId) -> Arc<Subscriber> {
        self.subscribers
            .entry(subscriber_id)
            .or_insert_with(|| {
                Arc::new(Subscriber::new(subscriber_id, self.config.default_buffer_size))
            })
            .clone()
    }

    /// Get subscriber by ID
    pub fn get_subscriber(&self, subscriber_id: SubscriberId) -> Option<Arc<Subscriber>> {
        self.subscribers.get(&subscriber_id).map(|r| r.value().clone())
    }

    /// Remove subscriber completely
    pub async fn remove_subscriber(&self, subscriber_id: SubscriberId) -> Result<(), PubSubError> {
        // Remove from all channels
        for channel_ref in self.channels.iter() {
            let channel = channel_ref.value();
            channel.remove_subscriber(subscriber_id);
        }

        // Remove from pattern subscriptions
        let mut patterns = self.pattern_subscriptions.write().await;
        patterns.retain(|p| p.subscriber_id != subscriber_id);

        // Remove from subscribers map
        self.subscribers.remove(&subscriber_id);

        Ok(())
    }

    /// Get system statistics
    pub fn get_stats(&self) -> PubSubStatsSnapshot {
        PubSubStatsSnapshot {
            total_channels: self.channels.len() as u64,
            total_subscribers: self.subscribers.len() as u64,
            total_messages_published: self.stats.total_messages_published.load(Ordering::Relaxed),
            total_messages_delivered: self.stats.total_messages_delivered.load(Ordering::Relaxed),
            total_messages_dropped: self.calculate_total_dropped(),
        }
    }

    /// Calculate total dropped messages across all subscribers
    fn calculate_total_dropped(&self) -> u64 {
        self.subscribers
            .iter()
            .map(|r| r.value().messages_dropped.load(Ordering::Relaxed))
            .sum()
    }

    /// List all channels
    pub fn list_channels(&self) -> Vec<String> {
        self.channels.iter().map(|r| r.key().clone()).collect()
    }

    /// Get channel statistics
    pub fn get_channel_stats(&self, channel_name: &str) -> Option<ChannelStats> {
        self.channels.get(channel_name).map(|r| r.value().stats())
    }
}

/// Pub/Sub configuration
#[derive(Debug, Clone)]
pub struct PubSubConfig {
    /// Maximum message size (default 1MB per requirement)
    pub max_message_size: usize,
    /// Default buffer size per subscriber (default 1000 per requirement)
    pub default_buffer_size: usize,
    /// Maximum number of channels
    pub max_channels: usize,
    /// Maximum number of subscribers
    pub max_subscribers: usize,
}

impl Default for PubSubConfig {
    fn default() -> Self {
        Self {
            max_message_size: 1024 * 1024, // 1MB
            default_buffer_size: 1000,     // 1000 messages
            max_channels: 10000,
            max_subscribers: 100000,
        }
    }
}

/// Pub/Sub statistics (internal with atomics)
#[derive(Debug, Default)]
pub struct PubSubStats {
    pub total_channels: u64,
    pub total_subscribers: u64,
    pub total_messages_published: AtomicU64,
    pub total_messages_delivered: AtomicU64,
    pub total_messages_dropped: u64,
}

/// Pub/Sub statistics snapshot (for external consumption)
#[derive(Debug, Clone)]
pub struct PubSubStatsSnapshot {
    pub total_channels: u64,
    pub total_subscribers: u64,
    pub total_messages_published: u64,
    pub total_messages_delivered: u64,
    pub total_messages_dropped: u64,
}

/// Channel statistics
#[derive(Debug, Clone)]
pub struct ChannelStats {
    pub name: String,
    pub subscriber_count: u64,
    pub message_count: u64,
}

/// Subscriber statistics
#[derive(Debug, Clone)]
pub struct SubscriberStats {
    pub id: SubscriberId,
    pub messages_received: u64,
    pub messages_dropped: u64,
}

/// Pub/Sub errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PubSubError {
    ChannelNotFound,
    SubscriberNotFound,
    MessageTooLarge,
    InvalidPattern,
    QueueFull,
    SystemError(String),
}

impl std::fmt::Display for PubSubError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PubSubError::ChannelNotFound => write!(f, "Channel not found"),
            PubSubError::SubscriberNotFound => write!(f, "Subscriber not found"),
            PubSubError::MessageTooLarge => write!(f, "Message exceeds maximum size"),
            PubSubError::InvalidPattern => write!(f, "Invalid pattern syntax"),
            PubSubError::QueueFull => write!(f, "Subscriber queue is full"),
            PubSubError::SystemError(msg) => write!(f, "System error: {}", msg),
        }
    }
}

impl std::error::Error for PubSubError {}

#[cfg(test)]
mod tests;

