//! Replication connection management

use crate::replication::{ReplicationMessage, ReplicationError, ReplicationResult, BackoffConfig};
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Connection to a replication peer (master or slave)
pub struct ReplicationConnection {
    /// TCP stream
    stream: TcpStream,
    /// Peer address
    peer_addr: SocketAddr,
    /// Connection ID for logging
    connection_id: String,
    /// Send timeout
    send_timeout: Duration,
    /// Receive timeout
    recv_timeout: Duration,
}

impl ReplicationConnection {
    /// Create a new replication connection
    pub fn new(stream: TcpStream, connection_id: String) -> ReplicationResult<Self> {
        let peer_addr = stream.peer_addr().map_err(ReplicationError::IoError)?;
        
        Ok(Self {
            stream,
            peer_addr,
            connection_id,
            send_timeout: Duration::from_secs(30),
            recv_timeout: Duration::from_secs(30),
        })
    }

    /// Get the peer address
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }

    /// Get the connection ID
    pub fn connection_id(&self) -> &str {
        &self.connection_id
    }

    /// Send a replication message
    pub async fn send_message(&mut self, message: &ReplicationMessage) -> ReplicationResult<()> {
        let bytes = message.to_bytes()?;
        let length = bytes.len() as u32;

        // Send length prefix (4 bytes) followed by message
        let send_future = async {
            self.stream.write_all(&length.to_le_bytes()).await?;
            self.stream.write_all(&bytes).await?;
            self.stream.flush().await?;
            Ok::<(), std::io::Error>(())
        };

        timeout(self.send_timeout, send_future)
            .await
            .map_err(|_| ReplicationError::Timeout)?
            .map_err(ReplicationError::IoError)?;

        debug!(
            "Sent {} message to {} ({})",
            message.message_type(),
            self.peer_addr,
            self.connection_id
        );

        Ok(())
    }

    /// Receive a replication message
    pub async fn receive_message(&mut self) -> ReplicationResult<ReplicationMessage> {
        let recv_future = async {
            // Read length prefix (4 bytes)
            let mut length_bytes = [0u8; 4];
            self.stream.read_exact(&mut length_bytes).await?;
            let length = u32::from_le_bytes(length_bytes) as usize;

            // Validate message size (max 100MB)
            if length > 100 * 1024 * 1024 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Message too large: {} bytes", length),
                ));
            }

            // Read message data
            let mut buffer = vec![0u8; length];
            self.stream.read_exact(&mut buffer).await?;

            Ok(buffer)
        };

        let buffer = timeout(self.recv_timeout, recv_future)
            .await
            .map_err(|_| ReplicationError::Timeout)?
            .map_err(ReplicationError::IoError)?;

        let message = ReplicationMessage::from_bytes(&buffer)?;

        debug!(
            "Received {} message from {} ({})",
            message.message_type(),
            self.peer_addr,
            self.connection_id
        );

        Ok(message)
    }

    /// Check if the connection is still alive
    pub async fn is_alive(&mut self) -> bool {
        // Try to send a heartbeat and see if it succeeds
        let heartbeat = ReplicationMessage::heartbeat(0);
        self.send_message(&heartbeat).await.is_ok()
    }

    /// Close the connection
    pub async fn close(mut self) -> ReplicationResult<()> {
        self.stream.shutdown().await.map_err(ReplicationError::IoError)?;
        info!("Closed connection to {} ({})", self.peer_addr, self.connection_id);
        Ok(())
    }
}

/// Manages multiple slave connections for a master node
pub struct SlaveConnectionManager {
    /// Active slave connections
    connections: Vec<SlaveConnectionHandle>,
    /// Maximum number of slaves
    max_slaves: usize,
    /// Message broadcast channel
    broadcast_tx: mpsc::UnboundedSender<ReplicationMessage>,
    /// Receiver for broadcast messages
    broadcast_rx: mpsc::UnboundedReceiver<ReplicationMessage>,
}

impl SlaveConnectionManager {
    /// Create a new slave connection manager
    pub fn new(max_slaves: usize) -> Self {
        let (broadcast_tx, broadcast_rx) = mpsc::unbounded_channel();
        
        Self {
            connections: Vec::new(),
            max_slaves,
            broadcast_tx,
            broadcast_rx,
        }
    }

