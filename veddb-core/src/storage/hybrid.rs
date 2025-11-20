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
use anyhow::{Context, Result};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Hybrid Storage Engine coordinating cache and persistent layers
pub struct HybridStorageEngine {
    /// Cache layer for in-memory storage
    cache_layer: Arc<CacheLayer>,
    /// Persistent layer for durable storage
    persistent_layer: Arc<PersistentLayer>,
    /// Collection schemas
    schemas: Arc<RwLock<HashMap<String, Schema>>>,
    /// Write-behind queue
    write_behind_queue: Arc<RwLock<Vec<WriteBehindEntry>>>,
    /// Statistics
    stats: Arc<HybridStorageStats>,
}

impl HybridStorageEngine {
    /// Create a new hybrid storage engine
    pub fn new(
        cache_config: CacheConfig,
        persistent_layer: Arc<PersistentLayer>,
    ) -> Self {
        Self {
            cache_layer: Arc::new(CacheLayer::new(cache_config)),
            persistent_layer,
            schemas: Arc::new(RwLock::new(HashMap::new())),
            write_behind_queue: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(HybridStorageStats::default()),
        }
    }

    /// Register a collection schema
    pub fn register_schema(&self, collection: String, schema: Schema) {
        self.schemas.write().insert(collection, schema);
    }

    /// Get a collection schema
    pub fn get_schema(&self, collection: &str) -> Option<Schema> {
        self.schemas.read().get(collection).cloned()
    }

    /// Start background tasks (write-behind processor, cache warming)
    pub async fn start_background_tasks(self: Arc<Self>) {
        // Start write-behind processor
        let engine = self.clone();
        tokio::spawn(async move {
            engine.process_write_behind_queue().await;
        });

        // Start cache warming for collections with warming strategies
        let engine = self.clone();
        tokio::spawn(async move {
            engine.warm_caches().await;
        });
    }

    /// Insert a document
    pub async fn insert_document(
        &self,
        collection: &str,
        doc: Document,
    ) -> Result<DocumentId> {
        let doc_id = doc.id;
        let schema = self.get_schema(collection);

        if let Some(schema) = schema {
            let strategy = &schema.cache_config.strategy;
            
            match strategy {
                CacheStrategy::None => {
                    // Only persistent storage
                    self.persistent_layer.insert_document(collection, doc_id, &doc)?;
                    self.stats.record_persistent_write();
                }
                CacheStrategy::WriteThrough => {
                    // Write to both cache and persistent storage atomically
                    self.write_through(collection, doc_id, &doc, &schema).await?;
                }
                CacheStrategy::WriteBehind { delay_ms } => {
                    // Write to cache immediately, queue persistent write
                    self.write_behind(collection, doc_id, &doc, &schema, *delay_ms).await?;
                }
                CacheStrategy::ReadThrough => {
                    // Write to persistent, invalidate cache
                    self.persistent_layer.insert_document(collection, doc_id, &doc)?;
                    self.invalidate_cache_entry(collection, doc_id);
                    self.stats.record_persistent_write();
                }
            }
        } else {
            // No schema, default to persistent only
            self.persistent_layer.insert_document(collection, doc_id, &doc)?;
            self.stats.record_persistent_write();
        }

        Ok(doc_id)
    }

    /// Get a document by ID
    pub async fn get_document(
        &self,
        collection: &str,
        doc_id: DocumentId,
    ) -> Result<Option<Document>> {
        let schema = self.get_schema(collection);

        if let Some(schema) = schema {
            let strategy = &schema.cache_config.strategy;
            
            match strategy {
                CacheStrategy::None => {
                    // Only persistent storage
                    let doc = self.persistent_layer.get_document(collection, doc_id)?;
                    self.stats.record_persistent_read();
                    Ok(doc)
                }
                CacheStrategy::WriteThrough | CacheStrategy::WriteBehind { .. } => {
                    // Try cache first
                    let cache_key = self.make_cache_key(collection, doc_id);
                    
                    if let Some(cached_data) = self.cache_layer.get(&cache_key) {
                        // Cache hit
                        self.stats.record_cache_hit();
                        Ok(Some(self.cache_data_to_document(cached_data, doc_id)?))
                    } else {
                        // Cache miss - read from persistent and populate cache
                        self.stats.record_cache_miss();
                        let doc = self.persistent_layer.get_document(collection, doc_id)?;
                        
                        if let Some(ref doc) = doc {
                            self.populate_cache(collection, doc_id, doc, &schema).await?;
                        }
                        
                        self.stats.record_persistent_read();
                        Ok(doc)
                    }
                }
                CacheStrategy::ReadThrough => {
                    // Always try cache first, populate on miss
                    let cache_key = self.make_cache_key(collection, doc_id);
                    
                    if let Some(cached_data) = self.cache_layer.get(&cache_key) {
                        self.stats.record_cache_hit();
                        Ok(Some(self.cache_data_to_document(cached_data, doc_id)?))
                    } else {
                        self.stats.record_cache_miss();
                        let doc = self.persistent_layer.get_document(collection, doc_id)?;
                        
                        if let Some(ref doc) = doc {
                            self.populate_cache(collection, doc_id, doc, &schema).await?;
                        }
                        
                        self.stats.record_persistent_read();
                        Ok(doc)
                    }
                }
            }
        } else {
            // No schema, default to persistent only
            let doc = self.persistent_layer.get_document(collection, doc_id)?;
            self.stats.record_persistent_read();
            Ok(doc)
        }
    }

