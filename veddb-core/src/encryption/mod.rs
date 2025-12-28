//! Encryption engine for VedDB
//!
//! This module provides:
//! - Data-at-rest encryption using AES-256-GCM
//! - Key management with per-collection keys
//! - Document and WAL entry encryption/decryption
//! - Key rotation support

use anyhow::anyhow;

pub mod key_manager;
pub mod document_encryption;
pub mod key_rotation;
pub mod rotation_state;
pub mod rotation_state_store;
pub mod tls;
pub mod encrypted_storage;

pub use key_manager::*;
pub use document_encryption::*;
pub use key_rotation::*;
pub use rotation_state::*;
pub use rotation_state_store::*;
pub use tls::*;
pub use encrypted_storage::*;

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
    key_rotation_scheduler: Option<KeyRotationScheduler>,
    tls_config: Option<TlsConfig>,
    tls_acceptor: Option<TlsAcceptor>,
}

impl EncryptionEngine {
    /// Create a new encryption engine
    pub fn new(config: EncryptionConfig, storage_path: &str) -> Result<Self> {
        let key_manager = KeyManager::new(storage_path, config.master_key.as_deref())?;
        let document_encryption = DocumentEncryption::new();
        
        let engine = Self {
        config,
        key_manager,
        document_encryption,
        key_rotation_scheduler: None,
        tls_config: None,
        tls_acceptor: None,
    };
    
    // CRITICAL: Enforce rotation state before allowing server startup
    // This prevents starting with incomplete/failed rotation state
    let encryption_path = std::path::Path::new(storage_path);
    enforce_rotation_state_on_startup(encryption_path)?;
    
    Ok(engine)
}

    /// Enable automatic key rotation
    pub fn enable_key_rotation(&mut self, rotation_config: KeyRotationConfig) -> Result<()> {
        // Use encryption directory for rotation state storage
        // CRITICAL: This must be bound to encryption metadata, not generic storage
        let encryption_path = std::path::PathBuf::from("./encryption");
        let scheduler = KeyRotationScheduler::new(rotation_config, encryption_path);
        self.key_rotation_scheduler = Some(scheduler);
        log::info!(" Key rotation scheduler: ENABLED (full re-encryption active)");
        Ok(())
    }

    /// Disable automatic key rotation
    pub fn disable_key_rotation(&mut self) {
        self.key_rotation_scheduler = None;
        log::info!("Disabled automatic key rotation");
    }

    /// Start key rotation scheduler (if enabled)
    /// Note: This method    
    /// Check if key rotation scheduler is enabled
    pub fn is_rotation_enabled(&self) -> bool {
        self.key_rotation_scheduler.is_some()
    }
    
    /// Manually rotate a specific key with full re-encryption
    /// 
    /// If scheduler is enabled, this performs FULL re-encryption of all documents.
    /// If scheduler is disabled, this performs key generation only (NO re-encryption).
    pub async fn rotate_key(&mut self, key_id: &str, storage: &dyn crate::encryption::EncryptedStorage) -> Result<()> {
        // SAFETY ASSERTION: Verify scheduler state matches expectation
        debug_assert!(
            self.key_rotation_scheduler.is_some(),
            "MISCONFIGURATION: Key rotation called without scheduler. \
             This will only generate new keys without re-encrypting data."
        );
        
        if let Some(mut scheduler) = self.key_rotation_scheduler.take() {
            // FULL ROTATION: Use scheduler for complete re-encryption
            log::info!("🔄 Key rotation started for '{}': scheduler-driven re-encryption ENABLED", key_id);
            let result = scheduler.rotate_key(self, storage, key_id).await;
            self.key_rotation_scheduler = Some(scheduler);
            result?;
            log::info!("✅ Key rotation completed for '{}': all documents re-encrypted", key_id);
        } else {
            // FALLBACK: Key generation only (NO re-encryption)
            log::warn!(
                "⚠️  Key rotation for '{}' performed WITHOUT scheduler: \
                 only generating new key, NO document re-encryption. \
                 This is a partial rotation.", 
                key_id
            );
            self.key_manager.rotate_key(key_id)?;
            log::info!("Manually rotated key: {} (metadata only)", key_id);
        }
        
        Ok(())
    }
    
