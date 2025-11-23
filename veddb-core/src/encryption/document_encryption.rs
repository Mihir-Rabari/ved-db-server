//! Document encryption using AES-256-GCM

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// Encrypted data container with nonce and ciphertext
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    /// Nonce used for encryption (12 bytes for GCM)
    pub nonce: Vec<u8>,
    
    /// Encrypted data with authentication tag
    pub ciphertext: Vec<u8>,
    
    /// Algorithm used for encryption
    pub algorithm: String,
    
    /// Version for future compatibility
    pub version: u8,
}

impl EncryptedData {
    /// Create new encrypted data container
    pub fn new(nonce: Vec<u8>, ciphertext: Vec<u8>) -> Self {
        Self {
            nonce,
            ciphertext,
            algorithm: "AES-256-GCM".to_string(),
            version: 1,
        }
    }

    /// Serialize to bytes for storage
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| anyhow!("Failed to serialize encrypted data: {}", e))
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data).map_err(|e| anyhow!("Failed to deserialize encrypted data: {}", e))
    }
}

/// Document encryption service using AES-256-GCM
pub struct DocumentEncryption {
    // No state needed for stateless encryption
}

impl DocumentEncryption {
    /// Create a new document encryption service
    pub fn new() -> Self {
        Self {}
    }

    /// Encrypt data using AES-256-GCM
    pub fn encrypt(&self, plaintext: &[u8], key: &[u8]) -> Result<Vec<u8>> {
        if key.len() != 32 {
            return Err(anyhow!("Key must be 32 bytes for AES-256"));
        }

        // Create cipher
        let key = Key::<Aes256Gcm>::from_slice(key);
        let cipher = Aes256Gcm::new(key);

        // Generate random nonce
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

        // Encrypt the data
        let ciphertext = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;

        // Create encrypted data container
        let encrypted_data = EncryptedData::new(nonce.to_vec(), ciphertext);
        encrypted_data.to_bytes()
    }

