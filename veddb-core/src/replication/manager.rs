//! Replication manager coordinating master/slave operations

use crate::replication::{
    ReplicationMessage, ReplicationError, ReplicationResult, ReplicationConfig, ReplicationStats,
    NodeRole, SlaveConnectionManager, ReplicationConnection, ReplicationListener, 
    ExponentialBackoff, SyncManager, ErrorCode, AckStatus
};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock, Mutex};
use tokio::time::{interval, timeout};
use tracing::{debug, error, info, warn};

/// Main replication manager
pub struct ReplicationManager {
    /// Configuration
    config: ReplicationConfig,
    /// Current node role
    role: Arc<RwLock<NodeRole>>,
    /// Synchronization manager
    sync_manager: Arc<SyncManager>,
    /// Statistics
    stats: Arc<RwLock<ReplicationStats>>,
    /// Slave connection manager (for master nodes)
    slave_manager: Arc<Mutex<Option<SlaveConnectionManager>>>,
    /// Master connection (for slave nodes)
    master_connection: Arc<Mutex<Option<ReplicationConnection>>>,
    /// Shutdown signal
    shutdown_tx: Option<mpsc::UnboundedSender<()>>,
    /// WAL entry broadcast channel
    wal_broadcast_tx: mpsc::UnboundedSender<crate::wal::WalEntry>,
    /// Replication lag tracking
    replication_lag: Arc<AtomicU64>,
}

impl ReplicationManager {
    /// Create a new replication manager
    pub fn new(config: ReplicationConfig, sync_manager: SyncManager) -> Self {
        let (wal_broadcast_tx, _) = mpsc::unbounded_channel();
        
        Self {
            role: Arc::new(RwLock::new(config.role.clone())),
            config,
            sync_manager: Arc::new(sync_manager),
            stats: Arc::new(RwLock::new(ReplicationStats::default())),
            slave_manager: Arc::new(Mutex::new(None)),
            master_connection: Arc::new(Mutex::new(None)),
            shutdown_tx: None,
            wal_broadcast_tx,
            replication_lag: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Start the replication manager
    pub async fn start(&mut self) -> ReplicationResult<()> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::unbounded_channel();
        self.shutdown_tx = Some(shutdown_tx);

        let role = self.role.read().await.clone();
        
        match role {
            NodeRole::Master => {
                info!("Starting replication manager as master");
                self.start_master().await?;
            }
            NodeRole::Slave { master_addr } => {
                info!("Starting replication manager as slave (master: {})", master_addr);
                self.start_slave(master_addr).await?;
            }
        }

        // Start background tasks
        self.start_background_tasks().await;

        // Wait for shutdown signal
        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("Replication manager shutting down");
            }
        }