    /// Perform scheduled key rotation check
    pub async fn check_and_rotate_keys(
        &mut self,
        storage: &dyn crate::encryption::EncryptedStorage,
    ) -> Result<()> {
        if let Some(scheduler) = self.key_rotation_scheduler.take() {
            let mut temp_scheduler = scheduler;
            temp_scheduler.check_and_rotate_keys(self, storage).await?;
            self.key_rotation_scheduler = Some(temp_scheduler);
        }
        Ok(())
    }

    /// Get key rotation status
    pub fn get_rotation_status(&self, key_id: &str) -> Option<&KeyRotationStatus> {
        self.key_rotation_scheduler
            .as_ref()
            .and_then(|scheduler| scheduler.get_rotation_status(key_id))
    }

    /// Get rotation statistics
    pub fn get_rotation_statistics(&self) -> Option<RotationStatistics> {
        self.key_rotation_scheduler
            .as_ref()
            .map(|scheduler| scheduler.get_rotation_statistics())
    }

    /// Configure TLS settings
    pub fn configure_tls(&mut self, tls_config: TlsConfig) -> Result<()> {
        if tls_config.enabled {
            let acceptor = TlsAcceptor::new(&tls_config)?;
            self.tls_acceptor = Some(acceptor);
            log::info!("TLS 1.3 configured and enabled");
        } else {
            self.tls_acceptor = None;
            log::info!("TLS disabled");
        }
        
        self.tls_config = Some(tls_config);
        Ok(())
    }

    /// Check if TLS is enabled
    pub fn is_tls_enabled(&self) -> bool {
        self.tls_config
            .as_ref()
            .map(|config| config.enabled)
            .unwrap_or(false)
    }

    /// Get TLS acceptor for server connections
    pub fn tls_acceptor(&self) -> Option<&TlsAcceptor> {
        self.tls_acceptor.as_ref()
    }

    /// Create TLS connector for client connections
    pub fn create_tls_connector(&self) -> Result<TlsConnector> {
        let tls_config = self.tls_config
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("TLS not configured"))?;
        
