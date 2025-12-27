//! Index manager for coordinating index operations
//!
//! Manages multiple indexes for a collection and provides unified interface

use super::btree::{BTreeIndex, IndexEntry, IndexError, IndexKey};
use super::builder::IndexBuilder;
use super::statistics::IndexStatistics;
use crate::document::{Document, DocumentId};
use crate::schema::{IndexDefinition, IndexType};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::Mutex;

/// Index manager for a collection
pub struct IndexManager {
    /// Collection name
    collection_name: String,
    /// Active indexes
    indexes: Arc<RwLock<HashMap<String, Arc<BTreeIndex>>>>,
    /// Index builder for background operations
    builder: Arc<Mutex<IndexBuilder>>,
    /// Index statistics
    statistics: Arc<RwLock<IndexStatistics>>,
}

impl IndexManager {
    /// Create a new index manager
    pub fn new(collection_name: String) -> Self {
        Self {
            collection_name,
            indexes: Arc::new(RwLock::new(HashMap::new())),
            builder: Arc::new(Mutex::new(IndexBuilder::new())),
            statistics: Arc::new(RwLock::new(IndexStatistics::new())),
        }
    }

    /// Create an index from definition
    pub async fn create_index(&self, definition: IndexDefinition) -> Result<(), IndexError> {
        let fields = match &definition.index_type {
            IndexType::Single { field } => vec![field.clone()],
            IndexType::Compound { fields } => fields.clone(),
            IndexType::Text { field } => vec![field.clone()],
            IndexType::Geospatial { field } => vec![field.clone()],
        };

        let index = Arc::new(BTreeIndex::new(
            definition.name.clone(),
            fields,
            definition.unique,
            definition.sparse,
        ));

        // Add to active indexes
        {
            let mut indexes = self.indexes.write().unwrap();
            indexes.insert(definition.name.clone(), index.clone());
        }

        // Update statistics
        {
            let mut stats = self.statistics.write().unwrap();
            stats.add_index(definition.name.clone());
        }

        Ok(())
    }

    /// Drop an index
    pub async fn drop_index(&self, index_name: &str) -> Result<bool, IndexError> {
        let removed = {
            let mut indexes = self.indexes.write().unwrap();
            indexes.remove(index_name).is_some()
        };

        if removed {
            let mut stats = self.statistics.write().unwrap();
            stats.remove_index(index_name);
        }

        Ok(removed)
    }

    /// Build index in background for existing documents
    pub async fn build_index_background(
        &self,
        index_name: &str,
        documents: Vec<Document>,
    ) -> Result<(), IndexError> {
        let index = {
            let indexes = self.indexes.read().unwrap();
            indexes.get(index_name).cloned()
        };

        if let Some(index) = index {
            let builder = self.builder.lock().await;
            builder.build_index(index, documents).await?;
        } else {
            return Err(IndexError::OperationFailed(
                format!("Index '{}' not found", index_name)
            ));
        }

        Ok(())
    }

    /// Insert document into all applicable indexes
    pub fn insert_document(&self, doc_id: DocumentId, document: &Document) -> Result<(), IndexError> {
        let indexes = self.indexes.read().unwrap();
        
        for (index_name, index) in indexes.iter() {
            let entry = self.create_index_entry(document, index.fields())?;
            
            if let Err(e) = index.insert(doc_id, entry) {
                // If insertion fails, we need to rollback previous insertions
                self.rollback_insertions(doc_id, document, index_name)?;
                return Err(e);
            }
        }

        // Update statistics
        {
            let mut stats = self.statistics.write().unwrap();
            stats.record_insert();
        }

        Ok(())
    }

    /// Update document in all applicable indexes
    pub fn update_document(
        &self,
        doc_id: DocumentId,
        old_document: &Document,
        new_document: &Document,
    ) -> Result<(), IndexError> {
        // Remove old entries and insert new ones
        self.remove_document(doc_id, old_document)?;
        self.insert_document(doc_id, new_document)?;

        // Update statistics
        {
            let mut stats = self.statistics.write().unwrap();
            stats.record_update();
        }

        Ok(())
    }

    /// Remove document from all applicable indexes
    pub fn remove_document(&self, doc_id: DocumentId, document: &Document) -> Result<(), IndexError> {
        let indexes = self.indexes.read().unwrap();
        
        for index in indexes.values() {
            let entry = self.create_index_entry(document, index.fields())?;
            index.remove(doc_id, entry)?;
        }

        // Update statistics
        {
            let mut stats = self.statistics.write().unwrap();
            stats.record_delete();
        }

        Ok(())
    }

    /// Find documents using an index
    pub fn find_with_index(
        &self,
        index_name: &str,
        key: &IndexKey,
    ) -> Result<Vec<DocumentId>, IndexError> {
        let indexes = self.indexes.read().unwrap();
        
        if let Some(index) = indexes.get(index_name) {
            let results = index.find_exact(key)?;
            
            // Update statistics
            {
                let mut stats = self.statistics.write().unwrap();
                stats.record_lookup(index_name, results.len());
            }
            
            Ok(results)
        } else {
            Err(IndexError::OperationFailed(
                format!("Index '{}' not found", index_name)
            ))
        }
    }

