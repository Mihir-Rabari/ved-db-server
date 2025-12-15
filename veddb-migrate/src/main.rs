//! VedDB v0.1.x to v0.2.0 Migration Tool
//!
//! This tool migrates data from VedDB v0.1.x (simple key-value store) to v0.2.0 
//! (hybrid document database with collections).

use anyhow::{Context, Result};
use clap::{Arg, Command};
use std::path::PathBuf;
use tracing::{info, warn, error};
use tracing_subscriber;

mod v1_reader;
mod v2_writer;
mod migration;
mod validation;

use migration::MigrationEngine;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("veddb_migrate=info")
        .init();

    let matches = Command::new("veddb-migrate")
        .version("0.2.0")
        .about("Migrate VedDB v0.1.x data to v0.2.0 format")
        .arg(
            Arg::new("input")
                .short('i')
                .long("input")
                .value_name("PATH")
                .help("Path to v0.1.x data directory or backup file")
                .required(true)
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("PATH")
                .help("Path to v0.2.0 data directory")
                .required(true)
        )
        .arg(
            Arg::new("collection")
                .short('c')
                .long("collection")
                .value_name("NAME")
                .help("Target collection name for migrated data")
                .default_value("_legacy_kv")
        )
        .arg(
            Arg::new("dry-run")
                .long("dry-run")
                .help("Perform validation without writing data")
                .action(clap::ArgAction::SetTrue)
        )
        .arg(
            Arg::new("verify")
                .long("verify")
                .help("Verify migration integrity after completion")
                .action(clap::ArgAction::SetTrue)
        )
        .arg(
            Arg::new("force")
                .long("force")
                .help("Overwrite existing v0.2.0 data")
                .action(clap::ArgAction::SetTrue)
        )
        .get_matches();

    let input_path = PathBuf::from(matches.get_one::<String>("input").unwrap());
    let output_path = PathBuf::from(matches.get_one::<String>("output").unwrap());
    let collection_name = matches.get_one::<String>("collection").unwrap();
    let dry_run = matches.get_flag("dry-run");
    let verify = matches.get_flag("verify");
    let force = matches.get_flag("force");

    info!("Starting VedDB v0.1.x to v0.2.0 migration");
    info!("Input: {}", input_path.display());
    info!("Output: {}", output_path.display());
    info!("Collection: {}", collection_name);
    
    if dry_run {
        info!("Running in dry-run mode - no data will be written");
    }

    // Create migration engine
    let mut migration_engine = MigrationEngine::new(
        input_path,
        output_path,
        collection_name.to_string(),
    )?;

    // Perform migration
    match migration_engine.migrate(dry_run, force).await {
        Ok(stats) => {
            info!("Migration completed successfully");
            info!("Migrated {} key-value pairs", stats.total_keys);
            info!("Total data size: {} bytes", stats.total_size);
            info!("Migration time: {:?}", stats.duration);

            if verify && !dry_run {
                info!("Verifying migration integrity...");
                match migration_engine.verify().await {
                    Ok(()) => info!("Migration verification passed"),
                    Err(e) => {
                        error!("Migration verification failed: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        Err(e) => {
            error!("Migration failed: {}", e);
            std::process::exit(1);
        }
    }

    info!("Migration tool completed successfully");
    Ok(())
}