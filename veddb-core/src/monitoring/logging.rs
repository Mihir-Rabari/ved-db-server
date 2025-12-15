//! Structured logging system with tracing
//!
//! Provides configurable logging with JSON output and slow query logging

use tracing::Level;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Registry,
};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use std::sync::Arc;
use parking_lot::RwLock;

use chrono::{DateTime, Utc};
use anyhow::Result;

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (ERROR, WARN, INFO, DEBUG, TRACE)
    pub level: String,
    
    /// Enable JSON format output
    pub json_format: bool,
    
    /// Enable slow query logging
    pub slow_query_logging: bool,
    
    /// Slow query threshold in milliseconds
    pub slow_query_threshold_ms: u64,
    
    /// Log file path (optional, logs to stdout if None)
    pub log_file: Option<String>,
    
    /// Enable audit logging
    pub audit_logging: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "INFO".to_string(),
            json_format: false,
            slow_query_logging: true,
            slow_query_threshold_ms: 100,
            log_file: None,
            audit_logging: true,
        }
    }
}

/// Slow query logger
#[derive(Debug)]
pub struct SlowQueryLogger {
    threshold: Duration,
    enabled: bool,
    queries: Arc<RwLock<Vec<SlowQuery>>>,
}

/// Slow query record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowQuery {
    pub timestamp: DateTime<Utc>,
    pub duration_ms: u64,
    pub query: String,
    pub collection: Option<String>,
    pub client_addr: Option<String>,
    pub user: Option<String>,
}

/// Query execution tracker
pub struct QueryTracker {
    start_time: Instant,
    query: String,
    collection: Option<String>,
    client_addr: Option<String>,
    user: Option<String>,
}

/// Audit event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEvent {
    Authentication {
        user: String,
        success: bool,
        client_addr: String,
        timestamp: DateTime<Utc>,
    },
    Authorization {
        user: String,
        operation: String,
        resource: String,
        success: bool,
        timestamp: DateTime<Utc>,
    },
    AdminOperation {
        user: String,
        operation: String,
        details: String,
        timestamp: DateTime<Utc>,
    },
    ConfigurationChange {
        user: String,
        setting: String,
        old_value: String,
        new_value: String,
        timestamp: DateTime<Utc>,
    },
}

/// Audit logger
#[derive(Debug)]
pub struct AuditLogger {
    enabled: bool,
    events: Arc<RwLock<Vec<AuditEvent>>>,
}

impl LoggingConfig {
    /// Parse log level from string
    pub fn parse_level(&self) -> Level {
        match self.level.to_uppercase().as_str() {
            "ERROR" => Level::ERROR,
            "WARN" => Level::WARN,
            "INFO" => Level::INFO,
            "DEBUG" => Level::DEBUG,
            "TRACE" => Level::TRACE,
            _ => Level::INFO,
        }
    }
}

/// Initialize the logging system
pub fn init_logging(config: LoggingConfig) -> Result<(SlowQueryLogger, AuditLogger)> {
    // Create environment filter
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.level));
    
    // Create the subscriber based on configuration
    let subscriber = Registry::default().with(env_filter);
    
    if config.json_format {
        // JSON format for structured logging
        let json_layer = fmt::layer()
            .json()
            .with_span_events(FmtSpan::CLOSE)
            .with_current_span(true)
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true);
        
        subscriber.with(json_layer).init();
    } else {
        // Human-readable format
        let fmt_layer = fmt::layer()
            .with_span_events(FmtSpan::CLOSE)
            .with_target(true)
            .with_thread_ids(false)
            .with_thread_names(false)
            .compact();
        
        subscriber.with(fmt_layer).init();
    }
    
    // Initialize slow query logger
    let slow_query_logger = SlowQueryLogger::new(
        Duration::from_millis(config.slow_query_threshold_ms),
        config.slow_query_logging,
    );
    
    // Initialize audit logger
    let audit_logger = AuditLogger::new(config.audit_logging);
    
    tracing::info!(
        "Logging initialized: level={}, json={}, slow_queries={}, audit={}",
        config.level,
        config.json_format,
        config.slow_query_logging,
        config.audit_logging
    );
    
    Ok((slow_query_logger, audit_logger))
}

