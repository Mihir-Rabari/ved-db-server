//! Tests for the pub/sub system

use super::*;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_basic_pubsub() {
    let config = PubSubConfig::default();
    let pubsub = PubSubSystem::new(config);

    // Subscribe to a channel
    pubsub.subscribe(1, "test-channel").await.unwrap();

    // Publish a message
    let delivered = pubsub
        .publish("test-channel", b"hello world".to_vec())
        .await
        .unwrap();

    assert_eq!(delivered, 1);

    // Get the message
    let subscriber = pubsub.get_subscriber(1).unwrap();
    let message = subscriber.get_next_message().await.unwrap();
    assert_eq!(message.channel, "test-channel");
    assert_eq!(message.payload, b"hello world");
}

#[tokio::test]
async fn test_pattern_subscriptions() {
    let config = PubSubConfig::default();
    let pubsub = PubSubSystem::new(config);

    // Subscribe to pattern "events.*"
    pubsub.psubscribe(1, "events.*").await.unwrap();

    // Publish to matching channels
    let delivered1 = pubsub
        .publish("events.user", b"user event".to_vec())
        .await
        .unwrap();
    assert_eq!(delivered1, 1);

    let delivered2 = pubsub
        .publish("events.order", b"order event".to_vec())
        .await
        .unwrap();
    assert_eq!(delivered2, 1);

    // Publish to non-matching channel
    let delivered3 = pubsub
        .publish("notifications.email", b"email notification".to_vec())
        .await
        .unwrap();
    assert_eq!(delivered3, 0);

    // Check messages received
    let subscriber = pubsub.get_subscriber(1).unwrap();
    
    let msg1 = subscriber.get_next_message().await.unwrap();
    assert_eq!(msg1.channel, "events.user");
    assert_eq!(msg1.payload, b"user event");

    let msg2 = subscriber.get_next_message().await.unwrap();
    assert_eq!(msg2.channel, "events.order");
    assert_eq!(msg2.payload, b"order event");

    // No more messages
    assert!(subscriber.get_next_message().await.is_none());
}

#[tokio::test]
async fn test_wildcard_patterns() {
    let config = PubSubConfig::default();
    let pubsub = PubSubSystem::new(config);

    // Test various wildcard patterns
    pubsub.psubscribe(1, "user.*").await.unwrap();
    pubsub.psubscribe(2, "*.created").await.unwrap();
    pubsub.psubscribe(3, "order.*.status").await.unwrap();

    // Test "user.*" pattern
    pubsub.publish("user.login", b"login".to_vec()).await.unwrap();
    pubsub.publish("user.logout", b"logout".to_vec()).await.unwrap();
    pubsub.publish("admin.login", b"admin".to_vec()).await.unwrap(); // Should not match

    // Test "*.created" pattern
    pubsub.publish("user.created", b"user created".to_vec()).await.unwrap();
    pubsub.publish("order.created", b"order created".to_vec()).await.unwrap();
    pubsub.publish("user.updated", b"user updated".to_vec()).await.unwrap(); // Should not match

    // Test "order.*.status" pattern
    pubsub.publish("order.123.status", b"pending".to_vec()).await.unwrap();
    pubsub.publish("order.456.status", b"completed".to_vec()).await.unwrap();
    pubsub.publish("order.status", b"invalid".to_vec()).await.unwrap(); // Should not match

    // Verify subscriber 1 received user.* messages
    let sub1 = pubsub.get_subscriber(1).unwrap();
    
    let msg1 = sub1.get_next_message().await.unwrap();
    assert_eq!(msg1.channel, "user.login");
    
    let msg2 = sub1.get_next_message().await.unwrap();
    assert_eq!(msg2.channel, "user.logout");
    
    let msg3 = sub1.get_next_message().await.unwrap();
    assert_eq!(msg3.channel, "user.created"); // This also matches user.*
    
    let msg4 = sub1.get_next_message().await.unwrap();
    assert_eq!(msg4.channel, "user.updated"); // This also matches user.*
    
    assert!(sub1.get_next_message().await.is_none());

    // Verify subscriber 2 received *.created messages
    let sub2 = pubsub.get_subscriber(2).unwrap();
    let msg3 = sub2.get_next_message().await.unwrap();
    assert_eq!(msg3.channel, "user.created");
    let msg4 = sub2.get_next_message().await.unwrap();
    assert_eq!(msg4.channel, "order.created");
    assert!(sub2.get_next_message().await.is_none());

    // Verify subscriber 3 received order.*.status messages
    let sub3 = pubsub.get_subscriber(3).unwrap();
    let msg5 = sub3.get_next_message().await.unwrap();
    assert_eq!(msg5.channel, "order.123.status");
    let msg6 = sub3.get_next_message().await.unwrap();
    assert_eq!(msg6.channel, "order.456.status");
    assert!(sub3.get_next_message().await.is_none());
}

