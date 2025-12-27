//! Persistent storage layer using RocksDB
//!
//! Note: RocksDB requires LLVM/Clang to be installed on Windows.
//! Install from https://releases.llvm.org/ and set LIBCLANG_PATH environment variable.
//! Enable with the "rocksdb-storage" feature flag.

use crate::document::{Document, DocumentId};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(feature = "rocksdb-storage")]
use rocksdb::{ColumnFamilyDescriptor, Options, DB};

#[cfg(not(feature = "rocksdb-storage"))]
use std::collections::HashMap;
#[cfg(not(feature = "rocksdb-storage"))]
use parking_lot::RwLock;

/// Persistent storage layer backed by RocksDB (or in-memory fallback)
pub struct PersistentLayer {
    #[cfg(feature = "rocksdb-storage")]
    /// RocksDB database instance
    db: Arc<DB>,
    
    #[cfg(not(feature = "rocksdb-storage"))]
    /// In-memory storage (fallback when RocksDB is not available)
    documents: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
    
    #[cfg(not(feature = "rocksdb-storage"))]
    /// In-memory metadata storage
    metadata: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    
    /// Data directory path
    data_dir: PathBuf,
}

impl PersistentLayer {
    /// Create a new persistent layer
    pub fn new<P: AsRef<Path>>(data_dir: P) -> Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        
        // Create data directory if it doesn't exist
        std::fs::create_dir_all(&data_dir)
            .context("Failed to create data directory")?;

        #[cfg(feature = "rocksdb-storage")]
        {
            // Configure RocksDB options
            let mut opts = Options::default();
            opts.create_if_missing(true);
            opts.create_missing_column_families(true);
            opts.set_max_open_files(1000);
            opts.set_keep_log_file_num(10);
            opts.set_max_background_jobs(4);
            opts.set_bytes_per_sync(1048576); // 1MB

            // Define column families
            let cf_descriptors = vec![
                ColumnFamilyDescriptor::new("default", Options::default()),
                ColumnFamilyDescriptor::new("documents", Options::default()),
                ColumnFamilyDescriptor::new("metadata", Options::default()),
                ColumnFamilyDescriptor::new("indexes", Options::default()),
            ];

            // Open database
            let db = DB::open_cf_descriptors(&opts, &data_dir, cf_descriptors)
                .context("Failed to open RocksDB")?;

            Ok(Self {
                db: Arc::new(db),
                data_dir,
            })
        }