    /// Add a new slave connection
    pub async fn add_slave(&mut self, mut connection: ReplicationConnection) -> ReplicationResult<()> {
        if self.connections.len() >= self.max_slaves {
            return Err(ReplicationError::SlaveLimit(self.max_slaves));
        }

        let connection_id = connection.connection_id().to_string();
        let peer_addr = connection.peer_addr();

        // Create channels for this slave
        let (tx, mut rx) = mpsc::unbounded_channel::<ReplicationMessage>();
        
        // Clone connection_id for the task
        let task_connection_id = connection_id.clone();
        
        // Spawn task to handle this slave
        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Receive message to send to slave
                    msg = rx.recv() => {
                        match msg {
                            Some(message) => {
                                if let Err(e) = connection.send_message(&message).await {
                                    error!("Failed to send message to slave {}: {}", task_connection_id, e);
                                    break;
                                }
                            }
                            None => {
                                debug!("Slave {} channel closed", task_connection_id);
                                break;
                            }
                        }
                    }
                    
                    // Receive message from slave
                    result = connection.receive_message() => {
                        match result {
                            Ok(message) => {
                                debug!("Received message from slave {}: {}", task_connection_id, message.message_type());
                                // Handle slave messages (heartbeats, acks, etc.)
                            }
                            Err(e) => {
                                warn!("Error receiving from slave {}: {}", task_connection_id, e);
                                break;
                            }
                        }
                    }
                }
            }

            info!("Slave connection {} terminated", task_connection_id);
        });

        let slave_handle = SlaveConnectionHandle {
            connection_id: connection_id.clone(),
            peer_addr,
            sender: tx,
            handle,
        };

        self.connections.push(slave_handle);
        info!("Added slave connection: {} ({})", peer_addr, connection_id);

        Ok(())
    }

    /// Broadcast a message to all connected slaves
    pub async fn broadcast_message(&self, message: ReplicationMessage) -> usize {
        let mut sent_count = 0;

        for slave in &self.connections {
            if slave.sender.send(message.clone()).is_ok() {
                sent_count += 1;
            } else {
                warn!("Failed to send message to slave {}", slave.connection_id);
            }
        }

        debug!("Broadcasted {} message to {} slaves", message.message_type(), sent_count);
        sent_count
    }

    /// Remove disconnected slaves
    pub async fn cleanup_disconnected(&mut self) {
        let mut i = 0;
        while i < self.connections.len() {
            if self.connections[i].handle.is_finished() {
                let slave = self.connections.remove(i);
                info!("Removed disconnected slave: {}", slave.connection_id);
            } else {
                i += 1;
            }
        }
    }

    /// Get the number of connected slaves
    pub fn slave_count(&self) -> usize {
        self.connections.len()
    }

    /// Get slave connection information
    pub fn get_slave_info(&self) -> Vec<SlaveInfo> {
        self.connections
            .iter()
            .map(|slave| SlaveInfo {
                connection_id: slave.connection_id.clone(),
                peer_addr: slave.peer_addr,
                connected: !slave.handle.is_finished(),
            })
            .collect()
    }

    /// Check if we can accept more slaves
    pub fn can_accept_more_slaves(&self) -> bool {
        self.connections.len() < self.max_slaves
    }

    /// Get the number of healthy (connected) slaves
    pub fn healthy_slave_count(&self) -> usize {
        self.connections
            .iter()
            .filter(|slave| !slave.handle.is_finished())
            .count()
    }

    /// Disconnect a specific slave by connection ID
    pub async fn disconnect_slave(&mut self, connection_id: &str) -> bool {
        if let Some(pos) = self.connections
            .iter()
            .position(|slave| slave.connection_id == connection_id) {
            
            let slave = self.connections.remove(pos);
            slave.handle.abort();
            info!("Disconnected slave: {}", connection_id);
            true
        } else {
            false
        }
    }

    /// Force synchronization with all connected slaves
    pub async fn force_sync(&self) -> usize {
        // Create a sync message (using heartbeat as sync trigger for now)
        let sync_message = ReplicationMessage::heartbeat(0);
        
        let mut synced_count = 0;
        for slave in &self.connections {
            if slave.sender.send(sync_message.clone()).is_ok() {
                synced_count += 1;
                debug!("Triggered sync for slave {}", slave.connection_id);
            } else {
                warn!("Failed to trigger sync for slave {}", slave.connection_id);
            }
        }
        
        info!("Triggered sync for {} out of {} slaves", synced_count, self.connections.len());
        synced_count
    }
}