    /// Update a document
    pub async fn update_document(
        &self,
        collection: &str,
        doc_id: DocumentId,
        doc: Document,
    ) -> Result<()> {
        let schema = self.get_schema(collection);

        if let Some(schema) = schema {
            let strategy = &schema.cache_config.strategy;
            
            match strategy {
                CacheStrategy::None => {
                    self.persistent_layer.update_document(collection, doc_id, &doc)?;
                    self.stats.record_persistent_write();
                }
                CacheStrategy::WriteThrough => {
                    self.write_through(collection, doc_id, &doc, &schema).await?;
                }
                CacheStrategy::WriteBehind { delay_ms } => {
                    self.write_behind(collection, doc_id, &doc, &schema, *delay_ms).await?;
                }
                CacheStrategy::ReadThrough => {
                    self.persistent_layer.update_document(collection, doc_id, &doc)?;
                    self.invalidate_cache_entry(collection, doc_id);
                    self.stats.record_persistent_write();
                }
            }
        } else {
            self.persistent_layer.update_document(collection, doc_id, &doc)?;
            self.stats.record_persistent_write();
        }

        Ok(())
    }

    /// Delete a document
    pub async fn delete_document(
        &self,
        collection: &str,
        doc_id: DocumentId,
    ) -> Result<bool> {
        let schema = self.get_schema(collection);

        if let Some(schema) = schema {
            let strategy = &schema.cache_config.strategy;
            
            match strategy {
                CacheStrategy::None => {
                    let deleted = self.persistent_layer.delete_document(collection, doc_id)?;
                    self.stats.record_persistent_write();
                    Ok(deleted)
                }
                CacheStrategy::WriteThrough => {
                    // Delete from both cache and persistent
                    let cache_key = self.make_cache_key(collection, doc_id);
                    self.cache_layer.remove(&cache_key);
                    let deleted = self.persistent_layer.delete_document(collection, doc_id)?;
                    self.stats.record_cache_write();
                    self.stats.record_persistent_write();
                    Ok(deleted)
                }
                CacheStrategy::WriteBehind { delay_ms } => {
                    // Delete from cache immediately, queue persistent delete
                    let cache_key = self.make_cache_key(collection, doc_id);
                    self.cache_layer.remove(&cache_key);
                    
                    self.queue_write_behind(WriteBehindEntry {
                        collection: collection.to_string(),
                        doc_id,
                        operation: WriteBehindOperation::Delete,
                        delay_ms: *delay_ms,
                        queued_at: std::time::Instant::now(),
                    });
                    
                    self.stats.record_cache_write();
                    Ok(true)
                }
                CacheStrategy::ReadThrough => {
                    let deleted = self.persistent_layer.delete_document(collection, doc_id)?;
                    self.invalidate_cache_entry(collection, doc_id);
                    self.stats.record_persistent_write();
                    Ok(deleted)
                }
            }
        } else {
            let deleted = self.persistent_layer.delete_document(collection, doc_id)?;
            self.stats.record_persistent_write();
            Ok(deleted)
        }
    }

    /// Write-through strategy: update both cache and persistent storage
    async fn write_through(
        &self,
        collection: &str,
        doc_id: DocumentId,
        doc: &Document,
        schema: &Schema,
    ) -> Result<()> {
        // Write to persistent storage first
        self.persistent_layer.insert_document(collection, doc_id, doc)?;
        
        // Then update cache
        self.populate_cache(collection, doc_id, doc, schema).await?;
        
        self.stats.record_cache_write();
        self.stats.record_persistent_write();
        
        Ok(())
    }

