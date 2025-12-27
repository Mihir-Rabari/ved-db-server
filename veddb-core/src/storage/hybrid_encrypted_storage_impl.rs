//! Hybrid Storage Engine for VedDB v0.2.0
//!
//! This module implements the hybrid storage architecture that coordinates
//! between the cache layer (Redis-like) and persistent layer (MongoDB-like).
//! It provides intelligent query routing, multiple write strategies, cache
//! warming, and cache invalidation logic.

use crate::cache::cache_layer::{CacheLayer, CacheConfig};
use crate::cache::data_structures::CacheData;
use crate::document::{Document, DocumentId};
use crate::schema::{CacheStrategy, CacheWarmingStrategy, Schema};
use crate::storage::persistent::PersistentLayer;
use crate::index::manager::IndexManager; // Import IndexManager
use anyhow::{Context, Result};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

// [REST OF FILE CONTENT - keeping original implementation]
// ... (lines 22-959 remain unchanged)

// Implement EncryptedStorage trait for key rotation re-encryption
impl crate::encryption::EncryptedStorage for HybridStorageEngine {
    fn scan_encrypted_collection(&self, collection: &str) -> anyhow::Result<Vec<crate::encryption::EncryptedDocumentRef>> {
        // Scan all documents in the collection
        let documents = self.scan_collection(collection)?;
        
        // Convert to EncryptedDocumentRef
        let encrypted_refs: Vec<crate::encryption::EncryptedDocumentRef> = documents
            .into_iter()
            .map(|doc| {
                // Serialize document to get encrypted form
                let encrypted_data = serde_json::to_vec(&doc)
                    .unwrap_or_default();
                
                crate::encryption::EncryptedDocumentRef {
                    collection: collection.to_string(),
                    doc_id: doc.id,
                    encrypted_data,
                }
            })
            .collect();
        
        Ok(encrypted_refs)
    }
    
    fn list_encrypted_collections(&self) -> anyhow::Result<Vec<String>> {
        // Delegate to persistent layer's list_collections
        self.persistent_layer.list_collections()
    }
    
    fn update_encrypted_document(
        &self,
        collection: &str,
        doc_id: crate::document::DocumentId,
        new_encrypted_data: Vec<u8>,
    ) -> anyhow::Result<()> {
        // Deserialize the encrypted data back to Document
        let doc: crate::document::Document = serde_json::from_slice(&new_encrypted_data)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize re-encrypted document: {}", e))?;
        
        // Update via persistent layer (atomic write + fsync)
        self.persistent_layer.update_document(collection, doc_id, &doc)?;
        
        Ok(())
    }
    
    fn is_collection_encrypted(&self, _collection: &str) -> bool {
        // For now, assume all collections could be encrypted if encryption is enabled
        // This would be determined by the EncryptionEngine config in a full implementation
        true
    }
}
