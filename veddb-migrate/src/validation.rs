//! Migration validation utilities

use anyhow::{Context, Result, bail};
use std::collections::HashSet;
use std::path::Path;
use tracing::{info, warn, debug};

use crate::v1_reader::V1KeyValue;

/// Validator for migration process
pub struct MigrationValidator {
    max_key_size: usize,
    max_value_size: usize,
    max_total_keys: usize,
}

impl MigrationValidator {
    /// Create a new validator with default limits
    pub fn new() -> Self {
        Self {
            max_key_size: 1024 * 1024,      // 1MB max key size
            max_value_size: 16 * 1024 * 1024, // 16MB max value size (v0.2.0 limit)
            max_total_keys: 10_000_000,     // 10M keys max
        }
    }

    /// Validate v0.1.x input data
    pub fn validate_v1_data(&self, data: &[V1KeyValue]) -> Result<()> {
        info!("Validating {} key-value pairs", data.len());

        if data.is_empty() {
            warn!("No data to migrate");
            return Ok(());
        }

        if data.len() > self.max_total_keys {
            bail!("Too many keys: {} (max: {})", data.len(), self.max_total_keys);
        }

        let mut key_set = HashSet::new();
        let mut total_size = 0;
        let mut oversized_keys = 0;
        let mut oversized_values = 0;
        let mut duplicate_keys = 0;

        for (i, kv) in data.iter().enumerate() {
            // Check key size
            if kv.key.len() > self.max_key_size {
                oversized_keys += 1;
                if oversized_keys <= 5 { // Log first 5 oversized keys
                    warn!("Oversized key at index {}: {} bytes", i, kv.key.len());
                }
            }

            // Check value size
            if kv.value.len() > self.max_value_size {
                oversized_values += 1;
                if oversized_values <= 5 { // Log first 5 oversized values
                    warn!("Oversized value at index {}: {} bytes", i, kv.value.len());
                }
            }

            // Check for duplicate keys
            if !key_set.insert(&kv.key) {
                duplicate_keys += 1;
                if duplicate_keys <= 5 { // Log first 5 duplicates
                    warn!("Duplicate key at index {}: {:?}", i, 
                          String::from_utf8_lossy(&kv.key));
                }
            }

            // Check for empty keys
            if kv.key.is_empty() {
                warn!("Empty key found at index {}", i);
            }

            total_size += kv.key.len() + kv.value.len();

            // Progress reporting
            if i > 0 && i % 100_000 == 0 {
                debug!("Validated {} records", i);
            }
        }

        // Report validation results
        info!("Validation completed:");
        info!("  Total records: {}", data.len());
        info!("  Total size: {} bytes ({:.2} MB)", total_size, total_size as f64 / 1_048_576.0);
        info!("  Unique keys: {}", key_set.len());

        if oversized_keys > 0 {
            warn!("  Oversized keys: {} (will be truncated or rejected)", oversized_keys);
        }

        if oversized_values > 0 {
            warn!("  Oversized values: {} (will be rejected)", oversized_values);
        }

        if duplicate_keys > 0 {
            warn!("  Duplicate keys: {} (later values will overwrite earlier ones)", duplicate_keys);
        }

        // Fail if there are critical issues
        if oversized_values > 0 {
            bail!("Migration cannot proceed: {} values exceed the 16MB limit", oversized_values);
        }

        Ok(())
    }

    /// Validate migration results
    pub async fn validate_migration(
        &self,
        output_path: &Path,
        collection_name: &str,
        original_data: &[V1KeyValue],
    ) -> Result<()> {
        info!("Validating migration results");

        // Check that output directory structure exists
        self.validate_output_structure(output_path)?;

        // Check collection exists
        let collection_path = output_path.join("collections").join(collection_name);
        if !collection_path.exists() {
            bail!("Collection directory not found: {}", collection_path.display());
        }

        // Validate collection structure
        self.validate_collection_structure(&collection_path)?;

        info!("Migration validation completed successfully");
        Ok(())
    }

