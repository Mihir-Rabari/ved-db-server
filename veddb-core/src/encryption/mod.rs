//! Encryption engine for VedDB
//!
//! This module provides:
//! - Data-at-rest encryption using AES-256-GCM
//! - Key management with per-collection keys
//! - Document and WAL entry encryption/decryption
//! - Key rotation support

pub mod key_manager;
pub mod document_encryption;

pub use key_manager::*;
pub use document_encryption::*;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Encryption configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    /// Whether encryption is enabled
    pub enabled: bool,
    
    /// Master key for key derivation
    pub master_key: Option<String>,
    
    /// Key rotation interval in days (default: 90)
    pub key_rotation_days: u32,
    
    /// Per-collection encryption settings
    pub collection_encryption: std::collections::HashMap<String, CollectionEncryptionConfig>,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            master_key: None,
            key_rotation_days: 90,
            collection_encryption: std::collections::HashMap::new(),
        }
    }
}

/// Per-collection encryption configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionEncryptionConfig {
    /// Whether this collection is encrypted
    pub enabled: bool,
    
    /// Key ID for this collection
    pub key_id: String,
    
    /// Encryption algorithm (currently only AES-256-GCM)
    pub algorithm: String,
}

impl Default for CollectionEncryptionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            key_id: String::new(),
            algorithm: "AES-256-GCM".to_string(),
        }
    }
}

/// Encryption engine coordinating key management and document encryption
pub struct EncryptionEngine {
    config: EncryptionConfig,
    key_manager: KeyManager,
    document_encryption: DocumentEncryption,
}

impl EncryptionEngine {
    /// Create a new encryption engine
    pub fn new(config: EncryptionConfig, storage_path: &str) -> Result<Self> {
        let key_manager = KeyManager::new(storage_path, config.master_key.as_deref())?;
        let document_encryption = DocumentEncryption::new();
        
        Ok(Self {
            config,
            key_manager,
            document_encryption,
        })
    }

    /// Check if encryption is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Check if a collection is encrypted
    pub fn is_collection_encrypted(&self, collection_name: &str) -> bool {
        if !self.config.enabled {
            return false;
        }
        
        self.config
            .collection_encryption
            .get(collection_name)
            .map(|config| config.enabled)
            .unwrap_or(false)
    }

    /// Encrypt document data
    pub fn encrypt_document(&self, collection_name: &str, data: &[u8]) -> Result<Vec<u8>> {
        if !self.is_collection_encrypted(collection_name) {
            return Ok(data.to_vec());
        }

        let key_id = self.get_collection_key_id(collection_name)?;
        let key = self.key_manager.get_key(&key_id)?;
        self.document_encryption.encrypt(data, &key)
    }

    /// Decrypt document data
    pub fn decrypt_document(&self, collection_name: &str, encrypted_data: &[u8]) -> Result<Vec<u8>> {
        if !self.is_collection_encrypted(collection_name) {
            return Ok(encrypted_data.to_vec());
        }

        let key_id = self.get_collection_key_id(collection_name)?;
        let key = self.key_manager.get_key(&key_id)?;
        self.document_encryption.decrypt(encrypted_data, &key)
    }

    /// Encrypt WAL entry data
    pub fn encrypt_wal_entry(&mut self, data: &[u8]) -> Result<Vec<u8>> {
        if !self.config.enabled {
            return Ok(data.to_vec());
        }

        let key_id = "wal_key";
        let key = self.key_manager.get_or_create_key(key_id)?;
        self.document_encryption.encrypt(data, &key)
    }

    /// Decrypt WAL entry data
    pub fn decrypt_wal_entry(&self, encrypted_data: &[u8]) -> Result<Vec<u8>> {
        if !self.config.enabled {
            return Ok(encrypted_data.to_vec());
        }

        let key_id = "wal_key";
        let key = self.key_manager.get_key(key_id)?;
        self.document_encryption.decrypt(encrypted_data, &key)
    }

    /// Encrypt snapshot data
    pub fn encrypt_snapshot(&mut self, data: &[u8]) -> Result<Vec<u8>> {
        if !self.config.enabled {
            return Ok(data.to_vec());
        }

        let key_id = "snapshot_key";
        let key = self.key_manager.get_or_create_key(key_id)?;
        self.document_encryption.encrypt(data, &key)
    }

