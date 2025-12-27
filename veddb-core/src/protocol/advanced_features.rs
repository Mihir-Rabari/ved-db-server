//! Advanced features protocol structures
//!
//! Request and response types for backup, replication, and key management operations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ============================================================================
// Backup & Recovery Requests/Responses
// ============================================================================

/// Request to create a new backup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBackupRequest {
    /// WAL sequence to backup up to (None = current)
    pub wal_sequence: Option<u64>,
    /// Whether to compress the backup
    pub compress: bool,
}

///Response containing backup information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    /// Unique backup identifier
    pub backup_id: String,
    /// When the backup was created
    pub created_at: DateTime<Utc>,
    /// WAL sequence at backup time
    pub wal_sequence: u64,
    /// Size in bytes
    pub size_bytes: u64,
    /// Whether backup is compressed
    pub compressed: bool,
    /// File path
    pub file_path: String,
}

/// Request to restore from backup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreBackupRequest {
    /// Backup ID or path
    pub backup_id: String,
}

/// Request to delete a backup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteBackupRequest {
    /// Backup ID to delete
    pub backup_id: String,
}

/// Request for Point-In-Time Recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointInTimeRecoverRequest {
    /// Target time to recover to
    pub target_time: DateTime<Utc>,
    /// Base backup ID to start from
    pub backup_id: String,
    /// WAL directory path
    pub wal_directory: String,
}

// ============================================================================
// Replication Requests/Responses
// ============================================================================

/// Request to add a slave to replication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddSlaveRequest {
    /// Slave server address
    pub slave_address: String,
    /// Optional slave ID (auto-generated if None)
    pub slave_id: Option<String>,
}

/// Request to remove a slave
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveSlaveRequest {
    /// Slave ID to remove
    pub slave_id: String,
}

/// Replication status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationStatusResponse {
    /// Server role ("master" or "slave")
    pub role: String,
    /// Connected slaves (master only)
    pub slaves: Vec<SlaveInfo>,
    /// Total replication lag in bytes
    pub lag_bytes: u64,
    /// Whether replication is healthy
    pub healthy: bool,
}

/// Information about a connected slave
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaveInfo {
    /// Unique slave identifier
    pub slave_id: String,
    /// Slave network address
    pub address: String,
    /// Last acknowledged WAL sequence
    pub last_ack_sequence: u64,
    /// Whether slave is currently connected
    pub connected: bool,
    /// Connection established time
    pub connected_at: Option<DateTime<Utc>>,
}

// ============================================================================
// Key Management Requests/Responses
// ============================================================================

/// Request to create a new encryption key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateKeyRequest {
    /// Unique key identifier
    pub key_id: String,
}

/// Request to rotate a key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotateKeyRequest {
    /// Key ID to rotate
    pub key_id: String,
}

/// Request to export a key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportKeyRequest {
    /// Key ID to export
    pub key_id: String,
}

/// Response containing exported key data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportKeyResponse {
    /// Key ID
    pub key_id: String,
    /// Encrypted key data (hex-encoded)
    pub encrypted_data: String,
}

/// Request to import a key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportKeyRequest {
    /// Encrypted key data from export
    pub encrypted_data: String,
}

/// Request to view a specific key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetKeyMetadataRequest {
    /// Key ID
    pub key_id: String,
}

/// Response containing encryption key metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionKeyMetadata {
    /// Key identifier
    pub key_id: String,
    /// Key version
    pub version: u32,
    /// Algorithm (AES-256-GCM, ChaCha20-Poly1305)
    pub algorithm: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Expiration timestamp
    pub expires_at: Option<DateTime<Utc>>,
    /// Whether active
    pub is_active: bool,
}

/// Alias for backwards compatibility
pub type KeyMetadataResponse = EncryptionKeyMetadata;


// ===========================================================================
// Aggregation Requests/Responses
// ============================================================================

/// Request to execute aggregation pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateRequest {
    /// Collection name
    pub collection: String,
    /// Aggregation pipeline stages
    pub pipeline: Vec<serde_json::Value>,
}

/// Response containing aggregation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateResponse {
    /// Result documents
    pub results: Vec<serde_json::Value>,
    /// Count of results
    pub count: usize,
}

/// Request to get keys approaching expiry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetKeysExpiringRequest {
    /// Rotation period in days
    pub rotation_days: u32,
}

/// Information about a key approaching expiry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpiringKeyInfo {
    /// Key identifier
    pub key_id: String,
    /// Days remaining until rotation recommended
    pub days_remaining: i64,
    /// Last rotation time
    pub last_rotated: DateTime<Utc>,
    /// Key creation time
    pub created_at: DateTime<Utc>,
}

/// List of keys approaching expiry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetKeysExpiringResponse {
    /// Keys approaching expiry
    pub expiring_keys: Vec<ExpiringKeyInfo>,
}

/// List of all keys response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListKeysResponse {
    /// All encryption keys
    pub keys: Vec<KeyMetadataResponse>,
}
