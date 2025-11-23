//! Audit logging for security events and administrative operations

use crate::auth::{Operation, Resource};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

#[cfg(feature = "rocksdb-storage")]
use rocksdb::{ColumnFamily, DB};

/// Types of audit events
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AuditEventType {
    /// Authentication events
    AuthenticationSuccess,
    AuthenticationFailure,
    
    /// Authorization events
    AuthorizationFailure,
    
    /// User management events
    UserCreated,
    UserUpdated,
    UserDeleted,
    UserEnabled,
    UserDisabled,
    PasswordChanged,
    
    /// Administrative operations
    ConfigurationChanged,
    BackupCreated,
    RestorePerformed,
    
    /// Data operations (for sensitive collections)
    DataAccessed,
    DataModified,
    DataDeleted,
    
    /// System events
    ServerStarted,
    ServerStopped,
    DatabaseCreated,
    DatabaseDropped,
    
    /// Security events
    TokenRevoked,
    SuspiciousActivity,
    RateLimitExceeded,
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique identifier for the audit entry
    pub id: String,
    
    /// Timestamp when the event occurred
    pub timestamp: DateTime<Utc>,
    
    /// Type of audit event
    pub event_type: AuditEventType,
    
    /// Username associated with the event (if applicable)
    pub username: Option<String>,
    
    /// Client IP address (if applicable)
    pub client_ip: Option<String>,
    
    /// Operation that was attempted or performed
    pub operation: Option<Operation>,
    
    /// Resource that was accessed (if applicable)
    pub resource: Option<Resource>,
    
    /// Whether the operation was successful
    pub success: bool,
    
    /// Error message (if operation failed)
    pub error_message: Option<String>,
    
    /// Additional details about the event
    pub details: serde_json::Value,
    
    /// Session ID or token ID (if applicable)
    pub session_id: Option<String>,
    
    /// User agent or client information
    pub user_agent: Option<String>,
}

impl AuditEntry {
    /// Create a new audit entry
    pub fn new(event_type: AuditEventType) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            event_type,
            username: None,
            client_ip: None,
            operation: None,
            resource: None,
            success: true,
            error_message: None,
            details: serde_json::Value::Null,
            session_id: None,
            user_agent: None,
        }
    }

    /// Set username
    pub fn with_username(mut self, username: &str) -> Self {
        self.username = Some(username.to_string());
        self
    }

    /// Set client IP
    pub fn with_client_ip(mut self, client_ip: &str) -> Self {
        self.client_ip = Some(client_ip.to_string());
        self
    }

    /// Set operation
    pub fn with_operation(mut self, operation: Operation) -> Self {
        self.operation = Some(operation);
        self
    }

    /// Set resource
    pub fn with_resource(mut self, resource: Resource) -> Self {
        self.resource = Some(resource);
        self
    }

    /// Set success status
    pub fn with_success(mut self, success: bool) -> Self {
        self.success = success;
        self
    }

    /// Set error message
    pub fn with_error(mut self, error_message: &str) -> Self {
        self.error_message = Some(error_message.to_string());
        self.success = false;
        self
    }

    /// Set additional details
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }

    /// Set session ID
    pub fn with_session_id(mut self, session_id: &str) -> Self {
        self.session_id = Some(session_id.to_string());
        self
    }

    /// Set user agent
    pub fn with_user_agent(mut self, user_agent: &str) -> Self {
        self.user_agent = Some(user_agent.to_string());
        self
    }
}

/// Audit logger for recording security events
#[cfg(feature = "rocksdb-storage")]
pub struct AuditLogger {
    db: DB,
}

/// Mock audit logger for when RocksDB is not available
#[cfg(not(feature = "rocksdb-storage"))]
pub struct AuditLogger {
    entries: Vec<AuditEntry>,
}

#[cfg(feature = "rocksdb-storage")]
impl AuditLogger {
    /// Create a new audit logger with RocksDB storage
    pub fn new(storage_path: &str) -> Result<Self> {
        let path = Path::new(storage_path).join("audit");
        
        let mut opts = rocksdb::Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        
        let cf_descriptors = vec![
            rocksdb::ColumnFamilyDescriptor::new("audit_log", rocksdb::Options::default()),
            rocksdb::ColumnFamilyDescriptor::new("audit_index", rocksdb::Options::default()),
        ];
        
        let db = DB::open_cf_descriptors(&opts, path, cf_descriptors)?;
        
        Ok(Self { db })
    }

