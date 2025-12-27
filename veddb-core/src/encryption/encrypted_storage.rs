//! Encrypted storage trait for key rotation re-encryption
//!
//! This module provides a minimal, focused interface for re-encrypting
//! documents during key rotation without exposing full storage internals.

use crate::document::{Document, DocumentId};
use anyhow::Result;

/// Reference to an encrypted document in storage
#[derive(Debug, Clone)]
pub struct EncryptedDocumentRef {
    /// Collection name
    pub collection: String,
    
    /// Document ID
    pub doc_id: DocumentId,
    
    /// Encrypted document data (serialized + encrypted)
    pub encrypted_data: Vec<u8>,
}

/// Minimal storage interface for re-encryption operations
/// 
/// This trait provides ONLY the operations needed for key rotation:
/// - Scan encrypted documents
/// - Update encrypted document ciphertext
/// 
/// This keeps coupling minimal and blast radius small.
pub trait EncryptedStorage: Send + Sync {
    /// Scan all encrypted documents in a collection
    /// 
    /// Returns an iterator over document references with their encrypted data.
    /// The iterator is lazy and processes documents on-demand to avoid
    /// loading everything into memory.
    fn scan_encrypted_collection(&self, collection: &str) -> Result<Vec<EncryptedDocumentRef>>;
    
    /// Get list of all collections that have encrypted documents
    fn list_encrypted_collections(&self) -> Result<Vec<String>>;
    
    /// Update an encrypted document's ciphertext atomically
    /// 
    /// This operation is atomic at the document level:
    /// - Write new ciphertext
    /// - Fsync/commit
    /// - Return success
    /// 
    /// If this fails mid-operation, the old ciphertext remains intact.
    fn update_encrypted_document(
        &self,
        collection: &str,
        doc_id: DocumentId,
        new_encrypted_data: Vec<u8>,
    ) -> Result<()>;
    
    /// Check if a collection uses encryption
    fn is_collection_encrypted(&self, collection: &str) -> bool;
}

/// Context for re-encryption operations during key rotation
#[derive(Debug, Clone)]
pub struct ReEncryptionContext {
    /// Old encryption key (for decryption)
    pub old_key: Vec<u8>,
    
    /// New encryption key (for encryption)
    pub new_key: Vec<u8>,
    
    /// Old key version (for tracking)
    pub old_version: u32,
    
    /// New key version (for tracking)
    pub new_version: u32,
    
    /// Key ID being rotated
    pub key_id: String,
}

impl ReEncryptionContext {
    /// Create a new re-encryption context
    pub fn new(
        key_id: String,
        old_key: Vec<u8>,
        old_version: u32,
        new_key: Vec<u8>,
        new_version: u32,
    ) -> Self {
        Self {
            old_key,
            new_key,
            old_version,
            new_version,
            key_id,
        }
    }
}
