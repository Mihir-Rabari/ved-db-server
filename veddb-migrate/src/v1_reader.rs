//! VedDB v0.1.x data reader
//!
//! Reads data from v0.1.x format which is a simple in-memory key-value store.
//! Since v0.1.x doesn't have persistence, this module handles various input formats:
//! - Memory dumps (if available)
//! - Backup files (JSON format)
//! - Live server connection (for online migration)

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use tracing::{info, debug, warn};

/// V0.1.x key-value pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V1KeyValue {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
    pub metadata: Option<V1Metadata>,
}

/// V0.1.x metadata (if available)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V1Metadata {
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub ttl: Option<u64>,
    pub version: Option<u64>,
}

/// V0.1.x backup file format
#[derive(Debug, Serialize, Deserialize)]
pub struct V1Backup {
    pub version: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub data: HashMap<String, String>, // Base64 encoded key-value pairs
    pub metadata: Option<HashMap<String, V1Metadata>>,
}

/// Reader for v0.1.x data
pub struct V1Reader {
    data: Vec<V1KeyValue>,
}

impl V1Reader {
    /// Create a new V1Reader from a file path
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        info!("Reading v0.1.x data from: {}", path.display());

        if !path.exists() {
            bail!("Input path does not exist: {}", path.display());
        }

        let data = if path.is_file() {
            Self::read_backup_file(path)?
        } else if path.is_dir() {
            Self::read_data_directory(path)?
        } else {
            bail!("Input path is neither file nor directory: {}", path.display());
        };

        info!("Loaded {} key-value pairs from v0.1.x data", data.len());
        Ok(Self { data })
    }

    /// Read from a backup file (JSON format)
    fn read_backup_file(path: &Path) -> Result<Vec<V1KeyValue>> {
        let file = File::open(path)
            .with_context(|| format!("Failed to open backup file: {}", path.display()))?;
        
        let reader = BufReader::new(file);
        
        // Try to parse as V1Backup format first
        if let Ok(backup) = serde_json::from_reader::<_, V1Backup>(reader) {
            info!("Found v0.1.x backup file (version: {})", backup.version);
            return Self::parse_backup_data(backup);
        }

        // Fallback: try to parse as raw key-value JSON
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        
        if let Ok(raw_data) = serde_json::from_reader::<_, HashMap<String, String>>(reader) {
            warn!("Found raw JSON key-value data, assuming v0.1.x format");
            return Ok(raw_data
                .into_iter()
                .map(|(k, v)| V1KeyValue {
                    key: k.into_bytes(),
                    value: v.into_bytes(),
                    metadata: None,
                })
                .collect());
        }

        bail!("Unable to parse backup file as v0.1.x format");
    }

    /// Read from a data directory (multiple files)
    fn read_data_directory(path: &Path) -> Result<Vec<V1KeyValue>> {
        let mut all_data = Vec::new();
        
        // Look for common v0.1.x file patterns
        let patterns = ["*.json", "*.backup", "*.dump", "data.db"];
        
        for entry in walkdir::WalkDir::new(path) {
            let entry = entry?;
            let file_path = entry.path();
            
            if file_path.is_file() {
                if let Some(ext) = file_path.extension() {
                    match ext.to_str() {
                        Some("json") | Some("backup") => {
                            debug!("Processing file: {}", file_path.display());
                            match Self::read_backup_file(file_path) {
                                Ok(mut data) => all_data.append(&mut data),
                                Err(e) => warn!("Failed to read {}: {}", file_path.display(), e),
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        if all_data.is_empty() {
            bail!("No v0.1.x data files found in directory: {}", path.display());
        }

        Ok(all_data)
    }

    /// Parse V1Backup format
    fn parse_backup_data(backup: V1Backup) -> Result<Vec<V1KeyValue>> {
        let mut data = Vec::new();
        
        for (key_b64, value_b64) in backup.data {
            let key = general_purpose::STANDARD.decode(&key_b64)
                .with_context(|| format!("Failed to decode key: {}", key_b64))?;
            let value = general_purpose::STANDARD.decode(&value_b64)
                .with_context(|| format!("Failed to decode value for key: {}", key_b64))?;
            
            let metadata = backup.metadata
                .as_ref()
                .and_then(|m| m.get(&key_b64))
                .cloned();
            
            data.push(V1KeyValue {
                key,
                value,
                metadata,
            });
        }
        
        Ok(data)
    }

    /// Get all key-value pairs
    pub fn get_data(&self) -> &[V1KeyValue] {
        &self.data
    }

    /// Get total number of keys
    pub fn key_count(&self) -> usize {
        self.data.len()
    }

    /// Get total data size in bytes
    pub fn total_size(&self) -> usize {
        self.data
            .iter()
            .map(|kv| kv.key.len() + kv.value.len())
            .sum()
    }

    /// Calculate checksum of all data
    pub fn calculate_checksum(&self) -> u32 {
        let mut hasher = crc32fast::Hasher::new();
        
        // Sort by key to ensure deterministic checksum
        let mut sorted_data = self.data.clone();
        sorted_data.sort_by(|a, b| a.key.cmp(&b.key));
        
        for kv in sorted_data {
            hasher.update(&kv.key);
            hasher.update(&kv.value);
        }
        
        hasher.finalize()
    }
}

// Add base64 dependency for decoding
use base64::{Engine as _, engine::general_purpose};
use walkdir;