    /// Log an audit entry
    pub async fn log_entry(&mut self, entry: AuditEntry) -> Result<()> {
        let audit_cf = self.get_audit_cf()?;
        let index_cf = self.get_index_cf()?;
        
        // Serialize entry
        let entry_data = serde_json::to_vec(&entry)?;
        
        // Create key with timestamp prefix for chronological ordering
        let key = format!("{}:{}", entry.timestamp.timestamp_nanos_opt().unwrap_or(0), entry.id);
        
        // Store entry
        self.db.put_cf(audit_cf, &key, entry_data)?;
        
        // Create indexes for efficient querying
        if let Some(username) = &entry.username {
            let user_key = format!("user:{}:{}", username, key);
            self.db.put_cf(index_cf, user_key, b"")?;
        }
        
        if let Some(operation) = entry.operation {
            let op_key = format!("operation:{}:{}", operation.as_str(), key);
            self.db.put_cf(index_cf, op_key, b"")?;
        }
        
        let event_key = format!("event:{:?}:{}", entry.event_type, key);
        self.db.put_cf(index_cf, event_key, b"")?;
        
        log::info!("Audit log: {:?} - {} - {:?}", 
                   entry.event_type, 
                   entry.username.as_deref().unwrap_or("system"),
                   entry.operation);
        
        Ok(())
    }

    /// Log successful authentication
    pub async fn log_auth_success(&mut self, username: &str, client_ip: Option<&str>) -> Result<()> {
        let mut entry = AuditEntry::new(AuditEventType::AuthenticationSuccess)
            .with_username(username)
            .with_success(true);
        
        if let Some(ip) = client_ip {
            entry = entry.with_client_ip(ip);
        }
        
        self.log_entry(entry).await
    }

    /// Log failed authentication
    pub async fn log_auth_failure(
        &mut self,
        username: &str,
        client_ip: Option<&str>,
        error: &str,
    ) -> Result<()> {
        let mut entry = AuditEntry::new(AuditEventType::AuthenticationFailure)
            .with_username(username)
            .with_error(error);
        
        if let Some(ip) = client_ip {
            entry = entry.with_client_ip(ip);
        }
        
        self.log_entry(entry).await
    }

    /// Log authorization failure
    pub async fn log_authorization_failure(
        &mut self,
        username: &str,
        operation: Operation,
        resource: Option<&str>,
    ) -> Result<()> {
        let mut entry = AuditEntry::new(AuditEventType::AuthorizationFailure)
            .with_username(username)
            .with_operation(operation)
            .with_error("Access denied");
        
        if let Some(res) = resource {
            let details = serde_json::json!({ "resource": res });
            entry = entry.with_details(details);
        }
        
        self.log_entry(entry).await
    }

    /// Log user creation
    pub async fn log_user_created(
        &mut self,
        created_username: &str,
        created_by: &str,
        role: &str,
    ) -> Result<()> {
        let details = serde_json::json!({
            "created_user": created_username,
            "role": role
        });
        
        let entry = AuditEntry::new(AuditEventType::UserCreated)
            .with_username(created_by)
            .with_details(details);
        
        self.log_entry(entry).await
    }

    /// Log user update
    pub async fn log_user_updated(
        &mut self,
        updated_username: &str,
        updated_by: &str,
        changes: serde_json::Value,
    ) -> Result<()> {
        let details = serde_json::json!({
            "updated_user": updated_username,
            "changes": changes
        });
        
        let entry = AuditEntry::new(AuditEventType::UserUpdated)
            .with_username(updated_by)
            .with_details(details);
        
        self.log_entry(entry).await
    }

    /// Log user deletion
    pub async fn log_user_deleted(&mut self, deleted_username: &str, deleted_by: &str) -> Result<()> {
        let details = serde_json::json!({
            "deleted_user": deleted_username
        });
        
        let entry = AuditEntry::new(AuditEventType::UserDeleted)
            .with_username(deleted_by)
            .with_details(details);
        
        self.log_entry(entry).await
    }