        TlsConnector::new(tls_config)
    }

    /// Create TLS connector with client certificate
    pub fn create_tls_connector_with_cert(&self, cert_file: &str, key_file: &str) -> Result<TlsConnector> {
        let tls_config = self.tls_config
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("TLS not configured"))?;
        
        TlsConnector::new_with_client_cert(tls_config, cert_file, key_file)
    }

    /// Get TLS configuration
    pub fn tls_config(&self) -> Option<&TlsConfig> {
        self.tls_config.as_ref()
    }

    /// Validate TLS certificate and key files
    pub fn validate_tls_certificates(&self) -> Result<()> {
        let tls_config = self.tls_config
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("TLS not configured"))?;

        if !tls_config.enabled {
            return Ok(());
        }

        let cert_file = tls_config.cert_file
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Certificate file not specified"))?;
        let key_file = tls_config.key_file
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Private key file not specified"))?;

        TlsCertificateGenerator::validate_cert_and_key(cert_file, key_file)?;
        log::info!("TLS certificates validated successfully");
        Ok(())
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
/// Enforce rotation state on server startup
/// 
/// CRITICAL: Server MUST NOT start with incomplete or failed rotation state.
/// This prevents undefined cryptographic state.
/// 
/// Rules:
/// - Idle/Completed: OK (normal startup)
/// - ReEncrypting: FATAL ERROR (incomplete rotation)
/// - Failed: FATAL ERROR (manual intervention required)
pub fn enforce_rotation_state_on_startup(
    encryption_path: &std::path::Path
) -> Result<()> {
    let state = load_rotation_state(encryption_path)?;
    
    match state {
        KeyRotationState::Idle => {
            log::info!("Rotation state: Idle (no rotation in progress)");
            Ok(())
        }
        KeyRotationState::Completed { key_id, completed_at, documents_processed } => {
            log::info!(
                "Rotation state: Completed (key '{}' rotated at {}, {} documents processed)",
                key_id, completed_at, documents_processed
            );
            Ok(())
        }
        KeyRotationState::ReEncrypting { key_id, processed, total, .. } => {
            Err(anyhow!(
                "FATAL: Incomplete key rotation detected for key '{}'.\n\
                 Progress: {}/{} documents re-encrypted.\n\
                 Server CANNOT start with undefined cryptographic state.\n\n\
                 Recovery options:\n\
                 1. Automatic resume: The scheduler will auto-resume on next clean start\n\
                 2. Manual intervention: Check logs and verify data integrity\n\n\
                 DO NOT proceed until rotation is resolved.",
                key_id, processed, total
            ))
        }
        KeyRotationState::Failed { key_id, reason, failed_at } => {
            Err(anyhow!(
                "FATAL: Previous key rotation FAILED for key '{}'.\n\
                 Failure time: {}\n\
                 Reason: {}\n\n\
                 MANUAL INTERVENTION REQUIRED.\n\
                 Server CANNOT start until this is resolved.\n\n\
                 Recovery steps:\n\
                 1. Review logs for rotation failure details\n\
                 2. Verify data integrity\n\
                 3. Contact database administrator\n\
                 4. Clear rotation state only after manual verification",
                key_id, failed_at, reason
            ))
        }
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

    #[tokio::test]
    async fn test_key_rotation_integration() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        
        let mut engine = EncryptionEngine::new(config, temp_dir.path().to_str().unwrap()).unwrap();
        
        // Enable key rotation
        let rotation_config = KeyRotationConfig {
            enabled: true,
            rotation_interval_days: 1,
            max_old_keys: 3,
            reencryption_batch_size: 100,
            reencryption_delay_ms: 10,
        };
        
        engine.enable_key_rotation(rotation_config).unwrap();
        assert!(engine.has_key_rotation_enabled());
        
        // Create a key
        engine.key_manager_mut().create_key("test_rotation_key").unwrap();
        let original_key = engine.key_manager().get_key("test_rotation_key").unwrap();
        
        // Rotate the key
        engine.rotate_key("test_rotation_key").await.unwrap();
        
        // Verify key was rotated
        let new_key = engine.key_manager().get_key("test_rotation_key").unwrap();
        assert_ne!(original_key, new_key);
    }

    #[test]
    fn test_key_rotation_enable_disable() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        
        let mut engine = EncryptionEngine::new(config, temp_dir.path().to_str().unwrap()).unwrap();
        
        // Initially no rotation scheduler
        assert!(engine.get_rotation_statistics().is_none());
        
        // Enable key rotation
        let rotation_config = KeyRotationConfig::default();
        engine.enable_key_rotation(rotation_config).unwrap();
        
        // Should now have rotation scheduler
        assert!(engine.get_rotation_statistics().is_some());
        
        // Disable key rotation
        engine.disable_key_rotation();
        
        // Should no longer have rotation scheduler
        assert!(engine.get_rotation_statistics().is_none());
    }

    #[test]
    fn test_tls_configuration() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        
        let mut engine = EncryptionEngine::new(config, temp_dir.path().to_str().unwrap()).unwrap();
        
        // Initially TLS not configured
        assert!(!engine.is_tls_enabled());
        assert!(engine.tls_acceptor().is_none());
        
        // Configure TLS (will fail due to missing cert files, but config should be set)
        let tls_config = TlsConfig {
            enabled: false, // Disabled to avoid file errors
            ..Default::default()
        };
        
        engine.configure_tls(tls_config).unwrap();
        assert!(engine.tls_config().is_some());
        assert!(!engine.is_tls_enabled()); // Still disabled
    }

    #[test]
    fn test_tls_connector_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        
        let mut engine = EncryptionEngine::new(config, temp_dir.path().to_str().unwrap()).unwrap();
        
        // Configure TLS for client-only (no server cert needed for connector)
        let tls_config = TlsConfig {
            enabled: false, // Don't enable server-side TLS
            ca_file: None, // Use system roots
            ..Default::default()
        };
        
        engine.configure_tls(tls_config).unwrap();
        
        // Should be able to create connector even without server TLS enabled
        let connector = engine.create_tls_connector();
        assert!(connector.is_ok());
    }

    #[test]
    fn test_tls_validation_without_config() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        
        let engine = EncryptionEngine::new(config, temp_dir.path().to_str().unwrap()).unwrap();
        
        // Should fail without TLS config
        let result = engine.validate_tls_certificates();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not configured"));
    }
}
