//! Collection management for VedDB

use crate::document::{Document, DocumentId};
use crate::schema::{IndexDefinition, Schema};
use crate::storage::persistent::PersistentLayer;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Collection managing documents and indexes
pub struct Collection {
    /// Collection name
    name: String,
    /// Schema definition
    schema: Schema,
    /// Document count
    document_count: AtomicU64,
    /// Total size in bytes
    total_size_bytes: AtomicU64,
    /// Index definitions
    indexes: Vec<IndexDefinition>,
    /// Creation timestamp
    created_at: DateTime<Utc>,
    /// Last update timestamp
    updated_at: DateTime<Utc>,
    /// Persistent storage layer
    persistent_layer: Arc<PersistentLayer>,
}

impl Collection {
    /// Create a new collection
    pub fn new(
        name: String,
        schema: Schema,
        persistent_layer: Arc<PersistentLayer>,
    ) -> Self {
        let now = Utc::now();
        Self {
            name,
            schema,
            document_count: AtomicU64::new(0),
            total_size_bytes: AtomicU64::new(0),
            indexes: Vec::new(),
            created_at: now,
            updated_at: now,
            persistent_layer,
        }
    }

    /// Get collection name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get schema
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// Get document count
    pub fn document_count(&self) -> u64 {
        self.document_count.load(Ordering::Relaxed)
    }

    /// Get total size in bytes
    pub fn total_size_bytes(&self) -> u64 {
        self.total_size_bytes.load(Ordering::Relaxed)
    }

    /// Insert a document
    pub fn insert(&self, mut doc: Document) -> Result<DocumentId> {
        // Apply defaults
        self.schema.apply_defaults(&mut doc)
            .context("Failed to apply defaults")?;

        // Validate against schema
        self.schema.validate(&doc)
            .context("Schema validation failed")?;

        // Validate document constraints
        doc.validate()
            .context("Document validation failed")?;

        let doc_id = doc.id;

        // Store in persistent layer
        self.persistent_layer.insert_document(&self.name, doc_id, &doc)
            .context("Failed to insert document")?;

        // Update statistics
        self.document_count.fetch_add(1, Ordering::Relaxed);
        self.total_size_bytes.fetch_add(doc.size_bytes() as u64, Ordering::Relaxed);

        Ok(doc_id)
    }

    /// Get a document by ID
    pub fn get(&self, doc_id: DocumentId) -> Result<Option<Document>> {
        self.persistent_layer.get_document(&self.name, doc_id)
    }

    /// Update a document
    pub fn update(&self, doc_id: DocumentId, doc: Document) -> Result<()> {
        // Get old document to calculate size difference
        let old_doc = self.get(doc_id)?;
        let old_size = old_doc.as_ref().map(|d| d.size_bytes()).unwrap_or(0);

        // Validate against schema
        self.schema.validate(&doc)
            .context("Schema validation failed")?;

        // Validate document constraints
        doc.validate()
            .context("Document validation failed")?;

        // Update in persistent layer
        self.persistent_layer.update_document(&self.name, doc_id, &doc)
            .context("Failed to update document")?;

        // Update size statistics
        let new_size = doc.size_bytes();
        if new_size > old_size {
            self.total_size_bytes.fetch_add((new_size - old_size) as u64, Ordering::Relaxed);
        } else {
            self.total_size_bytes.fetch_sub((old_size - new_size) as u64, Ordering::Relaxed);
        }

        Ok(())
    }

    /// Delete a document
    pub fn delete(&self, doc_id: DocumentId) -> Result<bool> {
        // Get document to update size statistics
        if let Some(doc) = self.get(doc_id)? {
            let deleted = self.persistent_layer.delete_document(&self.name, doc_id)?;
            
            if deleted {
                self.document_count.fetch_sub(1, Ordering::Relaxed);
                self.total_size_bytes.fetch_sub(doc.size_bytes() as u64, Ordering::Relaxed);
            }

            Ok(deleted)
        } else {
            Ok(false)
        }
    }

    /// Check if a document exists
    pub fn exists(&self, doc_id: DocumentId) -> Result<bool> {
        self.persistent_layer.exists(&self.name, doc_id)
    }

    /// Get all documents (for scanning)
    pub fn scan(&self) -> Result<Vec<Document>> {
        self.persistent_layer.scan_collection(&self.name)
    }

    /// Add an index
    pub fn add_index(&mut self, index: IndexDefinition) {
        self.indexes.push(index);
    }

    /// Get indexes
    pub fn indexes(&self) -> &[IndexDefinition] {
        &self.indexes
    }

