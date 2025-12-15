//! VedDB v0.2.0 data writer
//!
//! Writes migrated data to v0.2.0 format using the new document-based storage.

use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tracing::{info, debug};
use veddb_core::{
    document::{Document, DocumentId, Value},
    schema::{Schema, FieldDefinition, FieldType, CacheStrategy, CollectionCacheConfig},
};

use crate::v1_reader::V1KeyValue;

/// Writer for v0.2.0 format
pub struct V2Writer {
    output_path: PathBuf,
    collection_name: String,
}

impl V2Writer {
    /// Create a new V2Writer
    pub fn new<P: AsRef<Path>>(output_path: P, collection_name: String) -> Self {
        Self {
            output_path: output_path.as_ref().to_path_buf(),
            collection_name,
        }
    }

    /// Initialize the v0.2.0 data directory
    pub async fn initialize(&self, force: bool) -> Result<()> {
        if self.output_path.exists() && !force {
            bail!("Output directory already exists: {}. Use --force to overwrite.", 
                  self.output_path.display());
        }

        // Create directory structure
        std::fs::create_dir_all(&self.output_path)
            .with_context(|| format!("Failed to create output directory: {}", 
                                    self.output_path.display()))?;

        let collections_dir = self.output_path.join("collections");
        std::fs::create_dir_all(&collections_dir)?;

        let metadata_dir = self.output_path.join("metadata");
        std::fs::create_dir_all(&metadata_dir)?;

        let wal_dir = self.output_path.join("wal");
        std::fs::create_dir_all(&wal_dir)?;

        let snapshots_dir = self.output_path.join("snapshots");
        std::fs::create_dir_all(&snapshots_dir)?;

        info!("Initialized v0.2.0 data directory: {}", self.output_path.display());
        Ok(())
    }

    /// Write migrated data to v0.2.0 format
    pub async fn write_data(&self, data: &[V1KeyValue]) -> Result<MigrationStats> {
        let start_time = std::time::Instant::now();
        
        info!("Writing {} key-value pairs to collection '{}'", 
              data.len(), self.collection_name);

        // Create schema for legacy key-value data
        let schema = self.create_legacy_schema();
        
        // Write schema file
        let collection_path = self.output_path
            .join("collections")
            .join(&self.collection_name);
        std::fs::create_dir_all(&collection_path)?;
        
        let schema_file = collection_path.join("schema.json");
        let schema_json = serde_json::to_string_pretty(&schema)?;
        std::fs::write(&schema_file, schema_json)?;

        // Create a simple storage for documents (for now, just write to JSON files)
        // In a real implementation, this would use RocksDB
        let documents_dir = collection_path.join("documents");
        std::fs::create_dir_all(&documents_dir)?;

        let mut total_size = 0;
        let mut processed = 0;

        // Convert and store each key-value pair as a document
        for kv in data {
            let document = self.convert_kv_to_document(kv)?;
            total_size += kv.key.len() + kv.value.len();
            
            // Write document to file (simplified storage)
            let doc_file = documents_dir.join(format!("{}.json", document.id));
            let doc_json = serde_json::to_string_pretty(&document)?;
            std::fs::write(&doc_file, doc_json)
                .with_context(|| format!("Failed to write document for key: {:?}", 
                                       String::from_utf8_lossy(&kv.key)))?;
            
            processed += 1;
            
            if processed % 1000 == 0 {
                debug!("Processed {} documents", processed);
            }
        }

        let duration = start_time.elapsed();
        
        Ok(MigrationStats {
            total_keys: processed,
            total_size,
            duration,
        })
    }

    /// Create schema for legacy key-value data
    fn create_legacy_schema(&self) -> Schema {
        let mut schema = Schema::new();
        
        // Key field (stored as string for searchability)
        schema.add_field("key".to_string(), FieldDefinition::new(
            FieldType::String { max_length: None }
        ).required().unique().indexed());

        // Value field (stored as string - base64 encoded binary data)
        schema.add_field("value".to_string(), FieldDefinition::new(
            FieldType::String { max_length: None }
        ).required());

        // Original key as string (base64 encoded for exact preservation)
        schema.add_field("original_key".to_string(), FieldDefinition::new(
            FieldType::String { max_length: None }
        ).required().unique());

        // Metadata fields
        schema.add_field("migrated_at".to_string(), FieldDefinition::new(
            FieldType::Date
        ).required());

        schema.add_field("original_ttl".to_string(), FieldDefinition::new(
            FieldType::Int64
        ));

        schema.add_field("original_version".to_string(), FieldDefinition::new(
            FieldType::Int64
        ));

        // Set cache config to no caching for legacy data
        schema.cache_config = CollectionCacheConfig {
            strategy: CacheStrategy::None,
            ttl: None,
            fields: None,
            warming: veddb_core::schema::CacheWarmingStrategy::None,
        };

        schema
    }

