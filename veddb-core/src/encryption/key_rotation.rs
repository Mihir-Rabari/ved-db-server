//! Key rotation and re-encryption services

use crate::encryption::{EncryptionEngine, KeyManager};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::time::{interval, Duration as TokioDuration};

/// Key rotation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRotationConfig {
    /// Enable automatic key rotation
    pub enabled: bool,
    
    /// Rotation interval in days (default: 90)
    pub rotation_interval_days: u32,
    
    /// Maximum number of old keys to keep (default: 3)
    pub max_old_keys: u32,
    
    /// Re-encryption batch size (default: 1000)
    pub reencryption_batch_size: usize,
    
    /// Re-encryption delay between batches in milliseconds (default: 100)
    pub reencryption_delay_ms: u64,
}

impl Default for KeyRotationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rotation_interval_days: 90,
            max_old_keys: 3,
            reencryption_batch_size: 1000,
            reencryption_delay_ms: 100,
        }
    }
}

/// Key rotation status for a specific key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRotationStatus {
    /// Key identifier
    pub key_id: String,
    
    /// Current rotation status
    pub status: RotationStatus,
    
    /// When rotation started
    pub started_at: Option<DateTime<Utc>>,
    
    /// When rotation completed
    pub completed_at: Option<DateTime<Utc>>,
    
    /// Number of records re-encrypted
    pub records_processed: u64,
    
    /// Total number of records to process
    pub total_records: u64,
    
    /// Error message if rotation failed
    pub error_message: Option<String>,
}

/// Rotation status enumeration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RotationStatus {
    /// Rotation is pending
    Pending,
    
    /// Rotation is in progress
    InProgress,
    
    /// Rotation completed successfully
    Completed,
    
    /// Rotation failed
    Failed,
    
    /// Rotation was cancelled
    Cancelled,
}

/// Key rotation scheduler managing automatic key rotation
pub struct KeyRotationScheduler {
    config: KeyRotationConfig,
    rotation_statuses: HashMap<String, KeyRotationStatus>,
    last_check: DateTime<Utc>,
}

impl KeyRotationScheduler {
    /// Create a new key rotation scheduler
    pub fn new(config: KeyRotationConfig) -> Self {
        Self {
            config,
            rotation_statuses: HashMap::new(),
            last_check: Utc::now(),
        }
    }

