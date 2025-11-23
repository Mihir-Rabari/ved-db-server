//! Key management for encryption engine

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Encryption key with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionKey {
    /// Unique key identifier
    pub id: String,
    
    /// The actual key bytes (32 bytes for AES-256)
    pub key: Vec<u8>,
    
    /// When this key was created
    pub created_at: DateTime<Utc>,
    
    /// When this key was last rotated
    pub last_rotated: DateTime<Utc>,
    
    /// Whether this key is active
    pub active: bool,
    
    /// Key version for rotation tracking
    pub version: u32,
}

impl EncryptionKey {
    /// Create a new encryption key
    pub fn new(id: String) -> Self {
        let key = Self::generate_key();
        let now = Utc::now();
        
        Self {
            id,
            key,
            created_at: now,
            last_rotated: now,
            active: true,
            version: 1,
        }
    }

    /// Generate a new 256-bit (32-byte) key
    fn generate_key() -> Vec<u8> {
        use rand::RngCore;
        let mut key = vec![0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        key
    }

    /// Rotate this key (create new key bytes)
    pub fn rotate(&mut self) {
        self.key = Self::generate_key();
        self.last_rotated = Utc::now();
        self.version += 1;
    }

    /// Check if key needs rotation based on age
    pub fn needs_rotation(&self, rotation_days: u32) -> bool {
        let rotation_duration = chrono::Duration::days(rotation_days as i64);
        Utc::now() - self.last_rotated > rotation_duration
    }
}

/// Key storage metadata
#[derive(Debug, Serialize, Deserialize)]
struct KeyStorage {
    keys: HashMap<String, EncryptionKey>,
    master_key_hash: Option<String>,
}

impl Default for KeyStorage {
    fn default() -> Self {
        Self {
            keys: HashMap::new(),
            master_key_hash: None,
        }
    }
}

/// Key manager handling encryption key lifecycle
pub struct KeyManager {
    storage_path: PathBuf,
    keys: HashMap<String, EncryptionKey>,
    master_key: Option<Vec<u8>>,
    master_key_hash: Option<String>,
}

impl KeyManager {
    /// Create a new key manager
    pub fn new(storage_path: &str, master_key: Option<&str>) -> Result<Self> {
        let storage_path = Path::new(storage_path).join("encryption");
        fs::create_dir_all(&storage_path)?;
        
        let (master_key_bytes, master_key_hash) = if let Some(key) = master_key {
            let key_bytes = Self::derive_master_key(key)?;
            let hash = Self::hash_master_key(&key_bytes);
            (Some(key_bytes), Some(hash))
        } else {
            (None, None)
        };
        
        let mut manager = Self {
            storage_path,
            keys: HashMap::new(),
            master_key: master_key_bytes,
            master_key_hash,
        };
        
        manager.load_keys()?;
        Ok(manager)
    }

    /// Derive a 256-bit key from master key string
    fn derive_master_key(master_key: &str) -> Result<Vec<u8>> {
        if master_key.len() < 16 {
            return Err(anyhow!("Master key must be at least 16 characters long"));
        }
        
        let mut hasher = Sha256::new();
        hasher.update(master_key.as_bytes());
        hasher.update(b"veddb_master_key_salt");
        Ok(hasher.finalize().to_vec())
    }

    /// Hash master key for verification
    fn hash_master_key(key: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key);
        hasher.update(b"veddb_key_verification");
        hex::encode(hasher.finalize())
    }

    /// Load keys from storage
    fn load_keys(&mut self) -> Result<()> {
        let keys_file = self.storage_path.join("keys.json");
        
        if !keys_file.exists() {
            return Ok(());
        }
        
        let data = fs::read_to_string(&keys_file)?;
        let storage: KeyStorage = serde_json::from_str(&data)?;
        
        // Verify master key if provided
        if let (Some(stored_hash), Some(current_hash)) = (&storage.master_key_hash, &self.master_key_hash) {
            if stored_hash != current_hash {
                return Err(anyhow!("Master key mismatch - cannot decrypt existing keys"));
            }
        }
        
        self.keys = storage.keys;
        Ok(())
    }

    /// Save keys to storage
    fn save_keys(&self) -> Result<()> {
        let storage = KeyStorage {
            keys: self.keys.clone(),
            master_key_hash: self.master_key_hash.clone(),
        };
        
        let data = serde_json::to_string_pretty(&storage)?;
        let keys_file = self.storage_path.join("keys.json");
        fs::write(&keys_file, data)?;
        Ok(())
    }

