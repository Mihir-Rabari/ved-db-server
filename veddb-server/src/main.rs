//! VedDB Server v0.2.0 - High-performance document database
//!
//! Main server process providing TCP protocol access to VedDB's
//! document storage, caching, and query capabilities.

use anyhow::Result;
use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::RwLock;
use tracing::info;
use tracing_subscriber::EnvFilter;
use veddb_core::{
    AuthSystem, JwtService,
    CacheConfig, PersistentLayer, HybridStorageEngine,
    ConnectionManager,
};

#[derive(Parser, Debug)]
#[command(name = "veddb-server")]
#[command(about = "VedDB v0.2.0 - High-performance document database")]
#[command(version = "0.2.0")]
struct Args {
    /// Data directory path
    #[arg(short = 'D', long, default_value = "./veddb_data")]
    data_dir: PathBuf,

    /// TCP server bind address
    #[arg(short = 'H', long, default_value = "0.0.0.0")]
    host: String,

    /// TCP server port
    #[arg(short = 'p', long, default_value = "50051")]
    port: u16,

    /// Cache size in MB
    #[arg(short = 'c', long, default_value = "256")]
    cache_size_mb: usize,

    /// Enable debug logging
    #[arg(short = 'd', long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.debug { "debug" } else { "info" };
    let env_filter = EnvFilter::new(format!(
        "veddb_server={},veddb_core={}",
        log_level, log_level
    ));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    info!("╔═══════════════════════════════════════════════════════════╗");
    info!("║           VedDB Server v0.2.0 Starting                    ║");
    info!("╚═══════════════════════════════════════════════════════════╝");
    info!("");
    info!("Configuration:");
    info!("  • Data Directory: {}", args.data_dir.display());
    info!("  • Listen Address: {}:{}", args.host, args.port);
    info!("  • Cache Size: {}MB", args.cache_size_mb);
    info!("  • Debug Mode: {}", args.debug);
    info!("");

    // Create data directory if it doesn't exist
    std::fs::create_dir_all(&args.data_dir)?;

    info!("Initializing storage engine...");

    // Initialize cache configuration
    let mut cache_config = CacheConfig::default();
    cache_config.max_size_bytes = args.cache_size_mb * 1024 * 1024;

    // Initialize persistent layer
    let persistent_layer = Arc::new(PersistentLayer::new(&args.data_dir)?);

    // Initialize hybrid storage engine
    let _storage = Arc::new(HybridStorageEngine::new(
        cache_config,
        persistent_layer,
    ));

    info!("✓ Storage engine initialized");
    info!("");

    // Initialize authentication system
    info!("Initializing authentication system...");
    let auth_db_path = args
        .data_dir
        .join("users.db")
        .to_string_lossy()
        .to_string();

    let jwt_secret = b"veddb-secret-key-change-in-production";
    let session_timeout_hours = 24;

    let mut auth_system_instance =
        AuthSystem::new(&auth_db_path, jwt_secret, session_timeout_hours)?;
    auth_system_instance.initialize().await?;

    let auth_system = Arc::new(RwLock::new(auth_system_instance));

    // Create JWT service for connection manager
    let jwt_service = Arc::new(JwtService::new(
        jwt_secret,
        session_timeout_hours,
    )?);

    info!("✓ Authentication system initialized");
    info!("");

    // Create connection manager
    let connection_manager = ConnectionManager::new(
        auth_system,
        jwt_service,
        None, // No TLS for now
    );

    // Parse bind address
    let bind_addr: SocketAddr =
        format!("{}:{}", args.host, args.port).parse()?;

    info!("Starting TCP server on {}...", bind_addr);
    info!("✓ TCP server started");
    info!("");
    info!("VedDB Server is ready to accept connections");
    info!("Press Ctrl+C to shutdown");
    info!("");

    // Start server in background task
    let server_handle = tokio::spawn(async move {
        connection_manager.listen(bind_addr).await
    });

    // Wait for shutdown signal
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("");
            info!("Received shutdown signal, stopping server...");
        }
        result = server_handle => {
            match result {
                Ok(Ok(())) => info!("Server completed normally"),
                Ok(Err(e)) => info!("Server error: {:?}", e),
                Err(e) => info!("Server task error: {}", e),
            }
        }
    }

    info!("Shutting down storage engine...");
    info!("✓ VedDB Server shutdown complete");

    Ok(())
}