    /// Write-behind strategy: update cache immediately, queue persistent write
    async fn write_behind(
        &self,
        collection: &str,
        doc_id: DocumentId,
        doc: &Document,
        schema: &Schema,
        delay_ms: u64,
    ) -> Result<()> {
        // Update cache immediately
        self.populate_cache(collection, doc_id, doc, schema).await?;
        
        // Queue persistent write
        self.queue_write_behind(WriteBehindEntry {
            collection: collection.to_string(),
            doc_id,
            operation: WriteBehindOperation::Write(doc.clone()),
            delay_ms,
            queued_at: std::time::Instant::now(),
        });
        
        self.stats.record_cache_write();
        
        Ok(())
    }

    /// Populate cache with document data
    async fn populate_cache(
        &self,
        collection: &str,
        doc_id: DocumentId,
        doc: &Document,
        schema: &Schema,
    ) -> Result<()> {
        let cache_key = self.make_cache_key(collection, doc_id);
        let cache_data = self.document_to_cache_data(doc, schema)?;
        let ttl = schema.cache_config.ttl;
        
        self.cache_layer.set_with_ttl(cache_key, cache_data, ttl)?;
        
        Ok(())
    }

    /// Invalidate a cache entry
    fn invalidate_cache_entry(&self, collection: &str, doc_id: DocumentId) {
        let cache_key = self.make_cache_key(collection, doc_id);
        self.cache_layer.remove(&cache_key);
        self.stats.record_cache_invalidation();
    }

    /// Invalidate all cache entries for a collection
    pub fn invalidate_collection_cache(&self, collection: &str) {
        // This is a simplified implementation
        // In production, we'd need a more efficient way to track collection keys
        let _prefix = format!("doc:{}:", collection);
        
        // For now, we'll just clear the entire cache if it's a collection invalidation
        // A better approach would be to maintain a collection -> keys mapping
        self.cache_layer.clear();
        self.stats.record_cache_invalidation();
    }

    /// Queue a write-behind operation
    fn queue_write_behind(&self, entry: WriteBehindEntry) {
        self.write_behind_queue.write().push(entry);
    }

    /// Process write-behind queue (background task)
    async fn process_write_behind_queue(&self) {
        loop {
            sleep(Duration::from_millis(50)).await;
            
            let now = std::time::Instant::now();
            
            // Collect entries ready to be processed
            let entries_to_process: Vec<WriteBehindEntry> = {
                let mut queue = self.write_behind_queue.write();
                let mut entries = Vec::new();
                let mut i = 0;
                
                while i < queue.len() {
                    let entry = &queue[i];
                    let elapsed = now.duration_since(entry.queued_at).as_millis() as u64;
                    
                    if elapsed >= entry.delay_ms {
                        entries.push(queue.remove(i));
                    } else {
                        i += 1;
                    }
                }
                
                entries
            }; // Lock is released here
            
            // Process entries without holding the lock
            for entry in entries_to_process {
                if let Err(e) = self.process_write_behind_entry(entry).await {
                    eprintln!("Error processing write-behind entry: {}", e);
                }
            }
        }
    }

    /// Process a single write-behind entry
    async fn process_write_behind_entry(&self, entry: WriteBehindEntry) -> Result<()> {
        match entry.operation {
            WriteBehindOperation::Write(doc) => {
                self.persistent_layer.insert_document(&entry.collection, entry.doc_id, &doc)?;
                self.stats.record_persistent_write();
            }
            WriteBehindOperation::Delete => {
                self.persistent_layer.delete_document(&entry.collection, entry.doc_id)?;
                self.stats.record_persistent_write();
            }
        }
        
        Ok(())
    }