        #[cfg(not(feature = "rocksdb-storage"))]
        {
            // Use in-memory storage as fallback
            Ok(Self {
                documents: Arc::new(RwLock::new(HashMap::new())),
                metadata: Arc::new(RwLock::new(HashMap::new())),
                data_dir,
            })
        }
    }

    /// Get the data directory path
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Insert a document
    pub fn insert_document(
        &self,
        collection: &str,
        doc_id: DocumentId,
        doc: &Document,
    ) -> Result<()> {
        let key = Self::make_document_key(collection, doc_id);
        let value = serde_json::to_vec(doc)
            .context("Failed to serialize document")?;

        #[cfg(feature = "rocksdb-storage")]
        {
            let cf = self.db.cf_handle("documents")
                .context("Documents column family not found")?;
            self.db.put_cf(cf, key, value)
                .context("Failed to insert document")?;
        }

        #[cfg(not(feature = "rocksdb-storage"))]
        {
            self.documents.write().insert(key, value);
        }

        Ok(())
    }

    /// Get a document by ID
    pub fn get_document(
        &self,
        collection: &str,
        doc_id: DocumentId,
    ) -> Result<Option<Document>> {
        let key = Self::make_document_key(collection, doc_id);
        
        #[cfg(feature = "rocksdb-storage")]
        {
            let cf = self.db.cf_handle("documents")
                .context("Documents column family not found")?;
            
            match self.db.get_cf(cf, key)? {
                Some(value) => {
                    let doc = serde_json::from_slice(&value)
                        .context("Failed to deserialize document")?;
                    Ok(Some(doc))
                }
                None => Ok(None),
            }
        }

        #[cfg(not(feature = "rocksdb-storage"))]
        {
            match self.documents.read().get(&key) {
                Some(value) => {
                    let doc = serde_json::from_slice(value)
                        .context("Failed to deserialize document")?;
                    Ok(Some(doc))
                }
                None => Ok(None),
            }
        }
    }

    /// Update a document
    pub fn update_document(
        &self,
        collection: &str,
        doc_id: DocumentId,
        doc: &Document,
    ) -> Result<()> {
        // For now, update is the same as insert (overwrite)
        self.insert_document(collection, doc_id, doc)
    }

    /// Delete a document
    pub fn delete_document(
        &self,
        collection: &str,
        doc_id: DocumentId,
    ) -> Result<bool> {
        let key = Self::make_document_key(collection, doc_id);
        
        #[cfg(feature = "rocksdb-storage")]
        {
            let cf = self.db.cf_handle("documents")
                .context("Documents column family not found")?;
            
            // Check if document exists
            let exists = self.db.get_cf(cf, &key)?.is_some();
            
            if exists {
                self.db.delete_cf(cf, key)
                    .context("Failed to delete document")?;
            }

            Ok(exists)
        }

        #[cfg(not(feature = "rocksdb-storage"))]
        {
            Ok(self.documents.write().remove(&key).is_some())
        }
    }

    /// Check if a document exists
    pub fn exists(&self, collection: &str, doc_id: DocumentId) -> Result<bool> {
        let key = Self::make_document_key(collection, doc_id);
        
        #[cfg(feature = "rocksdb-storage")]
        {
            let cf = self.db.cf_handle("documents")
                .context("Documents column family not found")?;
            Ok(self.db.get_cf(cf, key)?.is_some())
        }

        #[cfg(not(feature = "rocksdb-storage"))]
        {
            Ok(self.documents.read().contains_key(&key))
        }
    }

    /// Get all documents in a collection (for iteration)
    pub fn scan_collection(&self, collection: &str) -> Result<Vec<Document>> {
        let prefix = format!("{}:", collection);
        let mut documents = Vec::new();

        #[cfg(feature = "rocksdb-storage")]
        {
            let cf = self.db.cf_handle("documents")
                .context("Documents column family not found")?;

            let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
            for item in iter {
                let (key, value) = item?;
                
                // Check if key still matches our collection prefix
                if !key.starts_with(prefix.as_bytes()) {
                    break;
                }

                let doc: Document = serde_json::from_slice(&value)
                    .context("Failed to deserialize document")?;
                documents.push(doc);
            }
        }

        #[cfg(not(feature = "rocksdb-storage"))]
        {
            let docs = self.documents.read();
            for (key, value) in docs.iter() {
                if key.starts_with(prefix.as_bytes()) {
                    let doc: Document = serde_json::from_slice(value)
                        .context("Failed to deserialize document")?;
                    documents.push(doc);
                }
            }
        }

        Ok(documents)
    }

    /// Store collection metadata
    pub fn store_metadata(&self, key: &str, value: &[u8]) -> Result<()> {
        #[cfg(feature = "rocksdb-storage")]
        {
            let cf = self.db.cf_handle("metadata")
                .context("Metadata column family not found")?;

            self.db.put_cf(cf, key.as_bytes(), value)
                .context("Failed to store metadata")?;
        }

        #[cfg(not(feature = "rocksdb-storage"))]
        {
            self.metadata.write().insert(key.to_string(), value.to_vec());
        }

        Ok(())
    }

    /// Get collection metadata
    pub fn get_metadata(&self, key: &str) -> Result<Option<Vec<u8>>> {
        #[cfg(feature = "rocksdb-storage")]
        {
            let cf = self.db.cf_handle("metadata")
                .context("Metadata column family not found")?;

            Ok(self.db.get_cf(cf, key.as_bytes())?)
        }

        #[cfg(not(feature = "rocksdb-storage"))]
        {
            Ok(self.metadata.read().get(key).cloned())
        }
    }

    /// Delete collection metadata
    pub fn delete_metadata(&self, key: &str) -> Result<()> {
        #[cfg(feature = "rocksdb-storage")]
        {
            let cf = self.db.cf_handle("metadata")
                .context("Metadata column family not found")?;

            self.db.delete_cf(cf, key.as_bytes())
                .context("Failed to delete metadata")?;
        }

        #[cfg(not(feature = "rocksdb-storage"))]
        {
            self.metadata.write().remove(key);
        }

        Ok(())
    }

    /// Get database statistics
    pub fn get_stats(&self) -> Result<StorageStats> {
        #[cfg(feature = "rocksdb-storage")]
        {
            let property = self.db.property_value("rocksdb.estimate-num-keys")?;
            let num_keys = property
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);

            let property = self.db.property_value("rocksdb.total-sst-files-size")?;
            let total_size = property
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);

            Ok(StorageStats {
                num_keys,
                total_size_bytes: total_size,
            })
        }

        #[cfg(not(feature = "rocksdb-storage"))]
        {
            let num_keys = self.documents.read().len() as u64;
            let total_size = self.documents.read()
                .values()
                .map(|v| v.len() as u64)
                .sum();

            Ok(StorageStats {
                num_keys,
                total_size_bytes: total_size,
            })
        }
    }

    /// Flush all data to disk
    pub fn flush(&self) -> Result<()> {
        #[cfg(feature = "rocksdb-storage")]
        {
            self.db.flush()
                .context("Failed to flush database")?;
        }

        #[cfg(not(feature = "rocksdb-storage"))]
        {
            // No-op for in-memory storage
        }

        Ok(())
    }

    /// Compact the database
    pub fn compact(&self) -> Result<()> {
        #[cfg(feature = "rocksdb-storage")]
        {
            self.db.compact_range::<&[u8], &[u8]>(None, None);
        }

        #[cfg(not(feature = "rocksdb-storage"))]
        {
            // No-op for in-memory storage
        }

        Ok(())
    }

    /// Create a collection
    pub fn create_collection(&self, name: &str) -> Result<()> {
        // Add to collections list in metadata
        let mut collections = self.get_collections_list()?;
        if !collections.contains(&name.to_string()) {
            collections.push(name.to_string());
            self.save_collections_list(&collections)?;
        }
        Ok(())
    }

    /// List all collections
    pub fn list_collections(&self) -> Result<Vec<String>> {
        self.get_collections_list()
    }

    /// Drop a collection (delete all documents in the collection)
    pub fn drop_collection(&self, collection: &str) -> Result<()> {
        let prefix = format!("{}:", collection);

        #[cfg(feature = "rocksdb-storage")]
        {
            let cf = self.db.cf_handle("documents")
                .context("Documents column family not found")?;

            // Collect all keys to delete
            let mut keys_to_delete = Vec::new();
            let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
            for item in iter {
                let (key, _) = item?;
                if !key.starts_with(prefix.as_bytes()) {
                    break;
                }
                keys_to_delete.push(key.to_vec());
            }

            // Delete all keys
            for key in keys_to_delete {
                self.db.delete_cf(cf, key)
                    .context("Failed to delete document")?;
            }
        }

        #[cfg(not(feature = "rocksdb-storage"))]
        {
            let mut docs = self.documents.write();
            let keys_to_remove: Vec<_> = docs.keys()
                .filter(|k| k.starts_with(prefix.as_bytes()))
                .cloned()
                .collect();
            
            for key in keys_to_remove {
                docs.remove(&key);
            }
        }

        // Also delete collection metadata
        self.delete_metadata(&format!("collection:{}", collection))?;
        
        // Remove from collections list
        let mut collections = self.get_collections_list()?;
        if let Some(pos) = collections.iter().position(|x| x == collection) {
            collections.remove(pos);
            self.save_collections_list(&collections)?;
        }
        
        // Remove indexes
        self.delete_metadata(&format!("indexes:{}", collection))?;

        Ok(())
    }

    /// Create an index
    pub fn create_index(&self, collection: &str, name: &str, fields: Vec<crate::protocol::IndexField>, unique: bool) -> Result<()> {
        let mut indexes = self.get_indexes_list(collection)?;
        
        // Check if index exists
        if indexes.iter().any(|idx| idx.get("name").and_then(|v| v.as_str()) == Some(name)) {
            return Ok(()); // Already exists
        }
        
        let mut index_def = serde_json::Map::new();
        index_def.insert("name".to_string(), serde_json::Value::String(name.to_string()));
        index_def.insert("unique".to_string(), serde_json::Value::Bool(unique));
        
        let fields_val: Vec<serde_json::Value> = fields.into_iter().map(|f| {
            let mut map = serde_json::Map::new();
            map.insert("field".to_string(), serde_json::Value::String(f.field));
            map.insert("direction".to_string(), serde_json::Value::Number(serde_json::Number::from(f.direction)));
            serde_json::Value::Object(map)
        }).collect();
        
        index_def.insert("fields".to_string(), serde_json::Value::Array(fields_val));
        
        indexes.push(serde_json::Value::Object(index_def));
        self.save_indexes_list(collection, &indexes)?;
        
        Ok(())
    }

    /// List indexes
    pub fn list_indexes(&self, collection: &str) -> Result<Vec<serde_json::Value>> {
        self.get_indexes_list(collection)
    }

    /// Drop an index
    pub fn drop_index(&self, collection: &str, name: &str) -> Result<()> {
        let mut indexes = self.get_indexes_list(collection)?;
        
        if let Some(pos) = indexes.iter().position(|idx| idx.get("name").and_then(|v| v.as_str()) == Some(name)) {
            indexes.remove(pos);
            self.save_indexes_list(collection, &indexes)?;
        }
        
        Ok(())
    }

    // Helper to get collections list by scanning actual keys
    fn get_collections_list(&self) -> Result<Vec<String>> {
        use std::collections::HashSet;
        
        let mut collections = HashSet::new();
        
        #[cfg(feature = "rocksdb-storage")]
        {
            // Scan all document keys to find collections
            let cf = self.db.cf_handle("documents")
                .context("Documents column family not found")?;
            
            let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            for item in iter {
                let (key, _) = item?;
                
                // Parse collection name from key format "collection:docid"
                if let Ok(key_str) = std::str::from_utf8(&key) {
                    if let Some(colon_pos) = key_str.find(':') {
                        let collection_name = &key_str[..colon_pos];
                        collections.insert(collection_name.to_string());
                    }
                }
            }
        }
        
        #[cfg(not(feature = "rocksdb-storage"))]
        {
            // Scan in-memory keys to find collections
            let docs = self.documents.read();
            for key in docs.keys() {
                if let Ok(key_str) = std::str::from_utf8(key) {
                    if let Some(colon_pos) = key_str.find(':') {
                        let collection_name = &key_str[..colon_pos];
                        collections.insert(collection_name.to_string());
                    }
                }
            }
        }
        
        // Convert HashSet to sorted Vec for consistent ordering
        let mut result: Vec<String> = collections.into_iter().collect();
        result.sort();
        Ok(result)
    }

    // Helper to save collections list
    fn save_collections_list(&self, collections: &[String]) -> Result<()> {
        let data = serde_json::to_vec(collections).context("Failed to serialize collections list")?;
        self.store_metadata("collections", &data)
    }

    // Helper to get indexes list
    fn get_indexes_list(&self, collection: &str) -> Result<Vec<serde_json::Value>> {
        match self.get_metadata(&format!("indexes:{}", collection))? {
            Some(data) => serde_json::from_slice(&data).context("Failed to parse indexes list"),
            None => Ok(Vec::new()),
        }
    }

    // Helper to save indexes list
    fn save_indexes_list(&self, collection: &str, indexes: &[serde_json::Value]) -> Result<()> {
        let data = serde_json::to_vec(indexes).context("Failed to serialize indexes list")?;
        self.store_metadata(&format!("indexes:{}", collection), &data)
    }

    /// Make a document key for RocksDB
    fn make_document_key(collection: &str, doc_id: DocumentId) -> Vec<u8> {
        format!("{}:{}", collection, doc_id).into_bytes()
    }
}

