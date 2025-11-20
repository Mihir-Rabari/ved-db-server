//! WAL replay and recovery logic

use super::{Operation, WalEntry, WalError, WalReader};
use crate::storage::persistent::PersistentLayer;
use std::path::Path;
use std::sync::Arc;

/// WAL replay statistics
#[derive(Debug, Clone, Default)]
pub struct ReplayStats {
    /// Number of entries replayed
    pub entries_replayed: u64,
    /// Number of entries skipped
    pub entries_skipped: u64,
    /// Number of errors encountered
    pub errors: u64,
    /// Last sequence number replayed
    pub last_sequence: u64,
}

/// Replay WAL entries from a specific sequence number
pub async fn replay_wal(
    wal_path: &Path,
    persistent_layer: Arc<PersistentLayer>,
    from_sequence: u64,
) -> Result<ReplayStats, WalError> {
    let mut reader = WalReader::open(wal_path)?;
    let mut stats = ReplayStats::default();

    while let Some(entry) = reader.next_entry()? {
        // Skip entries before the starting sequence
        if entry.sequence < from_sequence {
            stats.entries_skipped += 1;
            continue;
        }

        // Apply the operation
        if let Err(e) = apply_operation(&entry.operation, &persistent_layer).await {
            eprintln!(
                "Error replaying entry {}: {} - {}",
                entry.sequence,
                entry.operation.description(),
                e
            );
            stats.errors += 1;
            continue;
        }

        stats.entries_replayed += 1;
        stats.last_sequence = entry.sequence;
    }

    Ok(stats)
}

/// Replay all WAL files in a directory
pub async fn replay_all_wals(
    wal_dir: &Path,
    persistent_layer: Arc<PersistentLayer>,
    from_sequence: u64,
) -> Result<ReplayStats, WalError> {
    let wal_files = super::reader::scan_wal_files(wal_dir)?;

    if wal_files.is_empty() {
        return Ok(ReplayStats::default());
    }

    let mut total_stats = ReplayStats::default();

    for wal_file in wal_files {
        let stats = replay_wal(&wal_file, persistent_layer.clone(), from_sequence).await?;

        total_stats.entries_replayed += stats.entries_replayed;
        total_stats.entries_skipped += stats.entries_skipped;
        total_stats.errors += stats.errors;
        total_stats.last_sequence = total_stats.last_sequence.max(stats.last_sequence);
    }

    Ok(total_stats)
}

/// Apply a single operation to the persistent layer
async fn apply_operation(
    operation: &Operation,
    persistent_layer: &PersistentLayer,
) -> Result<(), anyhow::Error> {
    match operation {
        Operation::Insert { collection, doc } => {
            persistent_layer.insert_document(collection, doc.id, doc)?;
        }
        Operation::Update { collection, id, changes } => {
            // Get existing document
            if let Some(mut doc) = persistent_layer.get_document(collection, *id)? {
                // Apply changes
                for (key, value) in changes {
                    doc.insert(key.clone(), value.clone());
                }
                persistent_layer.update_document(collection, *id, &doc)?;
            }
        }
        Operation::Delete { collection, id } => {
            persistent_layer.delete_document(collection, *id)?;
        }
        Operation::CreateCollection { name, schema } => {
            // Store collection metadata
            let metadata_key = format!("collection:{}", name);
            let schema_json = serde_json::to_vec(schema)?;
            persistent_layer.store_metadata(&metadata_key, &schema_json)?;
        }
        Operation::DropCollection { name } => {
            // Remove collection metadata
            let metadata_key = format!("collection:{}", name);
            persistent_layer.delete_metadata(&metadata_key)?;

            // Note: Actual document deletion would require scanning and deleting all docs
            // This is a simplified implementation
        }
        Operation::CreateIndex { collection, index } => {
            // Store index metadata
            let index_key = format!("index:{}:{}", collection, index.name);
            let index_json = serde_json::to_vec(index)?;
            persistent_layer.store_metadata(&index_key, &index_json)?;
        }
        Operation::DropIndex { collection, index_name } => {
            // Remove index metadata
            let index_key = format!("index:{}:{}", collection, index_name);
            persistent_layer.delete_metadata(&index_key)?;
        }
    }

    Ok(())
}

