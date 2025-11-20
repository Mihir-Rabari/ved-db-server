//! WAL entry definitions and serialization

use crate::document::{Document, DocumentId, Value};
use crate::schema::{IndexDefinition, Schema};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// WAL entry representing a single operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEntry {
    /// Sequence number (monotonically increasing)
    pub sequence: u64,
    /// Timestamp when operation was logged
    pub timestamp: DateTime<Utc>,
    /// The operation to be performed
    pub operation: Operation,
    /// CRC32 checksum for integrity verification
    pub checksum: u32,
}

impl WalEntry {
    /// Create a new WAL entry
    pub fn new(sequence: u64, operation: Operation) -> Self {
        Self {
            sequence,
            timestamp: Utc::now(),
            operation,
            checksum: 0, // Will be computed during serialization
        }
    }

    /// Serialize entry to bytes (without checksum)
    pub fn serialize_without_checksum(&self) -> Result<Vec<u8>, WalError> {
        serde_json::to_vec(self).map_err(|e| WalError::SerializationError(e.to_string()))
    }

    /// Compute CRC32 checksum for the entry
    pub fn compute_checksum(&self) -> Result<u32, WalError> {
        // Create a copy with checksum set to 0 for consistent hashing
        let mut entry_for_hash = self.clone();
        entry_for_hash.checksum = 0;
        
        let bytes = serde_json::to_vec(&entry_for_hash)
            .map_err(|e| WalError::SerializationError(e.to_string()))?;
        Ok(crc32fast::hash(&bytes))
    }

    /// Verify the checksum of this entry
    pub fn verify_checksum(&self) -> Result<bool, WalError> {
        let computed = self.compute_checksum()?;
        Ok(computed == self.checksum)
    }
}

/// Operations that can be logged in the WAL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Insert a new document
    Insert {
        collection: String,
        doc: Document,
    },
    /// Update an existing document
    Update {
        collection: String,
        id: DocumentId,
        changes: BTreeMap<String, Value>,
    },
    /// Delete a document
    Delete {
        collection: String,
        id: DocumentId,
    },
    /// Create a new collection
    CreateCollection {
        name: String,
        schema: Schema,
    },
    /// Drop a collection
    DropCollection {
        name: String,
    },
    /// Create an index
    CreateIndex {
        collection: String,
        index: IndexDefinition,
    },
    /// Drop an index
    DropIndex {
        collection: String,
        index_name: String,
    },
}

impl Operation {
    /// Get a human-readable description of the operation
    pub fn description(&self) -> String {
        match self {
            Operation::Insert { collection, .. } => format!("Insert into {}", collection),
            Operation::Update { collection, id, .. } => format!("Update {} in {}", id, collection),
            Operation::Delete { collection, id } => format!("Delete {} from {}", id, collection),
            Operation::CreateCollection { name, .. } => format!("Create collection {}", name),
            Operation::DropCollection { name } => format!("Drop collection {}", name),
            Operation::CreateIndex { collection, index } => {
                format!("Create index {} on {}", index.name, collection)
            }
            Operation::DropIndex { collection, index_name } => {
                format!("Drop index {} from {}", index_name, collection)
            }
        }
    }
}

/// WAL-related errors
#[derive(Debug, thiserror::Error)]
pub enum WalError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: u32, actual: u32 },

    #[error("Corrupted WAL entry at sequence {0}")]
    CorruptedEntry(u64),

    #[error("WAL file not found: {0}")]
    FileNotFound(String),

    #[error("Invalid WAL format")]
    InvalidFormat,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wal_entry_creation() {
        let op = Operation::Insert {
            collection: "users".to_string(),
            doc: Document::new(),
        };

        let entry = WalEntry::new(1, op);
        assert_eq!(entry.sequence, 1);
        assert_eq!(entry.checksum, 0);
    }

    #[test]
    fn test_wal_entry_checksum() {
        let op = Operation::Delete {
            collection: "users".to_string(),
            id: DocumentId::new(),
        };

        let mut entry = WalEntry::new(1, op);
        let checksum = entry.compute_checksum().unwrap();
        entry.checksum = checksum;

        assert!(entry.verify_checksum().unwrap());
    }

    #[test]
    fn test_operation_description() {
        let op = Operation::CreateCollection {
            name: "test".to_string(),
            schema: Schema::new(),
        };

        assert_eq!(op.description(), "Create collection test");
    }
}