#[tokio::test]
async fn test_multiple_subscribers() {
    let config = PubSubConfig::default();
    let pubsub = PubSubSystem::new(config);

    // Multiple subscribers to same channel
    pubsub.subscribe(1, "broadcast").await.unwrap();
    pubsub.subscribe(2, "broadcast").await.unwrap();
    pubsub.subscribe(3, "broadcast").await.unwrap();

    // Publish message
    let delivered = pubsub
        .publish("broadcast", b"message to all".to_vec())
        .await
        .unwrap();
    assert_eq!(delivered, 3);

    // All subscribers should receive the message
    for subscriber_id in [1, 2, 3] {
        let subscriber = pubsub.get_subscriber(subscriber_id).unwrap();
        let message = subscriber.get_next_message().await.unwrap();
        assert_eq!(message.channel, "broadcast");
        assert_eq!(message.payload, b"message to all");
    }
}

#[tokio::test]
async fn test_queue_overflow() {
    let mut config = PubSubConfig::default();
    config.default_buffer_size = 2; // Small buffer for testing
    let pubsub = PubSubSystem::new(config);

    pubsub.subscribe(1, "overflow-test").await.unwrap();

    // Fill the queue beyond capacity
    pubsub.publish("overflow-test", b"msg1".to_vec()).await.unwrap();
    pubsub.publish("overflow-test", b"msg2".to_vec()).await.unwrap();
    pubsub.publish("overflow-test", b"msg3".to_vec()).await.unwrap(); // Should drop oldest

    let subscriber = pubsub.get_subscriber(1).unwrap();
    
    // Should have msg2 and msg3 (msg1 was dropped)
    let msg1 = subscriber.get_next_message().await.unwrap();
    assert_eq!(msg1.payload, b"msg2");
    
    let msg2 = subscriber.get_next_message().await.unwrap();
    assert_eq!(msg2.payload, b"msg3");
    
    assert!(subscriber.get_next_message().await.is_none());
}

#[tokio::test]
async fn test_unsubscribe() {
    let config = PubSubConfig::default();
    let pubsub = PubSubSystem::new(config);

    // Subscribe and then unsubscribe
    pubsub.subscribe(1, "temp-channel").await.unwrap();
    
    // Publish before unsubscribe
    let delivered1 = pubsub
        .publish("temp-channel", b"before unsubscribe".to_vec())
        .await
        .unwrap();
    assert_eq!(delivered1, 1);

    // Unsubscribe
    let unsubscribed = pubsub.unsubscribe(1, "temp-channel").await.unwrap();
    assert!(unsubscribed);

    // Publish after unsubscribe
    let delivered2 = pubsub
        .publish("temp-channel", b"after unsubscribe".to_vec())
        .await
        .unwrap();
    assert_eq!(delivered2, 0);
}

#[tokio::test]
async fn test_pattern_unsubscribe() {
    let config = PubSubConfig::default();
    let pubsub = PubSubSystem::new(config);

    // Subscribe to pattern
    pubsub.psubscribe(1, "test.*").await.unwrap();
    
    // Publish to matching channel
    let delivered1 = pubsub
        .publish("test.channel", b"before unsubscribe".to_vec())
        .await
        .unwrap();
    assert_eq!(delivered1, 1);

    // Unsubscribe from pattern
    let unsubscribed = pubsub.punsubscribe(1, "test.*").await.unwrap();
    assert!(unsubscribed);

    // Publish after unsubscribe
    let delivered2 = pubsub
        .publish("test.channel", b"after unsubscribe".to_vec())
        .await
        .unwrap();
    assert_eq!(delivered2, 0);
}

#[tokio::test]
async fn test_message_size_limit() {
    let mut config = PubSubConfig::default();
    config.max_message_size = 100; // Small limit for testing
    let pubsub = PubSubSystem::new(config);

    pubsub.subscribe(1, "size-test").await.unwrap();

    // Message within limit
    let small_msg = vec![b'a'; 50];
    let result1 = pubsub.publish("size-test", small_msg).await;
    assert!(result1.is_ok());

    // Message exceeding limit
    let large_msg = vec![b'b'; 200];
    let result2 = pubsub.publish("size-test", large_msg).await;
    assert_eq!(result2.unwrap_err(), PubSubError::MessageTooLarge);
}

#[tokio::test]
async fn test_statistics() {
    let config = PubSubConfig::default();
    let pubsub = PubSubSystem::new(config);

    // Initial stats
    let stats1 = pubsub.get_stats();
    assert_eq!(stats1.total_channels, 0);
    assert_eq!(stats1.total_subscribers, 0);

    // Add subscribers and channels
    pubsub.subscribe(1, "channel1").await.unwrap();
    pubsub.subscribe(2, "channel2").await.unwrap();
    pubsub.psubscribe(3, "pattern.*").await.unwrap();

    // Publish messages
    pubsub.publish("channel1", b"msg1".to_vec()).await.unwrap();
    pubsub.publish("channel2", b"msg2".to_vec()).await.unwrap();
    pubsub.publish("pattern.test", b"msg3".to_vec()).await.unwrap();

    let stats2 = pubsub.get_stats();
    assert_eq!(stats2.total_channels, 2); // channel1, channel2
    assert_eq!(stats2.total_subscribers, 3); // 1, 2, 3
    assert_eq!(stats2.total_messages_published, 3);
    assert_eq!(stats2.total_messages_delivered, 3); // 2 direct + 1 pattern match
}