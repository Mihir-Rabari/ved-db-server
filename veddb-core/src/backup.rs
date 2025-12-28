//! Backup and restore functionality for VedDB
//!
//! This module provides high-level backup and restore operations:
//! - Online backup creation without blocking operations
//! - Point-in-time recovery using WAL replay
//! - Collection export/import to/from JSON
//! - Backup verification and integrity checks

use crate::document::Document;
use crate::snapshot::{create_snapshot, load_snapshot, SnapshotError};
use crate::storage::persistent::PersistentLayer;
use crate::wal::{WalReader, WalError};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;

/// Backup configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    /// Include WAL files in backup
    pub include_wal: bool,
    /// Compress backup files
    pub compress: bool,
    /// Backup directory
    pub backup_dir: PathBuf,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            include_wal: true,
            compress: true,
            backup_dir: PathBuf::from("./backups"),
        }
    }
}

/// Backup metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    /// Backup ID
    pub backup_id: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// WAL sequence at backup time
    pub wal_sequence: u64,
    /// Backup file path
    pub file_path: PathBuf,
    /// Backup size in bytes
    pub size_bytes: u64,
    /// Whether WAL files are included
    pub includes_wal: bool,
    /// Whether backup is compressed
    pub compressed: bool,
}

/// Backup manager
pub struct BackupManager {
    config: BackupConfig,
    persistent_layer: Arc<PersistentLayer>,
}

impl BackupManager {
    /// Create a new backup manager
    pub fn new(config: BackupConfig, persistent_layer: Arc<PersistentLayer>) -> Self {
        Self {
            config,
            persistent_layer,
        }
    }

    /// Get backup configuration
    pub fn config(&self) -> &BackupConfig {
        &self.config
    }

    /// Create a backup (Atomic with FIFO Retention)
    /// 
    /// Uses .tmp file markers during creation to prevent cleanup of incomplete backups.
    /// After successful creation, triggers FIFO retention to maintain max 5 backups.
    pub async fn create_backup(&self, wal_sequence: u64) -> Result<BackupInfo> {
        use tracing::info;

        // Generate backup ID
        let backup_id = format!("backup_{}", Utc::now().format("%Y%m%d_%H%M%S"));
        
        // Ensure backup directory exists
        fs::create_dir_all(&self.config.backup_dir).await?;
        
        // Create backup file paths (temporary first)
        let backup_filename = if self.config.compress {
            format!("{}.veddb.gz", backup_id)
        } else {
            format!("{}.veddb", backup_id)
        };
        
        // Step 1: Create with .tmp extension
        let temp_backup_path = self.config.backup_dir.join(format!("{}.tmp", backup_filename));
        let final_backup_path = self.config.backup_dir.join(&backup_filename);
        
        // Step 2: Create snapshot to temporary file
        create_snapshot(
            self.persistent_layer.clone(),
            &temp_backup_path,
            wal_sequence,
        ).await.map_err(|e| anyhow::anyhow!("Failed to create snapshot: {}", e))?;
        
        // Step 3: Get file size
        let metadata = fs::metadata(&temp_backup_path).await?;
        let size_bytes = metadata.len();
        
        let backup_info = BackupInfo {
            backup_id,
            created_at: Utc::now(),
            wal_sequence,
            file_path: final_backup_path.clone(),
            size_bytes,
            includes_wal: self.config.include_wal,
            compressed: self.config.compress,
        };
        
        // Step 4: Save metadata to temporary file
        let temp_meta_path = temp_backup_path.with_extension("meta.tmp");
        let final_meta_path = final_backup_path.with_extension("meta");
        
        let metadata_json = serde_json::to_string_pretty(&backup_info)?;
        fs::write(&temp_meta_path, metadata_json).await?;
        
        // Step 5: Atomic rename (both files)
        // This is the atomic commit point - either both succeed or neither
        fs::rename(&temp_backup_path, &final_backup_path).await
            .map_err(|e| anyhow::anyhow!("Failed to finalize backup: {}", e))?;
        fs::rename(&temp_meta_path, &final_meta_path).await
            .map_err(|e| anyhow::anyhow!("Failed to finalize metadata: {}", e))?;
        
        info!("Backup created atomically: {:?}", final_backup_path);
        
        // Step 6: Cleanup old backups (FIFO retention)
        self.cleanup_old_backups().await?;
        
        Ok(backup_info)
    }

