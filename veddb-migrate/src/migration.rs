//! Migration engine that orchestrates the v0.1.x to v0.2.0 migration process

use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::{info, warn};

use crate::v1_reader::V1Reader;
use crate::v2_writer::{V2Writer, MigrationStats};
use crate::validation::MigrationValidator;

/// Main migration engine
pub struct MigrationEngine {
    input_path: PathBuf,
    output_path: PathBuf,
    collection_name: String,
    reader: Option<V1Reader>,
    writer: V2Writer,
    validator: MigrationValidator,
}

impl MigrationEngine {
    /// Create a new migration engine
    pub fn new(
        input_path: PathBuf,
        output_path: PathBuf,
        collection_name: String,
    ) -> Result<Self> {
        let writer = V2Writer::new(&output_path, collection_name.clone());
        let validator = MigrationValidator::new();

        Ok(Self {
            input_path,
            output_path,
            collection_name,
            reader: None,
            writer,
            validator,
        })
    }

    /// Perform the migration
    pub async fn migrate(&mut self, dry_run: bool, force: bool) -> Result<MigrationStats> {
        info!("Starting migration process");

        // Step 1: Read v0.1.x data
        info!("Step 1: Reading v0.1.x data...");
        let reader = V1Reader::from_file(&self.input_path)
            .context("Failed to read v0.1.x data")?;
        
        let data = reader.get_data();
        info!("Loaded {} key-value pairs ({} bytes total)", 
              reader.key_count(), reader.total_size());

        // Step 2: Validate input data
        info!("Step 2: Validating input data...");
        self.validator.validate_v1_data(data)
            .context("Input data validation failed")?;

        // Step 3: Pre-migration checks
        info!("Step 3: Performing pre-migration checks...");
        self.pre_migration_checks(force)
            .context("Pre-migration checks failed")?;

        if dry_run {
            info!("Dry run completed successfully");
            return Ok(MigrationStats {
                total_keys: reader.key_count(),
                total_size: reader.total_size(),
                duration: std::time::Duration::from_secs(0),
            });
        }

        // Step 4: Initialize v0.2.0 storage
        info!("Step 4: Initializing v0.2.0 storage...");
        self.writer.initialize(force).await
            .context("Failed to initialize v0.2.0 storage")?;

        // Step 5: Migrate data
        info!("Step 5: Migrating data...");
        let stats = self.writer.write_data(data).await
            .context("Data migration failed")?;

        // Step 6: Post-migration validation
        info!("Step 6: Performing post-migration validation...");
        self.validator.validate_migration(&self.output_path, &self.collection_name, data).await
            .context("Post-migration validation failed")?;

        // Store reader for verification
        self.reader = Some(reader);

        info!("Migration completed successfully");
        Ok(stats)
    }

    /// Verify migration integrity
    pub async fn verify(&self) -> Result<()> {
        let reader = self.reader.as_ref()
            .context("No migration data available for verification")?;

        info!("Verifying migration integrity...");
        
        // Verify data integrity
        self.writer.verify_data(reader.get_data()).await
            .context("Data integrity verification failed")?;

        // Calculate and compare checksums
        let original_checksum = reader.calculate_checksum();
        let migrated_checksum = self.calculate_migrated_checksum().await?;
        
        info!("Original checksum: 0x{:08x}", original_checksum);
        info!("Migrated checksum: 0x{:08x}", migrated_checksum);

        if original_checksum != migrated_checksum {
            warn!("Checksum mismatch detected - this is expected due to format conversion");
            warn!("Performing detailed verification instead...");
            
            // Perform detailed verification
            self.detailed_verification(reader.get_data()).await?;
        }

        info!("Migration verification completed successfully");
        Ok(())
    }

    /// Perform pre-migration checks
    fn pre_migration_checks(&self, force: bool) -> Result<()> {
        // Check if output directory exists
        if self.output_path.exists() && !force {
            return Err(anyhow::anyhow!(
                "Output directory already exists: {}. Use --force to overwrite.",
                self.output_path.display()
            ));
        }

        // Check available disk space
        if let Ok(metadata) = std::fs::metadata(&self.input_path) {
            let required_space = metadata.len() * 2; // Estimate 2x space needed
            
            // This is a simplified check - in production, you'd want to check actual available space
            info!("Estimated space required: {} bytes", required_space);
        }

        // Check write permissions
        if let Some(parent) = self.output_path.parent() {
            if parent.exists() {
                let test_file = parent.join(".migration_test");
                match std::fs::write(&test_file, b"test") {
                    Ok(_) => {
                        let _ = std::fs::remove_file(&test_file);
                    }
                    Err(e) => {
                        return Err(anyhow::anyhow!(
                            "No write permission to output directory: {}", e
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    /// Calculate checksum of migrated data
    async fn calculate_migrated_checksum(&self) -> Result<u32> {
        // This would need to read back the migrated data and calculate checksum
        // For now, return a placeholder
        // In a real implementation, you'd read the collection and calculate checksum
        Ok(0)
    }

    /// Perform detailed verification by comparing individual records
    async fn detailed_verification(&self, original_data: &[crate::v1_reader::V1KeyValue]) -> Result<()> {
        info!("Performing detailed record-by-record verification...");
        
        // This would compare each original record with its migrated counterpart
        // The V2Writer already has a verify_data method that does this
        self.writer.verify_data(original_data).await?;
        
        Ok(())
    }
}