    /// Decrypt snapshot data
    pub fn decrypt_snapshot(&self, encrypted_data: &[u8]) -> Result<Vec<u8>> {
        if !self.config.enabled {
            return Ok(encrypted_data.to_vec());
        }

        let key_id = "snapshot_key";
        let key = self.key_manager.get_key(key_id)?;
        self.document_encryption.decrypt(encrypted_data, &key)
    }

    /// Enable encryption for a collection
    pub fn enable_collection_encryption(&mut self, collection_name: &str) -> Result<()> {
        let key_id = format!("collection_{}", collection_name);
        self.key_manager.get_or_create_key(&key_id)?;
        
        let config = CollectionEncryptionConfig {
            enabled: true,
            key_id: key_id.clone(),
            algorithm: "AES-256-GCM".to_string(),
        };
        
        self.config.collection_encryption.insert(collection_name.to_string(), config);
        Ok(())
    }

    /// Disable encryption for a collection
    pub fn disable_collection_encryption(&mut self, collection_name: &str) -> Result<()> {
        if let Some(config) = self.config.collection_encryption.get_mut(collection_name) {
            config.enabled = false;
        }
        Ok(())
    }

    /// Get the key ID for a collection
    fn get_collection_key_id(&self, collection_name: &str) -> Result<String> {
        self.config
            .collection_encryption
            .get(collection_name)
            .map(|config| config.key_id.clone())
            .ok_or_else(|| anyhow::anyhow!("No encryption config for collection: {}", collection_name))
    }

    /// Get key manager for advanced operations
    pub fn key_manager(&self) -> &KeyManager {
        &self.key_manager
    }

    /// Get mutable key manager for advanced operations
    pub fn key_manager_mut(&mut self) -> &mut KeyManager {
        &mut self.key_manager
    }

    /// Update encryption configuration
    pub fn update_config(&mut self, config: EncryptionConfig) {
        self.config = config;
    }

    /// Get current encryption configuration
    pub fn config(&self) -> &EncryptionConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_config() -> EncryptionConfig {
        EncryptionConfig {
            enabled: true,
            master_key: Some("test_master_key_32_bytes_long_123".to_string()),
            key_rotation_days: 90,
            collection_encryption: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn test_encryption_engine_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        
        let engine = EncryptionEngine::new(config, temp_dir.path().to_str().unwrap()).unwrap();
        assert!(engine.is_enabled());
    }

    #[test]
    fn test_collection_encryption() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        
        let mut engine = EncryptionEngine::new(config, temp_dir.path().to_str().unwrap()).unwrap();
        
        // Enable encryption for a collection
        engine.enable_collection_encryption("test_collection").unwrap();
        assert!(engine.is_collection_encrypted("test_collection"));
        
        // Test document encryption/decryption
        let original_data = b"Hello, World! This is test document data.";
        let encrypted = engine.encrypt_document("test_collection", original_data).unwrap();
        assert_ne!(encrypted, original_data);
        
        let decrypted = engine.decrypt_document("test_collection", &encrypted).unwrap();
        assert_eq!(decrypted, original_data);
    }

    #[test]
    fn test_wal_encryption() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        
        let mut engine = EncryptionEngine::new(config, temp_dir.path().to_str().unwrap()).unwrap();
        
        let original_data = b"WAL entry data for testing encryption";
        let encrypted = engine.encrypt_wal_entry(original_data).unwrap();
        assert_ne!(encrypted, original_data);
        
        let decrypted = engine.decrypt_wal_entry(&encrypted).unwrap();
        assert_eq!(decrypted, original_data);
    }

    #[test]
    fn test_snapshot_encryption() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        
        let mut engine = EncryptionEngine::new(config, temp_dir.path().to_str().unwrap()).unwrap();
        
        let original_data = b"Snapshot data for testing encryption functionality";
        let encrypted = engine.encrypt_snapshot(original_data).unwrap();
        assert_ne!(encrypted, original_data);
        
        let decrypted = engine.decrypt_snapshot(&encrypted).unwrap();
        assert_eq!(decrypted, original_data);
    }

    #[test]
    fn test_disabled_encryption() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = create_test_config();
        config.enabled = false;
        
        let engine = EncryptionEngine::new(config, temp_dir.path().to_str().unwrap()).unwrap();
        assert!(!engine.is_enabled());
        
        let original_data = b"This should not be encrypted";
        let result = engine.encrypt_document("test_collection", original_data).unwrap();
        assert_eq!(result, original_data);
    }
}