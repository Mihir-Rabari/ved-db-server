//! Snapshot file format definitions

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Magic bytes for snapshot files
pub const SNAPSHOT_MAGIC: &[u8; 8] = b"VEDDB\0\0\0";

/// Current snapshot format version
pub const SNAPSHOT_VERSION: u32 = 1;
/// Version constant for future reference
pub const SNAPSHOT_VERSION_V1: u32 = 1;

/// End marker for snapshot files
pub const SNAPSHOT_END_MARKER: &[u8; 10] = b"VEDDB_END\0";

/// Checksum algorithm used in snapshot (future-proof)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum ChecksumAlgo {
    SHA256 = 1,
    // Future: BLAKE3 = 2,
}

impl Default for ChecksumAlgo {
    fn default() -> Self {
        Self::SHA256
    }
}

/// Snapshot header (256 bytes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotHeader {
    /// Magic bytes
    pub magic: [u8; 8],
    /// Format version
    pub version: u32,
    /// Creation timestamp
    pub timestamp: DateTime<Utc>,
    /// WAL sequence number at snapshot time
    pub sequence: u64,
    /// Checksum algorithm (future-proof)
    pub checksum_algo: ChecksumAlgo,
    /// SHA-256 checksum of header
    pub checksum: [u8; 32],
}

impl SnapshotHeader {
    /// Create a new snapshot header
    pub fn new(sequence: u64) -> Self {
        Self {
            magic: *SNAPSHOT_MAGIC,
            version: SNAPSHOT_VERSION,
            timestamp: Utc::now(),
            sequence,
            checksum_algo: ChecksumAlgo::default(),
            checksum: [0u8; 32],
        }
    }

    /// Verify the magic bytes
    pub fn verify_magic(&self) -> bool {
        &self.magic == SNAPSHOT_MAGIC
    }

    /// Compute checksum for the header
    pub fn compute_checksum(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(&self.magic);
        hasher.update(&self.version.to_le_bytes());
        hasher.update(&self.timestamp.timestamp().to_le_bytes());
        hasher.update(&self.sequence.to_le_bytes());

        let result = hasher.finalize();
        let mut checksum = [0u8; 32];
        checksum.copy_from_slice(&result);
        checksum
    }

    /// Verify the checksum
    pub fn verify_checksum(&self) -> bool {
        let computed = self.compute_checksum();
        computed == self.checksum
    }
}

/// Snapshot metadata section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Number of collections
    pub collections_count: u32,
    /// Number of users (for future use)
    pub users_count: u32,
    /// Server configuration (JSON)
    pub config: String,
}

impl Default for SnapshotMetadata {
    fn default() -> Self {
        Self {
            collections_count: 0,
            users_count: 0,
            config: "{}".to_string(),
        }
    }
}

/// Collection header in snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionHeader {
    /// Collection name
    pub name: String,
    /// Schema (JSON)
    pub schema_json: String,
    /// Number of documents
    pub document_count: u64,
    /// Number of indexes
    pub index_count: u32,
}

/// Snapshot footer (64 bytes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotFooter {
    /// End marker
    pub end_marker: [u8; 10],
    /// Total checksum (SHA-256)
    pub total_checksum: [u8; 32],
}

impl SnapshotFooter {
    /// Create a new footer
    pub fn new(total_checksum: [u8; 32]) -> Self {
        Self {
            end_marker: *SNAPSHOT_END_MARKER,
            total_checksum,
        }
    }

    /// Verify the end marker
    pub fn verify_end_marker(&self) -> bool {
        &self.end_marker == SNAPSHOT_END_MARKER
    }
}

/// Snapshot-related errors
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("Invalid snapshot magic bytes")]
    InvalidMagic,

    #[error("Invalid snapshot version: {0}")]
    InvalidVersion(u32),

    #[error("Checksum mismatch")]
    ChecksumMismatch,

    #[error("Invalid end marker")]
    InvalidEndMarker,

    #[error("Snapshot not found: {0}")]
    NotFound(String),

    #[error("Corrupted snapshot")]
    Corrupted,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_header() {
        let mut header = SnapshotHeader::new(12345);
        assert!(header.verify_magic());

        header.checksum = header.compute_checksum();
        assert!(header.verify_checksum());
    }

    #[test]
    fn test_snapshot_footer() {
        let checksum = [0u8; 32];
        let footer = SnapshotFooter::new(checksum);
        assert!(footer.verify_end_marker());
    }

    #[test]
    fn test_snapshot_metadata() {
        let metadata = SnapshotMetadata::default();
        assert_eq!(metadata.collections_count, 0);
        assert_eq!(metadata.config, "{}");
    }
}
