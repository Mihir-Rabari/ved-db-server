//! Prometheus metrics exporter
//!
//! Provides HTTP endpoint for Prometheus to scrape VedDB metrics

use crate::monitoring::metrics::ServerMetrics;
use prometheus::{
    Counter, Gauge, Histogram, HistogramOpts, Opts, Registry, Encoder, TextEncoder,
    register_counter_with_registry, register_gauge_with_registry, register_histogram_with_registry,
};
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use anyhow::Result;
use tracing::{info, error};

/// Prometheus metrics exporter
pub struct PrometheusExporter {
    registry: Registry,
    metrics: PrometheusMetrics,
    server_metrics: Arc<ServerMetrics>,
    port: u16,
}

/// Prometheus metric instances
struct PrometheusMetrics {
    // Operation counters
    total_operations: Counter,
    read_operations: Counter,
    write_operations: Counter,
    delete_operations: Counter,
    query_operations: Counter,
    
    // Connection metrics
    active_connections: Gauge,
    total_connections: Counter,
    connection_errors: Counter,
    
    // Memory metrics
    memory_usage_bytes: Gauge,
    cache_memory_bytes: Gauge,
    persistent_memory_bytes: Gauge,
    
    // Cache metrics
    cache_hits: Counter,
    cache_misses: Counter,
    cache_evictions: Counter,
    cache_hit_rate: Gauge,
    
    // Replication metrics
    replication_lag_ms: Gauge,
    replication_bytes_sent: Counter,
    replication_bytes_received: Counter,
    
    // Error counters
    authentication_failures: Counter,
    authorization_failures: Counter,
    query_errors: Counter,
    storage_errors: Counter,
    
    // Latency histograms
    read_latency_histogram: Histogram,
    write_latency_histogram: Histogram,
    query_latency_histogram: Histogram,
    
    // Server info
    uptime_seconds: Gauge,
    server_info: Gauge,
}

impl PrometheusExporter {
    /// Create a new Prometheus exporter
    pub fn new(server_metrics: Arc<ServerMetrics>, port: u16) -> Result<Self> {
        let registry = Registry::new();
        let metrics = PrometheusMetrics::new(&registry)?;
        
        Ok(Self {
            registry,
            metrics,
            server_metrics,
            port,
        })
    }
    