/// Verify WAL integrity by checking all checksums
pub fn verify_wal_integrity(wal_path: &Path) -> Result<bool, WalError> {
    let mut reader = WalReader::open(wal_path)?;
    let mut entry_count = 0;

    while let Some(entry) = reader.next_entry()? {
        // Checksum is already verified in next_entry()
        entry_count += 1;

        // Additional validation: check sequence numbers are monotonic
        if entry_count > 1 && entry.sequence < entry_count - 1 {
            return Ok(false);
        }
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Document, DocumentId, Value};
    use crate::wal::{WalConfig, WalWriter, FsyncPolicy};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_replay_wal() {
        let temp_dir = TempDir::new().unwrap();
        let wal_dir = temp_dir.path().join("wal");
        let data_dir = temp_dir.path().join("data");

        // Create WAL writer and write some operations
        let config = WalConfig {
            wal_dir: wal_dir.clone(),
            fsync_policy: FsyncPolicy::Always,
            ..Default::default()
        };

        let writer = WalWriter::new(config).unwrap();

        let mut doc = Document::new();
        let doc_id = doc.id;
        doc.insert("name".to_string(), Value::String("John".to_string()));

        writer.append(Operation::Insert {
            collection: "users".to_string(),
            doc: doc.clone(),
        }).await.unwrap();

        writer.flush().await.unwrap();

        // Create persistent layer and replay
        let persistent = Arc::new(PersistentLayer::new(&data_dir).unwrap());

        let stats = replay_all_wals(&wal_dir, persistent.clone(), 0).await.unwrap();

        assert_eq!(stats.entries_replayed, 1);
        assert_eq!(stats.entries_skipped, 0);
        assert_eq!(stats.errors, 0);

        // Verify document was inserted
        let retrieved = persistent.get_document("users", doc_id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().get("name").unwrap().as_str(), Some("John"));
    }

    #[tokio::test]
    async fn test_replay_with_skip() {
        let temp_dir = TempDir::new().unwrap();
        let wal_dir = temp_dir.path().join("wal");
        let data_dir = temp_dir.path().join("data");

        let config = WalConfig {
            wal_dir: wal_dir.clone(),
            fsync_policy: FsyncPolicy::Always,
            ..Default::default()
        };

        let writer = WalWriter::new(config).unwrap();

        // Write 3 operations
        for i in 0..3 {
            let mut doc = Document::new();
            doc.insert("index".to_string(), Value::Int32(i));

            writer.append(Operation::Insert {
                collection: "test".to_string(),
                doc,
            }).await.unwrap();
        }

        writer.flush().await.unwrap();

        // Replay from sequence 1 (skip first entry)
        let persistent = Arc::new(PersistentLayer::new(&data_dir).unwrap());
        let stats = replay_all_wals(&wal_dir, persistent, 1).await.unwrap();

        assert_eq!(stats.entries_replayed, 2);
        assert_eq!(stats.entries_skipped, 1);
    }

    #[tokio::test]
    async fn test_verify_wal_integrity() {
        let temp_dir = TempDir::new().unwrap();
        let config = WalConfig {
            wal_dir: temp_dir.path().to_path_buf(),
            fsync_policy: FsyncPolicy::Always,
            ..Default::default()
        };

        let writer = WalWriter::new(config.clone()).unwrap();

        writer.append(Operation::Insert {
            collection: "test".to_string(),
            doc: Document::new(),
        }).await.unwrap();

        writer.flush().await.unwrap();

        let wal_files = super::super::reader::scan_wal_files(&config.wal_dir).unwrap();
        assert_eq!(wal_files.len(), 1);

        let is_valid = verify_wal_integrity(&wal_files[0]).unwrap();
        assert!(is_valid);
    }
}