    /// Warm caches based on warming strategies (background task)
    async fn warm_caches(&self) {
        // Wait a bit before starting cache warming
        sleep(Duration::from_secs(5)).await;
        
        loop {
            let schemas = self.schemas.read().clone();
            
            for (collection, schema) in schemas.iter() {
                match &schema.cache_config.warming {
                    CacheWarmingStrategy::None => {
                        // No warming
                    }
                    CacheWarmingStrategy::PreloadOnStartup { limit } => {
                        // This runs once at startup (we check if already warmed)
                        if self.stats.cache_warmed.load(Ordering::Relaxed) == 0 {
                            if let Err(e) = self.preload_collection(collection, *limit).await {
                                eprintln!("Error preloading collection {}: {}", collection, e);
                            }
                        }
                    }
                    CacheWarmingStrategy::LazyLoad => {
                        // Lazy load happens on access, nothing to do here
                    }
                    CacheWarmingStrategy::ScheduledRefresh { interval_seconds } => {
                        // Refresh cache periodically
                        if let Err(e) = self.refresh_collection_cache(collection).await {
                            eprintln!("Error refreshing cache for {}: {}", collection, e);
                        }
                        sleep(Duration::from_secs(*interval_seconds)).await;
                    }
                }
            }
            
            // Mark cache as warmed after first pass
            self.stats.cache_warmed.store(1, Ordering::Relaxed);
            
            // Sleep before next warming cycle
            sleep(Duration::from_secs(60)).await;
        }
    }

    /// Preload a collection into cache
    async fn preload_collection(&self, collection: &str, limit: usize) -> Result<()> {
        let schema = self.get_schema(collection)
            .context("Schema not found")?;
        
        // Scan collection from persistent storage
        let documents = self.persistent_layer.scan_collection(collection)?;
        
        // Load up to limit documents into cache
        for (i, doc) in documents.iter().enumerate() {
            if i >= limit {
                break;
            }
            
            self.populate_cache(collection, doc.id, doc, &schema).await?;
        }
        
        Ok(())
    }

    /// Refresh collection cache
    async fn refresh_collection_cache(&self, collection: &str) -> Result<()> {
        let schema = self.get_schema(collection)
            .context("Schema not found")?;
        
        // Get all documents from persistent storage
        let documents = self.persistent_layer.scan_collection(collection)?;
        
        // Update cache for each document
        for doc in documents.iter() {
            self.populate_cache(collection, doc.id, doc, &schema).await?;
        }
        
        Ok(())
    }

    /// Make a cache key for a document
    fn make_cache_key(&self, collection: &str, doc_id: DocumentId) -> String {
        format!("doc:{}:{}", collection, doc_id)
    }

    /// Convert document to cache data
    fn document_to_cache_data(&self, doc: &Document, schema: &Schema) -> Result<CacheData> {
        // If specific fields are configured for caching, extract only those
        if let Some(fields) = &schema.cache_config.fields {
            let mut hash = crate::cache::data_structures::CacheHash::new();
            
            for field_name in fields {
                if let Some(value) = doc.get(field_name) {
                    hash.hset(field_name.clone(), value.clone());
                }
            }
            
            Ok(CacheData::Hash(hash))
        } else {
            // Cache entire document as a hash
            let mut hash = crate::cache::data_structures::CacheHash::new();
            for (key, value) in &doc.fields {
                hash.hset(key.clone(), value.clone());
            }
            Ok(CacheData::Hash(hash))
        }
    }

    /// Convert cache data to document
    fn cache_data_to_document(&self, cache_data: CacheData, doc_id: DocumentId) -> Result<Document> {
        match cache_data {
            CacheData::Hash(hash) => {
                let mut doc = Document::with_id(doc_id);
                doc.fields = hash.hgetall();
                Ok(doc)
            }
            _ => anyhow::bail!("Invalid cache data format"),
        }
    }

    /// Get cache layer reference
    pub fn cache_layer(&self) -> &Arc<CacheLayer> {
        &self.cache_layer
    }

    /// Get persistent layer reference
    pub fn persistent_layer(&self) -> &Arc<PersistentLayer> {
        &self.persistent_layer
    }

    /// Get statistics
    pub fn stats(&self) -> &HybridStorageStats {
        &self.stats
    }

    /// Flush all pending writes
    pub async fn flush(&self) -> Result<()> {
        // Process all pending write-behind entries
        let queue = self.write_behind_queue.write().drain(..).collect::<Vec<_>>();
        
        for entry in queue {
            self.process_write_behind_entry(entry).await?;
        }
        
        // Flush persistent layer
        self.persistent_layer.flush()?;
        
        Ok(())
    }
}

/// Write-behind queue entry
struct WriteBehindEntry {
    collection: String,
    doc_id: DocumentId,
    operation: WriteBehindOperation,
    delay_ms: u64,
    queued_at: std::time::Instant,
}

/// Write-behind operation type
enum WriteBehindOperation {
    Write(Document),
    Delete,
}