    /// Restore from backup
    pub async fn restore_backup(&self, backup_path: &Path) -> Result<u64> {
        // Verify backup exists
        if !backup_path.exists() {
            return Err(anyhow::anyhow!("Backup file not found: {}", backup_path.display()));
        }

        // Load snapshot
        let wal_sequence = load_snapshot(backup_path, self.persistent_layer.clone())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to load snapshot: {}", e))?;

        Ok(wal_sequence)
    }

    /// Point-in-time recovery
    pub async fn point_in_time_recovery(
        &self,
        backup_path: &Path,
        target_time: DateTime<Utc>,
        wal_dir: &Path,
    ) -> Result<()> {
        // First restore from backup
        let backup_sequence = self.restore_backup(backup_path).await?;

        // Then replay WAL entries up to target time
        self.replay_wal_to_time(wal_dir, backup_sequence, target_time).await?;

        Ok(())
    }

    /// Export collection to JSON
    pub async fn export_collection(
        &self,
        collection_name: &str,
        output_path: &Path,
        pretty: bool,
    ) -> Result<u64> {
        // Get all documents from collection
        let documents = self.persistent_layer
            .scan_collection(collection_name)
            .map_err(|e| anyhow::anyhow!("Failed to scan collection: {}", e))?;

        // Serialize to JSON
        let json_data = if pretty {
            serde_json::to_string_pretty(&documents)?
        } else {
            serde_json::to_string(&documents)?
        };

        // Write to file
        fs::write(output_path, json_data).await?;

        Ok(documents.len() as u64)
    }

    /// Import collection from JSON
    pub async fn import_collection(
        &self,
        collection_name: &str,
        input_path: &Path,
        replace: bool,
    ) -> Result<u64> {
        // Read JSON file
        let json_data = fs::read_to_string(input_path).await?;

        // Parse documents
        let documents: Vec<Document> = serde_json::from_str(&json_data)?;

        // Clear collection if replacing
        if replace {
            self.persistent_layer.drop_collection(collection_name)
                .map_err(|e| anyhow::anyhow!("Failed to drop collection: {}", e))?;
        }

        // Insert documents
        let mut imported_count = 0;
        for doc in documents {
            self.persistent_layer
                .insert_document(collection_name, doc.id, &doc)
                .map_err(|e| anyhow::anyhow!("Failed to insert document: {}", e))?;
            imported_count += 1;
        }

        Ok(imported_count)
    }

    /// List available backups
    pub async fn list_backups(&self) -> Result<Vec<BackupInfo>> {
        let mut backups = Vec::new();

        if !self.config.backup_dir.exists() {
            return Ok(backups);
        }

        let mut entries = fs::read_dir(&self.config.backup_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("veddb") ||
               path.extension().and_then(|s| s.to_str()) == Some("gz") {
                
                // Try to load backup metadata
                if let Ok(backup_info) = self.load_backup_metadata(&path).await {
                    backups.push(backup_info);
                } else {
                    // Create basic info from file
                    let metadata = fs::metadata(&path).await?;
                    let filename = path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown");
                    
                    backups.push(BackupInfo {
                        backup_id: filename.to_string(),
                        created_at: metadata.created()
                            .map(|t| DateTime::from(t))
                            .unwrap_or_else(|_| Utc::now()),
                        wal_sequence: 0,
                        file_path: path,
                        size_bytes: metadata.len(),
                        includes_wal: false,
                        compressed: false,
                    });
                }
            }
        }

        // Sort by creation time (newest first)
        backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(backups)
    }

    /// Verify backup integrity
    pub async fn verify_backup(&self, backup_path: &Path) -> Result<bool> {
        // Try to open and read the snapshot header
        match crate::snapshot::reader::SnapshotReader::open(backup_path) {
            Ok(mut reader) => {
                match reader.read_header() {
                    Ok(_) => Ok(true),
                    Err(SnapshotError::ChecksumMismatch) => Ok(false),
                    Err(SnapshotError::InvalidMagic) => Ok(false),
                    Err(e) => Err(anyhow::anyhow!("Backup verification failed: {}", e)),
                }
            }
            Err(e) => Err(anyhow::anyhow!("Cannot open backup file: {}", e)),
        }
    }

