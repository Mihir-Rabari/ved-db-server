//! VedDB Migration Library
//! 
//! This library provides functionality to migrate data from VedDB v0.1.x to v0.2.0.

pub mod v1_reader;
pub mod v2_writer;
pub mod migration;
pub mod validation;

pub use migration::MigrationEngine;
pub use v1_reader::{V1Reader, V1KeyValue, V1Metadata};
pub use v2_writer::{V2Writer, MigrationStats};
pub use validation::{MigrationValidator, MigrationReport};

/// Migration result type
pub type Result<T> = anyhow::Result<T>;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::collections::HashMap;

    #[test]
    fn test_migration_engine_creation() {
        let temp_input = TempDir::new().unwrap();
        let temp_output = TempDir::new().unwrap();
        
        let engine = MigrationEngine::new(
            temp_input.path().to_path_buf(),
            temp_output.path().to_path_buf(),
            "_test_collection".to_string(),
        );
        
        assert!(engine.is_ok());
    }

    #[test]
    fn test_v1_data_validation() {
        let validator = MigrationValidator::new();
        
        let test_data = vec![
            V1KeyValue {
                key: b"test_key".to_vec(),
                value: b"test_value".to_vec(),
                metadata: None,
            }
        ];
        
        let result = validator.validate_v1_data(&test_data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_migration_report() {
        let validator = MigrationValidator::new();
        
        let test_data = vec![
            V1KeyValue {
                key: b"key1".to_vec(),
                value: b"value1".to_vec(),
                metadata: None,
            },
            V1KeyValue {
                key: b"key2".to_vec(),
                value: b"longer_value".to_vec(),
                metadata: Some(crate::v1_reader::V1Metadata {
                    created_at: Some(chrono::Utc::now()),
                    ttl: Some(3600),
                    version: Some(1),
                }),
            }
        ];
        
        let report = validator.generate_report(&test_data);
        assert_eq!(report.total_records, 2);
        assert_eq!(report.records_with_metadata, 1);
        assert!(report.total_key_bytes > 0);
        assert!(report.total_value_bytes > 0);
    }

    #[tokio::test]
    async fn test_json_backup_creation() {
        let temp_dir = TempDir::new().unwrap();
        let backup_file = temp_dir.path().join("test_backup.json");
        
        // Create a test backup file
        let backup_data = serde_json::json!({
            "version": "0.1.21",
            "timestamp": "2025-01-15T10:30:00Z",
            "data": {
                "dGVzdA==": "dmFsdWU=",  // "test" -> "value"
                "a2V5Mg==": "dmFsdWUy"   // "key2" -> "value2"
            }
        });
        
        std::fs::write(&backup_file, backup_data.to_string()).unwrap();
        
        // Test reading the backup
        let reader = V1Reader::from_file(&backup_file);
        assert!(reader.is_ok());
        
        let reader = reader.unwrap();
        assert_eq!(reader.key_count(), 2);
        assert!(reader.total_size() > 0);
    }
}