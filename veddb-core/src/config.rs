//! Configuration management for VedDB
//!
//! This module provides:
//! - Configuration hot-reload for non-critical settings
//! - Graceful shutdown handling
//! - Configuration validation and defaults

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn};

/// VedDB server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server settings
    pub server: ServerSettings,
    /// Storage settings
    pub storage: StorageSettings,
    /// Cache settings
    pub cache: CacheSettings,
    /// Authentication settings
    pub auth: AuthSettings,
    /// Logging settings
    pub logging: LoggingSettings,
    /// Replication settings
    pub replication: ReplicationSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    /// Server port
    pub port: u16,
    /// TLS enabled
    pub tls_enabled: bool,
    /// TLS certificate path
    pub tls_cert_path: Option<PathBuf>,
    /// TLS private key path
    pub tls_key_path: Option<PathBuf>,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageSettings {
    /// Data directory
    pub data_dir: PathBuf,
    /// WAL fsync policy
    pub wal_fsync_policy: WalFsyncPolicy,
    /// Snapshot interval in minutes
    pub snapshot_interval_minutes: u64,
    /// WAL file size limit in MB
    pub wal_file_size_mb: u64,
    /// Enable compression
    pub enable_compression: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheSettings {
    /// Maximum memory usage in MB
    pub max_memory_mb: usize,
    /// Eviction policy
    pub eviction_policy: EvictionPolicy,
    /// TTL check interval in milliseconds
    pub ttl_check_interval_ms: u64,
    /// Cache warming enabled
    pub cache_warming_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSettings {
    /// Authentication required
    pub auth_required: bool,
    /// JWT secret key path
    pub jwt_secret_path: Option<PathBuf>,
    /// Session timeout in hours
    pub session_timeout_hours: u64,
    /// Password hash cost
    pub password_hash_cost: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingSettings {
    /// Log level
    pub level: LogLevel,
    /// Log format
    pub format: LogFormat,
    /// Log file path (None for stdout)
    pub file_path: Option<PathBuf>,
    /// Slow query threshold in milliseconds
    pub slow_query_threshold_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationSettings {
    /// Replication enabled
    pub enabled: bool,
    /// Node role
    pub role: NodeRole,
    /// Master address (for slaves)
    pub master_address: Option<String>,
    /// Replication port
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalFsyncPolicy {
    Always,
    EverySecond,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvictionPolicy {
    Lru,
    Lfu,
    Random,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeRole {
    Master,
    Slave,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server: ServerSettings {
                port: 50051,
                tls_enabled: true,
                tls_cert_path: None,
                tls_key_path: None,
                max_connections: 10000,
                connection_timeout_secs: 300,
            },
            storage: StorageSettings {
                data_dir: PathBuf::from("./data"),
                wal_fsync_policy: WalFsyncPolicy::Always,
                snapshot_interval_minutes: 5,
                wal_file_size_mb: 100,
                enable_compression: true,
            },
            cache: CacheSettings {
                max_memory_mb: 1024,
                eviction_policy: EvictionPolicy::Lru,
                ttl_check_interval_ms: 100,
                cache_warming_enabled: true,
            },
            auth: AuthSettings {
                auth_required: true,
                jwt_secret_path: None,
                session_timeout_hours: 24,
                password_hash_cost: 12,
            },
            logging: LoggingSettings {
                level: LogLevel::Info,
                format: LogFormat::Text,
                file_path: None,
                slow_query_threshold_ms: 100,
            },
            replication: ReplicationSettings {
                enabled: false,
                role: NodeRole::Master,
                master_address: None,
                port: 50052,
            },
        }
    }
}

/// Configuration change event
#[derive(Debug, Clone)]
pub struct ConfigChangeEvent {
    /// Changed section
    pub section: String,
    /// Old configuration (JSON)
    pub old_config: String,
    /// New configuration (JSON)
    pub new_config: String,
    /// Whether restart is required
    pub restart_required: bool,
}

/// Configuration manager
pub struct ConfigManager {
    /// Current configuration
    config: Arc<RwLock<ServerConfig>>,
    /// Configuration file path
    config_path: PathBuf,
    /// Change event broadcaster
    change_tx: broadcast::Sender<ConfigChangeEvent>,
    /// Shutdown signal
    shutdown_tx: broadcast::Sender<()>,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new(config_path: PathBuf) -> Result<Self> {
        let config = if config_path.exists() {
            Self::load_config(&config_path)?
        } else {
            let default_config = ServerConfig::default();
            Self::save_config(&config_path, &default_config)?;
            default_config
        };

        let (change_tx, _) = broadcast::channel(100);
        let (shutdown_tx, _) = broadcast::channel(1);

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            config_path,
            change_tx,
            shutdown_tx,
        })
    }

    /// Get current configuration
    pub async fn get_config(&self) -> ServerConfig {
        self.config.read().await.clone()
    }

    /// Update configuration section
    pub async fn update_config<F>(&self, section: &str, updater: F) -> Result<bool>
    where
        F: FnOnce(&mut ServerConfig) -> Result<bool>,
    {
        let mut config = self.config.write().await;
        let old_config = serde_json::to_string(&*config)?;

        let restart_required = updater(&mut *config)?;

        let new_config = serde_json::to_string(&*config)?;

        // Save to file
        Self::save_config(&self.config_path, &*config)?;

        // Broadcast change event
        let event = ConfigChangeEvent {
            section: section.to_string(),
            old_config,
            new_config,
            restart_required,
        };

        if let Err(e) = self.change_tx.send(event) {
            warn!("Failed to broadcast config change: {}", e);
        }

        info!("Configuration updated: section={}, restart_required={}", section, restart_required);

        Ok(restart_required)
    }

    /// Reload configuration from file
    pub async fn reload_config(&self) -> Result<()> {
        let new_config = Self::load_config(&self.config_path)?;
        let old_config = serde_json::to_string(&*self.config.read().await)?;
        let new_config_json = serde_json::to_string(&new_config)?;

        *self.config.write().await = new_config;

        // Broadcast change event
        let event = ConfigChangeEvent {
            section: "all".to_string(),
            old_config,
            new_config: new_config_json,
            restart_required: false, // Assume hot-reload for now
        };

        if let Err(e) = self.change_tx.send(event) {
            warn!("Failed to broadcast config reload: {}", e);
        }

        info!("Configuration reloaded from file");

        Ok(())
    }

    /// Subscribe to configuration changes
    pub fn subscribe_changes(&self) -> broadcast::Receiver<ConfigChangeEvent> {
        self.change_tx.subscribe()
    }

    /// Initiate graceful shutdown
    pub async fn shutdown(&self) -> Result<()> {
        info!("Initiating graceful shutdown...");

        // Broadcast shutdown signal
        if let Err(e) = self.shutdown_tx.send(()) {
            warn!("Failed to broadcast shutdown signal: {}", e);
        }

        // Give components time to shut down gracefully
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        info!("Graceful shutdown completed");

        Ok(())
    }

    /// Subscribe to shutdown signal
    pub fn subscribe_shutdown(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }

    /// Validate configuration
    pub fn validate_config(config: &ServerConfig) -> Result<()> {
        // Validate server settings
        if config.server.port == 0 {
            return Err(anyhow::anyhow!("Server port cannot be 0"));
        }

        if config.server.max_connections == 0 {
            return Err(anyhow::anyhow!("Max connections cannot be 0"));
        }

        // Validate TLS settings
        if config.server.tls_enabled {
            if config.server.tls_cert_path.is_none() {
                return Err(anyhow::anyhow!("TLS certificate path required when TLS is enabled"));
            }
            if config.server.tls_key_path.is_none() {
                return Err(anyhow::anyhow!("TLS key path required when TLS is enabled"));
            }
        }

        // Validate storage settings
        if config.storage.snapshot_interval_minutes == 0 {
            return Err(anyhow::anyhow!("Snapshot interval cannot be 0"));
        }

        if config.storage.wal_file_size_mb == 0 {
            return Err(anyhow::anyhow!("WAL file size cannot be 0"));
        }

        // Validate cache settings
        if config.cache.max_memory_mb == 0 {
            return Err(anyhow::anyhow!("Cache max memory cannot be 0"));
        }

        // Validate auth settings
        if config.auth.session_timeout_hours == 0 {
            return Err(anyhow::anyhow!("Session timeout cannot be 0"));
        }

        if config.auth.password_hash_cost < 4 || config.auth.password_hash_cost > 31 {
            return Err(anyhow::anyhow!("Password hash cost must be between 4 and 31"));
        }

        // Validate replication settings
        if config.replication.enabled {
            match config.replication.role {
                NodeRole::Slave => {
                    if config.replication.master_address.is_none() {
                        return Err(anyhow::anyhow!("Master address required for slave nodes"));
                    }
                }
                NodeRole::Master => {
                    // Master nodes don't need additional validation
                }
            }
        }

        Ok(())
    }

    /// Load configuration from file
    fn load_config(path: &Path) -> Result<ServerConfig> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: ServerConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        Self::validate_config(&config)?;

        Ok(config)
    }

    /// Save configuration to file
    fn save_config(path: &Path, config: &ServerConfig) -> Result<()> {
        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
        }

        let content = toml::to_string_pretty(config)
            .context("Failed to serialize configuration")?;

        std::fs::write(path, content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        Ok(())
    }

    /// Check if a configuration change requires restart
    pub fn requires_restart(section: &str, _old_config: &str, _new_config: &str) -> bool {
        // Configuration sections that require restart
        let restart_sections = [
            "server.port",
            "server.tls_cert_path",
            "server.tls_key_path",
            "storage.data_dir",
            "replication.role",
            "replication.port",
        ];

        restart_sections.iter().any(|&s| section.starts_with(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = ServerConfig::default();
        assert_eq!(config.server.port, 50051);
        assert!(config.server.tls_enabled);
        assert!(config.auth.auth_required);
    }

    #[test]
    fn test_config_validation() {
        let mut config = ServerConfig::default();
        
        // Valid config should pass
        assert!(ConfigManager::validate_config(&config).is_ok());

        // Invalid port should fail
        config.server.port = 0;
        assert!(ConfigManager::validate_config(&config).is_err());
    }

    #[tokio::test]
    async fn test_config_manager() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("veddb.toml");

        let manager = ConfigManager::new(config_path.clone()).unwrap();
        
        // Config file should be created
        assert!(config_path.exists());

        // Should be able to get config
        let config = manager.get_config().await;
        assert_eq!(config.server.port, 50051);
    }

    #[tokio::test]
    async fn test_config_update() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("veddb.toml");

        let manager = ConfigManager::new(config_path).unwrap();

        // Update a non-restart setting
        let restart_required = manager.update_config("logging", |config| {
            config.logging.level = LogLevel::Debug;
            Ok(false)
        }).await.unwrap();

        assert!(!restart_required);

        let config = manager.get_config().await;
        assert!(matches!(config.logging.level, LogLevel::Debug));
    }

    #[test]
    fn test_requires_restart() {
        assert!(ConfigManager::requires_restart("server.port", "", ""));
        assert!(ConfigManager::requires_restart("storage.data_dir", "", ""));
        assert!(!ConfigManager::requires_restart("logging.level", "", ""));
        assert!(!ConfigManager::requires_restart("cache.max_memory_mb", "", ""));
    }
}