    /// Log password change
    pub async fn log_password_changed(&mut self, username: &str, changed_by: &str) -> Result<()> {
        let details = serde_json::json!({
            "target_user": username
        });
        
        let entry = AuditEntry::new(AuditEventType::PasswordChanged)
            .with_username(changed_by)
            .with_details(details);
        
        self.log_entry(entry).await
    }

    /// Log configuration change
    pub async fn log_config_change(
        &mut self,
        username: &str,
        config_key: &str,
        old_value: Option<&str>,
        new_value: &str,
    ) -> Result<()> {
        let details = serde_json::json!({
            "config_key": config_key,
            "old_value": old_value,
            "new_value": new_value
        });
        
        let entry = AuditEntry::new(AuditEventType::ConfigurationChanged)
            .with_username(username)
            .with_details(details);
        
        self.log_entry(entry).await
    }

    /// Log backup creation
    pub async fn log_backup_created(&mut self, username: &str, backup_path: &str) -> Result<()> {
        let details = serde_json::json!({
            "backup_path": backup_path
        });
        
        let entry = AuditEntry::new(AuditEventType::BackupCreated)
            .with_username(username)
            .with_details(details);
        
        self.log_entry(entry).await
    }

    /// Log restore operation
    pub async fn log_restore_performed(&mut self, username: &str, restore_path: &str) -> Result<()> {
        let details = serde_json::json!({
            "restore_path": restore_path
        });
        
        let entry = AuditEntry::new(AuditEventType::RestorePerformed)
            .with_username(username)
            .with_details(details);
        
        self.log_entry(entry).await
    }

    /// Log server startup
    pub async fn log_server_started(&mut self, version: &str) -> Result<()> {
        let details = serde_json::json!({
            "version": version
        });
        
        let entry = AuditEntry::new(AuditEventType::ServerStarted)
            .with_details(details);
        
        self.log_entry(entry).await
    }

    /// Log server shutdown
    pub async fn log_server_stopped(&mut self) -> Result<()> {
        let entry = AuditEntry::new(AuditEventType::ServerStopped);
        self.log_entry(entry).await
    }

    /// Log suspicious activity
    pub async fn log_suspicious_activity(
        &mut self,
        username: Option<&str>,
        client_ip: Option<&str>,
        description: &str,
    ) -> Result<()> {
        let details = serde_json::json!({
            "description": description
        });
        
        let mut entry = AuditEntry::new(AuditEventType::SuspiciousActivity)
            .with_details(details);
        
        if let Some(user) = username {
            entry = entry.with_username(user);
        }
        
        if let Some(ip) = client_ip {
            entry = entry.with_client_ip(ip);
        }
        
        self.log_entry(entry).await
    }

    /// Query audit logs by username
    pub async fn query_by_username(
        &self,
        username: &str,
        limit: Option<usize>,
    ) -> Result<Vec<AuditEntry>> {
        let index_cf = self.get_index_cf()?;
        let audit_cf = self.get_audit_cf()?;
        
        let prefix = format!("user:{}", username);
        let mut entries = Vec::new();
        let mut count = 0;
        
        let iter = self.db.prefix_iterator_cf(index_cf, &prefix);
        for item in iter {
            if let Some(max_limit) = limit {
                if count >= max_limit {
                    break;
                }
            }
            
            let (key, _) = item?;
            let key_str = String::from_utf8_lossy(key.as_ref());
            
            // Extract the actual audit entry key
            if let Some(audit_key) = key_str.split(':').nth(2) {
                if let Some(entry_data) = self.db.get_cf(audit_cf, audit_key)? {
                    let entry: AuditEntry = serde_json::from_slice(entry_data.as_ref())?;
                    entries.push(entry);
                    count += 1;
                }
            }
        }
        
        // Sort by timestamp (most recent first)
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        
        Ok(entries)
    }