    /// Start the Prometheus HTTP server
    pub async fn start(&self) -> Result<()> {
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        
        info!("Prometheus metrics server listening on {}", addr);
        
        loop {
            match listener.accept().await {
                Ok((mut stream, addr)) => {
                    let registry = self.registry.clone();
                    let server_metrics = self.server_metrics.clone();
                    
                    tokio::spawn(async move {
                        if let Err(e) = handle_metrics_request(&mut stream, &registry, &server_metrics).await {
                            error!("Error handling metrics request from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                }
            }
        }
    }
    
    /// Update Prometheus metrics from server metrics
    pub fn update_metrics(&self) {
        let snapshot = self.server_metrics.snapshot();
        
        // Update counters (these are cumulative)
        self.metrics.total_operations.reset();
        self.metrics.total_operations.inc_by(snapshot.total_connections as f64);
        
        // Update gauges (these are current values)
        self.metrics.active_connections.set(snapshot.active_connections as f64);
        self.metrics.memory_usage_bytes.set(snapshot.memory_usage as f64);
        self.metrics.cache_memory_bytes.set(snapshot.cache_memory as f64);
        self.metrics.persistent_memory_bytes.set(snapshot.persistent_memory as f64);
        self.metrics.cache_hit_rate.set(snapshot.cache_hit_rate);
        self.metrics.replication_lag_ms.set(snapshot.replication_lag_ms as f64);
        self.metrics.uptime_seconds.set(snapshot.uptime_seconds as f64);
        
        // Update latency histograms with current percentiles
        // Note: In a real implementation, we'd observe individual latencies
        // Here we're approximating by observing the percentile values
        self.observe_latency_percentiles(&self.metrics.read_latency_histogram, &snapshot.read_latency);
        self.observe_latency_percentiles(&self.metrics.write_latency_histogram, &snapshot.write_latency);
        self.observe_latency_percentiles(&self.metrics.query_latency_histogram, &snapshot.query_latency);
    }
    
    fn observe_latency_percentiles(&self, histogram: &Histogram, percentiles: &crate::monitoring::metrics::LatencyPercentiles) {
        // This is a simplified approach - in production, we'd observe actual latencies
        histogram.observe(percentiles.p50.as_secs_f64());
        histogram.observe(percentiles.p90.as_secs_f64());
        histogram.observe(percentiles.p95.as_secs_f64());
        histogram.observe(percentiles.p99.as_secs_f64());
    }
}

impl PrometheusMetrics {
    fn new(registry: &Registry) -> Result<Self> {
        // Operation counters
        let total_operations = register_counter_with_registry!(
            Opts::new("veddb_operations_total", "Total number of operations"),
            registry
        )?;
        
        let read_operations = register_counter_with_registry!(
            Opts::new("veddb_read_operations_total", "Total number of read operations"),
            registry
        )?;
        
        let write_operations = register_counter_with_registry!(
            Opts::new("veddb_write_operations_total", "Total number of write operations"),
            registry
        )?;
        
        let delete_operations = register_counter_with_registry!(
            Opts::new("veddb_delete_operations_total", "Total number of delete operations"),
            registry
        )?;
        
        let query_operations = register_counter_with_registry!(
            Opts::new("veddb_query_operations_total", "Total number of query operations"),
            registry
        )?;
        
        // Connection metrics
        let active_connections = register_gauge_with_registry!(
            Opts::new("veddb_active_connections", "Number of active client connections"),
            registry
        )?;
        
        let total_connections = register_counter_with_registry!(
            Opts::new("veddb_connections_total", "Total number of connections established"),
            registry
        )?;
        
        let connection_errors = register_counter_with_registry!(
            Opts::new("veddb_connection_errors_total", "Total number of connection errors"),
            registry
        )?;
        
        // Memory metrics
        let memory_usage_bytes = register_gauge_with_registry!(
            Opts::new("veddb_memory_usage_bytes", "Total memory usage in bytes"),
            registry
        )?;
        
        let cache_memory_bytes = register_gauge_with_registry!(
            Opts::new("veddb_cache_memory_bytes", "Cache memory usage in bytes"),
            registry
        )?;
        
        let persistent_memory_bytes = register_gauge_with_registry!(
            Opts::new("veddb_persistent_memory_bytes", "Persistent storage memory usage in bytes"),
            registry
        )?;
        
        // Cache metrics
        let cache_hits = register_counter_with_registry!(
            Opts::new("veddb_cache_hits_total", "Total number of cache hits"),
            registry
        )?;
        
        let cache_misses = register_counter_with_registry!(
            Opts::new("veddb_cache_misses_total", "Total number of cache misses"),
            registry
        )?;
        
        let cache_evictions = register_counter_with_registry!(
            Opts::new("veddb_cache_evictions_total", "Total number of cache evictions"),
            registry
        )?;
        
        let cache_hit_rate = register_gauge_with_registry!(
            Opts::new("veddb_cache_hit_rate", "Cache hit rate percentage"),
            registry
        )?;
        
        // Replication metrics
        let replication_lag_ms = register_gauge_with_registry!(
            Opts::new("veddb_replication_lag_milliseconds", "Replication lag in milliseconds"),
            registry
        )?;
        
        let replication_bytes_sent = register_counter_with_registry!(
            Opts::new("veddb_replication_bytes_sent_total", "Total bytes sent for replication"),
            registry
        )?;
        
        let replication_bytes_received = register_counter_with_registry!(
            Opts::new("veddb_replication_bytes_received_total", "Total bytes received for replication"),
            registry
        )?;
        
        // Error counters
        let authentication_failures = register_counter_with_registry!(
            Opts::new("veddb_authentication_failures_total", "Total authentication failures"),
            registry
        )?;
        
        let authorization_failures = register_counter_with_registry!(
            Opts::new("veddb_authorization_failures_total", "Total authorization failures"),
            registry
        )?;
        
        let query_errors = register_counter_with_registry!(
            Opts::new("veddb_query_errors_total", "Total query errors"),
            registry
        )?;
        
        let storage_errors = register_counter_with_registry!(
            Opts::new("veddb_storage_errors_total", "Total storage errors"),
            registry
        )?;
        
        // Latency histograms
        let read_latency_histogram = register_histogram_with_registry!(
            HistogramOpts::new("veddb_read_latency_seconds", "Read operation latency in seconds")
                .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
            registry
        )?;
        
        let write_latency_histogram = register_histogram_with_registry!(
            HistogramOpts::new("veddb_write_latency_seconds", "Write operation latency in seconds")
                .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
            registry
        )?;
        
        let query_latency_histogram = register_histogram_with_registry!(
            HistogramOpts::new("veddb_query_latency_seconds", "Query operation latency in seconds")
                .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
            registry
        )?;
        
        // Server info
        let uptime_seconds = register_gauge_with_registry!(
            Opts::new("veddb_uptime_seconds", "Server uptime in seconds"),
            registry
        )?;
        
        let server_info = register_gauge_with_registry!(
            Opts::new("veddb_server_info", "Server information")
                .const_label("version", env!("CARGO_PKG_VERSION")),
            registry
        )?;
        server_info.set(1.0);
        
        Ok(Self {
            total_operations,
            read_operations,
            write_operations,
            delete_operations,
            query_operations,
            active_connections,
            total_connections,
            connection_errors,
            memory_usage_bytes,
            cache_memory_bytes,
            persistent_memory_bytes,
            cache_hits,
            cache_misses,
            cache_evictions,
            cache_hit_rate,
            replication_lag_ms,
            replication_bytes_sent,
            replication_bytes_received,
            authentication_failures,
            authorization_failures,
            query_errors,
            storage_errors,
            read_latency_histogram,
            write_latency_histogram,
            query_latency_histogram,
            uptime_seconds,
            server_info,
        })
    }
}

/// Handle HTTP request for metrics
async fn handle_metrics_request(
    stream: &mut tokio::net::TcpStream,
    registry: &Registry,
    _server_metrics: &Arc<ServerMetrics>,
) -> Result<()> {
    // Read the HTTP request (simplified - just read until double CRLF)
    let mut buffer = vec![0; 1024];
    let _bytes_read = stream.read(&mut buffer).await?;
    
    // Generate metrics
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    let mut metrics_output = Vec::new();
    encoder.encode(&metric_families, &mut metrics_output)?;
    
    // Send HTTP response
    let response = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: text/plain; version=0.0.4; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         \r\n{}",
        metrics_output.len(),
        String::from_utf8_lossy(&metrics_output)
    );
    
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;
    
    Ok(())
}

/// Start Prometheus metrics server
pub async fn start_prometheus_server(
    server_metrics: Arc<ServerMetrics>,
    port: u16,
) -> Result<()> {
    let exporter = PrometheusExporter::new(server_metrics.clone(), port)?;
    
    // Start background task to update metrics
    let exporter_clone = Arc::new(exporter);
    let update_exporter = exporter_clone.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            update_exporter.update_metrics();
        }
    });
    
    // Start the HTTP server
    exporter_clone.start().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;
    
    #[tokio::test]
    async fn test_prometheus_exporter_creation() {
        let metrics = Arc::new(ServerMetrics::new());
        let exporter = PrometheusExporter::new(metrics, 9090);
        assert!(exporter.is_ok());
    }
    
    #[tokio::test]
    async fn test_metrics_update() {
        let metrics = Arc::new(ServerMetrics::new());
        let exporter = PrometheusExporter::new(metrics.clone(), 9090).unwrap();
        
        // Record some operations
        metrics.record_operation(crate::monitoring::metrics::OperationType::Read, Duration::from_millis(1));
        metrics.record_cache_hit();
        
        // Update metrics (should not panic)
        exporter.update_metrics();
    }
    
    #[tokio::test]
    async fn test_metrics_endpoint() {
        let metrics = Arc::new(ServerMetrics::new());
        
        // Start server in background
        let server_handle = tokio::spawn(async move {
            if let Err(e) = start_prometheus_server(metrics, 19090).await {
                eprintln!("Server error: {}", e);
            }
        });
        
        // Give server time to start
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Try to connect (this is a basic test - in practice we'd make an HTTP request)
        let connect_result = timeout(
            Duration::from_millis(100),
            tokio::net::TcpStream::connect("127.0.0.1:19090")
        ).await;
        
        // Clean up
        server_handle.abort();
        
        // The connection should succeed (server is listening)
        assert!(connect_result.is_ok());
    }
}