//! Snapshot reader implementation

use super::{
    CollectionHeader, SnapshotError, SnapshotFooter, SnapshotHeader, SnapshotMetadata,
    SNAPSHOT_VERSION,
};
use crate::document::Document;
use crate::schema::IndexDefinition;
use crate::storage::persistent::PersistentLayer;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::sync::Arc;

/// Snapshot reader for loading database snapshots
pub struct SnapshotReader {
    /// Buffered file reader
    reader: BufReader<File>,
    /// Running checksum hasher
    hasher: Sha256,
    /// Snapshot header
    header: Option<SnapshotHeader>,
}

impl SnapshotReader {
    /// Open a snapshot file for reading
    pub fn open(path: &Path) -> Result<Self, SnapshotError> {
        let file = File::open(path)
            .map_err(|e| SnapshotError::NotFound(format!("{}: {}", path.display(), e)))?;

        Ok(Self {
            reader: BufReader::new(file),
            hasher: Sha256::new(),
            header: None,
        })
    }

    /// Read and verify snapshot header
    pub fn read_header(&mut self) -> Result<SnapshotHeader, SnapshotError> {
        // Read 256 bytes
        let mut header_bytes = vec![0u8; 256];
        self.read_bytes(&mut header_bytes)?;

        // Find actual JSON end
        let json_end = header_bytes
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(header_bytes.len());

        // Deserialize header
        let header: SnapshotHeader = serde_json::from_slice(&header_bytes[..json_end])
            .map_err(|e| SnapshotError::DeserializationError(e.to_string()))?;

        // Verify magic
        if !header.verify_magic() {
            return Err(SnapshotError::InvalidMagic);
        }

        // Verify version
        if header.version != SNAPSHOT_VERSION {
            return Err(SnapshotError::InvalidVersion(header.version));
        }

        // Verify checksum
        if !header.verify_checksum() {
            return Err(SnapshotError::ChecksumMismatch);
        }

        self.header = Some(header.clone());
        Ok(header)
    }

    /// Read snapshot metadata
    pub fn read_metadata(&mut self) -> Result<SnapshotMetadata, SnapshotError> {
        // Read length
        let mut len_bytes = [0u8; 4];
        self.read_bytes(&mut len_bytes)?;
        let len = u32::from_le_bytes(len_bytes) as usize;

        // Read metadata
        let mut metadata_bytes = vec![0u8; len];
        self.read_bytes(&mut metadata_bytes)?;

        let metadata: SnapshotMetadata = serde_json::from_slice(&metadata_bytes)
            .map_err(|e| SnapshotError::DeserializationError(e.to_string()))?;

        Ok(metadata)
    }

    /// Read collection header
    pub fn read_collection_header(&mut self) -> Result<CollectionHeader, SnapshotError> {
        // Read length
        let mut len_bytes = [0u8; 4];
        self.read_bytes(&mut len_bytes)?;
        let len = u32::from_le_bytes(len_bytes) as usize;

        // Read header
        let mut header_bytes = vec![0u8; len];
        self.read_bytes(&mut header_bytes)?;

        let header: CollectionHeader = serde_json::from_slice(&header_bytes)
            .map_err(|e| SnapshotError::DeserializationError(e.to_string()))?;

        Ok(header)
    }

    /// Read a document
    pub fn read_document(&mut self) -> Result<Document, SnapshotError> {
        // Read length
        let mut len_bytes = [0u8; 4];
        self.read_bytes(&mut len_bytes)?;
        let len = u32::from_le_bytes(len_bytes) as usize;

        // Read document
        let mut doc_bytes = vec![0u8; len];
        self.read_bytes(&mut doc_bytes)?;

        let doc: Document = serde_json::from_slice(&doc_bytes)
            .map_err(|e| SnapshotError::DeserializationError(e.to_string()))?;

        Ok(doc)
    }