    /// Query audit logs by operation
    pub async fn query_by_operation(
        &self,
        operation: Operation,
        limit: Option<usize>,
    ) -> Result<Vec<AuditEntry>> {
        let index_cf = self.get_index_cf()?;
        let audit_cf = self.get_audit_cf()?;
        
        let prefix = format!("operation:{}", operation.as_str());
        let mut entries = Vec::new();
        let mut count = 0;
        
        let iter = self.db.prefix_iterator_cf(index_cf, &prefix);
        for item in iter {
            if let Some(max_limit) = limit {
                if count >= max_limit {
                    break;
                }
            }
            
            let (key, _) = item?;
            let key_str = String::from_utf8_lossy(key.as_ref());
            
            // Extract the actual audit entry key
            if let Some(audit_key) = key_str.split(':').nth(2) {
                if let Some(entry_data) = self.db.get_cf(audit_cf, audit_key)? {
                    let entry: AuditEntry = serde_json::from_slice(entry_data.as_ref())?;
                    entries.push(entry);
                    count += 1;
                }
            }
        }
        
        // Sort by timestamp (most recent first)
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        
        Ok(entries)
    }

    /// Query recent audit logs
    pub async fn query_recent(&self, limit: usize) -> Result<Vec<AuditEntry>> {
        let audit_cf = self.get_audit_cf()?;
        let mut entries = Vec::new();
        let mut count = 0;
        
        // Iterate in reverse order to get most recent entries first
        let iter = self.db.iterator_cf(audit_cf, rocksdb::IteratorMode::End);
        for item in iter {
            if count >= limit {
                break;
            }
            
            let (_, value) = item?;
            let entry: AuditEntry = serde_json::from_slice(value.as_ref())?;
            entries.push(entry);
            count += 1;
        }
        
        Ok(entries)
    }

    /// Get audit column family
    fn get_audit_cf(&self) -> Result<&ColumnFamily> {
        self.db
            .cf_handle("audit_log")
            .ok_or_else(|| anyhow::anyhow!("Audit log column family not found"))
    }

    /// Get index column family
    fn get_index_cf(&self) -> Result<&ColumnFamily> {
        self.db
            .cf_handle("audit_index")
            .ok_or_else(|| anyhow::anyhow!("Audit index column family not found"))
    }
}

#[cfg(not(feature = "rocksdb-storage"))]
impl AuditLogger {
    /// Create a new audit logger with in-memory storage (mock implementation)
    pub fn new(_storage_path: &str) -> Result<Self> {
        Ok(Self {
            entries: Vec::new(),
        })
    }

    /// Log an audit entry
    pub async fn log_entry(&mut self, entry: AuditEntry) -> Result<()> {
        log::info!("Audit log: {:?} - {} - {:?}", 
                   entry.event_type, 
                   entry.username.as_deref().unwrap_or("system"),
                   entry.operation);
        
        self.entries.push(entry);
        Ok(())
    }

    /// Log successful authentication
    pub async fn log_auth_success(&mut self, username: &str, client_ip: Option<&str>) -> Result<()> {
        let mut entry = AuditEntry::new(AuditEventType::AuthenticationSuccess)
            .with_username(username)
            .with_success(true);
        
        if let Some(ip) = client_ip {
            entry = entry.with_client_ip(ip);
        }
        
        self.log_entry(entry).await
    }

    /// Log failed authentication
    pub async fn log_auth_failure(
        &mut self,
        username: &str,
        client_ip: Option<&str>,
        error: &str,
    ) -> Result<()> {
        let mut entry = AuditEntry::new(AuditEventType::AuthenticationFailure)
            .with_username(username)
            .with_error(error);
        
        if let Some(ip) = client_ip {
            entry = entry.with_client_ip(ip);
        }
        
        self.log_entry(entry).await
    }

    /// Log authorization failure
    pub async fn log_authorization_failure(
        &mut self,
        username: &str,
        operation: Operation,
        resource: Option<&str>,
    ) -> Result<()> {
        let mut entry = AuditEntry::new(AuditEventType::AuthorizationFailure)
            .with_username(username)
            .with_operation(operation)
            .with_error("Access denied");
        
        if let Some(res) = resource {
            let details = serde_json::json!({ "resource": res });
            entry = entry.with_details(details);
        }
        
        self.log_entry(entry).await
    }