    /// Start the key rotation scheduler
    pub async fn start(
        &mut self,
        encryption_engine: &mut EncryptionEngine,
        storage: &dyn crate::encryption::EncryptedStorage,
    ) -> Result<()> {
        if !self.config.enabled {
            log::info!("Key rotation scheduler is disabled");
            return Ok(());
        }

        log::info!("Starting key rotation scheduler with {} day interval", 
                   self.config.rotation_interval_days);

        // Check for keys needing rotation immediately
        self.check_and_rotate_keys(encryption_engine, storage).await?;

        // Schedule periodic checks (every 24 hours)
        let mut interval = interval(TokioDuration::from_secs(24 * 60 * 60));
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.check_and_rotate_keys(encryption_engine, storage).await {
                log::error!("Key rotation check failed: {}", e);
            }
        }
    }

    /// Check for keys needing rotation and rotate them
    pub async fn check_and_rotate_keys(
        &mut self,
        encryption_engine: &mut EncryptionEngine,
        storage: &dyn crate::encryption::EncryptedStorage,
    ) -> Result<()> {
        let now = Utc::now();
        self.last_check = now;

        // Check for keys approaching expiration (warn at 80% threshold)
        {
            let key_manager = encryption_engine.key_manager();
            let expiry_warnings = key_manager.get_keys_with_expiry_warnings(self.config.rotation_interval_days);
            for (key, days_remaining) in expiry_warnings {
                log::warn!(
                    "Key rotation warning: Key '{}' will expire in {} days (created: {}, last rotated: {})",
                    key.id,
                    days_remaining,
                    key.created_at.format("%Y-%m-%d"),
                    key.last_rotated.format("%Y-%m-%d")
                );
            }
        }

        // Get list of key IDs that need rotation (avoid borrowing conflicts)
        let key_ids_needing_rotation: Vec<String> = {
            let key_manager = encryption_engine.key_manager();
            let keys_needing_rotation = key_manager.get_keys_needing_rotation(self.config.rotation_interval_days);
            keys_needing_rotation.iter().map(|k| k.id.clone()).collect()
        };

        if key_ids_needing_rotation.is_empty() {
            log::debug!("No keys need rotation at this time");
            return Ok(());
        }

        log::info!("Found {} keys needing rotation", key_ids_needing_rotation.len());

        for key_id in key_ids_needing_rotation {
            // Skip if already in progress
            if let Some(status) = self.rotation_statuses.get(&key_id) {
                if status.status == RotationStatus::InProgress {
                    log::debug!("Key rotation already in progress for: {}", key_id);
                    continue;
                }
            }

            // Start rotation for this key
            match self.rotate_key(encryption_engine, storage, &key_id).await {
                Ok(()) => {
                    log::info!("Successfully rotated key: {}", key_id);
                }
                Err(e) => {
                    log::error!("Failed to rotate key {}: {}", key_id, e);
                    self.mark_rotation_failed(&key_id, &e.to_string());
                }
            }
        }

        Ok(())
    }

    /// Rotate a specific key with real re-encryption
    pub async fn rotate_key(
        &mut self,
        encryption_engine: &mut EncryptionEngine,
        storage: &dyn crate::encryption::EncryptedStorage,
        key_id: &str,
    ) -> Result<()> {
        log::info!("Starting key rotation with re-encryption for: {}", key_id);

        // Initialize rotation status
        let mut status = KeyRotationStatus {
            key_id: key_id.to_string(),
            status: RotationStatus::InProgress,
            started_at: Some(Utc::now()),
            completed_at: None,
            records_processed: 0,
            total_records: 0,
            error_message: None,
        };

        self.rotation_statuses.insert(key_id.to_string(), status.clone());

        // Step 1: Rotate the key and get old key for re-encryption
        let (old_key, old_version) = encryption_engine
            .key_manager_mut()
            .rotate_key_with_backup(key_id)?;
        
        let new_key = encryption_engine.key_manager().get_key(key_id)?;
        let new_version = encryption_engine.key_manager().get_key_metadata(key_id)?.version;

        // Create re-encryption context
        let context = crate::encryption::ReEncryptionContext::new(
            key_id.to_string(),
            old_key,
            old_version,
            new_key,
            new_version,
        );

        // Step 2: Scan storage for documents to re-encrypt
        let collections = storage.list_encrypted_collections()?;
        let mut all_documents = Vec::new();
        
        for collection in &collections {
            // Only process collections that use this key
            if self.is_collection_using_key(collection, key_id) {
                let docs = storage.scan_encrypted_collection(collection)?;
                all_documents.extend(docs);
            }
        }
        
        status.total_records = all_documents.len() as u64;
        self.rotation_statuses.insert(key_id.to_string(), status.clone());
        
        log::info!("Found {} documents to re-encrypt for key {}", status.total_records, key_id);

        // Step 3: Re-encrypt in batches
        let batch_size = self.config.reencryption_batch_size;
        let delay = TokioDuration::from_millis(self.config.reencryption_delay_ms);
        let mut processed = 0;

        for chunk in all_documents.chunks(batch_size) {
            // Re-encrypt this batch
            let count = self.reencrypt_batch(encryption_engine, storage, &context, chunk).await?;
            
            processed += count as u64;
            status.records_processed = processed;
            
            // Update status
            self.rotation_statuses.insert(key_id.to_string(), status.clone());
            
            log::info!("Re-encrypted {}/{} documents for key {}", 
                      processed, status.total_records, key_id);
            
            // Add delay between batches to avoid overwhelming the system
            if processed < status.total_records {
                tokio::time::sleep(delay).await;
            }
        }

        // Step 4: Mark rotation as completed
        status.status = RotationStatus::Completed;
        status.completed_at = Some(Utc::now());
        self.rotation_statuses.insert(key_id.to_string(), status);

        log::info!("Completed key rotation for: {} ({} records re-encrypted)", key_id, processed);
        Ok(())
    }
    
    /// Check if a collection uses a specific key
    /// For now, we check if the collection name matches the key pattern
    fn is_collection_using_key(&self, collection: &str, key_id: &str) -> bool {
        // Key pattern: "collection_{name}" → matches collection "{name}"
        if key_id.starts_with("collection_") {
            let collection_part = &key_id["collection_".len()..];
            collection == collection_part
        } else {
            // WAL, snapshot, and other keys don't match regular collections
            false
        }
    }

    /// Force rotation of a specific key (manual trigger)
    pub async fn force_rotate_key(&mut self, encryption_engine: &mut EncryptionEngine, key_id: &str) -> Result<()> {
        log::info!("Force rotating key: {}", key_id);
        self.rotate_key(encryption_engine, key_id).await
    }

    /// Cancel an in-progress key rotation
    pub fn cancel_rotation(&mut self, key_id: &str) -> Result<()> {
        if let Some(status) = self.rotation_statuses.get_mut(key_id) {
            if status.status == RotationStatus::InProgress {
                status.status = RotationStatus::Cancelled;
                log::info!("Cancelled key rotation for: {}", key_id);
                return Ok(());
            }
        }
        
        Err(anyhow!("No active rotation found for key: {}", key_id))
    }

    /// Get rotation status for a key
    pub fn get_rotation_status(&self, key_id: &str) -> Option<&KeyRotationStatus> {
        self.rotation_statuses.get(key_id)
    }

    /// Get all rotation statuses
    pub fn get_all_rotation_statuses(&self) -> &HashMap<String, KeyRotationStatus> {
        &self.rotation_statuses
    }

    /// Mark a rotation as failed
    fn mark_rotation_failed(&mut self, key_id: &str, error_message: &str) {
        if let Some(status) = self.rotation_statuses.get_mut(key_id) {
            status.status = RotationStatus::Failed;
            status.error_message = Some(error_message.to_string());
        }
    }

    /// Estimate number of records that need re-encryption for a key
    async fn estimate_records_for_key(&self, key_id: &str) -> Result<u64> {
        // In a real implementation, this would query the database
        // For now, we'll return a simulated count based on key type
        let count = match key_id {
            id if id.starts_with("collection_") => 10000, // Simulate collection data
            "wal_key" => 5000,                             // Simulate WAL entries
            "snapshot_key" => 1000,                        // Simulate snapshot files
            _ => 1000,                                     // Default count
        };
        
        Ok(count)
    }

    /// Re-encrypt a batch of documents
    async fn reencrypt_batch(
        &self,
        encryption_engine: &mut EncryptionEngine,
        storage: &dyn crate::encryption::EncryptedStorage,
        context: &crate::encryption::ReEncryptionContext,
        documents: &[crate::encryption::EncryptedDocumentRef],
    ) -> Result<usize> {
        use crate::encryption::DocumentEncryption;
        
        let doc_encryption = DocumentEncryption::new();
        let mut reencrypted_count = 0;
        
        for doc_ref in documents {
            // Decrypt with old key
            let decrypted_data = match doc_encryption.decrypt(&doc_ref.encrypted_data, &context.old_key) {
                Ok(data) => data,
                Err(e) => {
                    log::error!("Failed to decrypt document {} in collection {}: {}", 
                               doc_ref.doc_id, doc_ref.collection, e);
                    continue; // Skip this document, continue with others
                }
            };
            
            // Re-encrypt with new key
            let new_encrypted_data = doc_encryption.encrypt(&decrypted_data, &context.new_key)?;
            
            // Atomically update the encrypted document
            storage.update_encrypted_document(
                &doc_ref.collection,
                doc_ref.doc_id,
                new_encrypted_data,
            )?;
            
            reencrypted_count += 1;
        }
        
        log::debug!("Re-encrypted {} documents for key {}", reencrypted_count, context.key_id);
        Ok(reencrypted_count)
    }

    /// Clean up old rotation statuses
    pub fn cleanup_old_statuses(&mut self, retention_days: u32) {
        let cutoff = Utc::now() - Duration::days(retention_days as i64);
        
        self.rotation_statuses.retain(|_, status| {
            if let Some(completed_at) = status.completed_at {
                completed_at > cutoff
            } else {
                true // Keep in-progress rotations
            }
        });
    }

    /// Get rotation statistics
    pub fn get_rotation_statistics(&self) -> RotationStatistics {
        let mut stats = RotationStatistics::default();
        
        for status in self.rotation_statuses.values() {
            match status.status {
                RotationStatus::Pending => stats.pending += 1,
                RotationStatus::InProgress => stats.in_progress += 1,
                RotationStatus::Completed => stats.completed += 1,
                RotationStatus::Failed => stats.failed += 1,
                RotationStatus::Cancelled => stats.cancelled += 1,
            }
            
            stats.total_records_processed += status.records_processed;
        }
        
        stats.total_rotations = self.rotation_statuses.len() as u64;
        stats
    }

    /// Update configuration
    pub fn update_config(&mut self, config: KeyRotationConfig) {
        self.config = config;
        log::info!("Updated key rotation configuration");
    }

    /// Get current configuration
    pub fn config(&self) -> &KeyRotationConfig {
        &self.config
    }
}