/// Storage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    /// Number of keys in the database
    pub num_keys: u64,
    /// Total size in bytes
    pub total_size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Value;
    use tempfile::TempDir;

    fn create_test_storage() -> (PersistentLayer, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = PersistentLayer::new(temp_dir.path()).unwrap();
        (storage, temp_dir)
    }

    #[test]
    fn test_persistent_layer_creation() {
        let (storage, _temp_dir) = create_test_storage();
        assert!(storage.data_dir().exists());
    }

    #[test]
    fn test_insert_and_get_document() {
        let (storage, _temp_dir) = create_test_storage();
        
        let mut doc = Document::new();
        let doc_id = doc.id;
        doc.insert("name".to_string(), Value::String("John".to_string()));
        doc.insert("age".to_string(), Value::Int32(30));

        storage.insert_document("users", doc_id, &doc).unwrap();

        let retrieved = storage.get_document("users", doc_id).unwrap();
        assert!(retrieved.is_some());
        
        let retrieved_doc = retrieved.unwrap();
        assert_eq!(retrieved_doc.get("name").unwrap().as_str(), Some("John"));
        assert_eq!(retrieved_doc.get("age").unwrap().as_i64(), Some(30));
    }

    #[test]
    fn test_update_document() {
        let (storage, _temp_dir) = create_test_storage();
        
        let mut doc = Document::new();
        let doc_id = doc.id;
        doc.insert("name".to_string(), Value::String("John".to_string()));

        storage.insert_document("users", doc_id, &doc).unwrap();

        // Update
        doc.insert("name".to_string(), Value::String("Jane".to_string()));
        storage.update_document("users", doc_id, &doc).unwrap();

        let retrieved = storage.get_document("users", doc_id).unwrap().unwrap();
        assert_eq!(retrieved.get("name").unwrap().as_str(), Some("Jane"));
    }

    #[test]
    fn test_delete_document() {
        let (storage, _temp_dir) = create_test_storage();
        
        let mut doc = Document::new();
        let doc_id = doc.id;
        doc.insert("name".to_string(), Value::String("John".to_string()));

        storage.insert_document("users", doc_id, &doc).unwrap();
        assert!(storage.exists("users", doc_id).unwrap());

        let deleted = storage.delete_document("users", doc_id).unwrap();
        assert!(deleted);
        assert!(!storage.exists("users", doc_id).unwrap());

        // Try to delete again
        let deleted_again = storage.delete_document("users", doc_id).unwrap();
        assert!(!deleted_again);
    }

    #[test]
    fn test_scan_collection() {
        let (storage, _temp_dir) = create_test_storage();
        
        // Insert multiple documents
        for i in 0..5 {
            let mut doc = Document::new();
            doc.insert("index".to_string(), Value::Int32(i));
            storage.insert_document("test", doc.id, &doc).unwrap();
        }

        let documents = storage.scan_collection("test").unwrap();
        assert_eq!(documents.len(), 5);
    }

    #[test]
    fn test_metadata_operations() {
        let (storage, _temp_dir) = create_test_storage();
        
        let key = "test_key";
        let value = b"test_value";

        storage.store_metadata(key, value).unwrap();
        
        let retrieved = storage.get_metadata(key).unwrap();
        assert_eq!(retrieved, Some(value.to_vec()));

        storage.delete_metadata(key).unwrap();
        
        let deleted = storage.get_metadata(key).unwrap();
        assert_eq!(deleted, None);
    }

    #[test]
    fn test_storage_stats() {
        let (storage, _temp_dir) = create_test_storage();
        
        let stats = storage.get_stats().unwrap();
        assert!(stats.num_keys >= 0);
        assert!(stats.total_size_bytes >= 0);
    }

    #[test]
    fn test_flush_and_compact() {
        let (storage, _temp_dir) = create_test_storage();
        
        // Insert some data
        let mut doc = Document::new();
        doc.insert("test".to_string(), Value::String("data".to_string()));
        storage.insert_document("test", doc.id, &doc).unwrap();

        // Flush and compact should not error
        storage.flush().unwrap();
        storage.compact().unwrap();
    }
}
