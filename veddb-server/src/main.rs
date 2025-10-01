//! VedDB Server - High-performance shared memory KV store with Pub/Sub
//!
//! Main server process that manages worker threads, handles client sessions,
//! and provides gRPC endpoints for remote access.

use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;
use veddb_core::{VedDb, VedDbConfig};

mod server;
mod worker;

use server::VedDbServer;
use worker::WorkerPool;

#[derive(Parser, Debug)]
#[command(name = "veddb-server")]
#[command(about = "VedDB - High-performance shared memory KV store + Pub/Sub")]
struct Args {
    /// Shared memory name/identifier
    #[arg(short, long, default_value = "veddb_main")]
    name: String,

    /// Memory size in MB
    #[arg(short, long, default_value = "64")]
    memory_mb: usize,

    /// Number of worker threads
    #[arg(short, long, default_value = "4")]
    workers: usize,

    /// gRPC server port
    #[arg(short, long, default_value = "50051")]
    port: u16,

    /// Session timeout in seconds
    #[arg(short, long, default_value = "300")]
    session_timeout: u64,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,

    /// Create new instance (vs opening existing)
    #[arg(long)]
    create: bool,
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
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    info!("Starting VedDB Server");
    info!(
        "Memory: {}MB, Workers: {}, Port: {}",
        args.memory_mb, args.workers, args.port
    );

    // Create VedDB configuration
    let config = VedDbConfig {
        memory_size: args.memory_mb * 1024 * 1024,
        session_timeout_secs: args.session_timeout,
        ..Default::default()
    };

    // Initialize VedDB instance
    let veddb = if args.create {
        info!("Creating new VedDB instance: {}", args.name);
        VedDb::create(&args.name, config)?
    } else {
        info!("Opening existing VedDB instance: {}", args.name);
        match VedDb::open(&args.name) {
            Ok(db) => db,
            Err(_) => {
                warn!("Failed to open existing instance, creating new one");
                VedDb::create(&args.name, config)?
            }
        }
    };

    let veddb = Arc::new(veddb);

    // Print initial statistics
    let stats = veddb.get_stats();
    info!(
        "VedDB initialized - Memory: {:.1}MB used / {:.1}MB total",
        stats.memory_used as f64 / (1024.0 * 1024.0),
        stats.memory_size as f64 / (1024.0 * 1024.0)
    );

    // Start worker pool
    info!("Starting {} worker threads", args.workers);
    let worker_pool = WorkerPool::new(veddb.clone(), args.workers).await?;

    // Start gRPC server
    info!("Starting gRPC server on port {}", args.port);
    let grpc_server = VedDbServer::new(veddb.clone());
    let grpc_handle = tokio::spawn(async move { grpc_server.serve(args.port).await });

    // Start cleanup task
    let cleanup_veddb = veddb.clone();
    let cleanup_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            let cleaned = cleanup_veddb.cleanup_stale_sessions();
            if cleaned > 0 {
                info!("Cleaned up {} stale sessions", cleaned);
            }
        }
    });

    // Start stats reporting task
    let stats_veddb = veddb.clone();
    let stats_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            let stats = stats_veddb.get_stats();
            info!(
                "Stats - Ops: {}, Keys: {}, Sessions: {}, Topics: {}, Memory: {:.1}MB",
                stats.total_operations,
                stats.kv_keys,
                stats.active_sessions,
                stats.active_topics,
                stats.memory_used as f64 / (1024.0 * 1024.0)
            );
        }
    });

    // Wait for shutdown signal
    info!("VedDB Server running. Press Ctrl+C to shutdown.");

    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down...");
        }
        result = grpc_handle => {
            match result {
                Ok(Ok(())) => info!("gRPC server completed"),
                Ok(Err(e)) => error!("gRPC server error: {}", e),
                Err(e) => error!("gRPC server task error: {}", e),
            }
        }
    }

    // Graceful shutdown
    info!("Shutting down worker pool...");
    worker_pool.shutdown().await;

    cleanup_handle.abort();
    stats_handle.abort();

    // Final statistics
    let final_stats = veddb.get_stats();
    info!(
        "Final stats - Total operations: {}, Uptime: {}s",
        final_stats.total_operations, final_stats.uptime_secs
    );

    info!("VedDB Server shutdown complete");
    Ok(())
}