    /// Decrypt data using AES-256-GCM
    pub fn decrypt(&self, encrypted_bytes: &[u8], key: &[u8]) -> Result<Vec<u8>> {
        if key.len() != 32 {
            return Err(anyhow!("Key must be 32 bytes for AES-256"));
        }

        // Deserialize encrypted data
        let encrypted_data = EncryptedData::from_bytes(encrypted_bytes)?;

        // Validate algorithm
        if encrypted_data.algorithm != "AES-256-GCM" {
            return Err(anyhow!("Unsupported encryption algorithm: {}", encrypted_data.algorithm));
        }

        // Validate nonce length
        if encrypted_data.nonce.len() != 12 {
            return Err(anyhow!("Invalid nonce length: expected 12, got {}", encrypted_data.nonce.len()));
        }

        // Create cipher
        let key = Key::<Aes256Gcm>::from_slice(key);
        let cipher = Aes256Gcm::new(key);

        // Create nonce
        let nonce = Nonce::from_slice(&encrypted_data.nonce);

        // Decrypt the data
        cipher
            .decrypt(nonce, encrypted_data.ciphertext.as_ref())
            .map_err(|e| anyhow!("Decryption failed: {}", e))
    }
}
impl Default for DocumentEncryption {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_test_key() -> Vec<u8> {
        vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
            0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
            0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
            0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
        ]
    }

    #[test]
    fn test_document_encryption_creation() {
        let encryption = DocumentEncryption::new();
        // Just verify it can be created
        assert_eq!(std::mem::size_of_val(&encryption), 0);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let encryption = DocumentEncryption::new();
        let key = generate_test_key();
        let plaintext = b"Hello, World! This is a test document for encryption.";

        // Encrypt
        let encrypted = encryption.encrypt(plaintext, &key).unwrap();
        assert_ne!(encrypted, plaintext);

        // Decrypt
        let decrypted = encryption.decrypt(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_different_data_produces_different_ciphertext() {
        let encryption = DocumentEncryption::new();
        let key = generate_test_key();
        let plaintext1 = b"First document";
        let plaintext2 = b"Second document";

        let encrypted1 = encryption.encrypt(plaintext1, &key).unwrap();
        let encrypted2 = encryption.encrypt(plaintext2, &key).unwrap();

        assert_ne!(encrypted1, encrypted2);
    }

    #[test]
    fn test_encrypt_same_data_produces_different_ciphertext() {
        let encryption = DocumentEncryption::new();
        let key = generate_test_key();
        let plaintext = b"Same document content";

        let encrypted1 = encryption.encrypt(plaintext, &key).unwrap();
        let encrypted2 = encryption.encrypt(plaintext, &key).unwrap();

        // Should be different due to random nonce
        assert_ne!(encrypted1, encrypted2);

        // But both should decrypt to same plaintext
        let decrypted1 = encryption.decrypt(&encrypted1, &key).unwrap();
        let decrypted2 = encryption.decrypt(&encrypted2, &key).unwrap();
        assert_eq!(decrypted1, plaintext);
        assert_eq!(decrypted2, plaintext);
    }

    #[test]
    fn test_decrypt_with_wrong_key_fails() {
        let encryption = DocumentEncryption::new();
        let key1 = generate_test_key();
        let mut key2 = generate_test_key();
        key2[0] = 0xFF; // Make it different
        
        let plaintext = b"Secret document";
        let encrypted = encryption.encrypt(plaintext, &key1).unwrap();

        // Should fail with wrong key
        assert!(encryption.decrypt(&encrypted, &key2).is_err());
    }

    #[test]
    fn test_invalid_key_length() {
        let encryption = DocumentEncryption::new();
        let short_key = vec![0u8; 16]; // Too short
        let long_key = vec![0u8; 64];  // Too long
        let plaintext = b"Test data";

        assert!(encryption.encrypt(plaintext, &short_key).is_err());
        assert!(encryption.encrypt(plaintext, &long_key).is_err());
    }

    #[test]
    fn test_encrypted_data_serialization() {
        let nonce = vec![0u8; 12];
        let ciphertext = vec![1, 2, 3, 4, 5];
        let encrypted_data = EncryptedData::new(nonce.clone(), ciphertext.clone());

        // Serialize
        let bytes = encrypted_data.to_bytes().unwrap();
        assert!(!bytes.is_empty());

        // Deserialize
        let deserialized = EncryptedData::from_bytes(&bytes).unwrap();
        assert_eq!(deserialized.nonce, nonce);
        assert_eq!(deserialized.ciphertext, ciphertext);
        assert_eq!(deserialized.algorithm, "AES-256-GCM");
        assert_eq!(deserialized.version, 1);
    }

    #[test]
    fn test_corrupted_encrypted_data() {
        let encryption = DocumentEncryption::new();
        let key = generate_test_key();
        let plaintext = b"Test document";

        let mut encrypted = encryption.encrypt(plaintext, &key).unwrap();
        
        // Corrupt the ciphertext part (not the metadata)
        // We need to corrupt the actual encrypted data, not the serialized metadata
        if encrypted.len() > 50 {
            // Corrupt somewhere in the middle of the encrypted data
            let corrupt_pos = encrypted.len() / 2;
            encrypted[corrupt_pos] = encrypted[corrupt_pos].wrapping_add(1);
        }

        // Should fail to decrypt
        assert!(encryption.decrypt(&encrypted, &key).is_err());
    }

    #[test]
    fn test_empty_data_encryption() {
        let encryption = DocumentEncryption::new();
        let key = generate_test_key();
        let plaintext = b"";

        let encrypted = encryption.encrypt(plaintext, &key).unwrap();
        let decrypted = encryption.decrypt(&encrypted, &key).unwrap();
        
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_large_data_encryption() {
        let encryption = DocumentEncryption::new();
        let key = generate_test_key();
        let plaintext = vec![42u8; 1024 * 1024]; // 1MB of data

        let encrypted = encryption.encrypt(&plaintext, &key).unwrap();
        let decrypted = encryption.decrypt(&encrypted, &key).unwrap();
        
        assert_eq!(decrypted, plaintext);
    }
}