impl SlowQueryLogger {
    /// Create a new slow query logger
    pub fn new(threshold: Duration, enabled: bool) -> Self {
        Self {
            threshold,
            enabled,
            queries: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Start tracking a query
    pub fn start_query(
        &self,
        query: String,
        collection: Option<String>,
        client_addr: Option<String>,
        user: Option<String>,
    ) -> QueryTracker {
        QueryTracker {
            start_time: Instant::now(),
            query,
            collection,
            client_addr,
            user,
        }
    }
    
    /// Finish tracking a query and log if slow
    pub fn finish_query(&self, tracker: QueryTracker) {
        if !self.enabled {
            return;
        }
        
        let duration = tracker.start_time.elapsed();
        
        if duration >= self.threshold {
            let slow_query = SlowQuery {
                timestamp: Utc::now(),
                duration_ms: duration.as_millis() as u64,
                query: tracker.query.clone(),
                collection: tracker.collection.clone(),
                client_addr: tracker.client_addr.clone(),
                user: tracker.user.clone(),
            };
            
            // Log the slow query
            tracing::warn!(
                target: "slow_query",
                duration_ms = slow_query.duration_ms,
                query = %slow_query.query,
                collection = ?slow_query.collection,
                client_addr = ?slow_query.client_addr,
                user = ?slow_query.user,
                "Slow query detected"
            );
            
            // Store for metrics/reporting
            let mut queries = self.queries.write();
            queries.push(slow_query);
            
            // Keep only last 1000 slow queries
            if queries.len() > 1000 {
                let len = queries.len();
                queries.drain(0..len - 1000);
            }
        }
    }
    
    /// Get recent slow queries
    pub fn get_slow_queries(&self, limit: usize) -> Vec<SlowQuery> {
        let queries = self.queries.read();
        queries.iter().rev().take(limit).cloned().collect()
    }
    
    /// Get slow query statistics
    pub fn get_stats(&self) -> SlowQueryStats {
        let queries = self.queries.read();
        
        if queries.is_empty() {
            return SlowQueryStats::default();
        }
        
        let total_count = queries.len();
        let total_duration: u64 = queries.iter().map(|q| q.duration_ms).sum();
        let avg_duration = total_duration / total_count as u64;
        let max_duration = queries.iter().map(|q| q.duration_ms).max().unwrap_or(0);
        
        SlowQueryStats {
            total_count,
            avg_duration_ms: avg_duration,
            max_duration_ms: max_duration,
            threshold_ms: self.threshold.as_millis() as u64,
        }
    }
}

/// Slow query statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowQueryStats {
    pub total_count: usize,
    pub avg_duration_ms: u64,
    pub max_duration_ms: u64,
    pub threshold_ms: u64,
}

impl Default for SlowQueryStats {
    fn default() -> Self {
        Self {
            total_count: 0,
            avg_duration_ms: 0,
            max_duration_ms: 0,
            threshold_ms: 100,
        }
    }
}

impl AuditLogger {
    /// Create a new audit logger
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            events: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Log an audit event
    pub fn log_event(&self, event: AuditEvent) {
        if !self.enabled {
            return;
        }
        
        // Log to tracing
        match &event {
            AuditEvent::Authentication { user, success, client_addr, .. } => {
                if *success {
                    tracing::info!(
                        target: "audit",
                        event_type = "authentication",
                        user = %user,
                        client_addr = %client_addr,
                        success = true,
                        "User authentication successful"
                    );
                } else {
                    tracing::warn!(
                        target: "audit",
                        event_type = "authentication",
                        user = %user,
                        client_addr = %client_addr,
                        success = false,
                        "User authentication failed"
                    );
                }
            }
            AuditEvent::Authorization { user, operation, resource, success, .. } => {
                if *success {
                    tracing::debug!(
                        target: "audit",
                        event_type = "authorization",
                        user = %user,
                        operation = %operation,
                        resource = %resource,
                        success = true,
                        "Authorization granted"
                    );
                } else {
                    tracing::warn!(
                        target: "audit",
                        event_type = "authorization",
                        user = %user,
                        operation = %operation,
                        resource = %resource,
                        success = false,
                        "Authorization denied"
                    );
                }
            }
            AuditEvent::AdminOperation { user, operation, details, .. } => {
                tracing::info!(
                    target: "audit",
                    event_type = "admin_operation",
                    user = %user,
                    operation = %operation,
                    details = %details,
                    "Admin operation performed"
                );
            }
            AuditEvent::ConfigurationChange { user, setting, old_value, new_value, .. } => {
                tracing::info!(
                    target: "audit",
                    event_type = "config_change",
                    user = %user,
                    setting = %setting,
                    old_value = %old_value,
                    new_value = %new_value,
                    "Configuration changed"
                );
            }
        }
        
        // Store for reporting
        let mut events = self.events.write();
        events.push(event);
        
        // Keep only last 10000 events
        if events.len() > 10000 {
            let len = events.len();
            events.drain(0..len - 10000);
        }
    }
    