    /// Validate v0.2.0 output directory structure
    fn validate_output_structure(&self, output_path: &Path) -> Result<()> {
        let required_dirs = ["collections", "metadata", "wal", "snapshots"];

        for dir_name in &required_dirs {
            let dir_path = output_path.join(dir_name);
            if !dir_path.exists() {
                bail!("Required directory missing: {}", dir_path.display());
            }
            if !dir_path.is_dir() {
                bail!("Path is not a directory: {}", dir_path.display());
            }
        }

        Ok(())
    }

    /// Validate collection directory structure
    fn validate_collection_structure(&self, collection_path: &Path) -> Result<()> {
        // Check for schema file
        let schema_file = collection_path.join("schema.json");
        if !schema_file.exists() {
            bail!("Schema file missing: {}", schema_file.display());
        }

        // Check for documents directory (simplified storage for migration tool)
        let documents_dir = collection_path.join("documents");
        if !documents_dir.exists() {
            bail!("Documents directory missing: {}", documents_dir.display());
        }

        // Validate schema file format
        let schema_content = std::fs::read_to_string(&schema_file)
            .with_context(|| format!("Failed to read schema file: {}", schema_file.display()))?;

        let _schema: serde_json::Value = serde_json::from_str(&schema_content)
            .with_context(|| format!("Invalid JSON in schema file: {}", schema_file.display()))?;

        debug!("Collection structure validation passed");
        Ok(())
    }

    /// Generate migration report
    pub fn generate_report(&self, original_data: &[V1KeyValue]) -> MigrationReport {
        let mut report = MigrationReport::default();
        
        report.total_records = original_data.len();
        
        let mut key_sizes = Vec::new();
        let mut value_sizes = Vec::new();
        
        for kv in original_data {
            key_sizes.push(kv.key.len());
            value_sizes.push(kv.value.len());
            
            report.total_key_bytes += kv.key.len();
            report.total_value_bytes += kv.value.len();
            
            if kv.key.len() > report.max_key_size {
                report.max_key_size = kv.key.len();
            }
            
            if kv.value.len() > report.max_value_size {
                report.max_value_size = kv.value.len();
            }
            
            if kv.metadata.is_some() {
                report.records_with_metadata += 1;
            }
        }
        
        // Calculate statistics
        if !key_sizes.is_empty() {
            key_sizes.sort_unstable();
            value_sizes.sort_unstable();
            
            report.avg_key_size = report.total_key_bytes / key_sizes.len();
            report.avg_value_size = report.total_value_bytes / value_sizes.len();
            
            report.median_key_size = key_sizes[key_sizes.len() / 2];
            report.median_value_size = value_sizes[value_sizes.len() / 2];
        }
        
        report
    }
}

/// Migration report with statistics
#[derive(Debug, Default)]
pub struct MigrationReport {
    pub total_records: usize,
    pub total_key_bytes: usize,
    pub total_value_bytes: usize,
    pub max_key_size: usize,
    pub max_value_size: usize,
    pub avg_key_size: usize,
    pub avg_value_size: usize,
    pub median_key_size: usize,
    pub median_value_size: usize,
    pub records_with_metadata: usize,
}

impl MigrationReport {
    /// Print the report
    pub fn print(&self) {
        println!("\n=== Migration Report ===");
        println!("Total records: {}", self.total_records);
        println!("Total data size: {} bytes ({:.2} MB)", 
                 self.total_key_bytes + self.total_value_bytes,
                 (self.total_key_bytes + self.total_value_bytes) as f64 / 1_048_576.0);
        println!("Key statistics:");
        println!("  Total key bytes: {}", self.total_key_bytes);
        println!("  Average key size: {} bytes", self.avg_key_size);
        println!("  Median key size: {} bytes", self.median_key_size);
        println!("  Maximum key size: {} bytes", self.max_key_size);
        println!("Value statistics:");
        println!("  Total value bytes: {}", self.total_value_bytes);
        println!("  Average value size: {} bytes", self.avg_value_size);
        println!("  Median value size: {} bytes", self.median_value_size);
        println!("  Maximum value size: {} bytes", self.max_value_size);
        println!("Records with metadata: {}", self.records_with_metadata);
        println!("========================\n");
    }
}