/// Handle for a slave connection
struct SlaveConnectionHandle {
    /// Connection ID
    connection_id: String,
    /// Peer address
    peer_addr: SocketAddr,
    /// Message sender
    sender: mpsc::UnboundedSender<ReplicationMessage>,
    /// Task handle
    handle: tokio::task::JoinHandle<()>,
}

/// Information about a slave connection
#[derive(Debug, Clone)]
pub struct SlaveInfo {
    /// Connection ID
    pub connection_id: String,
    /// Peer address
    pub peer_addr: SocketAddr,
    /// Whether the slave is connected
    pub connected: bool,
}

/// Exponential backoff implementation
pub struct ExponentialBackoff {
    /// Current backoff duration
    current: Duration,
    /// Configuration
    config: BackoffConfig,
    /// Number of attempts
    attempts: u32,
}

impl ExponentialBackoff {
    /// Create a new exponential backoff
    pub fn new(config: BackoffConfig) -> Self {
        Self {
            current: config.initial,
            config,
            attempts: 0,
        }
    }

    /// Get the next backoff duration
    pub fn next(&mut self) -> Duration {
        let backoff = self.current;
        
        self.attempts += 1;
        self.current = Duration::from_secs_f64(
            (self.current.as_secs_f64() * self.config.multiplier).min(self.config.max.as_secs_f64())
        );

        debug!("Backoff attempt {}: waiting {:?}", self.attempts, backoff);
        backoff
    }

    /// Reset the backoff to initial value
    pub fn reset(&mut self) {
        self.current = self.config.initial;
        self.attempts = 0;
        debug!("Backoff reset");
    }

    /// Get the number of attempts
    pub fn attempts(&self) -> u32 {
        self.attempts
    }
}

/// Replication listener for accepting slave connections
pub struct ReplicationListener {
    /// TCP listener
    listener: TcpListener,
    /// Bind address
    bind_addr: SocketAddr,
}

impl ReplicationListener {
    /// Create a new replication listener
    pub async fn bind(addr: SocketAddr) -> ReplicationResult<Self> {
        let listener = TcpListener::bind(addr).await.map_err(ReplicationError::IoError)?;
        let bind_addr = listener.local_addr().map_err(ReplicationError::IoError)?;
        
        info!("Replication listener bound to {}", bind_addr);
        
        Ok(Self {
            listener,
            bind_addr,
        })
    }

    /// Accept a new slave connection
    pub async fn accept(&mut self) -> ReplicationResult<ReplicationConnection> {
        let (stream, peer_addr) = self.listener.accept().await.map_err(ReplicationError::IoError)?;
        
        let connection_id = format!("slave-{}-{}", peer_addr, Instant::now().elapsed().as_millis());
        let connection = ReplicationConnection::new(stream, connection_id)?;
        
        info!("Accepted slave connection from {}", peer_addr);
        Ok(connection)
    }

    /// Get the bind address
    pub fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_exponential_backoff() {
        let config = BackoffConfig {
            initial: Duration::from_millis(100),
            max: Duration::from_secs(1),
            multiplier: 2.0,
        };

        let mut backoff = ExponentialBackoff::new(config);
        
        assert_eq!(backoff.next(), Duration::from_millis(100));
        assert_eq!(backoff.attempts(), 1);
        
        assert_eq!(backoff.next(), Duration::from_millis(200));
        assert_eq!(backoff.attempts(), 2);
        
        backoff.reset();
        assert_eq!(backoff.attempts(), 0);
        assert_eq!(backoff.next(), Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_slave_connection_manager() {
        let manager = SlaveConnectionManager::new(2);
        assert_eq!(manager.slave_count(), 0);
        assert_eq!(manager.max_slaves, 2);
    }

    #[tokio::test]
    async fn test_replication_listener() {
        let listener = ReplicationListener::bind("127.0.0.1:0".parse().unwrap()).await.unwrap();
        assert!(listener.bind_addr().port() > 0);
    }
}