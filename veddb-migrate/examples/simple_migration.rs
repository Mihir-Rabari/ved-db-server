//! Simple migration example
//!
//! This example demonstrates how to use the migration tool programmatically.

use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;
use tempfile::TempDir;
use veddb_migrate::{MigrationEngine, V1Reader};

#[tokio::main]
async fn main() -> Result<()> {
    // Create temporary directories for input and output
    let input_dir = TempDir::new()?;
    let output_dir = TempDir::new()?;

    // Create a sample v0.1.x backup file
    let backup_file = input_dir.path().join("sample_backup.json");
    let backup_data = json!({
        "version": "0.1.21",
        "timestamp": "2025-01-15T10:30:00Z",
        "data": {
            "dXNlcjE=": "QWxpY2U=",      // user1 -> Alice
            "dXNlcjI=": "Qm9i",          // user2 -> Bob  
            "Y29uZmlnOmFwcA==": "eyJ0aGVtZSI6ImRhcmsifQ==", // config:app -> {"theme":"dark"}
        },
        "metadata": {
            "dXNlcjE=": {
                "created_at": "2025-01-15T10:00:00Z",
                "ttl": 3600,
                "version": 1
            }
        }
    });

    std::fs::write(&backup_file, backup_data.to_string())?;
    println!("Created sample backup file: {}", backup_file.display());

    // Read the v0.1.x data
    let reader = V1Reader::from_file(&backup_file)?;
    println!("Loaded {} key-value pairs", reader.key_count());
    println!("Total data size: {} bytes", reader.total_size());

    // Create migration engine
    let mut migration_engine = MigrationEngine::new(
        backup_file,
        output_dir.path().to_path_buf(),
        "sample_legacy_data".to_string(),
    )?;

    // Perform dry run first
    println!("\nPerforming dry run...");
    let dry_run_stats = migration_engine.migrate(true, true).await?; // Use force for dry run too
    println!("Dry run completed: {} keys, {} bytes", 
             dry_run_stats.total_keys, dry_run_stats.total_size);

    // Perform actual migration
    println!("\nPerforming actual migration...");
    let stats = migration_engine.migrate(false, true).await?; // Use force=true for example
    println!("Migration completed successfully!");
    println!("  Migrated keys: {}", stats.total_keys);
    println!("  Total size: {} bytes", stats.total_size);
    println!("  Duration: {:?}", stats.duration);

    // Verify migration
    println!("\nVerifying migration...");
    migration_engine.verify().await?;
    println!("Migration verification passed!");

    // Show output directory structure
    println!("\nOutput directory structure:");
    show_directory_tree(output_dir.path(), 0)?;

    println!("\nMigration example completed successfully!");
    println!("Output directory: {}", output_dir.path().display());
    println!("(Note: Temporary directory will be cleaned up when program exits)");

    Ok(())
}

fn show_directory_tree(path: &std::path::Path, depth: usize) -> Result<()> {
    let indent = "  ".repeat(depth);
    
    if path.is_dir() {
        println!("{}📁 {}", indent, path.file_name().unwrap_or_default().to_string_lossy());
        
        let mut entries: Vec<_> = std::fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|e| e.file_name());
        
        for entry in entries {
            show_directory_tree(&entry.path(), depth + 1)?;
        }
    } else {
        let size = std::fs::metadata(path)?.len();
        println!("{}📄 {} ({} bytes)", 
                 indent, 
                 path.file_name().unwrap_or_default().to_string_lossy(),
                 size);
    }
    
    Ok(())
}