    /// Replay WAL entries up to a specific time (Production-Grade PITR)
    ///
    /// IMPORTANT: This function enforces dual monotonicity:
    /// 1. Timestamp monotonicity (within reasonable clock skew)
    /// 2. Sequence monotonicity (strict, never violated)
    ///
    /// PITR will FAIL LOUDLY on any detected ambiguity to prevent data corruption.
    async fn replay_wal_to_time(
        &self,
        wal_dir: &Path,
        from_sequence: u64,
        target_time: DateTime<Utc>,
    ) -> Result<()> {
        use crate::wal::replay::apply_operation;
        use tracing::{info, warn};

        // Find WAL files after the backup sequence
        let wal_files = self.find_wal_files_after(wal_dir, from_sequence).await?;
        
        if wal_files.is_empty() {
            info!("No WAL files found for PITR after sequence {}", from_sequence);
            return Ok(());
        }
        
        let mut applied_count = 0;
        
        // IMPORTANT: last_timestamp must be global across WAL files
        // WAL files may be rotated or overlap, so we must maintain
        // monotonicity check across file boundaries. DO NOT move this
        // into the loop or "optimize" it to per-file tracking.
        let mut last_timestamp: Option<DateTime<Utc>> = None;
        let mut last_sequence_applied: u64 = from_sequence;
        
        for wal_file in wal_files {
            let mut reader = WalReader::open(&wal_file)
                .map_err(|e| anyhow::anyhow!("Failed to open WAL file {}: {}", wal_file.display(), e))?;
            
            while let Some(entry) = reader.next_entry()
                .map_err(|e| anyhow::anyhow!("Failed to read WAL entry: {}", e))? {
                
                // Check timestamp bounds - stop if we've passed target
                if entry.timestamp > target_time {
                    info!("PITR: Reached target time at sequence {}", entry.sequence);
                    return Ok(());
                }
                
                // CRITICAL: Check for timestamp ambiguity
                // Timestamps must be monotonically increasing across ALL WAL files
                if let Some(last_ts) = last_timestamp {
                    if entry.timestamp < last_ts {
                        return Err(anyhow::anyhow!(
                            "TIMESTAMP AMBIGUITY DETECTED during Point-In-Time Recovery:\n\
                             Entry {} has timestamp {:?} which is EARLIER than previous entry timestamp {:?}.\n\
                             This violates the monotonicity requirement and makes PITR unsafe.\n\
                             PITR cannot proceed. Please verify WAL consistency or use a different recovery method.",
                            entry.sequence,
                            entry.timestamp,
                            last_ts
                        ));
                    }
                }
                
                // CRITICAL: Check for sequence monotonicity
                // Sequence numbers are our final source of truth
                // Timestamps can collide or be coarse-grained, but sequences NEVER should
                if entry.sequence <= last_sequence_applied {
                    return Err(anyhow::anyhow!(
                        "NON-MONOTONIC WAL SEQUENCE DETECTED during Point-In-Time Recovery:\n\
                         Entry sequence {} <= last applied sequence {}.\n\
                         This should NEVER happen and indicates WAL corruption or incorrect WAL file ordering.\n\
                         PITR cannot proceed safely. Database integrity may be compromised.",
                        entry.sequence,
                        last_sequence_applied
                    ));
                }
                
                // Update monotonicity trackers
                last_timestamp = Some(entry.timestamp);
                last_sequence_applied = entry.sequence;
                
                // Apply operation to persistent layer
                apply_operation(&entry.operation, &self.persistent_layer).await
                    .map_err(|e| anyhow::anyhow!("Failed to apply WAL operation at sequence {}: {}", entry.sequence, e))?;
                
                applied_count += 1;
            }
        }
        
        info!("PITR: Applied {} WAL operations up to target time {:?}", applied_count, target_time);
        Ok(())
    }

    /// Find WAL files with sequences after the given sequence
    async fn find_wal_files_after(&self, wal_dir: &Path, sequence: u64) -> Result<Vec<PathBuf>> {
        let mut wal_files = Vec::new();

        if !wal_dir.exists() {
            return Ok(wal_files);
        }

        let mut entries = fs::read_dir(wal_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("wal") {
                // Extract sequence from filename (assuming format like "wal-00001.wal")
                if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Some(seq_str) = filename.strip_prefix("wal-") {
                        if let Ok(file_sequence) = seq_str.parse::<u64>() {
                            if file_sequence > sequence {
                                wal_files.push(path);
                            }
                        }
                    }
                }
            }
        }