    /// Convert a v0.1.x key-value pair to a v0.2.0 document
    fn convert_kv_to_document(&self, kv: &V1KeyValue) -> Result<Document> {
        let mut fields = BTreeMap::new();
        
        // Convert key to string for searchability (with fallback for non-UTF8)
        let key_string = String::from_utf8_lossy(&kv.key).to_string();
        fields.insert("key".to_string(), Value::String(key_string));
        
        // Store value as base64-encoded string (since JSON doesn't support raw binary)
        let value_b64 = general_purpose::STANDARD.encode(&kv.value);
        fields.insert("value".to_string(), Value::String(value_b64));
        
        // Store original key as base64 for exact preservation
        let original_key_b64 = general_purpose::STANDARD.encode(&kv.key);
        fields.insert("original_key".to_string(), Value::String(original_key_b64));
        
        // Migration timestamp
        fields.insert("migrated_at".to_string(), 
                     Value::DateTime(chrono::Utc::now()));
        
        // Preserve original metadata if available
        if let Some(metadata) = &kv.metadata {
            if let Some(ttl) = metadata.ttl {
                fields.insert("original_ttl".to_string(), Value::Int64(ttl as i64));
            }
            if let Some(version) = metadata.version {
                fields.insert("original_version".to_string(), Value::Int64(version as i64));
            }
        }

        Ok(Document::from_fields(fields))
    }

    /// Verify written data by reading it back
    pub async fn verify_data(&self, original_data: &[V1KeyValue]) -> Result<()> {
        info!("Verifying migrated data integrity...");
        
        let documents_dir = self.output_path
            .join("collections")
            .join(&self.collection_name)
            .join("documents");
        
        if !documents_dir.exists() {
            bail!("Documents directory not found: {}", documents_dir.display());
        }

        // Count documents
        let doc_files: Vec<_> = std::fs::read_dir(&documents_dir)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().map_or(false, |ext| ext == "json"))
            .collect();

        if doc_files.len() != original_data.len() {
            bail!("Document count mismatch: expected {}, found {}", 
                  original_data.len(), doc_files.len());
        }

        // Verify a sample of documents
        let sample_size = std::cmp::min(100, original_data.len());
        for (i, original_kv) in original_data.iter().enumerate().take(sample_size) {
            // Find document by searching through files (simplified verification)
            let key_string = String::from_utf8_lossy(&original_kv.key);
            let original_key_b64 = general_purpose::STANDARD.encode(&original_kv.key);
            
            let mut found = false;
            for doc_file in &doc_files {
                let doc_content = std::fs::read_to_string(doc_file.path())?;
                let doc: Document = serde_json::from_str(&doc_content)?;
                
                // Check if this is the document we're looking for
                if let Some(stored_key) = doc.get("original_key").and_then(|v| v.as_str()) {
                    if stored_key == original_key_b64 {
                        found = true;
                        
                        // Verify value
                        let value_b64 = doc.get("value")
                            .and_then(|v| v.as_str())
                            .context("Missing value field")?;
                        
                        let decoded_value = general_purpose::STANDARD.decode(value_b64)
                            .context("Failed to decode value")?;
                        
                        if decoded_value != original_kv.value {
                            bail!("Value mismatch for document {}", i);
                        }
                        break;
                    }
                }
            }
            
            if !found {
                bail!("Document not found for key: {}", key_string);
            }
        }

        info!("Data verification completed successfully");
        Ok(())
    }
}

/// Migration statistics
#[derive(Debug)]
pub struct MigrationStats {
    pub total_keys: usize,
    pub total_size: usize,
    pub duration: std::time::Duration,
}

use base64::{Engine as _, engine::general_purpose};