    /// Log authentication event
    pub fn log_authentication(&self, user: &str, success: bool, client_addr: &str) {
        self.log_event(AuditEvent::Authentication {
            user: user.to_string(),
            success,
            client_addr: client_addr.to_string(),
            timestamp: Utc::now(),
        });
    }
    
    /// Log authorization event
    pub fn log_authorization(&self, user: &str, operation: &str, resource: &str, success: bool) {
        self.log_event(AuditEvent::Authorization {
            user: user.to_string(),
            operation: operation.to_string(),
            resource: resource.to_string(),
            success,
            timestamp: Utc::now(),
        });
    }
    
    /// Log admin operation
    pub fn log_admin_operation(&self, user: &str, operation: &str, details: &str) {
        self.log_event(AuditEvent::AdminOperation {
            user: user.to_string(),
            operation: operation.to_string(),
            details: details.to_string(),
            timestamp: Utc::now(),
        });
    }
    
    /// Log configuration change
    pub fn log_config_change(&self, user: &str, setting: &str, old_value: &str, new_value: &str) {
        self.log_event(AuditEvent::ConfigurationChange {
            user: user.to_string(),
            setting: setting.to_string(),
            old_value: old_value.to_string(),
            new_value: new_value.to_string(),
            timestamp: Utc::now(),
        });
    }
    
    /// Get recent audit events
    pub fn get_events(&self, limit: usize) -> Vec<AuditEvent> {
        let events = self.events.read();
        events.iter().rev().take(limit).cloned().collect()
    }
    
    /// Get audit statistics
    pub fn get_stats(&self) -> AuditStats {
        let events = self.events.read();
        
        let mut auth_success = 0;
        let mut auth_failure = 0;
        let mut authz_failure = 0;
        let mut admin_ops = 0;
        let mut config_changes = 0;
        
        for event in events.iter() {
            match event {
                AuditEvent::Authentication { success: true, .. } => auth_success += 1,
                AuditEvent::Authentication { success: false, .. } => auth_failure += 1,
                AuditEvent::Authorization { success: false, .. } => authz_failure += 1,
                AuditEvent::AdminOperation { .. } => admin_ops += 1,
                AuditEvent::ConfigurationChange { .. } => config_changes += 1,
                _ => {}
            }
        }
        
        AuditStats {
            total_events: events.len(),
            authentication_success: auth_success,
            authentication_failure: auth_failure,
            authorization_failure: authz_failure,
            admin_operations: admin_ops,
            config_changes,
        }
    }
}

/// Audit statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStats {
    pub total_events: usize,
    pub authentication_success: usize,
    pub authentication_failure: usize,
    pub authorization_failure: usize,
    pub admin_operations: usize,
    pub config_changes: usize,
}