    /// Log user creation
    pub async fn log_user_created(
        &mut self,
        created_username: &str,
        created_by: &str,
        role: &str,
    ) -> Result<()> {
        let details = serde_json::json!({
            "created_user": created_username,
            "role": role
        });
        
        let entry = AuditEntry::new(AuditEventType::UserCreated)
            .with_username(created_by)
            .with_details(details);
        
        self.log_entry(entry).await
    }

    /// Log user update
    pub async fn log_user_updated(
        &mut self,
        updated_username: &str,
        updated_by: &str,
        changes: serde_json::Value,
    ) -> Result<()> {
        let details = serde_json::json!({
            "updated_user": updated_username,
            "changes": changes
        });
        
        let entry = AuditEntry::new(AuditEventType::UserUpdated)
            .with_username(updated_by)
            .with_details(details);
        
        self.log_entry(entry).await
    }

    /// Log user deletion
    pub async fn log_user_deleted(&mut self, deleted_username: &str, deleted_by: &str) -> Result<()> {
        let details = serde_json::json!({
            "deleted_user": deleted_username
        });
        
        let entry = AuditEntry::new(AuditEventType::UserDeleted)
            .with_username(deleted_by)
            .with_details(details);
        
        self.log_entry(entry).await
    }

    /// Log password change
    pub async fn log_password_changed(&mut self, username: &str, changed_by: &str) -> Result<()> {
        let details = serde_json::json!({
            "target_user": username
        });
        
        let entry = AuditEntry::new(AuditEventType::PasswordChanged)
            .with_username(changed_by)
            .with_details(details);
        
        self.log_entry(entry).await
    }

    /// Log configuration change
    pub async fn log_config_change(
        &mut self,
        username: &str,
        config_key: &str,
        old_value: Option<&str>,
        new_value: &str,
    ) -> Result<()> {
        let details = serde_json::json!({
            "config_key": config_key,
            "old_value": old_value,
            "new_value": new_value
        });
        
        let entry = AuditEntry::new(AuditEventType::ConfigurationChanged)
            .with_username(username)
            .with_details(details);
        
        self.log_entry(entry).await
    }

    /// Log backup creation
    pub async fn log_backup_created(&mut self, username: &str, backup_path: &str) -> Result<()> {
        let details = serde_json::json!({
            "backup_path": backup_path
        });
        
        let entry = AuditEntry::new(AuditEventType::BackupCreated)
            .with_username(username)
            .with_details(details);
        
        self.log_entry(entry).await
    }

    /// Log restore operation
    pub async fn log_restore_performed(&mut self, username: &str, restore_path: &str) -> Result<()> {
        let details = serde_json::json!({
            "restore_path": restore_path
        });
        
        let entry = AuditEntry::new(AuditEventType::RestorePerformed)
            .with_username(username)
            .with_details(details);
        
        self.log_entry(entry).await
    }

    /// Log server startup
    pub async fn log_server_started(&mut self, version: &str) -> Result<()> {
        let details = serde_json::json!({
            "version": version
        });
        
        let entry = AuditEntry::new(AuditEventType::ServerStarted)
            .with_details(details);
        
        self.log_entry(entry).await
    }

    /// Log server shutdown
    pub async fn log_server_stopped(&mut self) -> Result<()> {
        let entry = AuditEntry::new(AuditEventType::ServerStopped);
        self.log_entry(entry).await
    }

    /// Log suspicious activity
    pub async fn log_suspicious_activity(
        &mut self,
        username: Option<&str>,
        client_ip: Option<&str>,
        description: &str,
    ) -> Result<()> {
        let details = serde_json::json!({
            "description": description
        });
        
        let mut entry = AuditEntry::new(AuditEventType::SuspiciousActivity)
            .with_details(details);
        
        if let Some(user) = username {
            entry = entry.with_username(user);
        }
        
        if let Some(ip) = client_ip {
            entry = entry.with_client_ip(ip);
        }
        
        self.log_entry(entry).await
    }