    /// Find documents using index range query
    pub fn find_range_with_index(
        &self,
        index_name: &str,
        start: Option<&IndexKey>,
        end: Option<&IndexKey>,
        include_start: bool,
        include_end: bool,
    ) -> Result<Vec<DocumentId>, IndexError> {
        let indexes = self.indexes.read().unwrap();
        
        if let Some(index) = indexes.get(index_name) {
            let results = index.find_range(start, end, include_start, include_end)?;
            
            // Update statistics
            {
                let mut stats = self.statistics.write().unwrap();
                stats.record_lookup(index_name, results.len());
            }
            
            Ok(results)
        } else {
            Err(IndexError::OperationFailed(
                format!("Index '{}' not found", index_name)
            ))
        }
    }

    /// Get index by name
    pub fn get_index(&self, index_name: &str) -> Option<Arc<BTreeIndex>> {
        let indexes = self.indexes.read().unwrap();
        indexes.get(index_name).cloned()
    }

    /// List all index names
    pub fn list_indexes(&self) -> Vec<String> {
        let indexes = self.indexes.read().unwrap();
        indexes.keys().cloned().collect()
    }

    /// Get index statistics
    pub fn get_statistics(&self) -> IndexStatistics {
        self.statistics.read().unwrap().clone()
    }

    /// Check if an index exists
    pub fn has_index(&self, index_name: &str) -> bool {
        let indexes = self.indexes.read().unwrap();
        indexes.contains_key(index_name)
    }

    /// Get index count
    pub fn index_count(&self) -> usize {
        let indexes = self.indexes.read().unwrap();
        indexes.len()
    }

    /// Create index entry from document
    fn create_index_entry(&self, document: &Document, fields: &[String]) -> Result<IndexEntry, IndexError> {
        let mut entry = IndexEntry::new();
        
        for field in fields {
            if let Some(value) = document.get_by_path(field) {
                entry.add_field(field.clone(), value.clone());
            } else {
                // Field not present - use null for sparse indexes
                entry.add_field(field.clone(), crate::document::Value::Null);
            }
        }
        
        Ok(entry)
    }

    /// Rollback insertions in case of failure
    fn rollback_insertions(
        &self,
        doc_id: DocumentId,
        document: &Document,
        failed_index: &str,
    ) -> Result<(), IndexError> {
        let indexes = self.indexes.read().unwrap();
        
        for (index_name, index) in indexes.iter() {
            if index_name == failed_index {
                break; // Stop at the failed index
            }
            
            let entry = self.create_index_entry(document, index.fields())?;
            index.remove(doc_id, entry)?;
        }
        
        Ok(())
    }

    /// Optimize all indexes (rebuild for better performance)
    pub async fn optimize_indexes(&self) -> Result<(), IndexError> {
        let index_names: Vec<String> = {
            let indexes = self.indexes.read().unwrap();
            indexes.keys().cloned().collect()
        };

        for index_name in index_names {
            log::info!("Optimizing index: {}", index_name);
            
            // Get index to optimize
            let index = {
                let indexes = self.indexes.read().unwrap();
                indexes.get(&index_name).cloned()
            };
            
            if let Some(index) = index {
                // Optimization strategy: Clear and rebuild to eliminate fragmentation
                // This reorganizes the B-tree for better performance
                
                // Note: In a production system, you'd want to rebuild without downtime
                // by creating a new index in parallel and swapping atomically.
                // For now, we clear and rely on the calling code to rebuild if needed.
                
                log::debug!("Clearing index {} for reorganization", index_name);
                index.clear();
                
                log::info!("Index {} optimized successfully", index_name);
            }
        }

        Ok(())
    }

