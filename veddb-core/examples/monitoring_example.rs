//! Example demonstrating VedDB monitoring and metrics system
//!
//! This example shows how to:
//! - Initialize the monitoring system
//! - Record operations and metrics
//! - Start Prometheus and health check servers
//! - Use structured logging and audit logging

use veddb_core::monitoring::{
    init_global_logging, get_metrics, get_audit_logger, get_slow_query_logger,
    start_prometheus_server, start_health_server,
    LoggingConfig, OperationType, AuditEvent,
};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging system
    let logging_config = LoggingConfig {
        level: "INFO".to_string(),
        json_format: false,
        slow_query_logging: true,
        slow_query_threshold_ms: 100,
        log_file: None,
        audit_logging: true,
    };
    
    init_global_logging(logging_config)?;
    
    // Initialize metrics
    let metrics = veddb_core::monitoring::init_metrics();
    
    println!("VedDB Monitoring System Example");
    println!("================================");
    
    // Start Prometheus server in background
    let prometheus_metrics = metrics.clone();
    tokio::spawn(async move {
        if let Err(e) = start_prometheus_server(prometheus_metrics, 9090).await {
            eprintln!("Prometheus server error: {}", e);
        }
    });
    
    // Start health check server in background
    let health_metrics = metrics.clone();
    tokio::spawn(async move {
        if let Err(e) = start_health_server(health_metrics, 8080).await {
            eprintln!("Health server error: {}", e);
        }
    });
    
    println!("Started Prometheus server on port 9090");
    println!("Started health check server on port 8080");
    println!("Try: curl http://localhost:9090/metrics");
    println!("Try: curl http://localhost:8080/health");
    
    // Simulate some database operations
    println!("\nSimulating database operations...");
    
    for i in 0..100 {
        // Simulate different types of operations
        let operation_type = match i % 4 {
            0 => OperationType::Read,
            1 => OperationType::Write,
            2 => OperationType::Delete,
            _ => OperationType::Query,
        };
        
        // Simulate operation latency
        let latency = Duration::from_millis(1 + (i % 10));
        
        // Record the operation
        metrics.record_operation(operation_type, latency);
        
        // Simulate cache hits/misses
        if i % 3 == 0 {
            metrics.record_cache_hit();
        } else {
            metrics.record_cache_miss();
        }
        
        // Simulate connections
        if i % 10 == 0 {
            metrics.record_connection_opened();
        }
        
        // Update memory usage
        metrics.update_memory_usage(
            1024 * 1024 * (100 + i), // Total memory
            1024 * 1024 * (50 + i/2), // Cache memory
            1024 * 1024 * (50 + i/2), // Persistent memory
        );
        
        sleep(Duration::from_millis(10)).await;
    }
    
    // Demonstrate audit logging
    println!("\nDemonstrating audit logging...");
    let audit_logger = get_audit_logger();
    
    audit_logger.log_authentication("admin", true, "127.0.0.1");
    audit_logger.log_authentication("user", false, "192.168.1.100");
    audit_logger.log_authorization("admin", "CREATE_USER", "users", true);
    audit_logger.log_admin_operation("admin", "BACKUP", "Created backup snapshot-001");
    
    // Demonstrate slow query logging
    println!("Demonstrating slow query logging...");
    let slow_query_logger = get_slow_query_logger();
    
    let mut tracker = slow_query_logger.start_query(
        "SELECT * FROM large_table WHERE complex_condition = ?".to_string(),
        Some("large_table".to_string()),
        Some("127.0.0.1".to_string()),
        Some("admin".to_string()),
    );
    
    // Simulate slow query by setting start time in the past
    tracker.start_time = std::time::Instant::now() - Duration::from_millis(150);
    slow_query_logger.finish_query(tracker);
    
    // Display metrics snapshot
    println!("\nCurrent metrics snapshot:");
    let snapshot = metrics.snapshot();
    println!("  Operations per second: {:.2}", snapshot.ops_per_second);
    println!("  Active connections: {}", snapshot.active_connections);
    println!("  Cache hit rate: {:.1}%", snapshot.cache_hit_rate);
    println!("  Memory usage: {:.2} MB", snapshot.memory_usage as f64 / (1024.0 * 1024.0));
    println!("  Read latency p99: {:.2}ms", snapshot.read_latency.p99.as_millis());
    
    // Display audit statistics
    let audit_stats = audit_logger.get_stats();
    println!("\nAudit statistics:");
    println!("  Total events: {}", audit_stats.total_events);
    println!("  Authentication successes: {}", audit_stats.authentication_success);
    println!("  Authentication failures: {}", audit_stats.authentication_failure);
    println!("  Admin operations: {}", audit_stats.admin_operations);
    
    // Display slow query statistics
    let slow_query_stats = slow_query_logger.get_stats();
    println!("\nSlow query statistics:");
    println!("  Total slow queries: {}", slow_query_stats.total_count);
    println!("  Average duration: {}ms", slow_query_stats.avg_duration_ms);
    println!("  Max duration: {}ms", slow_query_stats.max_duration_ms);
    
    println!("\nMonitoring example completed!");
    println!("Servers will continue running. Press Ctrl+C to exit.");
    
    // Keep the servers running
    loop {
        sleep(Duration::from_secs(1)).await;
    }
}