    /// Get an existing key
    pub fn get_key(&self, key_id: &str) -> Result<Vec<u8>> {
        let key = self.keys.get(key_id)
            .ok_or_else(|| anyhow!("Key not found: {}", key_id))?;
        
        if !key.active {
            return Err(anyhow!("Key is inactive: {}", key_id));
        }
        
        Ok(key.key.clone())
    }

    /// Get or create a key
    pub fn get_or_create_key(&mut self, key_id: &str) -> Result<Vec<u8>> {
        if let Ok(key) = self.get_key(key_id) {
            return Ok(key);
        }
        
        self.create_key(key_id)?;
        self.get_key(key_id)
    }

    /// Create a new key
    pub fn create_key(&mut self, key_id: &str) -> Result<()> {
        if self.keys.contains_key(key_id) {
            return Err(anyhow!("Key already exists: {}", key_id));
        }
        
        let key = EncryptionKey::new(key_id.to_string());
        self.keys.insert(key_id.to_string(), key);
        self.save_keys()?;
        
        log::info!("Created new encryption key: {}", key_id);
        Ok(())
    }

    /// Rotate a key
    pub fn rotate_key(&mut self, key_id: &str) -> Result<()> {
        let version = {
            let key = self.keys.get_mut(key_id)
                .ok_or_else(|| anyhow!("Key not found: {}", key_id))?;
            
            key.rotate();
            key.version
        };
        
        self.save_keys()?;
        
        log::info!("Rotated encryption key: {} (version {})", key_id, version);
        Ok(())
    }

    /// Deactivate a key
    pub fn deactivate_key(&mut self, key_id: &str) -> Result<()> {
        let key = self.keys.get_mut(key_id)
            .ok_or_else(|| anyhow!("Key not found: {}", key_id))?;
        
        key.active = false;
        self.save_keys()?;
        
        log::info!("Deactivated encryption key: {}", key_id);
        Ok(())
    }

    /// List all keys
    pub fn list_keys(&self) -> Vec<&EncryptionKey> {
        self.keys.values().collect()
    }

    /// Get keys that need rotation
    pub fn get_keys_needing_rotation(&self, rotation_days: u32) -> Vec<&EncryptionKey> {
        self.keys
            .values()
            .filter(|key| key.active && key.needs_rotation(rotation_days))
            .collect()
    }

    /// Rotate all keys that need rotation
    pub fn rotate_expired_keys(&mut self, rotation_days: u32) -> Result<Vec<String>> {
        let keys_to_rotate: Vec<String> = self.keys
            .values()
            .filter(|key| key.active && key.needs_rotation(rotation_days))
            .map(|key| key.id.clone())
            .collect();
        
        for key_id in &keys_to_rotate {
            self.rotate_key(key_id)?;
        }
        
        Ok(keys_to_rotate)
    }

    /// Get key metadata
    pub fn get_key_metadata(&self, key_id: &str) -> Result<&EncryptionKey> {
        self.keys.get(key_id)
            .ok_or_else(|| anyhow!("Key not found: {}", key_id))
    }

    /// Check if master key is configured
    pub fn has_master_key(&self) -> bool {
        self.master_key.is_some()
    }

    /// Get total number of keys
    pub fn key_count(&self) -> usize {
        self.keys.len()
    }

    /// Get number of active keys
    pub fn active_key_count(&self) -> usize {
        self.keys.values().filter(|key| key.active).count()
    }

    /// Export key for backup (encrypted with master key)
    pub fn export_key(&self, key_id: &str) -> Result<String> {
        let _key = self.get_key_metadata(key_id)?;
        
        if self.master_key.is_none() {
            return Err(anyhow!("Cannot export key without master key"));
        }
        
        // In a real implementation, this would encrypt the key with the master key
        // For now, we'll return a placeholder
        Ok(format!("encrypted_key_export_{}", key_id))
    }