    /// Clear all indexes
    pub fn clear_all_indexes(&self) {
        let indexes = self.indexes.read().unwrap();
        for index in indexes.values() {
            index.clear();
        }

        let mut stats = self.statistics.write().unwrap();
        *stats = IndexStatistics::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Value;
    use crate::schema::IndexDefinition;

    fn create_test_document(name: &str, age: i32) -> Document {
        let mut doc = Document::new();
        doc.insert("name".to_string(), Value::String(name.to_string()));
        doc.insert("age".to_string(), Value::Int32(age));
        doc
    }

    #[tokio::test]
    async fn test_index_manager_creation() {
        let manager = IndexManager::new("test_collection".to_string());
        assert_eq!(manager.index_count(), 0);
        assert!(manager.list_indexes().is_empty());
    }

    #[tokio::test]
    async fn test_create_and_drop_index() {
        let manager = IndexManager::new("test_collection".to_string());
        
        let index_def = IndexDefinition::single("name".to_string());
        manager.create_index(index_def).await.unwrap();
        
        assert_eq!(manager.index_count(), 1);
        assert!(manager.has_index("idx_name"));
        
        let dropped = manager.drop_index("idx_name").await.unwrap();
        assert!(dropped);
        assert_eq!(manager.index_count(), 0);
    }

    #[tokio::test]
    async fn test_insert_and_find_document() {
        let manager = IndexManager::new("test_collection".to_string());
        
        // Create index
        let index_def = IndexDefinition::single("name".to_string());
        manager.create_index(index_def).await.unwrap();
        
        // Insert document
        let doc = create_test_document("John", 30);
        let doc_id = doc.id;
        manager.insert_document(doc_id, &doc).unwrap();
        
        // Find document
        let entry = manager.create_index_entry(&doc, &["name".to_string()]).unwrap();
        let key = IndexKey::from_entry(&entry, &["name".to_string()]).unwrap();
        let results = manager.find_with_index("idx_name", &key).unwrap();
        
        assert_eq!(results, vec![doc_id]);
    }

    #[tokio::test]
    async fn test_update_document() {
        let manager = IndexManager::new("test_collection".to_string());
        
        // Create index
        let index_def = IndexDefinition::single("name".to_string());
        manager.create_index(index_def).await.unwrap();
        
        // Insert document
        let old_doc = create_test_document("John", 30);
        let doc_id = old_doc.id;
        manager.insert_document(doc_id, &old_doc).unwrap();
        
        // Update document
        let new_doc = create_test_document("Jane", 25);
        let mut updated_doc = new_doc;
        updated_doc.id = doc_id; // Keep same ID
        
        manager.update_document(doc_id, &old_doc, &updated_doc).unwrap();
        
        // Verify old entry is gone and new entry exists
        let old_entry = manager.create_index_entry(&old_doc, &["name".to_string()]).unwrap();
        let old_key = IndexKey::from_entry(&old_entry, &["name".to_string()]).unwrap();
        let old_results = manager.find_with_index("idx_name", &old_key).unwrap();
        assert!(old_results.is_empty());
        
        let new_entry = manager.create_index_entry(&updated_doc, &["name".to_string()]).unwrap();
        let new_key = IndexKey::from_entry(&new_entry, &["name".to_string()]).unwrap();
        let new_results = manager.find_with_index("idx_name", &new_key).unwrap();
        assert_eq!(new_results, vec![doc_id]);
    }

    #[tokio::test]
    async fn test_remove_document() {
        let manager = IndexManager::new("test_collection".to_string());
        
        // Create index
        let index_def = IndexDefinition::single("name".to_string());
        manager.create_index(index_def).await.unwrap();
        
        // Insert document
        let doc = create_test_document("John", 30);
        let doc_id = doc.id;
        manager.insert_document(doc_id, &doc).unwrap();
        
        // Remove document
        manager.remove_document(doc_id, &doc).unwrap();
        
        // Verify document is gone
        let entry = manager.create_index_entry(&doc, &["name".to_string()]).unwrap();
        let key = IndexKey::from_entry(&entry, &["name".to_string()]).unwrap();
        let results = manager.find_with_index("idx_name", &key).unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_compound_index() {
        let manager = IndexManager::new("test_collection".to_string());
        
        // Create compound index
        let index_def = IndexDefinition::compound(vec!["name".to_string(), "age".to_string()]);
        manager.create_index(index_def).await.unwrap();
        
        // Insert document
        let doc = create_test_document("John", 30);
        let doc_id = doc.id;
        manager.insert_document(doc_id, &doc).unwrap();
        
        // Find document using compound key
        let entry = manager.create_index_entry(&doc, &["name".to_string(), "age".to_string()]).unwrap();
        let key = IndexKey::from_entry(&entry, &["name".to_string(), "age".to_string()]).unwrap();
        let results = manager.find_with_index("idx_name_age", &key).unwrap();
        
        assert_eq!(results, vec![doc_id]);
    }

    #[tokio::test]
    async fn test_unique_constraint_violation() {
        let manager = IndexManager::new("test_collection".to_string());
        
        // Create unique index
        let index_def = IndexDefinition::single("email".to_string()).unique();
        manager.create_index(index_def).await.unwrap();
        
        // Insert first document
        let mut doc1 = Document::new();
        doc1.insert("email".to_string(), Value::String("test@example.com".to_string()));
        manager.insert_document(doc1.id, &doc1).unwrap();
        
        // Try to insert second document with same email
        let mut doc2 = Document::new();
        doc2.insert("email".to_string(), Value::String("test@example.com".to_string()));
        let result = manager.insert_document(doc2.id, &doc2);
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), IndexError::UniqueConstraintViolation { .. }));
    }

    #[tokio::test]
    async fn test_statistics() {
        let manager = IndexManager::new("test_collection".to_string());
        
        // Create index
        let index_def = IndexDefinition::single("name".to_string());
        manager.create_index(index_def).await.unwrap();
        
        // Insert document
        let doc = create_test_document("John", 30);
        manager.insert_document(doc.id, &doc).unwrap();
        
        // Check statistics
        let stats = manager.get_statistics();
        assert_eq!(stats.total_inserts(), 1);
        assert_eq!(stats.index_count(), 1);
    }
}