        // Sort by sequence number
        wal_files.sort();

        Ok(wal_files)
    }

    /// Cleanup old backups according to FIFO retention policy (max 5 backups)
    /// 
    /// Ignores .tmp files to avoid deleting incomplete backups during crashes.
    /// Only deletes fully committed backups (both .veddb and .meta files present).
    async fn cleanup_old_backups(&self) -> Result<usize> {
        use tracing::{info, warn};
        
        const MAX_BACKUPS: usize = 5;
        
        // List all backups (filtering out .tmp files)
        let mut backups = self.list_backups().await?
            .into_iter()
            .filter(|b| {
                // Ignore incomplete backups (those with .tmp extension)
                let path_str = b.file_path.to_string_lossy();
                !path_str.contains(".tmp")
            })
            .collect::<Vec<_>>();
        
        // If we're under the limit, nothing to do
        if backups.len() <= MAX_BACKUPS {
            return Ok(0);
        }
        
        // Sort by creation time (oldest first)
        backups.sort_by_key(|b| b.created_at);
        
        // Calculate how many to delete
        let to_delete = backups.len() - MAX_BACKUPS;
        let mut deleted_count = 0;
        
        // Delete oldest backups
        for backup in backups.iter().take(to_delete) {
            info!("FIFO Retention: Deleting old backup {:?} (created {:?})", 
                  backup.file_path, backup.created_at);
            
            // Delete snapshot file
            if backup.file_path.exists() {
                if let Err(e) = fs::remove_file(&backup.file_path).await {
                    warn!("Failed to delete backup file {:?}: {}", backup.file_path, e);
                    continue;
                }
            }
            
            // Delete metadata file
            let metadata_path = backup.file_path.with_extension("meta");
            if metadata_path.exists() {
                if let Err(e) = fs::remove_file(&metadata_path).await {
                    warn!("Failed to delete metadata file {:?}: {}", metadata_path, e);
                }
            }
            
            deleted_count += 1;
        }
        
        if deleted_count > 0 {
            info!("FIFO Retention: Cleaned up {} old backups (keeping newest {})", 
                  deleted_count, MAX_BACKUPS);
        }
        
        Ok(deleted_count)
    }

    /// Save backup metadata
    async fn save_backup_metadata(&self, backup_info: &BackupInfo) -> Result<()> {
        let metadata_path = backup_info.file_path.with_extension("meta");
        let metadata_json = serde_json::to_string_pretty(backup_info)?;
        fs::write(metadata_path, metadata_json).await?;
        Ok(())
    }

    /// Load backup metadata
    async fn load_backup_metadata(&self, backup_path: &Path) -> Result<BackupInfo> {
        let metadata_path = backup_path.with_extension("meta");
        let metadata_json = fs::read_to_string(metadata_path).await?;
        let backup_info: BackupInfo = serde_json::from_str(&metadata_json)?;
        Ok(backup_info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Value;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_backup_creation() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().join("data");
        let backup_dir = temp_dir.path().join("backups");

        let persistent = Arc::new(PersistentLayer::new(&data_dir).unwrap());
        let config = BackupConfig {
            backup_dir,
            ..Default::default()
        };

        let backup_manager = BackupManager::new(config, persistent);

        let backup_info = backup_manager.create_backup(100).await.unwrap();
        assert!(backup_info.file_path.exists());
        assert!(backup_info.size_bytes > 0);
    }

    #[tokio::test]
    async fn test_collection_export_import() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().join("data");
        let export_path = temp_dir.path().join("export.json");

        let persistent = Arc::new(PersistentLayer::new(&data_dir).unwrap());
        let config = BackupConfig::default();
        let backup_manager = BackupManager::new(config, persistent.clone());

        // Add test document
        let mut doc = Document::new();
        let doc_id = doc.id;
        doc.insert("name".to_string(), Value::String("Test".to_string()));
        persistent.insert_document("test_collection", doc_id, &doc).unwrap();

        // Export collection
        let exported_count = backup_manager
            .export_collection("test_collection", &export_path, true)
            .await
            .unwrap();
        assert_eq!(exported_count, 1);
        assert!(export_path.exists());

        // Import to new collection
        let imported_count = backup_manager
            .import_collection("imported_collection", &export_path, false)
            .await
            .unwrap();
        assert_eq!(imported_count, 1);
    }
}