/// Key rotation statistics
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RotationStatistics {
    pub total_rotations: u64,
    pub pending: u64,
    pub in_progress: u64,
    pub completed: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub total_records_processed: u64,
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::encryption::{EncryptionConfig, EncryptionEngine};
    use tempfile::TempDir;

    fn create_test_config() -> EncryptionConfig {
        EncryptionConfig {
            enabled: true,
            master_key: Some("test_master_key_32_bytes_long_123".to_string()),
            key_rotation_days: 1, // Short interval for testing
            collection_encryption: std::collections::HashMap::new(),
        }
    }

    fn create_test_rotation_config() -> KeyRotationConfig {
        KeyRotationConfig {
            enabled: true,
            rotation_interval_days: 1, // Short interval for testing
            max_old_keys: 3,
            reencryption_batch_size: 100,
            reencryption_delay_ms: 10,
        }
    }

    #[test]
    fn test_key_rotation_scheduler_creation() {
        let config = create_test_rotation_config();
        let scheduler = KeyRotationScheduler::new(config.clone());
        
        assert_eq!(scheduler.config.rotation_interval_days, config.rotation_interval_days);
        assert_eq!(scheduler.rotation_statuses.len(), 0);
    }

    #[tokio::test]
    async fn test_manual_key_rotation() {
        let temp_dir = TempDir::new().unwrap();
        let encryption_config = create_test_config();
        let rotation_config = create_test_rotation_config();
        
        let mut engine = EncryptionEngine::new(encryption_config, temp_dir.path().to_str().unwrap()).unwrap();
        let mut scheduler = KeyRotationScheduler::new(rotation_config);
        
        // Create a key first
        engine.key_manager_mut().create_key("test_key").unwrap();
        
        // Get original key
        let original_key = engine.key_manager().get_key("test_key").unwrap();
        
        // Rotate the key
        scheduler.rotate_key(&mut engine, "test_key").await.unwrap();
        
        // Get new key
        let new_key = engine.key_manager().get_key("test_key").unwrap();
        
        // Keys should be different
        assert_ne!(original_key, new_key);
        
        // Check rotation status
        let status = scheduler.get_rotation_status("test_key").unwrap();
        assert_eq!(status.status, RotationStatus::Completed);
        assert!(status.started_at.is_some());
        assert!(status.completed_at.is_some());
    }

    #[tokio::test]
    async fn test_rotation_statistics() {
        let temp_dir = TempDir::new().unwrap();
        let encryption_config = create_test_config();
        let rotation_config = create_test_rotation_config();
        
        let mut engine = EncryptionEngine::new(encryption_config, temp_dir.path().to_str().unwrap()).unwrap();
        let mut scheduler = KeyRotationScheduler::new(rotation_config);
        
        // Create and rotate multiple keys
        for i in 0..3 {
            let key_id = format!("test_key_{}", i);
            engine.key_manager_mut().create_key(&key_id).unwrap();
            scheduler.rotate_key(&mut engine, &key_id).await.unwrap();
        }
        
        let stats = scheduler.get_rotation_statistics();
        assert_eq!(stats.total_rotations, 3);
        assert_eq!(stats.completed, 3);
        assert!(stats.total_records_processed > 0);
    }

    #[test]
    fn test_rotation_status_tracking() {
        let config = create_test_rotation_config();
        let mut scheduler = KeyRotationScheduler::new(config);
        
        // Test status tracking
        let status = KeyRotationStatus {
            key_id: "test_key".to_string(),
            status: RotationStatus::InProgress,
            started_at: Some(Utc::now()),
            completed_at: None,
            records_processed: 500,
            total_records: 1000,
            error_message: None,
        };
        
        scheduler.rotation_statuses.insert("test_key".to_string(), status.clone());
        
        let retrieved_status = scheduler.get_rotation_status("test_key").unwrap();
        assert_eq!(retrieved_status.status, RotationStatus::InProgress);
        assert_eq!(retrieved_status.records_processed, 500);
        assert_eq!(retrieved_status.total_records, 1000);
    }

    #[test]
    fn test_rotation_cancellation() {
        let config = create_test_rotation_config();
        let mut scheduler = KeyRotationScheduler::new(config);
        
        // Add an in-progress rotation
        let status = KeyRotationStatus {
            key_id: "test_key".to_string(),
            status: RotationStatus::InProgress,
            started_at: Some(Utc::now()),
            completed_at: None,
            records_processed: 0,
            total_records: 1000,
            error_message: None,
        };
        
        scheduler.rotation_statuses.insert("test_key".to_string(), status);
        
        // Cancel the rotation
        scheduler.cancel_rotation("test_key").unwrap();
        
        let status = scheduler.get_rotation_status("test_key").unwrap();
        assert_eq!(status.status, RotationStatus::Cancelled);
    }

    #[test]
    fn test_cleanup_old_statuses() {
        let config = create_test_rotation_config();
        let mut scheduler = KeyRotationScheduler::new(config);
        
        // Add old completed rotation
        let old_status = KeyRotationStatus {
            key_id: "old_key".to_string(),
            status: RotationStatus::Completed,
            started_at: Some(Utc::now() - Duration::days(100)),
            completed_at: Some(Utc::now() - Duration::days(100)),
            records_processed: 1000,
            total_records: 1000,
            error_message: None,
        };
        
        // Add recent completed rotation
        let recent_status = KeyRotationStatus {
            key_id: "recent_key".to_string(),
            status: RotationStatus::Completed,
            started_at: Some(Utc::now() - Duration::days(1)),
            completed_at: Some(Utc::now() - Duration::days(1)),
            records_processed: 1000,
            total_records: 1000,
            error_message: None,
        };
        
        scheduler.rotation_statuses.insert("old_key".to_string(), old_status);
        scheduler.rotation_statuses.insert("recent_key".to_string(), recent_status);
        
        assert_eq!(scheduler.rotation_statuses.len(), 2);
        
        // Clean up statuses older than 30 days
        scheduler.cleanup_old_statuses(30);
        
        // Should only have the recent one
        assert_eq!(scheduler.rotation_statuses.len(), 1);
        assert!(scheduler.rotation_statuses.contains_key("recent_key"));
        assert!(!scheduler.rotation_statuses.contains_key("old_key"));
    }

    #[test]
    fn test_rotation_config_update() {
        let config = create_test_rotation_config();
        let mut scheduler = KeyRotationScheduler::new(config);
        
        let mut new_config = create_test_rotation_config();
        new_config.rotation_interval_days = 180;
        new_config.reencryption_batch_size = 2000;
        
        scheduler.update_config(new_config.clone());
        
        assert_eq!(scheduler.config.rotation_interval_days, 180);
        assert_eq!(scheduler.config.reencryption_batch_size, 2000);
    }

    #[test]
    fn test_rotation_status_enum() {
        // Test serialization/deserialization
        let statuses = vec![
            RotationStatus::Pending,
            RotationStatus::InProgress,
            RotationStatus::Completed,
            RotationStatus::Failed,
            RotationStatus::Cancelled,
        ];
        
        for status in statuses {
            let serialized = serde_json::to_string(&status).unwrap();
            let deserialized: RotationStatus = serde_json::from_str(&serialized).unwrap();
            assert_eq!(status, deserialized);
        }
    }

    #[tokio::test]
    async fn test_estimate_records_for_key() {
        let config = create_test_rotation_config();
        let scheduler = KeyRotationScheduler::new(config);
        
        // Test different key types
        let collection_count = scheduler.estimate_records_for_key("collection_test").await.unwrap();
        let wal_count = scheduler.estimate_records_for_key("wal_key").await.unwrap();
        let snapshot_count = scheduler.estimate_records_for_key("snapshot_key").await.unwrap();
        let other_count = scheduler.estimate_records_for_key("other_key").await.unwrap();
        
        assert_eq!(collection_count, 10000);
        assert_eq!(wal_count, 5000);
        assert_eq!(snapshot_count, 1000);
        assert_eq!(other_count, 1000);
    }
}