/// Global logging instances
static SLOW_QUERY_LOGGER: std::sync::OnceLock<SlowQueryLogger> = std::sync::OnceLock::new();
static AUDIT_LOGGER: std::sync::OnceLock<AuditLogger> = std::sync::OnceLock::new();

/// Initialize global logging
pub fn init_global_logging(config: LoggingConfig) -> Result<()> {
    let (slow_query_logger, audit_logger) = init_logging(config)?;
    
    SLOW_QUERY_LOGGER.set(slow_query_logger)
        .map_err(|_| anyhow::anyhow!("Slow query logger already initialized"))?;
    
    AUDIT_LOGGER.set(audit_logger)
        .map_err(|_| anyhow::anyhow!("Audit logger already initialized"))?;
    
    Ok(())
}

/// Get global slow query logger
pub fn get_slow_query_logger() -> &'static SlowQueryLogger {
    SLOW_QUERY_LOGGER.get().expect("Slow query logger not initialized")
}

/// Get global audit logger
pub fn get_audit_logger() -> &'static AuditLogger {
    AUDIT_LOGGER.get().expect("Audit logger not initialized")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    
    #[test]
    fn test_logging_config() {
        let config = LoggingConfig::default();
        assert_eq!(config.level, "INFO");
        assert_eq!(config.parse_level(), Level::INFO);
        
        let config = LoggingConfig {
            level: "DEBUG".to_string(),
            ..Default::default()
        };
        assert_eq!(config.parse_level(), Level::DEBUG);
    }
    
    #[test]
    fn test_slow_query_logger() {
        let logger = SlowQueryLogger::new(Duration::from_millis(100), true);
        
        // Test fast query (should not be logged)
        let tracker = logger.start_query(
            "SELECT * FROM users".to_string(),
            Some("users".to_string()),
            Some("127.0.0.1".to_string()),
            Some("admin".to_string()),
        );
        logger.finish_query(tracker);
        
        let slow_queries = logger.get_slow_queries(10);
        assert_eq!(slow_queries.len(), 0);
        
        // Test slow query simulation
        let mut tracker = logger.start_query(
            "SELECT * FROM large_table".to_string(),
            Some("large_table".to_string()),
            Some("127.0.0.1".to_string()),
            Some("admin".to_string()),
        );
        
        // Simulate slow query by setting start time in the past
        tracker.start_time = Instant::now() - Duration::from_millis(200);
        logger.finish_query(tracker);
        
        let slow_queries = logger.get_slow_queries(10);
        assert_eq!(slow_queries.len(), 1);
        assert!(slow_queries[0].duration_ms >= 200);
    }
    
    #[test]
    fn test_audit_logger() {
        let logger = AuditLogger::new(true);
        
        // Test authentication logging
        logger.log_authentication("admin", true, "127.0.0.1");
        logger.log_authentication("user", false, "192.168.1.1");
        
        // Test authorization logging
        logger.log_authorization("admin", "READ", "users", true);
        logger.log_authorization("user", "WRITE", "admin", false);
        
        // Test admin operation logging
        logger.log_admin_operation("admin", "CREATE_USER", "Created user 'newuser'");
        
        let events = logger.get_events(10);
        assert_eq!(events.len(), 5);
        
        let stats = logger.get_stats();
        assert_eq!(stats.authentication_success, 1);
        assert_eq!(stats.authentication_failure, 1);
        assert_eq!(stats.authorization_failure, 1);
        assert_eq!(stats.admin_operations, 1);
    }
    
    #[test]
    fn test_query_tracker() {
        let logger = SlowQueryLogger::new(Duration::from_millis(50), true);
        
        let tracker = logger.start_query(
            "INSERT INTO test VALUES (1)".to_string(),
            Some("test".to_string()),
            None,
            None,
        );
        
        assert_eq!(tracker.query, "INSERT INTO test VALUES (1)");
        assert_eq!(tracker.collection, Some("test".to_string()));
        assert!(tracker.start_time.elapsed() < Duration::from_millis(10));
    }
}