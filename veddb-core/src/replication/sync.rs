//! Replication synchronization logic

use crate::replication::{ReplicationMessage, ReplicationError, ReplicationResult, ReplicationConnection};
use crate::wal::{WalEntry, WalReader};
use crate::snapshot::{SnapshotWriter, SnapshotReader, format::SnapshotHeader};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info, warn};

/// Synchronization manager for handling full and incremental sync
pub struct SyncManager {
    /// Current WAL sequence number
    current_sequence: Arc<AtomicU64>,
    /// WAL directory path
    wal_dir: std::path::PathBuf,
    /// Snapshot directory path
    snapshot_dir: std::path::PathBuf,
    /// WAL entry broadcast channel for streaming
    wal_broadcast_tx: broadcast::Sender<WalEntry>,
    /// Replication lag tracking
    replication_lag: Arc<AtomicU64>,
}

impl SyncManager {
    /// Create a new sync manager
    pub fn new<P: AsRef<Path>>(wal_dir: P, snapshot_dir: P) -> Self {
        let (wal_broadcast_tx, _) = broadcast::channel(1000);
        
        Self {
            current_sequence: Arc::new(AtomicU64::new(0)),
            wal_dir: wal_dir.as_ref().to_path_buf(),
            snapshot_dir: snapshot_dir.as_ref().to_path_buf(),
            wal_broadcast_tx,
            replication_lag: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Update the current sequence number
    pub fn update_sequence(&self, sequence: u64) {
        self.current_sequence.store(sequence, Ordering::SeqCst);
    }

    /// Get the current sequence number
    pub fn current_sequence(&self) -> u64 {
        self.current_sequence.load(Ordering::SeqCst)
    }

    /// Subscribe to WAL entry stream
    pub fn subscribe_to_wal_stream(&self) -> broadcast::Receiver<WalEntry> {
        self.wal_broadcast_tx.subscribe()
    }

    /// Broadcast a new WAL entry to all subscribers
    pub async fn broadcast_wal_entry(&self, entry: WalEntry) -> ReplicationResult<usize> {
        let subscriber_count = self.wal_broadcast_tx.receiver_count();
        
        if subscriber_count > 0 {
            match self.wal_broadcast_tx.send(entry.clone()) {
                Ok(_) => {
                    debug!("Broadcasted WAL entry {} to {} subscribers", 
                           entry.sequence, subscriber_count);
                    Ok(subscriber_count)
                }
                Err(_) => {
                    // No active receivers
                    Ok(0)
                }
            }
        } else {
            Ok(0)
        }
    }

    /// Get replication lag in milliseconds
    pub fn get_replication_lag(&self) -> u64 {
        self.replication_lag.load(Ordering::Relaxed)
    }

    /// Update replication lag
    pub fn update_replication_lag(&self, lag_ms: u64) {
        self.replication_lag.store(lag_ms, Ordering::Relaxed);
    }

    /// Get synchronization statistics
    pub fn get_sync_stats(&self) -> SyncStats {
        SyncStats {
            current_sequence: self.current_sequence(),
            replication_lag_ms: self.get_replication_lag(),
            active_subscribers: self.wal_broadcast_tx.receiver_count(),
        }
    }

    /// Perform full synchronization by sending a snapshot
    pub async fn perform_full_sync(
        &self,
        connection: &mut ReplicationConnection,
    ) -> ReplicationResult<()> {
        info!("Starting full sync to {}", connection.peer_addr());

        // Create a snapshot
        let snapshot_path = self.create_temp_snapshot().await?;
        
        // Read snapshot header
        let header = self.read_snapshot_header(&snapshot_path).await?;
        
        // Read and compress snapshot data
        let snapshot_data = self.read_and_compress_snapshot(&snapshot_path).await?;
        
        // Send full sync message
        let full_sync_msg = ReplicationMessage::FullSync {
            header,
            snapshot_data,
        };
        
        connection.send_message(&full_sync_msg).await?;
        
        // Wait for acknowledgment
        let ack = connection.receive_message().await?;
        match ack {
            ReplicationMessage::Ack { status, .. } => {
                if status == crate::replication::AckStatus::Success {
                    info!("Full sync completed successfully to {}", connection.peer_addr());
                } else {
                    return Err(ReplicationError::ProtocolError(
                        "Full sync failed on slave".to_string()
                    ));
                }
            }
            _ => {
                return Err(ReplicationError::ProtocolError(
                    "Expected ACK message".to_string()
                ));
            }
        }

        // Clean up temporary snapshot
        if let Err(e) = tokio::fs::remove_file(&snapshot_path).await {
            warn!("Failed to remove temporary snapshot: {}", e);
        }

        Ok(())
    }

    /// Perform incremental synchronization by sending WAL entries
    pub async fn perform_incremental_sync(
        &self,
        connection: &mut ReplicationConnection,
        from_sequence: u64,
    ) -> ReplicationResult<()> {
        debug!("Starting incremental sync from sequence {} to {}", 
               from_sequence, connection.peer_addr());

        // Get WAL entries since the specified sequence
        let entries = self.get_wal_entries_since(from_sequence).await?;
        
        if entries.is_empty() {
            debug!("No new WAL entries to sync");
            return Ok(());
        }

        // Send incremental sync message
        let incremental_sync_msg = ReplicationMessage::IncrementalSync { entries };
        connection.send_message(&incremental_sync_msg).await?;

        // Wait for acknowledgment
        let ack = connection.receive_message().await?;
        match ack {
            ReplicationMessage::Ack { status, sequence } => {
                if status == crate::replication::AckStatus::Success {
                    debug!("Incremental sync completed successfully to {} (sequence: {})", 
                           connection.peer_addr(), sequence);
                } else {
                    return Err(ReplicationError::ProtocolError(
                        format!("Incremental sync failed on slave at sequence {}", sequence)
                    ));
                }
            }
            _ => {
                return Err(ReplicationError::ProtocolError(
                    "Expected ACK message".to_string()
                ));
            }
        }

        Ok(())
    }

    /// Apply full sync data received from master
    pub async fn apply_full_sync(
        &self,
        header: SnapshotHeader,
        snapshot_data: Vec<u8>,
    ) -> ReplicationResult<()> {
        info!("Applying full sync (sequence: {})", header.sequence);

        // Decompress snapshot data
        let decompressed_data = self.decompress_snapshot_data(snapshot_data)?;
        
        // Write to temporary file
        let temp_path = self.snapshot_dir.join("temp_received_snapshot.veddb");
        let mut file = File::create(&temp_path).await
            .map_err(ReplicationError::IoError)?;
        
        file.write_all(&decompressed_data).await
            .map_err(ReplicationError::IoError)?;
        file.flush().await.map_err(ReplicationError::IoError)?;
        
        // Load the snapshot
        let mut reader = SnapshotReader::open(&temp_path)
            .map_err(|e| ReplicationError::SnapshotError(e.to_string()))?;
        
        // Apply the snapshot to the database
        self.apply_snapshot(&mut reader).await?;
        
        // Update our sequence number
        self.update_sequence(header.sequence);
        
        // Clean up temporary file
        if let Err(e) = tokio::fs::remove_file(&temp_path).await {
            warn!("Failed to remove temporary snapshot file: {}", e);
        }

        info!("Full sync applied successfully");
        Ok(())
    }

    /// Apply incremental sync data received from master
    pub async fn apply_incremental_sync(
        &self,
        entries: Vec<WalEntry>,
    ) -> ReplicationResult<u64> {
        let start_time = Instant::now();
        debug!("Applying {} WAL entries", entries.len());

        let mut last_sequence = 0;
        let mut last_timestamp = None;
        
        for entry in entries {
            // Verify entry integrity
            if !entry.verify_checksum().map_err(|e| {
                ReplicationError::WalError(e.to_string())
            })? {
                return Err(ReplicationError::WalError(
                    format!("Checksum verification failed for entry {}", entry.sequence)
                ));
            }

            // Track the timestamp of the last entry for lag calculation
            last_timestamp = Some(entry.timestamp);

            // Apply the operation
            self.apply_wal_entry(&entry).await?;
            last_sequence = entry.sequence;
        }

        // Update our sequence number
        if last_sequence > 0 {
            self.update_sequence(last_sequence);
        }

        // Calculate and update replication lag
        if let Some(last_ts) = last_timestamp {
            let now = chrono::Utc::now();
            let lag = now.signed_duration_since(last_ts);
            let lag_ms = lag.num_milliseconds().max(0) as u64;
            self.update_replication_lag(lag_ms);
            
            debug!("Replication lag: {}ms", lag_ms);
        }

        let apply_time = start_time.elapsed();
        debug!("Applied incremental sync up to sequence {} in {:?}", 
               last_sequence, apply_time);
        Ok(last_sequence)
    }

    /// Start WAL streaming to a connection
    pub async fn start_wal_streaming(
        &self,
        mut connection: ReplicationConnection,
    ) -> ReplicationResult<()> {
        info!("Starting WAL streaming to {}", connection.peer_addr());
        
        let mut wal_receiver = self.subscribe_to_wal_stream();
        let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(30));
        
        loop {
            tokio::select! {
                // Receive new WAL entry to stream
                result = wal_receiver.recv() => {
                    match result {
                        Ok(entry) => {
                            let message = ReplicationMessage::IncrementalSync {
                                entries: vec![entry],
                            };
                            
                            if let Err(e) = connection.send_message(&message).await {
                                error!("Failed to stream WAL entry: {}", e);
                                return Err(e);
                            }
                            
                            // Wait for acknowledgment
                            match connection.receive_message().await {
                                Ok(ReplicationMessage::Ack { status, .. }) => {
                                    if status != crate::replication::AckStatus::Success {
                                        warn!("Slave failed to apply WAL entry");
                                    }
                                }
                                Ok(_) => {
                                    warn!("Unexpected message from slave during WAL streaming");
                                }
                                Err(e) => {
                                    error!("Error receiving ACK from slave: {}", e);
                                    return Err(e);
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            warn!("WAL streaming lagged, skipped {} entries", skipped);
                            // Could trigger a full sync here if lag is too high
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("WAL broadcast channel closed");
                            break;
                        }
                    }
                }
                
                // Send periodic heartbeat
                _ = heartbeat_interval.tick() => {
                    let heartbeat = ReplicationMessage::heartbeat(self.current_sequence());
                    if let Err(e) = connection.send_message(&heartbeat).await {
                        error!("Failed to send heartbeat: {}", e);
                        return Err(e);
                    }
                }
            }
        }
        
        info!("WAL streaming ended for {}", connection.peer_addr());
        Ok(())
    }

    /// Determine if full sync is needed
    pub async fn needs_full_sync(&self, slave_sequence: u64) -> ReplicationResult<bool> {
        let current = self.current_sequence();
        
        // If slave is too far behind, do full sync
        if current > slave_sequence + 10000 {
            return Ok(true);
        }

        // Check if we have the required WAL entries
        let available_entries = self.get_available_wal_range().await?;
        Ok(slave_sequence < available_entries.0)
    }

    /// Create a temporary snapshot for replication
    async fn create_temp_snapshot(&self) -> ReplicationResult<std::path::PathBuf> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let snapshot_path = self.snapshot_dir.join(format!("temp_repl_{}.veddb", timestamp));
        
        // Create snapshot writer
        let mut writer = SnapshotWriter::create(&snapshot_path)
            .map_err(|e| ReplicationError::SnapshotError(e.to_string()))?;
        
        // Write snapshot data (this would integrate with the actual storage engine)
        self.write_snapshot_data(&mut writer).await?;
        
        writer.finalize()
            .map_err(|e| ReplicationError::SnapshotError(e.to_string()))?;
        
        Ok(snapshot_path)
    }

    /// Read snapshot header from file
    async fn read_snapshot_header(&self, path: &Path) -> ReplicationResult<SnapshotHeader> {
        let mut reader = SnapshotReader::open(path)
            .map_err(|e| ReplicationError::SnapshotError(e.to_string()))?;
        
        let header = reader.read_header()
            .map_err(|e| ReplicationError::SnapshotError(e.to_string()))?;
        
        Ok(header)
    }

    /// Read and compress snapshot data
    async fn read_and_compress_snapshot(&self, path: &Path) -> ReplicationResult<Vec<u8>> {
        let data = tokio::fs::read(path).await
            .map_err(ReplicationError::IoError)?;
        
        // Compress using zstd
        let compressed = zstd::bulk::compress(&data, 3)
            .map_err(|e| ReplicationError::SerializationError(e.to_string()))?;
        
        debug!("Compressed snapshot: {} -> {} bytes", data.len(), compressed.len());
        Ok(compressed)
    }

    /// Decompress snapshot data
    fn decompress_snapshot_data(&self, compressed_data: Vec<u8>) -> ReplicationResult<Vec<u8>> {
        zstd::bulk::decompress(&compressed_data, 1024 * 1024 * 1024) // Max 1GB
            .map_err(|e| ReplicationError::DeserializationError(e.to_string()))
    }

    /// Get WAL entries since a specific sequence number
    async fn get_wal_entries_since(&self, from_sequence: u64) -> ReplicationResult<Vec<WalEntry>> {
        let mut entries = Vec::new();
        
        // Find WAL files that might contain entries after from_sequence
        let wal_files = self.find_wal_files_after(from_sequence).await?;
        
        for wal_file in wal_files {
            let mut reader = WalReader::open(&wal_file)
                .map_err(|e| ReplicationError::WalError(e.to_string()))?;
            
            while let Some(entry) = reader.next_entry()
                .map_err(|e| ReplicationError::WalError(e.to_string()))? {
                
                if entry.sequence > from_sequence {
                    entries.push(entry);
                }
            }
        }

        // Sort by sequence number
        entries.sort_by_key(|e| e.sequence);
        
        debug!("Found {} WAL entries since sequence {}", entries.len(), from_sequence);
        Ok(entries)
    }

    /// Find WAL files that might contain entries after a sequence
    async fn find_wal_files_after(&self, sequence: u64) -> ReplicationResult<Vec<std::path::PathBuf>> {
        let mut files = Vec::new();
        
        let mut dir = tokio::fs::read_dir(&self.wal_dir).await
            .map_err(ReplicationError::IoError)?;
        
        while let Some(entry) = dir.next_entry().await
            .map_err(ReplicationError::IoError)? {
            
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("log") {
                files.push(path);
            }
        }

        // Sort files by name (assuming they're named sequentially)
        files.sort();
        
        Ok(files)
    }

    /// Get the range of available WAL entries
    async fn get_available_wal_range(&self) -> ReplicationResult<(u64, u64)> {
        let wal_files = self.find_wal_files_after(0).await?;
        
        if wal_files.is_empty() {
            return Ok((0, 0));
        }

        // Get first sequence from first file
        let mut first_reader = WalReader::open(&wal_files[0])
            .map_err(|e| ReplicationError::WalError(e.to_string()))?;
        
        let first_entry = first_reader.next_entry()
            .map_err(|e| ReplicationError::WalError(e.to_string()))?;
        
        let min_sequence = first_entry.map(|e| e.sequence).unwrap_or(0);
        let max_sequence = self.current_sequence();
        
        Ok((min_sequence, max_sequence))
    }

    /// Write snapshot data (placeholder - would integrate with storage engine)
    async fn write_snapshot_data(&self, _writer: &mut SnapshotWriter) -> ReplicationResult<()> {
        // This would integrate with the actual storage engine to write
        // all collections, documents, indexes, etc.
        // For now, this is a placeholder
        Ok(())
    }

    /// Apply snapshot to database (placeholder - would integrate with storage engine)
    async fn apply_snapshot(&self, _reader: &mut SnapshotReader) -> ReplicationResult<()> {
        // This would integrate with the actual storage engine to load
        // all collections, documents, indexes, etc. from the snapshot
        // For now, this is a placeholder
        Ok(())
    }

    /// Apply a WAL entry to the database (placeholder - would integrate with storage engine)
    async fn apply_wal_entry(&self, _entry: &WalEntry) -> ReplicationResult<()> {
        // This would integrate with the actual storage engine to apply
        // the operation described in the WAL entry
        // For now, this is a placeholder
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_sync_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let wal_dir = temp_dir.path().join("wal");
        let snapshot_dir = temp_dir.path().join("snapshots");
        
        tokio::fs::create_dir_all(&wal_dir).await.unwrap();
        tokio::fs::create_dir_all(&snapshot_dir).await.unwrap();
        
        let sync_manager = SyncManager::new(&wal_dir, &snapshot_dir);
        assert_eq!(sync_manager.current_sequence(), 0);
        
        sync_manager.update_sequence(12345);
        assert_eq!(sync_manager.current_sequence(), 12345);
    }

    #[tokio::test]
    async fn test_needs_full_sync() {
        let temp_dir = TempDir::new().unwrap();
        let wal_dir = temp_dir.path().join("wal");
        let snapshot_dir = temp_dir.path().join("snapshots");
        
        tokio::fs::create_dir_all(&wal_dir).await.unwrap();
        tokio::fs::create_dir_all(&snapshot_dir).await.unwrap();
        
        let sync_manager = SyncManager::new(&wal_dir, &snapshot_dir);
        sync_manager.update_sequence(20000);
        
        // Slave too far behind - needs full sync
        let needs_full = sync_manager.needs_full_sync(5000).await.unwrap();
        assert!(needs_full);
        
        // Slave close enough - incremental sync
        let needs_full = sync_manager.needs_full_sync(19000).await.unwrap();
        assert!(!needs_full);
    }

    #[test]
    fn test_compress_decompress() {
        let sync_manager = SyncManager::new("/tmp", "/tmp");
        let original_data = b"Hello, World! This is test data for compression.".to_vec();
        
        let compressed = zstd::bulk::compress(&original_data, 3).unwrap();
        let decompressed = sync_manager.decompress_snapshot_data(compressed).unwrap();
        
        assert_eq!(original_data, decompressed);
    }
}

/// Synchronization statistics
#[derive(Debug, Clone)]
pub struct SyncStats {
    /// Current WAL sequence number
    pub current_sequence: u64,
    /// Replication lag in milliseconds
    pub replication_lag_ms: u64,
    /// Number of active WAL stream subscribers
    pub active_subscribers: usize,
}