    /// Get collection metadata
    pub fn metadata(&self) -> CollectionMetadata {
        CollectionMetadata {
            name: self.name.clone(),
            document_count: self.document_count(),
            total_size_bytes: self.total_size_bytes(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            index_count: self.indexes.len(),
        }
    }

    /// Save collection metadata to persistent storage
    pub fn save_metadata(&self) -> Result<()> {
        let metadata = self.metadata();
        let key = format!("collection:{}", self.name);
        let value = bincode::serialize(&metadata)
            .context("Failed to serialize metadata")?;
        
        self.persistent_layer.store_metadata(&key, &value)
    }

    /// Load collection metadata from persistent storage
    pub fn load_metadata(
        name: &str,
        persistent_layer: &PersistentLayer,
    ) -> Result<Option<CollectionMetadata>> {
        let key = format!("collection:{}", name);
        
        if let Some(value) = persistent_layer.get_metadata(&key)? {
            let metadata = bincode::deserialize(&value)
                .context("Failed to deserialize metadata")?;
            Ok(Some(metadata))
        } else {
            Ok(None)
        }
    }
}

/// Collection metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionMetadata {
    /// Collection name
    pub name: String,
    /// Number of documents
    pub document_count: u64,
    /// Total size in bytes
    pub total_size_bytes: u64,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Number of indexes
    pub index_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Value;
    use crate::schema::{FieldDefinition, FieldType};
    use tempfile::TempDir;

    fn create_test_collection() -> (Collection, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let persistent = Arc::new(PersistentLayer::new(temp_dir.path()).unwrap());
        
        let mut schema = Schema::new();
        schema.add_field(
            "name".to_string(),
            FieldDefinition::new(FieldType::String { max_length: None }).required(),
        );
        schema.add_field(
            "age".to_string(),
            FieldDefinition::new(FieldType::Int32),
        );

        let collection = Collection::new("users".to_string(), schema, persistent);
        (collection, temp_dir)
    }

    #[test]
    fn test_collection_creation() {
        let (collection, _temp_dir) = create_test_collection();
        assert_eq!(collection.name(), "users");
        assert_eq!(collection.document_count(), 0);
    }

    #[test]
    fn test_insert_document() {
        let (collection, _temp_dir) = create_test_collection();
        
        let mut doc = Document::new();
        doc.insert("name".to_string(), Value::String("John".to_string()));
        doc.insert("age".to_string(), Value::Int32(30));

        let doc_id = collection.insert(doc).unwrap();
        assert_eq!(collection.document_count(), 1);

        let retrieved = collection.get(doc_id).unwrap();
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_insert_validation_failure() {
        let (collection, _temp_dir) = create_test_collection();
        
        // Missing required field "name"
        let mut doc = Document::new();
        doc.insert("age".to_string(), Value::Int32(30));

        let result = collection.insert(doc);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_document() {
        let (collection, _temp_dir) = create_test_collection();
        
        let mut doc = Document::new();
        let doc_id = doc.id;
        doc.insert("name".to_string(), Value::String("John".to_string()));
        doc.insert("age".to_string(), Value::Int32(30));

        collection.insert(doc.clone()).unwrap();

        // Update
        doc.insert("age".to_string(), Value::Int32(31));
        collection.update(doc_id, doc).unwrap();

        let retrieved = collection.get(doc_id).unwrap().unwrap();
        assert_eq!(retrieved.get("age").unwrap().as_i64(), Some(31));
    }

    #[test]
    fn test_delete_document() {
        let (collection, _temp_dir) = create_test_collection();
        
        let mut doc = Document::new();
        let doc_id = doc.id;
        doc.insert("name".to_string(), Value::String("John".to_string()));

        collection.insert(doc).unwrap();
        assert_eq!(collection.document_count(), 1);

        let deleted = collection.delete(doc_id).unwrap();
        assert!(deleted);
        assert_eq!(collection.document_count(), 0);
    }

    #[test]
    fn test_scan_collection() {
        let (collection, _temp_dir) = create_test_collection();
        
        // Insert multiple documents
        for i in 0..5 {
            let mut doc = Document::new();
            doc.insert("name".to_string(), Value::String(format!("User{}", i)));
            doc.insert("age".to_string(), Value::Int32(20 + i));
            collection.insert(doc).unwrap();
        }

        let documents = collection.scan().unwrap();
        assert_eq!(documents.len(), 5);
    }

    #[test]
    fn test_collection_metadata() {
        let (collection, _temp_dir) = create_test_collection();
        
        let metadata = collection.metadata();
        assert_eq!(metadata.name, "users");
        assert_eq!(metadata.document_count, 0);
    }

    #[test]
    fn test_save_and_load_metadata() {
        let (collection, _temp_dir) = create_test_collection();
        
        // Insert a document to change metadata
        let mut doc = Document::new();
        doc.insert("name".to_string(), Value::String("John".to_string()));
        collection.insert(doc).unwrap();

        // Save metadata
        collection.save_metadata().unwrap();

        // Load metadata
        let loaded = Collection::load_metadata(
            "users",
            &collection.persistent_layer,
        ).unwrap();

        assert!(loaded.is_some());
        let metadata = loaded.unwrap();
        assert_eq!(metadata.name, "users");
        assert_eq!(metadata.document_count, 1);
    }
}
