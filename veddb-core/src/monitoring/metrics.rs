//! Core metrics collection and aggregation
//!
//! Provides thread-safe metrics collection for all VedDB operations

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use dashmap::DashMap;
use parking_lot::RwLock;
use chrono::{DateTime, Utc};

/// Comprehensive server metrics tracking all aspects of VedDB performance
#[derive(Debug)]
pub struct ServerMetrics {
    // Operation counters
    pub total_operations: AtomicU64,
    pub read_operations: AtomicU64,
    pub write_operations: AtomicU64,
    pub delete_operations: AtomicU64,
    pub query_operations: AtomicU64,
    
    // Connection metrics
    pub active_connections: AtomicUsize,
    pub total_connections: AtomicU64,
    pub connection_errors: AtomicU64,
    
    // Memory metrics
    pub memory_usage_bytes: AtomicU64,
    pub cache_memory_bytes: AtomicU64,
    pub persistent_memory_bytes: AtomicU64,
    
    // Cache metrics
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub cache_evictions: AtomicU64,
    
    // Replication metrics
    pub replication_lag_ms: AtomicU64,
    pub replication_bytes_sent: AtomicU64,
    pub replication_bytes_received: AtomicU64,
    
    // Error counters
    pub authentication_failures: AtomicU64,
    pub authorization_failures: AtomicU64,
    pub query_errors: AtomicU64,
    pub storage_errors: AtomicU64,
    
    // Latency tracking
    pub latency_tracker: Arc<LatencyTracker>,
    
    // Per-collection metrics
    pub collection_metrics: DashMap<String, CollectionMetrics>,
    
    // Server start time
    pub start_time: DateTime<Utc>,
}

/// Tracks latency percentiles for different operation types
#[derive(Debug)]
pub struct LatencyTracker {
    // Circular buffers for recent latency samples
    read_latencies: RwLock<LatencyBuffer>,
    write_latencies: RwLock<LatencyBuffer>,
    query_latencies: RwLock<LatencyBuffer>,
    
    // Pre-computed percentiles (updated periodically)
    read_percentiles: RwLock<LatencyPercentiles>,
    write_percentiles: RwLock<LatencyPercentiles>,
    query_percentiles: RwLock<LatencyPercentiles>,
}

/// Circular buffer for storing recent latency measurements
#[derive(Debug)]
struct LatencyBuffer {
    samples: Vec<u64>, // Latency in microseconds
    index: usize,
    full: bool,
}

/// Pre-computed latency percentiles
#[derive(Debug, Clone)]
pub struct LatencyPercentiles {
    pub p50: Duration,
    pub p90: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub p999: Duration,
    pub max: Duration,
    pub last_updated: Instant,
}

/// Per-collection metrics
#[derive(Debug)]
pub struct CollectionMetrics {
    pub document_count: AtomicU64,
    pub total_size_bytes: AtomicU64,
    pub read_count: AtomicU64,
    pub write_count: AtomicU64,
    pub index_count: AtomicUsize,
    pub cache_hit_rate: AtomicU64, // Stored as percentage * 100
}

impl Clone for CollectionMetrics {
    fn clone(&self) -> Self {
        Self {
            document_count: AtomicU64::new(self.document_count.load(Ordering::Relaxed)),
            total_size_bytes: AtomicU64::new(self.total_size_bytes.load(Ordering::Relaxed)),
            read_count: AtomicU64::new(self.read_count.load(Ordering::Relaxed)),
            write_count: AtomicU64::new(self.write_count.load(Ordering::Relaxed)),
            index_count: AtomicUsize::new(self.index_count.load(Ordering::Relaxed)),
            cache_hit_rate: AtomicU64::new(self.cache_hit_rate.load(Ordering::Relaxed)),
        }
    }
}

/// Operation type for metrics tracking
#[derive(Debug, Clone, Copy)]
pub enum OperationType {
    Read,
    Write,
    Delete,
    Query,
}

/// Metrics snapshot for reporting
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub timestamp: DateTime<Utc>,
    pub uptime_seconds: u64,
    
    // Operations per second (calculated over last minute)
    pub ops_per_second: f64,
    pub reads_per_second: f64,
    pub writes_per_second: f64,
    pub queries_per_second: f64,
    
    // Connection metrics
    pub active_connections: usize,
    pub total_connections: u64,
    
    // Memory metrics (in bytes)
    pub memory_usage: u64,
    pub cache_memory: u64,
    pub persistent_memory: u64,
    
    // Cache performance
    pub cache_hit_rate: f64, // Percentage
    pub cache_evictions: u64,
    
    // Latency percentiles
    pub read_latency: LatencyPercentiles,
    pub write_latency: LatencyPercentiles,
    pub query_latency: LatencyPercentiles,
    
    // Replication metrics
    pub replication_lag_ms: u64,
    
    // Error rates
    pub error_rate: f64, // Percentage of operations that failed
}

