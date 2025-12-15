//! Replication protocol messages

use crate::wal::WalEntry;
use crate::snapshot::format::SnapshotHeader;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Replication protocol messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReplicationMessage {
    /// Slave requests synchronization from master
    SyncRequest {
        /// Last WAL sequence number the slave has
        last_sequence: u64,
        /// Slave node ID for identification
        slave_id: String,
    },

    /// Master sends full database snapshot to slave
    FullSync {
        /// Snapshot header information
        header: SnapshotHeader,
        /// Compressed snapshot data
        snapshot_data: Vec<u8>,
    },

    /// Master sends incremental WAL entries to slave
    IncrementalSync {
        /// WAL entries to apply
        entries: Vec<WalEntry>,
    },

    /// Heartbeat message to keep connection alive
    Heartbeat {
        /// Timestamp when heartbeat was sent
        timestamp: DateTime<Utc>,
        /// Current WAL sequence number
        current_sequence: u64,
    },

    /// Acknowledgment message
    Ack {
        /// Sequence number being acknowledged
        sequence: u64,
        /// Status of the operation
        status: AckStatus,
    },

    /// Error message
    Error {
        /// Error code
        code: ErrorCode,
        /// Error message
        message: String,
    },

    /// Slave promotion request (admin command)
    PromoteToMaster {
        /// Authentication token
        auth_token: String,
    },

    /// Master shutdown notification
    MasterShutdown {
        /// Reason for shutdown
        reason: String,
    },
}

impl ReplicationMessage {
    /// Get the message type as a string
    pub fn message_type(&self) -> &'static str {
        match self {
            ReplicationMessage::SyncRequest { .. } => "SyncRequest",
            ReplicationMessage::FullSync { .. } => "FullSync",
            ReplicationMessage::IncrementalSync { .. } => "IncrementalSync",
            ReplicationMessage::Heartbeat { .. } => "Heartbeat",
            ReplicationMessage::Ack { .. } => "Ack",
            ReplicationMessage::Error { .. } => "Error",
            ReplicationMessage::PromoteToMaster { .. } => "PromoteToMaster",
            ReplicationMessage::MasterShutdown { .. } => "MasterShutdown",
        }
    }

    /// Check if this is a control message (not data)
    pub fn is_control_message(&self) -> bool {
        matches!(
            self,
            ReplicationMessage::Heartbeat { .. }
                | ReplicationMessage::Ack { .. }
                | ReplicationMessage::Error { .. }
                | ReplicationMessage::PromoteToMaster { .. }
                | ReplicationMessage::MasterShutdown { .. }
        )
    }

    /// Get the size of the message in bytes (approximate)
    pub fn size_bytes(&self) -> usize {
        match self {
            ReplicationMessage::FullSync { snapshot_data, .. } => {
                std::mem::size_of::<SnapshotHeader>() + snapshot_data.len()
            }
            ReplicationMessage::IncrementalSync { entries } => {
                entries.len() * std::mem::size_of::<WalEntry>()
            }
            _ => std::mem::size_of::<Self>(),
        }
    }
}

/// Acknowledgment status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AckStatus {
    /// Operation completed successfully
    Success,
    /// Operation failed
    Failed,
    /// Operation partially completed
    Partial,
}

/// Error codes for replication protocol
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ErrorCode {
    /// Invalid message format
    InvalidMessage,
    /// Authentication failed
    AuthenticationFailed,
    /// Sequence number out of order
    SequenceError,
    /// Snapshot corruption
    SnapshotCorrupted,
    /// WAL entry corruption
    WalCorrupted,
    /// Internal server error
    InternalError,
    /// Slave limit exceeded
    SlaveLimit,
    /// Operation timeout
    Timeout,
}

impl ErrorCode {
    /// Get a human-readable description of the error
    pub fn description(&self) -> &'static str {
        match self {
            ErrorCode::InvalidMessage => "Invalid message format",
            ErrorCode::AuthenticationFailed => "Authentication failed",
            ErrorCode::SequenceError => "Sequence number out of order",
            ErrorCode::SnapshotCorrupted => "Snapshot data is corrupted",
            ErrorCode::WalCorrupted => "WAL entry is corrupted",
            ErrorCode::InternalError => "Internal server error",
            ErrorCode::SlaveLimit => "Maximum number of slaves exceeded",
            ErrorCode::Timeout => "Operation timed out",
        }
    }
}

/// Message serialization and deserialization
impl ReplicationMessage {
    /// Serialize message to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, crate::replication::ReplicationError> {
        serde_json::to_vec(self).map_err(|e| {
            crate::replication::ReplicationError::SerializationError(e.to_string())
        })
    }

    /// Deserialize message from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, crate::replication::ReplicationError> {
        serde_json::from_slice(bytes).map_err(|e| {
            crate::replication::ReplicationError::DeserializationError(e.to_string())
        })
    }

    /// Create a heartbeat message
    pub fn heartbeat(current_sequence: u64) -> Self {
        Self::Heartbeat {
            timestamp: Utc::now(),
            current_sequence,
        }
    }

    /// Create a success acknowledgment
    pub fn ack_success(sequence: u64) -> Self {
        Self::Ack {
            sequence,
            status: AckStatus::Success,
        }
    }

    /// Create a failure acknowledgment
    pub fn ack_failed(sequence: u64) -> Self {
        Self::Ack {
            sequence,
            status: AckStatus::Failed,
        }
    }

    /// Create an error message
    pub fn error(code: ErrorCode, message: String) -> Self {
        Self::Error { code, message }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = ReplicationMessage::SyncRequest {
            last_sequence: 12345,
            slave_id: "slave-001".to_string(),
        };

        let bytes = msg.to_bytes().unwrap();
        let deserialized = ReplicationMessage::from_bytes(&bytes).unwrap();

        match deserialized {
            ReplicationMessage::SyncRequest { last_sequence, slave_id } => {
                assert_eq!(last_sequence, 12345);
                assert_eq!(slave_id, "slave-001");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_message_type() {
        let msg = ReplicationMessage::Heartbeat {
            timestamp: Utc::now(),
            current_sequence: 100,
        };
        assert_eq!(msg.message_type(), "Heartbeat");
        assert!(msg.is_control_message());
    }

    #[test]
    fn test_ack_messages() {
        let success = ReplicationMessage::ack_success(123);
        match success {
            ReplicationMessage::Ack { sequence, status } => {
                assert_eq!(sequence, 123);
                assert_eq!(status, AckStatus::Success);
            }
            _ => panic!("Wrong message type"),
        }

        let failed = ReplicationMessage::ack_failed(456);
        match failed {
            ReplicationMessage::Ack { sequence, status } => {
                assert_eq!(sequence, 456);
                assert_eq!(status, AckStatus::Failed);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_error_code_description() {
        assert_eq!(
            ErrorCode::InvalidMessage.description(),
            "Invalid message format"
        );
        assert_eq!(
            ErrorCode::AuthenticationFailed.description(),
            "Authentication failed"
        );
    }

    #[test]
    fn test_heartbeat_creation() {
        let heartbeat = ReplicationMessage::heartbeat(999);
        match heartbeat {
            ReplicationMessage::Heartbeat { current_sequence, .. } => {
                assert_eq!(current_sequence, 999);
            }
            _ => panic!("Wrong message type"),
        }
    }
}