/// Hybrid storage statistics
#[derive(Debug, Default)]
pub struct HybridStorageStats {
    /// Cache hits
    cache_hits: AtomicU64,
    /// Cache misses
    cache_misses: AtomicU64,
    /// Cache writes
    cache_writes: AtomicU64,
    /// Cache invalidations
    cache_invalidations: AtomicU64,
    /// Persistent reads
    persistent_reads: AtomicU64,
    /// Persistent writes
    persistent_writes: AtomicU64,
    /// Cache warmed flag
    cache_warmed: AtomicU64,
}

impl HybridStorageStats {
    /// Record a cache hit
    fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache miss
    fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache write
    fn record_cache_write(&self) {
        self.cache_writes.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache invalidation
    fn record_cache_invalidation(&self) {
        self.cache_invalidations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a persistent read
    fn record_persistent_read(&self) {
        self.persistent_reads.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a persistent write
    fn record_persistent_write(&self) {
        self.persistent_writes.fetch_add(1, Ordering::Relaxed);
    }

    /// Get cache hits
    pub fn cache_hits(&self) -> u64 {
        self.cache_hits.load(Ordering::Relaxed)
    }

    /// Get cache misses
    pub fn cache_misses(&self) -> u64 {
        self.cache_misses.load(Ordering::Relaxed)
    }

    /// Get cache writes
    pub fn cache_writes(&self) -> u64 {
        self.cache_writes.load(Ordering::Relaxed)
    }

    /// Get cache invalidations
    pub fn cache_invalidations(&self) -> u64 {
        self.cache_invalidations.load(Ordering::Relaxed)
    }

    /// Get persistent reads
    pub fn persistent_reads(&self) -> u64 {
        self.persistent_reads.load(Ordering::Relaxed)
    }

    /// Get persistent writes
    pub fn persistent_writes(&self) -> u64 {
        self.persistent_writes.load(Ordering::Relaxed)
    }

    /// Calculate cache hit rate
    pub fn cache_hit_rate(&self) -> f64 {
        let hits = self.cache_hits();
        let total = hits + self.cache_misses();
        
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    /// Get total operations
    pub fn total_operations(&self) -> u64 {
        self.cache_hits() + self.cache_misses() + self.cache_writes()
            + self.persistent_reads() + self.persistent_writes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Value;
    use crate::schema::{FieldDefinition, FieldType};
    use tempfile::TempDir;

    fn create_test_engine() -> (Arc<HybridStorageEngine>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let persistent = Arc::new(PersistentLayer::new(temp_dir.path()).unwrap());
        let cache_config = CacheConfig::default();
        
        let engine = Arc::new(HybridStorageEngine::new(cache_config, persistent));
        (engine, temp_dir)
    }

    fn create_test_schema(strategy: CacheStrategy) -> Schema {
        let mut schema = Schema::new();
        schema.add_field(
            "name".to_string(),
            FieldDefinition::new(FieldType::String { max_length: None }),
        );
        schema.cache_config.strategy = strategy;
        schema
    }

    #[tokio::test]
    async fn test_hybrid_engine_creation() {
        let (engine, _temp_dir) = create_test_engine();
        assert_eq!(engine.stats().total_operations(), 0);
    }

    #[tokio::test]
    async fn test_insert_with_no_cache_strategy() {
        let (engine, _temp_dir) = create_test_engine();
        
        let schema = create_test_schema(CacheStrategy::None);
        engine.register_schema("users".to_string(), schema);
        
        let mut doc = Document::new();
        doc.insert("name".to_string(), Value::String("John".to_string()));
        
        let doc_id = engine.insert_document("users", doc).await.unwrap();
        
        // Should only write to persistent
        assert_eq!(engine.stats().persistent_writes(), 1);
        assert_eq!(engine.stats().cache_writes(), 0);
        
        // Retrieve document
        let retrieved = engine.get_document("users", doc_id).await.unwrap();
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_insert_with_write_through() {
        let (engine, _temp_dir) = create_test_engine();
        
        let schema = create_test_schema(CacheStrategy::WriteThrough);
        engine.register_schema("users".to_string(), schema);
        
        let mut doc = Document::new();
        doc.insert("name".to_string(), Value::String("John".to_string()));
        let doc_id = doc.id;
        
        engine.insert_document("users", doc).await.unwrap();
        
        // Should write to both cache and persistent
        assert_eq!(engine.stats().persistent_writes(), 1);
        assert_eq!(engine.stats().cache_writes(), 1);
        
        // Retrieve should hit cache
        let retrieved = engine.get_document("users", doc_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(engine.stats().cache_hits(), 1);
    }

    #[tokio::test]
    async fn test_insert_with_write_behind() {
        let (engine, _temp_dir) = create_test_engine();
        
        let schema = create_test_schema(CacheStrategy::WriteBehind { delay_ms: 100 });
        engine.register_schema("users".to_string(), schema);
        
        let mut doc = Document::new();
        doc.insert("name".to_string(), Value::String("John".to_string()));
        let doc_id = doc.id;
        
        engine.insert_document("users", doc).await.unwrap();
        
        // Should write to cache immediately
        assert_eq!(engine.stats().cache_writes(), 1);
        assert_eq!(engine.stats().persistent_writes(), 0);
        
        // Wait for write-behind to process
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Flush to ensure write-behind is processed
        engine.flush().await.unwrap();
        
        // Now persistent write should have happened
        assert!(engine.stats().persistent_writes() >= 1);
    }

    #[tokio::test]
    async fn test_get_with_cache_miss() {
        let (engine, _temp_dir) = create_test_engine();
        
        let schema = create_test_schema(CacheStrategy::WriteThrough);
        engine.register_schema("users".to_string(), schema);
        
        // Insert directly to persistent (bypassing cache)
        let mut doc = Document::new();
        let doc_id = doc.id;
        doc.insert("name".to_string(), Value::String("John".to_string()));
        engine.persistent_layer().insert_document("users", doc_id, &doc).unwrap();
        
        // Get should miss cache, then populate it
        let retrieved = engine.get_document("users", doc_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(engine.stats().cache_misses(), 1);
        assert_eq!(engine.stats().persistent_reads(), 1);
        
        // Second get should hit cache
        let retrieved2 = engine.get_document("users", doc_id).await.unwrap();
        assert!(retrieved2.is_some());
        assert_eq!(engine.stats().cache_hits(), 1);
    }

    #[tokio::test]
    async fn test_update_document() {
        let (engine, _temp_dir) = create_test_engine();
        
        let schema = create_test_schema(CacheStrategy::WriteThrough);
        engine.register_schema("users".to_string(), schema);
        
        let mut doc = Document::new();
        let doc_id = doc.id;
        doc.insert("name".to_string(), Value::String("John".to_string()));
        
        engine.insert_document("users", doc.clone()).await.unwrap();
        
        // Update
        doc.insert("name".to_string(), Value::String("Jane".to_string()));
        engine.update_document("users", doc_id, doc).await.unwrap();
        
        // Retrieve updated document
        let retrieved = engine.get_document("users", doc_id).await.unwrap().unwrap();
        assert_eq!(retrieved.get("name").unwrap().as_str(), Some("Jane"));
    }

    #[tokio::test]
    async fn test_delete_document() {
        let (engine, _temp_dir) = create_test_engine();
        
        let schema = create_test_schema(CacheStrategy::WriteThrough);
        engine.register_schema("users".to_string(), schema);
        
        let mut doc = Document::new();
        let doc_id = doc.id;
        doc.insert("name".to_string(), Value::String("John".to_string()));
        
        engine.insert_document("users", doc).await.unwrap();
        
        // Delete
        let deleted = engine.delete_document("users", doc_id).await.unwrap();
        assert!(deleted);
        
        // Should not exist
        let retrieved = engine.get_document("users", doc_id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let (engine, _temp_dir) = create_test_engine();
        
        let schema = create_test_schema(CacheStrategy::ReadThrough);
        engine.register_schema("users".to_string(), schema);
        
        let mut doc = Document::new();
        let doc_id = doc.id;
        doc.insert("name".to_string(), Value::String("John".to_string()));
        
        // Insert with ReadThrough invalidates cache
        engine.insert_document("users", doc).await.unwrap();
        
        assert_eq!(engine.stats().cache_invalidations(), 1);
    }

    #[tokio::test]
    async fn test_statistics() {
        let (engine, _temp_dir) = create_test_engine();
        
        let schema = create_test_schema(CacheStrategy::WriteThrough);
        engine.register_schema("users".to_string(), schema);
        
        let mut doc = Document::new();
        let doc_id = doc.id;
        doc.insert("name".to_string(), Value::String("John".to_string()));
        
        engine.insert_document("users", doc).await.unwrap();
        engine.get_document("users", doc_id).await.unwrap();
        
        let stats = engine.stats();
        assert!(stats.total_operations() > 0);
        assert!(stats.cache_hit_rate() >= 0.0 && stats.cache_hit_rate() <= 1.0);
    }
}