    /// Read an index definition
    pub fn read_index(&mut self) -> Result<IndexDefinition, SnapshotError> {
        // Read length
        let mut len_bytes = [0u8; 4];
        self.read_bytes(&mut len_bytes)?;
        let len = u32::from_le_bytes(len_bytes) as usize;

        // Read index
        let mut index_bytes = vec![0u8; len];
        self.read_bytes(&mut index_bytes)?;

        let index: IndexDefinition = serde_json::from_slice(&index_bytes)
            .map_err(|e| SnapshotError::DeserializationError(e.to_string()))?;

        Ok(index)
    }

    /// Read and verify footer
    pub fn read_footer(&mut self) -> Result<SnapshotFooter, SnapshotError> {
        // Read 64 bytes (don't update checksum)
        let mut footer_bytes = vec![0u8; 64];
        self.reader.read_exact(&mut footer_bytes)?;

        // Parse binary footer format
        let mut end_marker = [0u8; 10];
        end_marker.copy_from_slice(&footer_bytes[..10]);

        let mut total_checksum = [0u8; 32];
        total_checksum.copy_from_slice(&footer_bytes[10..42]);

        let footer = SnapshotFooter {
            end_marker,
            total_checksum,
        };

        // Verify end marker
        if !footer.verify_end_marker() {
            return Err(SnapshotError::InvalidEndMarker);
        }

        // Verify total checksum
        let computed_checksum = self.hasher.clone().finalize();
        let mut checksum_bytes = [0u8; 32];
        checksum_bytes.copy_from_slice(&computed_checksum);

        if checksum_bytes != footer.total_checksum {
            return Err(SnapshotError::ChecksumMismatch);
        }

        Ok(footer)
    }

    /// Read bytes and update checksum
    fn read_bytes(&mut self, buf: &mut [u8]) -> Result<(), SnapshotError> {
        self.reader.read_exact(buf)?;
        self.hasher.update(buf);
        Ok(())
    }

    /// Get the snapshot header
    pub fn header(&self) -> Option<&SnapshotHeader> {
        self.header.as_ref()
    }
}

/// Load a snapshot into a persistent layer
pub async fn load_snapshot(
    snapshot_path: &Path,
    persistent_layer: Arc<PersistentLayer>,
) -> Result<u64, SnapshotError> {
    let mut reader = SnapshotReader::open(snapshot_path)?;

    // Read header
    let header = reader.read_header()?;
    let wal_sequence = header.sequence;

    // Read metadata
    let metadata = reader.read_metadata()?;

    // Read each collection
    for _ in 0..metadata.collections_count {
        let col_header = reader.read_collection_header()?;

        // Read documents
        for _ in 0..col_header.document_count {
            let doc = reader.read_document()?;
            persistent_layer
                .insert_document(&col_header.name, doc.id, &doc)
                .map_err(|e| SnapshotError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )))?;
        }

        // Read indexes
        for _ in 0..col_header.index_count {
            let _index = reader.read_index()?;
            // Store index metadata
        }
    }

    // Read and verify footer
    reader.read_footer()?;

    Ok(wal_sequence)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Value;
    use crate::snapshot::writer::create_snapshot;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_snapshot_round_trip() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().join("data");
        let snapshot_path = temp_dir.path().join("test.snapshot");

        // Create persistent layer and add some data
        let persistent = Arc::new(PersistentLayer::new(&data_dir).unwrap());

        let mut doc = Document::new();
        let doc_id = doc.id;
        doc.insert("name".to_string(), Value::String("Alice".to_string()));
        persistent.insert_document("users", doc_id, &doc).unwrap();

        // Create snapshot
        create_snapshot(persistent.clone(), &snapshot_path, 100).await.unwrap();

        // Load snapshot into new persistent layer
        let data_dir2 = temp_dir.path().join("data2");
        let persistent2 = Arc::new(PersistentLayer::new(&data_dir2).unwrap());

        let sequence = load_snapshot(&snapshot_path, persistent2.clone()).await.unwrap();
        assert_eq!(sequence, 100);

        // Verify data was loaded
        let loaded_doc = persistent2.get_document("users", doc_id).unwrap();
        assert!(loaded_doc.is_some());
        assert_eq!(loaded_doc.unwrap().get("name").unwrap().as_str(), Some("Alice"));
    }
}