    /// Query audit logs by username
    pub async fn query_by_username(
        &self,
        username: &str,
        limit: Option<usize>,
    ) -> Result<Vec<AuditEntry>> {
        let mut entries: Vec<_> = self.entries
            .iter()
            .filter(|e| e.username.as_deref() == Some(username))
            .cloned()
            .collect();
        
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        
        if let Some(limit) = limit {
            entries.truncate(limit);
        }
        
        Ok(entries)
    }

    /// Query audit logs by operation
    pub async fn query_by_operation(
        &self,
        operation: Operation,
        limit: Option<usize>,
    ) -> Result<Vec<AuditEntry>> {
        let mut entries: Vec<_> = self.entries
            .iter()
            .filter(|e| e.operation == Some(operation))
            .cloned()
            .collect();
        
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        
        if let Some(limit) = limit {
            entries.truncate(limit);
        }
        
        Ok(entries)
    }

    /// Query recent audit logs
    pub async fn query_recent(&self, limit: usize) -> Result<Vec<AuditEntry>> {
        let mut entries = self.entries.clone();
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        entries.truncate(limit);
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_audit_logger() -> (AuditLogger, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_str().unwrap()).unwrap();
        (logger, temp_dir)
    }

    #[tokio::test]
    async fn test_audit_entry_creation() {
        let entry = AuditEntry::new(AuditEventType::AuthenticationSuccess)
            .with_username("testuser")
            .with_client_ip("192.168.1.1")
            .with_success(true);
        
        assert_eq!(entry.event_type, AuditEventType::AuthenticationSuccess);
        assert_eq!(entry.username, Some("testuser".to_string()));
        assert_eq!(entry.client_ip, Some("192.168.1.1".to_string()));
        assert!(entry.success);
    }

    #[tokio::test]
    async fn test_audit_logging() {
        let (mut logger, _temp_dir) = create_test_audit_logger().await;
        
        // Log authentication success
        logger.log_auth_success("testuser", Some("192.168.1.1")).await.unwrap();
        
        // Log authentication failure
        logger.log_auth_failure("baduser", Some("192.168.1.2"), "Invalid password").await.unwrap();
        
        // Query recent logs
        let recent_logs = logger.query_recent(10).await.unwrap();
        assert_eq!(recent_logs.len(), 2);
        
        // Check that logs are in reverse chronological order
        assert!(recent_logs[0].timestamp >= recent_logs[1].timestamp);
    }

    #[tokio::test]
    async fn test_query_by_username() {
        let (mut logger, _temp_dir) = create_test_audit_logger().await;
        
        // Log events for different users
        logger.log_auth_success("user1", None).await.unwrap();
        logger.log_auth_success("user2", None).await.unwrap();
        logger.log_auth_failure("user1", None, "Wrong password").await.unwrap();
        
        // Query logs for user1
        let user1_logs = logger.query_by_username("user1", None).await.unwrap();
        assert_eq!(user1_logs.len(), 2);
        
        for log in &user1_logs {
            assert_eq!(log.username, Some("user1".to_string()));
        }
        
        // Query logs for user2
        let user2_logs = logger.query_by_username("user2", None).await.unwrap();
        assert_eq!(user2_logs.len(), 1);
        assert_eq!(user2_logs[0].username, Some("user2".to_string()));
    }

    #[tokio::test]
    async fn test_administrative_logging() {
        let (mut logger, _temp_dir) = create_test_audit_logger().await;
        
        // Log user creation
        logger.log_user_created("newuser", "admin", "read_write").await.unwrap();
        
        // Log configuration change
        logger.log_config_change("admin", "max_connections", Some("100"), "200").await.unwrap();
        
        // Log backup creation
        logger.log_backup_created("admin", "/backups/backup-2025-01-15.veddb").await.unwrap();
        
        let admin_logs = logger.query_by_username("admin", None).await.unwrap();
        assert_eq!(admin_logs.len(), 3);
        
        // Check event types
        let event_types: Vec<_> = admin_logs.iter().map(|log| &log.event_type).collect();
        assert!(event_types.contains(&&AuditEventType::UserCreated));
        assert!(event_types.contains(&&AuditEventType::ConfigurationChanged));
        assert!(event_types.contains(&&AuditEventType::BackupCreated));
    }
}