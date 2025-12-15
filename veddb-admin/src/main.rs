use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, error};

mod client;
mod commands;
mod config;

use client::AdminClient;
use commands::*;
use config::AdminConfig;

#[derive(Parser)]
#[command(name = "veddb-admin")]
#[command(about = "VedDB Administration CLI Tool")]
#[command(version = "0.2.0")]
struct Cli {
    /// Configuration file path
    #[arg(short, long, default_value = "veddb-admin.toml")]
    config: PathBuf,
    
    /// Server address
    #[arg(short, long, default_value = "127.0.0.1:50051")]
    server: String,
    
    /// Username for authentication
    #[arg(short, long)]
    username: Option<String>,
    
    /// Password for authentication (will prompt if not provided)
    #[arg(short, long)]
    password: Option<String>,
    
    /// Use TLS connection
    #[arg(long, default_value = "true")]
    tls: bool,
    
    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
    
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// User management commands
    User {
        #[command(subcommand)]
        action: UserCommands,
    },
    /// Backup and restore commands
    Backup {
        #[command(subcommand)]
        action: BackupCommands,
    },
    /// Configuration management commands
    Config {
        #[command(subcommand)]
        action: ConfigCommands,
    },
    /// Database statistics and information
    Stats {
        #[command(subcommand)]
        action: StatsCommands,
    },
    /// Collection import/export commands
    Collection {
        #[command(subcommand)]
        action: CollectionCommands,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .init();
    
    info!("VedDB Admin CLI v0.2.0 starting");
    
    // Load configuration
    let config = AdminConfig::load(&cli.config).unwrap_or_default();
    
    // Create admin client
    let mut client = AdminClient::new(
        &cli.server,
        cli.tls,
        cli.username.or(config.username),
        cli.password.or(config.password),
    ).await?;
    
    // Execute command
    let result = match cli.command {
        Commands::User { action } => {
            execute_user_command(&mut client, action).await
        }
        Commands::Backup { action } => {
            execute_backup_command(&mut client, action).await
        }
        Commands::Config { action } => {
            execute_config_command(&mut client, action).await
        }
        Commands::Stats { action } => {
            execute_stats_command(&mut client, action).await
        }
        Commands::Collection { action } => {
            execute_collection_command(&mut client, action).await
        }
    };
    
    match result {
        Ok(_) => {
            info!("Command completed successfully");
            Ok(())
        }
        Err(e) => {
            error!("Command failed: {}", e);
            Err(e)
        }
    }
}