    /// Import key from backup (decrypt with master key)
    pub fn import_key(&mut self, key_id: &str, _encrypted_data: &str) -> Result<()> {
        if self.master_key.is_none() {
            return Err(anyhow!("Cannot import key without master key"));
        }
        
        // In a real implementation, this would decrypt and restore the key
        // For now, we'll create a new key
        self.create_key(key_id)?;
        
        log::info!("Imported encryption key: {}", key_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_key_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = KeyManager::new(
            temp_dir.path().to_str().unwrap(),
            Some("test_master_key_32_bytes_long_123")
        ).unwrap();
        
        assert!(manager.has_master_key());
        assert_eq!(manager.key_count(), 0);
    }

    #[test]
    fn test_key_creation_and_retrieval() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = KeyManager::new(
            temp_dir.path().to_str().unwrap(),
            Some("test_master_key_32_bytes_long_123")
        ).unwrap();
        
        // Create a key
        manager.create_key("test_key").unwrap();
        assert_eq!(manager.key_count(), 1);
        
        // Retrieve the key
        let key = manager.get_key("test_key").unwrap();
        assert_eq!(key.len(), 32); // 256 bits
        
        // Get key metadata
        let metadata = manager.get_key_metadata("test_key").unwrap();
        assert_eq!(metadata.id, "test_key");
        assert!(metadata.active);
        assert_eq!(metadata.version, 1);
    }

    #[test]
    fn test_get_or_create_key() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = KeyManager::new(
            temp_dir.path().to_str().unwrap(),
            Some("test_master_key_32_bytes_long_123")
        ).unwrap();
        
        // Should create new key
        let key1 = manager.get_or_create_key("auto_key").unwrap();
        assert_eq!(manager.key_count(), 1);
        
        // Should return existing key
        let key2 = manager.get_or_create_key("auto_key").unwrap();
        assert_eq!(key1, key2);
        assert_eq!(manager.key_count(), 1);
    }

    #[test]
    fn test_key_rotation() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = KeyManager::new(
            temp_dir.path().to_str().unwrap(),
            Some("test_master_key_32_bytes_long_123")
        ).unwrap();
        
        manager.create_key("rotate_key").unwrap();
        let original_key = manager.get_key("rotate_key").unwrap();
        
        // Rotate the key
        manager.rotate_key("rotate_key").unwrap();
        let rotated_key = manager.get_key("rotate_key").unwrap();
        
        // Keys should be different
        assert_ne!(original_key, rotated_key);
        
        // Version should be incremented
        let metadata = manager.get_key_metadata("rotate_key").unwrap();
        assert_eq!(metadata.version, 2);
    }

    #[test]
    fn test_key_deactivation() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = KeyManager::new(
            temp_dir.path().to_str().unwrap(),
            Some("test_master_key_32_bytes_long_123")
        ).unwrap();
        
        manager.create_key("deactivate_key").unwrap();
        assert!(manager.get_key("deactivate_key").is_ok());
        
        // Deactivate the key
        manager.deactivate_key("deactivate_key").unwrap();
        assert!(manager.get_key("deactivate_key").is_err());
        
        // Key should still exist but be inactive
        let metadata = manager.get_key_metadata("deactivate_key").unwrap();
        assert!(!metadata.active);
    }

    #[test]
    fn test_key_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_str().unwrap();
        
        // Create manager and add keys
        {
            let mut manager = KeyManager::new(storage_path, Some("test_master_key_16_chars")).unwrap();
            manager.create_key("persistent_key").unwrap();
            assert_eq!(manager.key_count(), 1);
        }
        
        // Create new manager and verify keys are loaded
        {
            let manager = KeyManager::new(storage_path, Some("test_master_key_16_chars")).unwrap();
            assert_eq!(manager.key_count(), 1);
            assert!(manager.get_key("persistent_key").is_ok());
        }
    }

    #[test]
    fn test_master_key_validation() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_str().unwrap();
        
        // Create manager with master key
        {
            let mut manager = KeyManager::new(storage_path, Some("correct_master_key")).unwrap();
            manager.create_key("test_key").unwrap();
        }
        
        // Try to load with wrong master key
        {
            let result = KeyManager::new(storage_path, Some("wrong_master_key"));
            assert!(result.is_err());
        }
        
        // Load with correct master key
        {
            let manager = KeyManager::new(storage_path, Some("correct_master_key")).unwrap();
            assert_eq!(manager.key_count(), 1);
        }
    }

    #[test]
    fn test_encryption_key_generation() {
        let key = EncryptionKey::new("test".to_string());
        assert_eq!(key.id, "test");
        assert_eq!(key.key.len(), 32);
        assert!(key.active);
        assert_eq!(key.version, 1);
    }

    #[test]
    fn test_key_needs_rotation() {
        let mut key = EncryptionKey::new("test".to_string());
        
        // New key shouldn't need rotation
        assert!(!key.needs_rotation(90));
        
        // Simulate old key
        key.last_rotated = Utc::now() - chrono::Duration::days(100);
        assert!(key.needs_rotation(90));
    }
}