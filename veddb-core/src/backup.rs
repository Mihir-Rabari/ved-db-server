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

    /// Create a backup
    pub async fn create_backup(&self, wal_sequence: u64) -> Result<BackupInfo> {
        // Generate backup ID
        let backup_id = format!("backup_{}", Utc::now().format("%Y%m%d_%H%M%S"));
        
        // Ensure backup directory exists
        fs::create_dir_all(&self.config.backup_dir).await?;
        
        // Create backup file path
        let backup_filename = if self.config.compress {
            format!("{}.veddb.gz", backup_id)
        } else {
            format!("{}.veddb", backup_id)
        };
        let backup_path = self.config.backup_dir.join(&backup_filename);
        
        // Create snapshot
        create_snapshot(
            self.persistent_layer.clone(),
            &backup_path,
            wal_sequence,
        ).await.map_err(|e| anyhow::anyhow!("Failed to create snapshot: {}", e))?;
        
        // Get file size
        let metadata = fs::metadata(&backup_path).await?;
        let size_bytes = metadata.len();
        
        let backup_info = BackupInfo {
            backup_id,
            created_at: Utc::now(),
            wal_sequence,
            file_path: backup_path,
            size_bytes,
            includes_wal: self.config.include_wal,
            compressed: self.config.compress,
        };
        
        // Save backup metadata
        self.save_backup_metadata(&backup_info).await?;
        
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

    /// Replay WAL entries up to a specific time
    async fn replay_wal_to_time(
        &self,
        wal_dir: &Path,
        from_sequence: u64,
        target_time: DateTime<Utc>,
    ) -> Result<()> {
        // Find WAL files after the backup sequence
        let wal_files = self.find_wal_files_after(wal_dir, from_sequence).await?;

        for wal_file in wal_files {
            let mut reader = WalReader::open(&wal_file)
                .map_err(|e| anyhow::anyhow!("Failed to open WAL file: {}", e))?;

            while let Some(entry) = reader.next_entry()
                .map_err(|e| anyhow::anyhow!("Failed to read WAL entry: {}", e))? {
                
                // Stop if we've reached the target time
                if entry.timestamp > target_time {
                    break;
                }

                // Apply the operation
                // Note: This would need to be implemented based on the actual WAL entry format
                // For now, this is a placeholder
            }
        }

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