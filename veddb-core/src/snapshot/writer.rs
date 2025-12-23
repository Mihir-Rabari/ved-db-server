//! Snapshot writer implementation

use super::{
    CollectionHeader, SnapshotError, SnapshotFooter, SnapshotHeader, SnapshotMetadata,
};
use crate::document::Document;
use crate::schema::{IndexDefinition, Schema};
use crate::storage::persistent::PersistentLayer;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Snapshot writer for creating database snapshots
pub struct SnapshotWriter {
    /// Buffered file writer
    writer: BufWriter<File>,
    /// Path to snapshot file
    path: PathBuf,
    /// Running checksum hasher
    hasher: Sha256,
    /// Number of bytes written
    bytes_written: u64,
}

impl SnapshotWriter {
    /// Create a new snapshot writer
    pub fn create(path: &Path) -> Result<Self, SnapshotError> {
        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = File::create(path)?;

        Ok(Self {
            writer: BufWriter::new(file),
            path: path.to_path_buf(),
            hasher: Sha256::new(),
            bytes_written: 0,
        })
    }

    /// Write snapshot header
    pub fn write_header(&mut self, mut header: SnapshotHeader) -> Result<(), SnapshotError> {
        // Compute and set checksum
        header.checksum = header.compute_checksum();

        // Serialize header
        let header_json = serde_json::to_vec(&header)
            .map_err(|e| SnapshotError::SerializationError(e.to_string()))?;

        // Pad to 256 bytes
        let mut padded = vec![0u8; 256];
        padded[..header_json.len()].copy_from_slice(&header_json);

        self.write_bytes(&padded)?;

        Ok(())
    }

    /// Write snapshot metadata
    pub fn write_metadata(&mut self, metadata: &SnapshotMetadata) -> Result<(), SnapshotError> {
        let metadata_json = serde_json::to_vec(metadata)
            .map_err(|e| SnapshotError::SerializationError(e.to_string()))?;

        // Write length prefix
        self.write_bytes(&(metadata_json.len() as u32).to_le_bytes())?;
        // Write metadata
        self.write_bytes(&metadata_json)?;

        Ok(())
    }

    /// Write collection header
    pub fn write_collection_header(
        &mut self,
        header: &CollectionHeader,
    ) -> Result<(), SnapshotError> {
        let header_json = serde_json::to_vec(header)
            .map_err(|e| SnapshotError::SerializationError(e.to_string()))?;

        // Write length prefix
        self.write_bytes(&(header_json.len() as u32).to_le_bytes())?;
        // Write header
        self.write_bytes(&header_json)?;

        Ok(())
    }

    /// Write a document
    pub fn write_document(&mut self, doc: &Document) -> Result<(), SnapshotError> {
        let doc_json = serde_json::to_vec(doc)
            .map_err(|e| SnapshotError::SerializationError(e.to_string()))?;

        // Write length prefix
        self.write_bytes(&(doc_json.len() as u32).to_le_bytes())?;
        // Write document
        self.write_bytes(&doc_json)?;

        Ok(())
    }

    /// Write an index definition
    pub fn write_index(&mut self, index: &IndexDefinition) -> Result<(), SnapshotError> {
        let index_json = serde_json::to_vec(index)
            .map_err(|e| SnapshotError::SerializationError(e.to_string()))?;

        // Write length prefix
        self.write_bytes(&(index_json.len() as u32).to_le_bytes())?;
        // Write index
        self.write_bytes(&index_json)?;

        Ok(())
    }

    /// Finalize the snapshot with footer
    pub fn finalize(mut self) -> Result<(), SnapshotError> {
        // Get final checksum
        let total_checksum = self.hasher.finalize();
        let mut checksum_bytes = [0u8; 32];
        checksum_bytes.copy_from_slice(&total_checksum);

        // Create footer
        let footer = SnapshotFooter::new(checksum_bytes);

        // Write footer in binary format (64 bytes total)
        // 10 bytes for end marker + 32 bytes for checksum + 22 bytes padding
        let mut footer_bytes = vec![0u8; 64];
        footer_bytes[..10].copy_from_slice(&footer.end_marker);
        footer_bytes[10..42].copy_from_slice(&footer.total_checksum);

        // Write footer (don't update checksum)
        self.writer.write_all(&footer_bytes)?;
        self.writer.flush()?;

        Ok(())
    }

    /// Write bytes and update checksum
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), SnapshotError> {
        self.writer.write_all(bytes)?;
        self.hasher.update(bytes);
        self.bytes_written += bytes.len() as u64;
        Ok(())
    }

    /// Get the number of bytes written
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }
}

/// Create a snapshot from a persistent layer
pub async fn create_snapshot(
    persistent_layer: Arc<PersistentLayer>,
    output_path: &Path,
    wal_sequence: u64,
) -> Result<(), SnapshotError> {
    let mut writer = SnapshotWriter::create(output_path)?;

    // Write header
    let header = SnapshotHeader::new(wal_sequence);
    writer.write_header(header)?;

    // Get all collection names
    let collection_names = persistent_layer
        .list_collections()
        .unwrap_or_default(); // Fallback to empty if listing fails

    // Write metadata
    let metadata = SnapshotMetadata {
        collections_count: collection_names.len() as u32,
        users_count: 0,
        config: "{}".to_string(),
    };
    writer.write_metadata(&metadata)?;

    // Write each collection
    for collection_name in &collection_names {
        // Get documents from collection
        let documents = persistent_layer
            .scan_collection(collection_name)
            .unwrap_or_default();

        // Write collection header
        let col_header = CollectionHeader {
            name: collection_name.to_string(),
            schema_json: "{}".to_string(), // Placeholder
            document_count: documents.len() as u64,
            index_count: 0,
        };
        writer.write_collection_header(&col_header)?;

        // Write documents
        for doc in &documents {
            writer.write_document(doc)?;
        }
    }

    // Finalize
    writer.finalize()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Value;
    use tempfile::TempDir;

    #[test]
    fn test_snapshot_writer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let snapshot_path = temp_dir.path().join("test.snapshot");

        let writer = SnapshotWriter::create(&snapshot_path);
        assert!(writer.is_ok());
    }

    #[test]
    fn test_write_header() {
        let temp_dir = TempDir::new().unwrap();
        let snapshot_path = temp_dir.path().join("test.snapshot");

        let mut writer = SnapshotWriter::create(&snapshot_path).unwrap();
        let header = SnapshotHeader::new(100);

        let result = writer.write_header(header);
        assert!(result.is_ok());
    }

    #[test]
    fn test_write_document() {
        let temp_dir = TempDir::new().unwrap();
        let snapshot_path = temp_dir.path().join("test.snapshot");

        let mut writer = SnapshotWriter::create(&snapshot_path).unwrap();

        let mut doc = Document::new();
        doc.insert("name".to_string(), Value::String("John".to_string()));

        let result = writer.write_document(&doc);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().join("data");
        let snapshot_path = temp_dir.path().join("test.snapshot");

        let persistent = Arc::new(PersistentLayer::new(&data_dir).unwrap());

        let result = create_snapshot(persistent, &snapshot_path, 12345).await;
        assert!(result.is_ok());

        // Verify file was created
        assert!(snapshot_path.exists());
    }
}
