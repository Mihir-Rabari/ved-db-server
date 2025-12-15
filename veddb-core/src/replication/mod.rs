//! Replication system for VedDB
//!
//! This module implements master-slave replication with the following features:
//! - Full synchronization using snapshots
//! - Incremental synchronization using WAL streaming
//! - Automatic reconnection with exponential backoff
//! - Support for multiple slaves per master (up to 10)
//! - Read operations on slave nodes
//! - Slave promotion to master

pub mod manager;
pub mod message;
pub mod connection;
pub mod sync;

pub use manager::*;
pub use message::*;
pub use connection::*;
pub use sync::*;

use crate::wal::WalEntry;
use crate::snapshot::format::SnapshotHeader;
use std::net::SocketAddr;
use std::time::Duration;
use thiserror::Error;

/// Node role in replication
#[derive(Debug, Clone, PartialEq)]
pub enum NodeRole {
    /// Master node that accepts writes and replicates to slaves
    Master,
    /// Slave node that replicates from master
    Slave { master_addr: SocketAddr },
}

/// Replication configuration
#[derive(Debug, Clone)]
pub struct ReplicationConfig {
    /// Node role
    pub role: NodeRole,
    /// Maximum number of slaves (for master nodes)
    pub max_slaves: usize,
    /// Replication timeout
    pub timeout: Duration,
    /// Heartbeat interval
    pub heartbeat_interval: Duration,
    /// Reconnection backoff configuration
    pub backoff_config: BackoffConfig,
}

impl Default for ReplicationConfig {
    fn default() -> Self {
        Self {
            role: NodeRole::Master,
            max_slaves: 10,
            timeout: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(10),
            backoff_config: BackoffConfig::default(),
        }
    }
}

/// Exponential backoff configuration
#[derive(Debug, Clone)]
pub struct BackoffConfig {
    /// Initial backoff duration
    pub initial: Duration,
    /// Maximum backoff duration
    pub max: Duration,
    /// Backoff multiplier
    pub multiplier: f64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial: Duration::from_secs(1),
            max: Duration::from_secs(60),
            multiplier: 2.0,
        }
    }
}

/// Replication statistics
#[derive(Debug, Clone, Default)]
pub struct ReplicationStats {
    /// Number of connected slaves (master only)
    pub connected_slaves: usize,
    /// Replication lag in milliseconds (slave only)
    pub replication_lag_ms: u64,
    /// Total messages sent
    pub messages_sent: u64,
    /// Total messages received
    pub messages_received: u64,
    /// Number of reconnection attempts (slave only)
    pub reconnection_attempts: u64,
    /// Last sync timestamp
    pub last_sync: Option<chrono::DateTime<chrono::Utc>>,
}

/// Replication errors
#[derive(Debug, Error)]
pub enum ReplicationError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("Timeout error")]
    Timeout,

    #[error("Invalid message type: {0}")]
    InvalidMessageType(String),

    #[error("Slave limit exceeded: max {0}")]
    SlaveLimit(usize),

    #[error("Not a master node")]
    NotMaster,

    #[error("Not a slave node")]
    NotSlave,

    #[error("Replication not configured")]
    NotConfigured,

    #[error("Snapshot error: {0}")]
    SnapshotError(String),

    #[error("WAL error: {0}")]
    WalError(String),
}

pub type ReplicationResult<T> = Result<T, ReplicationError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_role() {
        let master = NodeRole::Master;
        assert_eq!(master, NodeRole::Master);

        let slave = NodeRole::Slave {
            master_addr: "127.0.0.1:50051".parse().unwrap(),
        };
        assert!(matches!(slave, NodeRole::Slave { .. }));
    }

    #[test]
    fn test_replication_config_default() {
        let config = ReplicationConfig::default();
        assert_eq!(config.role, NodeRole::Master);
        assert_eq!(config.max_slaves, 10);
        assert_eq!(config.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_backoff_config() {
        let config = BackoffConfig::default();
        assert_eq!(config.initial, Duration::from_secs(1));
        assert_eq!(config.max, Duration::from_secs(60));
        assert_eq!(config.multiplier, 2.0);
    }
}