impl ServerMetrics {
    /// Create a new metrics instance
    pub fn new() -> Self {
        Self {
            total_operations: AtomicU64::new(0),
            read_operations: AtomicU64::new(0),
            write_operations: AtomicU64::new(0),
            delete_operations: AtomicU64::new(0),
            query_operations: AtomicU64::new(0),
            
            active_connections: AtomicUsize::new(0),
            total_connections: AtomicU64::new(0),
            connection_errors: AtomicU64::new(0),
            
            memory_usage_bytes: AtomicU64::new(0),
            cache_memory_bytes: AtomicU64::new(0),
            persistent_memory_bytes: AtomicU64::new(0),
            
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            cache_evictions: AtomicU64::new(0),
            
            replication_lag_ms: AtomicU64::new(0),
            replication_bytes_sent: AtomicU64::new(0),
            replication_bytes_received: AtomicU64::new(0),
            
            authentication_failures: AtomicU64::new(0),
            authorization_failures: AtomicU64::new(0),
            query_errors: AtomicU64::new(0),
            storage_errors: AtomicU64::new(0),
            
            latency_tracker: Arc::new(LatencyTracker::new()),
            collection_metrics: DashMap::new(),
            start_time: Utc::now(),
        }
    }
    
    /// Record an operation with its latency
    pub fn record_operation(&self, op_type: OperationType, latency: Duration) {
        // Increment operation counters
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        
        match op_type {
            OperationType::Read => {
                self.read_operations.fetch_add(1, Ordering::Relaxed);
                self.latency_tracker.record_read_latency(latency);
            }
            OperationType::Write => {
                self.write_operations.fetch_add(1, Ordering::Relaxed);
                self.latency_tracker.record_write_latency(latency);
            }
            OperationType::Delete => {
                self.delete_operations.fetch_add(1, Ordering::Relaxed);
                self.latency_tracker.record_write_latency(latency);
            }
            OperationType::Query => {
                self.query_operations.fetch_add(1, Ordering::Relaxed);
                self.latency_tracker.record_query_latency(latency);
            }
        }
    }
    
