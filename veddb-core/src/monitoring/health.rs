//! Health check endpoint and system health monitoring
//!
//! Provides HTTP endpoint for health checks and monitors system health

use crate::monitoring::metrics::ServerMetrics;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use anyhow::Result;
use tracing::{info, error};
use chrono::{DateTime, Utc};

/// Health check status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResponse {
    pub status: HealthStatus,
    pub timestamp: DateTime<Utc>,
    pub version: String,
    pub uptime_seconds: u64,
    pub checks: Vec<ComponentHealth>,
}

/// Individual component health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub component: String,
    pub status: HealthStatus,
    pub message: Option<String>,
    pub last_check: DateTime<Utc>,
}

/// Health checker
pub struct HealthChecker {
    server_metrics: Arc<ServerMetrics>,
    port: u16,
}

impl HealthChecker {
    /// Create a new health checker
    pub fn new(server_metrics: Arc<ServerMetrics>, port: u16) -> Self {
        Self {
            server_metrics,
            port,
        }
    }
    
    /// Start the health check HTTP server
    pub async fn start(&self) -> Result<()> {
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        
        info!("Health check server listening on {}", addr);
        
        loop {
            match listener.accept().await {
                Ok((mut stream, addr)) => {
                    let server_metrics = self.server_metrics.clone();
                    
                    tokio::spawn(async move {
                        if let Err(e) = handle_health_request(&mut stream, &server_metrics).await {
                            error!("Error handling health request from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting health check connection: {}", e);
                }
            }
        }
    }
    
    /// Perform comprehensive health check
    pub fn check_health(&self) -> HealthCheckResponse {
        let snapshot = self.server_metrics.snapshot();
        let now = Utc::now();
        
        let mut checks = Vec::new();
        let mut overall_status = HealthStatus::Healthy;
        
        // Check memory usage
        let memory_check = self.check_memory_health(&snapshot);
        if matches!(memory_check.status, HealthStatus::Degraded | HealthStatus::Unhealthy) {
            overall_status = memory_check.status.clone();
        }
        checks.push(memory_check);
        
        // Check error rates
        let error_check = self.check_error_rates(&snapshot);
        if matches!(error_check.status, HealthStatus::Degraded | HealthStatus::Unhealthy) {
            overall_status = error_check.status.clone();
        }
        checks.push(error_check);
        
        // Check latency
        let latency_check = self.check_latency_health(&snapshot);
        if matches!(latency_check.status, HealthStatus::Degraded | HealthStatus::Unhealthy) {
            overall_status = latency_check.status.clone();
        }
        checks.push(latency_check);
        
        // Check replication lag
        let replication_check = self.check_replication_health(&snapshot);
        if matches!(replication_check.status, HealthStatus::Degraded | HealthStatus::Unhealthy) {
            overall_status = replication_check.status.clone();
        }
        checks.push(replication_check);
        
        // Check cache performance
        let cache_check = self.check_cache_health(&snapshot);
        if matches!(cache_check.status, HealthStatus::Degraded | HealthStatus::Unhealthy) {
            overall_status = cache_check.status.clone();
        }
        checks.push(cache_check);
        
        HealthCheckResponse {
            status: overall_status,
            timestamp: now,
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: snapshot.uptime_seconds,
            checks,
        }
    }
    
    fn check_memory_health(&self, snapshot: &crate::monitoring::metrics::MetricsSnapshot) -> ComponentHealth {
        let memory_gb = snapshot.memory_usage as f64 / (1024.0 * 1024.0 * 1024.0);
        
        let (status, message) = if memory_gb > 8.0 {
            (HealthStatus::Unhealthy, Some(format!("High memory usage: {:.2} GB", memory_gb)))
        } else if memory_gb > 4.0 {
            (HealthStatus::Degraded, Some(format!("Elevated memory usage: {:.2} GB", memory_gb)))
        } else {
            (HealthStatus::Healthy, Some(format!("Memory usage: {:.2} GB", memory_gb)))
        };
        
        ComponentHealth {
            component: "memory".to_string(),
            status,
            message,
            last_check: Utc::now(),
        }
    }
    
    fn check_error_rates(&self, snapshot: &crate::monitoring::metrics::MetricsSnapshot) -> ComponentHealth {
        let error_rate = snapshot.error_rate;
        
        let (status, message) = if error_rate > 10.0 {
            (HealthStatus::Unhealthy, Some(format!("High error rate: {:.2}%", error_rate)))
        } else if error_rate > 5.0 {
            (HealthStatus::Degraded, Some(format!("Elevated error rate: {:.2}%", error_rate)))
        } else {
            (HealthStatus::Healthy, Some(format!("Error rate: {:.2}%", error_rate)))
        };
        
        ComponentHealth {
            component: "errors".to_string(),
            status,
            message,
            last_check: Utc::now(),
        }
    }
    
    fn check_latency_health(&self, snapshot: &crate::monitoring::metrics::MetricsSnapshot) -> ComponentHealth {
        let p99_ms = snapshot.read_latency.p99.as_millis() as f64;
        
        let (status, message) = if p99_ms > 100.0 {
            (HealthStatus::Unhealthy, Some(format!("High latency: {:.2}ms p99", p99_ms)))
        } else if p99_ms > 50.0 {
            (HealthStatus::Degraded, Some(format!("Elevated latency: {:.2}ms p99", p99_ms)))
        } else {
            (HealthStatus::Healthy, Some(format!("Latency: {:.2}ms p99", p99_ms)))
        };
        
        ComponentHealth {
            component: "latency".to_string(),
            status,
            message,
            last_check: Utc::now(),
        }
    }
    
    fn check_replication_health(&self, snapshot: &crate::monitoring::metrics::MetricsSnapshot) -> ComponentHealth {
        let lag_ms = snapshot.replication_lag_ms;
        
        let (status, message) = if lag_ms > 5000 {
            (HealthStatus::Unhealthy, Some(format!("High replication lag: {}ms", lag_ms)))
        } else if lag_ms > 1000 {
            (HealthStatus::Degraded, Some(format!("Elevated replication lag: {}ms", lag_ms)))
        } else {
            (HealthStatus::Healthy, Some(format!("Replication lag: {}ms", lag_ms)))
        };
        
        ComponentHealth {
            component: "replication".to_string(),
            status,
            message,
            last_check: Utc::now(),
        }
    }
    
    fn check_cache_health(&self, snapshot: &crate::monitoring::metrics::MetricsSnapshot) -> ComponentHealth {
        let hit_rate = snapshot.cache_hit_rate;
        
        let (status, message) = if hit_rate < 50.0 {
            (HealthStatus::Degraded, Some(format!("Low cache hit rate: {:.1}%", hit_rate)))
        } else if hit_rate < 80.0 {
            (HealthStatus::Healthy, Some(format!("Cache hit rate: {:.1}%", hit_rate)))
        } else {
            (HealthStatus::Healthy, Some(format!("Good cache hit rate: {:.1}%", hit_rate)))
        };
        
        ComponentHealth {
            component: "cache".to_string(),
            status,
            message,
            last_check: Utc::now(),
        }
    }
}

/// Handle HTTP request for health check
async fn handle_health_request(
    stream: &mut tokio::net::TcpStream,
    server_metrics: &Arc<ServerMetrics>,
) -> Result<()> {
    // Read the HTTP request (simplified)
    let mut buffer = vec![0; 1024];
    let _bytes_read = stream.read(&mut buffer).await?;
    
    // Create health checker and get status
    let health_checker = HealthChecker::new(server_metrics.clone(), 0); // Port not used here
    let health_response = health_checker.check_health();
    
    // Serialize response
    let json_response = serde_json::to_string_pretty(&health_response)?;
    
    // Determine HTTP status code
    let status_code = match health_response.status {
        HealthStatus::Healthy => "200 OK",
        HealthStatus::Degraded => "200 OK", // Still operational
        HealthStatus::Unhealthy => "503 Service Unavailable",
    };
    
    // Send HTTP response
    let response = format!(
        "HTTP/1.1 {}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         \r\n{}",
        status_code,
        json_response.len(),
        json_response
    );
    
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;
    
    Ok(())
}

/// Start health check server
pub async fn start_health_server(
    server_metrics: Arc<ServerMetrics>,
    port: u16,
) -> Result<()> {
    let health_checker = HealthChecker::new(server_metrics, port);
    health_checker.start().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    
    #[test]
    fn test_health_checker_creation() {
        let metrics = Arc::new(ServerMetrics::new());
        let checker = HealthChecker::new(metrics, 8080);
        
        let health = checker.check_health();
        // Health status might be degraded due to default metrics, so just check it's not unhealthy
        assert!(!matches!(health.status, HealthStatus::Unhealthy));
        assert_eq!(health.version, env!("CARGO_PKG_VERSION"));
    }
    
    #[test]
    fn test_component_health_checks() {
        let metrics = Arc::new(ServerMetrics::new());
        let checker = HealthChecker::new(metrics.clone(), 8080);
        
        // Test with default metrics (should be healthy)
        let snapshot = metrics.snapshot();
        
        let memory_check = checker.check_memory_health(&snapshot);
        assert!(matches!(memory_check.status, HealthStatus::Healthy));
        
        let error_check = checker.check_error_rates(&snapshot);
        assert!(matches!(error_check.status, HealthStatus::Healthy));
        
        let latency_check = checker.check_latency_health(&snapshot);
        assert!(matches!(latency_check.status, HealthStatus::Healthy));
    }
    
    #[tokio::test]
    async fn test_health_endpoint() {
        let metrics = Arc::new(ServerMetrics::new());
        
        // Start server in background
        let server_handle = tokio::spawn(async move {
            if let Err(e) = start_health_server(metrics, 18080).await {
                eprintln!("Health server error: {}", e);
            }
        });
        
        // Give server time to start
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Try to connect
        let connect_result = tokio::time::timeout(
            Duration::from_millis(100),
            tokio::net::TcpStream::connect("127.0.0.1:18080")
        ).await;
        
        // Clean up
        server_handle.abort();
        
        // The connection should succeed
        assert!(connect_result.is_ok());
    }
}