        Ok(())
    }

    /// Stop the replication manager
    pub async fn stop(&mut self) -> ReplicationResult<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        // Close master connection if we're a slave
        let mut master_conn = self.master_connection.lock().await;
        if let Some(connection) = master_conn.take() {
            connection.close().await?;
        }

        info!("Replication manager stopped");
        Ok(())
    }

    /// Start master mode
    async fn start_master(&self) -> ReplicationResult<()> {
        // Initialize slave connection manager
        let slave_manager = SlaveConnectionManager::new(self.config.max_slaves);
        *self.slave_manager.lock().await = Some(slave_manager);

        // Start listening for slave connections
        let bind_addr = "0.0.0.0:50052".parse().unwrap(); // TODO: Make configurable
        let mut listener = ReplicationListener::bind(bind_addr).await?;

        let slave_manager = Arc::clone(&self.slave_manager);
        let sync_manager = Arc::clone(&self.sync_manager);
        let stats = Arc::clone(&self.stats);

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok(connection) => {
                        info!("New slave connection from {}", connection.peer_addr());
                        
                        if let Some(ref mut manager) = *slave_manager.lock().await {
                            if let Err(e) = Self::handle_new_slave(
                                connection, 
                                manager, 
                                Arc::clone(&sync_manager),
                                Arc::clone(&stats)
                            ).await {
                                error!("Failed to handle new slave: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to accept slave connection: {}", e);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });

        Ok(())
    }

    /// Start slave mode
    async fn start_slave(&self, master_addr: SocketAddr) -> ReplicationResult<()> {
        let sync_manager = Arc::clone(&self.sync_manager);
        let master_connection = Arc::clone(&self.master_connection);
        let stats = Arc::clone(&self.stats);
        let replication_lag = Arc::clone(&self.replication_lag);
        let backoff_config = self.config.backoff_config.clone();

        tokio::spawn(async move {
            let mut backoff = ExponentialBackoff::new(backoff_config);
            
            loop {
                match Self::connect_to_master(
                    master_addr,
                    Arc::clone(&sync_manager),
                    Arc::clone(&stats),
                    Arc::clone(&replication_lag)
                ).await {
                    Ok(connection) => {
                        info!("Connected to master at {}", master_addr);
                        *master_connection.lock().await = Some(connection);
                        backoff.reset();
                        
                        // Connection established, wait for it to fail
                        // The connection handling is done in connect_to_master
                        // Monitor connection health
                        Self::monitor_master_connection(Arc::clone(&master_connection)).await;
                    }
                    Err(e) => {
                        error!("Failed to connect to master: {}", e);
                        let delay = backoff.next();
                        warn!("Retrying connection in {:?}", delay);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        });

        Ok(())
    }

    /// Monitor master connection health
    async fn monitor_master_connection(
        master_connection: Arc<Mutex<Option<ReplicationConnection>>>,
    ) {
        let mut check_interval = tokio::time::interval(Duration::from_secs(30));
        
        loop {
            check_interval.tick().await;
            
            let mut conn = master_connection.lock().await;
            if let Some(ref mut connection) = *conn {
                // Check if connection is still alive
                if !connection.is_alive().await {
                    warn!("Master connection lost, will trigger reconnection");
                    *conn = None;
                    break;
                }
            } else {
                // No connection, exit monitoring
                break;
            }
        }
    }

    /// Handle a new slave connection
    async fn handle_new_slave(
        mut connection: ReplicationConnection,
        slave_manager: &mut SlaveConnectionManager,
        sync_manager: Arc<SyncManager>,
        stats: Arc<RwLock<ReplicationStats>>,
    ) -> ReplicationResult<()> {
        // Wait for sync request
        let sync_request = timeout(Duration::from_secs(30), connection.receive_message()).await
            .map_err(|_| ReplicationError::Timeout)?
            .map_err(|e| ReplicationError::ConnectionError(e.to_string()))?;

        let (last_sequence, slave_id) = match sync_request {
            ReplicationMessage::SyncRequest { last_sequence, slave_id } => {
                (last_sequence, slave_id)
            }
            _ => {
                let error_msg = ReplicationMessage::error(
                    ErrorCode::InvalidMessage,
                    "Expected SyncRequest".to_string()
                );
                connection.send_message(&error_msg).await?;
                return Err(ReplicationError::ProtocolError("Expected SyncRequest".to_string()));
            }
        };

        info!("Slave {} requesting sync from sequence {}", slave_id, last_sequence);

        // Determine sync strategy
        if sync_manager.needs_full_sync(last_sequence).await? {
            info!("Performing full sync for slave {}", slave_id);
            sync_manager.perform_full_sync(&mut connection).await?;
        } else {
            info!("Performing incremental sync for slave {}", slave_id);
            sync_manager.perform_incremental_sync(&mut connection, last_sequence).await?;
        }

        // Add to slave manager for ongoing replication
        slave_manager.add_slave(connection).await?;
        
        // Start WAL streaming for this slave (in background)
        let streaming_sync_manager = Arc::clone(&sync_manager);
        tokio::spawn(async move {
            // Note: In a real implementation, we'd need to get the connection back
            // from the slave manager to start streaming. This is a simplified version.
            debug!("WAL streaming would start here for slave {}", slave_id);
        });

        // Update stats
        {
            let mut stats = stats.write().await;
            stats.connected_slaves = slave_manager.slave_count();
            stats.last_sync = Some(chrono::Utc::now());
        }

        Ok(())
    }

    /// Connect to master as a slave with retry logic
    async fn connect_to_master(
        master_addr: SocketAddr,
        sync_manager: Arc<SyncManager>,
        stats: Arc<RwLock<ReplicationStats>>,
        replication_lag: Arc<AtomicU64>,
    ) -> ReplicationResult<ReplicationConnection> {
        // Connect to master
        let stream = TcpStream::connect(master_addr).await
            .map_err(ReplicationError::IoError)?;

        let connection_id = format!("master-{}", Instant::now().elapsed().as_millis());
        let mut connection = ReplicationConnection::new(stream, connection_id)?;

        // Send sync request
        let last_sequence = sync_manager.current_sequence();
        let sync_request = ReplicationMessage::SyncRequest {
            last_sequence,
            slave_id: "slave-001".to_string(), // TODO: Generate unique slave ID
        };

        connection.send_message(&sync_request).await?;

        // Handle sync response
        let sync_response = connection.receive_message().await?;
        match sync_response {
            ReplicationMessage::FullSync { header, snapshot_data } => {
                info!("Receiving full sync from master");
                let sequence = header.sequence;
                sync_manager.apply_full_sync(header, snapshot_data).await?;
                
                // Send acknowledgment
                let ack = ReplicationMessage::ack_success(sequence);
                connection.send_message(&ack).await?;
            }
            ReplicationMessage::IncrementalSync { entries } => {
                info!("Receiving incremental sync from master ({} entries)", entries.len());
                let last_seq = sync_manager.apply_incremental_sync(entries).await?;
                
                // Send acknowledgment
                let ack = ReplicationMessage::ack_success(last_seq);
                connection.send_message(&ack).await?;
            }
            ReplicationMessage::Error { code, message } => {
                return Err(ReplicationError::ProtocolError(
                    format!("Master error: {:?} - {}", code, message)
                ));
            }
            _ => {
                return Err(ReplicationError::ProtocolError(
                    "Unexpected sync response".to_string()
                ));
            }
        }

        // Start receiving ongoing replication stream
        Self::handle_replication_stream(connection, sync_manager, stats, replication_lag).await
    }

    /// Handle ongoing replication stream from master
    async fn handle_replication_stream(
        mut connection: ReplicationConnection,
        sync_manager: Arc<SyncManager>,
        stats: Arc<RwLock<ReplicationStats>>,
        replication_lag: Arc<AtomicU64>,
    ) -> ReplicationResult<ReplicationConnection> {
        let mut heartbeat_interval = interval(Duration::from_secs(10));
        
        loop {
            tokio::select! {
                // Receive message from master
                result = connection.receive_message() => {
                    match result {
                        Ok(message) => {
                            match message {
                                ReplicationMessage::IncrementalSync { entries } => {
                                    let start_time = Instant::now();
                                    let last_seq = sync_manager.apply_incremental_sync(entries).await?;
                                    
                                    // Calculate and update replication lag
                                    let lag = start_time.elapsed().as_millis() as u64;
                                    replication_lag.store(lag, Ordering::Relaxed);
                                    
                                    // Send acknowledgment
                                    let ack = ReplicationMessage::ack_success(last_seq);
                                    connection.send_message(&ack).await?;
                                    
                                    // Update stats
                                    {
                                        let mut stats = stats.write().await;
                                        stats.messages_received += 1;
                                        stats.replication_lag_ms = lag;
                                        stats.last_sync = Some(chrono::Utc::now());
                                    }
                                }
                                ReplicationMessage::Heartbeat { current_sequence, .. } => {
                                    debug!("Received heartbeat from master (sequence: {})", current_sequence);
                                    
                                    // Update stats
                                    {
                                        let mut stats = stats.write().await;
                                        stats.messages_received += 1;
                                    }
                                }
                                ReplicationMessage::MasterShutdown { reason } => {
                                    warn!("Master is shutting down: {}", reason);
                                    return Err(ReplicationError::ConnectionError(
                                        "Master shutdown".to_string()
                                    ));
                                }
                                _ => {
                                    warn!("Unexpected message from master: {}", message.message_type());
                                }
                            }
                        }
                        Err(e) => {
                            error!("Error receiving from master: {}", e);
                            return Err(e);
                        }
                    }
                }
                
                // Send periodic heartbeat
                _ = heartbeat_interval.tick() => {
                    let heartbeat = ReplicationMessage::heartbeat(sync_manager.current_sequence());
                    if let Err(e) = connection.send_message(&heartbeat).await {
                        error!("Failed to send heartbeat to master: {}", e);
                        return Err(e);
                    }
                    
                    // Update stats
                    {
                        let mut stats = stats.write().await;
                        stats.messages_sent += 1;
                    }
                }
            }
        }
    }

    /// Start background tasks
    async fn start_background_tasks(&self) {
        // Cleanup disconnected slaves (master only)
        if matches!(*self.role.read().await, NodeRole::Master) {
            let slave_manager = Arc::clone(&self.slave_manager);
            tokio::spawn(async move {
                let mut cleanup_interval = interval(Duration::from_secs(30));
                
                loop {
                    cleanup_interval.tick().await;
                    
                    if let Some(ref mut manager) = *slave_manager.lock().await {
                        manager.cleanup_disconnected().await;
                    }
                }
            });
        }
    }

    /// Broadcast WAL entry to all slaves (master only)
    pub async fn broadcast_wal_entry(&self, entry: crate::wal::WalEntry) -> ReplicationResult<()> {
        let role = self.role.read().await;
        if !matches!(*role, NodeRole::Master) {
            return Err(ReplicationError::NotMaster);
        }

        let slave_manager = self.slave_manager.lock().await;
        if let Some(ref manager) = *slave_manager {
            let message = ReplicationMessage::IncrementalSync {
                entries: vec![entry],
            };
            
            let sent_count = manager.broadcast_message(message).await;
            debug!("Broadcasted WAL entry to {} slaves", sent_count);
            
            // Update stats
            {
                let mut stats = self.stats.write().await;
                stats.messages_sent += sent_count as u64;
            }
        }

        Ok(())
    }

    /// Promote this slave to master
    pub async fn promote_to_master(&self) -> ReplicationResult<()> {
        let mut role = self.role.write().await;
        if matches!(*role, NodeRole::Master) {
            return Ok(()); // Already master
        }

        info!("Promoting slave to master");

        // Close connection to old master
        let mut master_conn = self.master_connection.lock().await;
        if let Some(connection) = master_conn.take() {
            connection.close().await?;
        }

        // Change role to master
        *role = NodeRole::Master;

        // Initialize slave manager
        let slave_manager = SlaveConnectionManager::new(self.config.max_slaves);
        *self.slave_manager.lock().await = Some(slave_manager);

        // Start master mode operations
        self.start_master_operations().await?;

        info!("Successfully promoted to master");
        Ok(())
    }

    /// Start master operations after promotion
    async fn start_master_operations(&self) -> ReplicationResult<()> {
        // Start listening for new slave connections
        let bind_addr = "0.0.0.0:50052".parse().unwrap(); // TODO: Make configurable
        let mut listener = ReplicationListener::bind(bind_addr).await?;

        let slave_manager = Arc::clone(&self.slave_manager);
        let sync_manager = Arc::clone(&self.sync_manager);
        let stats = Arc::clone(&self.stats);

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok(connection) => {
                        info!("New slave connection from {} after promotion", connection.peer_addr());
                        
                        if let Some(ref mut manager) = *slave_manager.lock().await {
                            if let Err(e) = Self::handle_new_slave(
                                connection, 
                                manager, 
                                Arc::clone(&sync_manager),
                                Arc::clone(&stats)
                            ).await {
                                error!("Failed to handle new slave after promotion: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to accept slave connection after promotion: {}", e);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });

        Ok(())
    }

    /// Check if this node can perform read operations
    pub async fn can_read(&self) -> bool {
        // Both master and slave nodes can handle read operations
        true
    }

    /// Check if this node can perform write operations
    pub async fn can_write(&self) -> bool {
        // Only master nodes can handle write operations
        matches!(*self.role.read().await, NodeRole::Master)
    }

    /// Get the maximum number of slaves this master can support
    pub fn max_slaves(&self) -> usize {
        self.config.max_slaves
    }

    /// Force reconnection to master (slave only)
    pub async fn force_reconnect(&self) -> ReplicationResult<()> {
        let role = self.role.read().await;
        if let NodeRole::Slave { master_addr } = *role {
            // Close existing connection
            let mut master_conn = self.master_connection.lock().await;
            if let Some(connection) = master_conn.take() {
                connection.close().await?;
            }

            info!("Forcing reconnection to master at {}", master_addr);
            
            // The background task will automatically attempt to reconnect
            Ok(())
        } else {
            Err(ReplicationError::NotSlave)
        }
    }

    /// Get replication statistics
    pub async fn get_stats(&self) -> ReplicationStats {
        let mut stats = self.stats.read().await.clone();
        stats.replication_lag_ms = self.replication_lag.load(Ordering::Relaxed);
        
        if let Some(ref manager) = *self.slave_manager.lock().await {
            stats.connected_slaves = manager.slave_count();
        }
        
        stats
    }

    /// Get current node role
    pub async fn get_role(&self) -> NodeRole {
        self.role.read().await.clone()
    }

    /// Handle read operation (allowed on both master and slave)
    pub async fn handle_read_operation<T, F, Fut>(&self, operation: F) -> ReplicationResult<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = ReplicationResult<T>>,
    {
        // Read operations are allowed on both master and slave nodes
        operation().await
    }

    /// Handle write operation (only allowed on master)
    pub async fn handle_write_operation<T, F, Fut>(&self, operation: F) -> ReplicationResult<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = ReplicationResult<T>>,
    {
        // Check if this node can handle writes
        if !self.can_write().await {
            return Err(ReplicationError::NotMaster);
        }

        // Execute the write operation
        let result = operation().await?;

        // Note: In a real implementation, this would also trigger WAL entry creation
        // and broadcasting to slaves

        Ok(result)
    }

    /// Get cluster health information
    pub async fn get_cluster_health(&self) -> ClusterHealth {
        let role = self.role.read().await.clone();
        let stats = self.get_stats().await;

        match role {
            NodeRole::Master => {
                let slave_manager = self.slave_manager.lock().await;
                let (total_slaves, healthy_slaves) = if let Some(ref manager) = *slave_manager {
                    (manager.slave_count(), manager.healthy_slave_count())
                } else {
                    (0, 0)
                };

                ClusterHealth {
                    role: role.clone(),
                    is_healthy: true, // Master is always considered healthy if running
                    total_slaves,
                    healthy_slaves,
                    replication_lag_ms: 0, // Master has no lag
                    can_read: true,
                    can_write: true,
                }
            }
            NodeRole::Slave { master_addr } => {
                let is_connected = self.master_connection.lock().await.is_some();
                
                ClusterHealth {
                    role: role.clone(),
                    is_healthy: is_connected,
                    total_slaves: 0, // Slaves don't track other slaves
                    healthy_slaves: 0,
                    replication_lag_ms: stats.replication_lag_ms,
                    can_read: true,
                    can_write: false,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_replication_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let wal_dir = temp_dir.path().join("wal");
        let snapshot_dir = temp_dir.path().join("snapshots");
        
        tokio::fs::create_dir_all(&wal_dir).await.unwrap();
        tokio::fs::create_dir_all(&snapshot_dir).await.unwrap();
        
        let config = ReplicationConfig::default();
        let sync_manager = SyncManager::new(&wal_dir, &snapshot_dir);
        let manager = ReplicationManager::new(config, sync_manager);
        
        assert!(matches!(manager.get_role().await, NodeRole::Master));
    }

    #[tokio::test]
    async fn test_promote_to_master() {
        let temp_dir = TempDir::new().unwrap();
        let wal_dir = temp_dir.path().join("wal");
        let snapshot_dir = temp_dir.path().join("snapshots");
        
        tokio::fs::create_dir_all(&wal_dir).await.unwrap();
        tokio::fs::create_dir_all(&snapshot_dir).await.unwrap();
        
        let config = ReplicationConfig {
            role: NodeRole::Slave {
                master_addr: "127.0.0.1:50051".parse().unwrap(),
            },
            ..Default::default()
        };
        
        let sync_manager = SyncManager::new(&wal_dir, &snapshot_dir);
        let manager = ReplicationManager::new(config, sync_manager);
        
        assert!(matches!(manager.get_role().await, NodeRole::Slave { .. }));
        
        manager.promote_to_master().await.unwrap();
        assert!(matches!(manager.get_role().await, NodeRole::Master));
    }
}

/// Cluster health information
#[derive(Debug, Clone)]
pub struct ClusterHealth {
    /// Current node role
    pub role: NodeRole,
    /// Whether this node is healthy
    pub is_healthy: bool,
    /// Total number of slaves (master only)
    pub total_slaves: usize,
    /// Number of healthy slaves (master only)
    pub healthy_slaves: usize,
    /// Replication lag in milliseconds (slave only)
    pub replication_lag_ms: u64,
    /// Whether this node can handle read operations
    pub can_read: bool,
    /// Whether this node can handle write operations
    pub can_write: bool,
}