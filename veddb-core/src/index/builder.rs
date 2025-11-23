//! Index builder for background index construction
//!
//! Builds indexes without blocking normal operations

use super::btree::{BTreeIndex, IndexEntry, IndexError};
use crate::document::Document;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Index builder for background operations
pub struct IndexBuilder {
    /// Maximum documents to process per batch
    batch_size: usize,
    /// Delay between batches to avoid blocking
    batch_delay_ms: u64,
}

impl IndexBuilder {
    /// Create a new index builder
    pub fn new() -> Self {
        Self {
            batch_size: 1000,
            batch_delay_ms: 10,
        }
    }

    /// Create index builder with custom settings
    pub fn with_settings(batch_size: usize, batch_delay_ms: u64) -> Self {
        Self {
            batch_size,
            batch_delay_ms,
        }
    }

    /// Build index for existing documents in background
    pub async fn build_index(
        &self,
        index: Arc<BTreeIndex>,
        documents: Vec<Document>,
    ) -> Result<(), IndexError> {
        let total_docs = documents.len();
        let mut processed = 0;

        log::info!(
            "Starting background index build for '{}' with {} documents",
            index.name(),
            total_docs
        );

        // Process documents in batches
        for batch in documents.chunks(self.batch_size) {
            for document in batch {
                let entry = self.create_index_entry(document, index.fields())?;
                index.insert(document.id, entry)?;
                processed += 1;
            }

            // Log progress
            if processed % (self.batch_size * 10) == 0 {
                log::info!(
                    "Index build progress for '{}': {}/{} documents ({}%)",
                    index.name(),
                    processed,
                    total_docs,
                    (processed * 100) / total_docs
                );
            }

            // Small delay to avoid blocking other operations
            if self.batch_delay_ms > 0 {
                sleep(Duration::from_millis(self.batch_delay_ms)).await;
            }
        }

        log::info!(
            "Completed background index build for '{}': {} documents processed",
            index.name(),
            processed
        );

        Ok(())
    }

    /// Rebuild an existing index
    pub async fn rebuild_index(
        &self,
        index: Arc<BTreeIndex>,
        documents: Vec<Document>,
    ) -> Result<(), IndexError> {
        log::info!("Rebuilding index '{}'", index.name());

        // Clear existing index
        index.clear();

        // Build from scratch
        self.build_index(index, documents).await
    }

    /// Create index entry from document
    fn create_index_entry(&self, document: &Document, fields: &[String]) -> Result<IndexEntry, IndexError> {
        let mut entry = IndexEntry::new();
        
        for field in fields {
            if let Some(value) = document.get_by_path(field) {
                entry.add_field(field.clone(), value.clone());
            } else {
                // Field not present - use null
                entry.add_field(field.clone(), crate::document::Value::Null);
            }
        }
        
        Ok(entry)
    }

    /// Estimate build time for an index
    pub fn estimate_build_time(&self, document_count: usize) -> Duration {
        // Rough estimate: 1ms per document + batch delays
        let processing_time = document_count as u64;
        let batch_count = (document_count + self.batch_size - 1) / self.batch_size;
        let delay_time = batch_count as u64 * self.batch_delay_ms;
        
        Duration::from_millis(processing_time + delay_time)
    }
}

impl Default for IndexBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{DocumentId, Value};
    use std::sync::Arc;

    fn create_test_documents(count: usize) -> Vec<Document> {
        (0..count)
            .map(|i| {
                let mut doc = Document::with_id(DocumentId::new());
                doc.insert("name".to_string(), Value::String(format!("User{}", i)));
                doc.insert("age".to_string(), Value::Int32(20 + (i as i32 % 50)));
                doc
            })
            .collect()
    }

    #[tokio::test]
    async fn test_index_builder_creation() {
        let builder = IndexBuilder::new();
        assert_eq!(builder.batch_size, 1000);
        assert_eq!(builder.batch_delay_ms, 10);
    }

    #[tokio::test]
    async fn test_build_index() {
        let builder = IndexBuilder::with_settings(10, 1); // Small batches for testing
        let index = Arc::new(super::super::btree::BTreeIndex::new(
            "test_index".to_string(),
            vec!["name".to_string()],
            false,
            false,
        ));

        let documents = create_test_documents(25);
        let doc_count = documents.len();

        builder.build_index(index.clone(), documents).await.unwrap();

        // Verify all documents were indexed
        assert_eq!(index.key_count(), doc_count);
    }

    #[tokio::test]
    async fn test_rebuild_index() {
        let builder = IndexBuilder::with_settings(10, 1);
        let index = Arc::new(super::super::btree::BTreeIndex::new(
            "test_index".to_string(),
            vec!["name".to_string()],
            false,
            false,
        ));

        // Build initial index
        let documents = create_test_documents(10);
        builder.build_index(index.clone(), documents.clone()).await.unwrap();
        assert_eq!(index.key_count(), 10);

        // Rebuild with more documents
        let more_documents = create_test_documents(20);
        builder.rebuild_index(index.clone(), more_documents).await.unwrap();
        assert_eq!(index.key_count(), 20);
    }

    #[tokio::test]
    async fn test_estimate_build_time() {
        let builder = IndexBuilder::with_settings(100, 5);
        
        let estimate = builder.estimate_build_time(1000);
        assert!(estimate.as_millis() > 1000); // At least 1ms per document
        
        let small_estimate = builder.estimate_build_time(10);
        let large_estimate = builder.estimate_build_time(1000);
        assert!(large_estimate > small_estimate);
    }

    #[tokio::test]
    async fn test_create_index_entry() {
        let builder = IndexBuilder::new();
        
        let mut doc = Document::new();
        doc.insert("name".to_string(), Value::String("John".to_string()));
        doc.insert("age".to_string(), Value::Int32(30));
        
        let entry = builder.create_index_entry(&doc, &["name".to_string(), "age".to_string()]).unwrap();
        
        assert_eq!(entry.get_field("name"), Some(&Value::String("John".to_string())));
        assert_eq!(entry.get_field("age"), Some(&Value::Int32(30)));
    }

    #[tokio::test]
    async fn test_missing_field_handling() {
        let builder = IndexBuilder::new();
        
        let mut doc = Document::new();
        doc.insert("name".to_string(), Value::String("John".to_string()));
        // Missing "age" field
        
        let entry = builder.create_index_entry(&doc, &["name".to_string(), "age".to_string()]).unwrap();
        
        assert_eq!(entry.get_field("name"), Some(&Value::String("John".to_string())));
        assert_eq!(entry.get_field("age"), Some(&Value::Null));
    }
}