    /// Record a cache hit
    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record a cache miss
    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record a cache eviction
    pub fn record_cache_eviction(&self) {
        self.cache_evictions.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record a new connection
    pub fn record_connection_opened(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
        self.total_connections.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record a closed connection
    pub fn record_connection_closed(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }
    
    /// Record a connection error
    pub fn record_connection_error(&self) {
        self.connection_errors.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Update memory usage
    pub fn update_memory_usage(&self, total: u64, cache: u64, persistent: u64) {
        self.memory_usage_bytes.store(total, Ordering::Relaxed);
        self.cache_memory_bytes.store(cache, Ordering::Relaxed);
        self.persistent_memory_bytes.store(persistent, Ordering::Relaxed);
    }
    
    /// Update replication lag
    pub fn update_replication_lag(&self, lag_ms: u64) {
        self.replication_lag_ms.store(lag_ms, Ordering::Relaxed);
    }
    
    /// Record authentication failure
    pub fn record_auth_failure(&self) {
        self.authentication_failures.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record authorization failure
    pub fn record_authz_failure(&self) {
        self.authorization_failures.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record query error
    pub fn record_query_error(&self) {
        self.query_errors.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record storage error
    pub fn record_storage_error(&self) {
        self.storage_errors.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Get or create collection metrics
    pub fn get_collection_metrics(&self, collection: &str) -> CollectionMetrics {
        self.collection_metrics
            .entry(collection.to_string())
            .or_insert_with(|| CollectionMetrics::new())
            .clone()
    }
    
    /// Generate a metrics snapshot
    pub fn snapshot(&self) -> MetricsSnapshot {
        let now = Utc::now();
        let uptime = (now - self.start_time).num_seconds() as u64;
        
        // Calculate operations per second (simple approximation)
        let total_ops = self.total_operations.load(Ordering::Relaxed);
        let ops_per_second = if uptime > 0 { total_ops as f64 / uptime as f64 } else { 0.0 };
        
        let reads = self.read_operations.load(Ordering::Relaxed);
        let writes = self.write_operations.load(Ordering::Relaxed);
        let queries = self.query_operations.load(Ordering::Relaxed);
        
        let reads_per_second = if uptime > 0 { reads as f64 / uptime as f64 } else { 0.0 };
        let writes_per_second = if uptime > 0 { writes as f64 / uptime as f64 } else { 0.0 };
        let queries_per_second = if uptime > 0 { queries as f64 / uptime as f64 } else { 0.0 };
        
        // Calculate cache hit rate
        let hits = self.cache_hits.load(Ordering::Relaxed);
        let misses = self.cache_misses.load(Ordering::Relaxed);
        let cache_hit_rate = if hits + misses > 0 {
            hits as f64 / (hits + misses) as f64 * 100.0
        } else {
            0.0
        };
        
        // Calculate error rate
        let total_errors = self.authentication_failures.load(Ordering::Relaxed)
            + self.authorization_failures.load(Ordering::Relaxed)
            + self.query_errors.load(Ordering::Relaxed)
            + self.storage_errors.load(Ordering::Relaxed);
        
        let error_rate = if total_ops > 0 {
            total_errors as f64 / total_ops as f64 * 100.0
        } else {
            0.0
        };
        
        MetricsSnapshot {
            timestamp: now,
            uptime_seconds: uptime,
            ops_per_second,
            reads_per_second,
            writes_per_second,
            queries_per_second,
            active_connections: self.active_connections.load(Ordering::Relaxed),
            total_connections: self.total_connections.load(Ordering::Relaxed),
            memory_usage: self.memory_usage_bytes.load(Ordering::Relaxed),
            cache_memory: self.cache_memory_bytes.load(Ordering::Relaxed),
            persistent_memory: self.persistent_memory_bytes.load(Ordering::Relaxed),
            cache_hit_rate,
            cache_evictions: self.cache_evictions.load(Ordering::Relaxed),
            read_latency: self.latency_tracker.get_read_percentiles(),
            write_latency: self.latency_tracker.get_write_percentiles(),
            query_latency: self.latency_tracker.get_query_percentiles(),
            replication_lag_ms: self.replication_lag_ms.load(Ordering::Relaxed),
            error_rate,
        }
    }
}

impl LatencyTracker {
    const BUFFER_SIZE: usize = 10000; // Keep last 10k samples
    
    pub fn new() -> Self {
        Self {
            read_latencies: RwLock::new(LatencyBuffer::new()),
            write_latencies: RwLock::new(LatencyBuffer::new()),
            query_latencies: RwLock::new(LatencyBuffer::new()),
            read_percentiles: RwLock::new(LatencyPercentiles::default()),
            write_percentiles: RwLock::new(LatencyPercentiles::default()),
            query_percentiles: RwLock::new(LatencyPercentiles::default()),
        }
    }
    
    pub fn record_read_latency(&self, latency: Duration) {
        let mut buffer = self.read_latencies.write();
        buffer.add_sample(latency.as_micros() as u64);
    }
    
    pub fn record_write_latency(&self, latency: Duration) {
        let mut buffer = self.write_latencies.write();
        buffer.add_sample(latency.as_micros() as u64);
    }
    
    pub fn record_query_latency(&self, latency: Duration) {
        let mut buffer = self.query_latencies.write();
        buffer.add_sample(latency.as_micros() as u64);
    }
    
    pub fn get_read_percentiles(&self) -> LatencyPercentiles {
        self.read_percentiles.read().clone()
    }
    
    pub fn get_write_percentiles(&self) -> LatencyPercentiles {
        self.write_percentiles.read().clone()
    }
    
    pub fn get_query_percentiles(&self) -> LatencyPercentiles {
        self.query_percentiles.read().clone()
    }
    
    /// Update percentiles from current samples (should be called periodically)
    pub fn update_percentiles(&self) {
        self.update_read_percentiles();
        self.update_write_percentiles();
        self.update_query_percentiles();
    }
    
    fn update_read_percentiles(&self) {
        let buffer = self.read_latencies.read();
        let percentiles = buffer.calculate_percentiles();
        *self.read_percentiles.write() = percentiles;
    }
    
    fn update_write_percentiles(&self) {
        let buffer = self.write_latencies.read();
        let percentiles = buffer.calculate_percentiles();
        *self.write_percentiles.write() = percentiles;
    }
    
    fn update_query_percentiles(&self) {
        let buffer = self.query_latencies.read();
        let percentiles = buffer.calculate_percentiles();
        *self.query_percentiles.write() = percentiles;
    }
}

impl LatencyBuffer {
    fn new() -> Self {
        Self {
            samples: vec![0; LatencyTracker::BUFFER_SIZE],
            index: 0,
            full: false,
        }
    }
    
    fn add_sample(&mut self, latency_us: u64) {
        self.samples[self.index] = latency_us;
        self.index = (self.index + 1) % self.samples.len();
        if self.index == 0 {
            self.full = true;
        }
    }
    
    fn calculate_percentiles(&self) -> LatencyPercentiles {
        let mut samples: Vec<u64> = if self.full {
            self.samples.clone()
        } else {
            self.samples[..self.index].to_vec()
        };
        
        if samples.is_empty() {
            return LatencyPercentiles::default();
        }
        
        samples.sort_unstable();
        
        let len = samples.len();
        let p50_idx = (len as f64 * 0.50) as usize;
        let p90_idx = (len as f64 * 0.90) as usize;
        let p95_idx = (len as f64 * 0.95) as usize;
        let p99_idx = (len as f64 * 0.99) as usize;
        let p999_idx = (len as f64 * 0.999) as usize;
        
        LatencyPercentiles {
            p50: Duration::from_micros(samples[p50_idx.min(len - 1)]),
            p90: Duration::from_micros(samples[p90_idx.min(len - 1)]),
            p95: Duration::from_micros(samples[p95_idx.min(len - 1)]),
            p99: Duration::from_micros(samples[p99_idx.min(len - 1)]),
            p999: Duration::from_micros(samples[p999_idx.min(len - 1)]),
            max: Duration::from_micros(samples[len - 1]),
            last_updated: Instant::now(),
        }
    }
}

impl Default for LatencyPercentiles {
    fn default() -> Self {
        Self {
            p50: Duration::ZERO,
            p90: Duration::ZERO,
            p95: Duration::ZERO,
            p99: Duration::ZERO,
            p999: Duration::ZERO,
            max: Duration::ZERO,
            last_updated: Instant::now(),
        }
    }
}

impl CollectionMetrics {
    pub fn new() -> Self {
        Self {
            document_count: AtomicU64::new(0),
            total_size_bytes: AtomicU64::new(0),
            read_count: AtomicU64::new(0),
            write_count: AtomicU64::new(0),
            index_count: AtomicUsize::new(0),
            cache_hit_rate: AtomicU64::new(0),
        }
    }
    
    pub fn record_read(&self) {
        self.read_count.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_write(&self) {
        self.write_count.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn update_document_count(&self, count: u64) {
        self.document_count.store(count, Ordering::Relaxed);
    }
    
    pub fn update_size(&self, size_bytes: u64) {
        self.total_size_bytes.store(size_bytes, Ordering::Relaxed);
    }
    
    pub fn update_index_count(&self, count: usize) {
        self.index_count.store(count, Ordering::Relaxed);
    }
}

/// Global metrics instance
static METRICS: std::sync::OnceLock<Arc<ServerMetrics>> = std::sync::OnceLock::new();

/// Initialize global metrics
pub fn init_metrics() -> Arc<ServerMetrics> {
    let metrics = Arc::new(ServerMetrics::new());
    METRICS.set(metrics.clone()).expect("Metrics already initialized");
    
    // Start background task to update percentiles
    let metrics_clone = metrics.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            metrics_clone.latency_tracker.update_percentiles();
        }
    });
    
    metrics
}

/// Get global metrics instance
pub fn get_metrics() -> &'static Arc<ServerMetrics> {
    METRICS.get().expect("Metrics not initialized")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    
    #[test]
    fn test_metrics_creation() {
        let metrics = ServerMetrics::new();
        assert_eq!(metrics.total_operations.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.active_connections.load(Ordering::Relaxed), 0);
    }
    
    #[test]
    fn test_operation_recording() {
        let metrics = ServerMetrics::new();
        
        metrics.record_operation(OperationType::Read, Duration::from_millis(1));
        metrics.record_operation(OperationType::Write, Duration::from_millis(2));
        
        assert_eq!(metrics.total_operations.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.read_operations.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.write_operations.load(Ordering::Relaxed), 1);
    }
    
    #[test]
    fn test_cache_metrics() {
        let metrics = ServerMetrics::new();
        
        metrics.record_cache_hit();
        metrics.record_cache_hit();
        metrics.record_cache_miss();
        
        let snapshot = metrics.snapshot();
        assert!((snapshot.cache_hit_rate - 66.66666666666667).abs() < 0.001); // 2/3 * 100
    }
    
    #[test]
    fn test_latency_buffer() {
        let mut buffer = LatencyBuffer::new();
        
        // Add some samples
        for i in 1..=100 {
            buffer.add_sample(i * 1000); // 1ms, 2ms, ..., 100ms in microseconds
        }
        
        let percentiles = buffer.calculate_percentiles();
        
        // Check that percentiles are reasonable
        assert!(percentiles.p50.as_millis() > 0);
        assert!(percentiles.p99 > percentiles.p95);
        assert!(percentiles.p95 > percentiles.p90);
        assert!(percentiles.p90 > percentiles.p50);
    }
}