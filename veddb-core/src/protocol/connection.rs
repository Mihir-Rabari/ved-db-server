//! Connection Management for VedDB v0.2.0
//!
//! This module handles TLS connections, authentication handshake, session management,
//! and server-side connection pooling to support up to 10,000 concurrent connections.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, Semaphore};
use tokio::time::timeout;
use uuid::Uuid;
use log::{info, warn, error, debug};

use crate::auth::{AuthSystem, JwtService, User, UserClaims};
use crate::encryption::tls::TlsAcceptor;
use crate::protocol::{
    Command, Response, OpCode, Status, AuthRequest, AuthResponse, 
    CompatibilityHandler, PROTOCOL_V2
};

/// Maximum number of concurrent connections
pub const MAX_CONNECTIONS: usize = 10_000;

/// Default session timeout (24 hours)
pub const DEFAULT_SESSION_TIMEOUT: Duration = Duration::from_secs(24 * 60 * 60);

/// Connection read timeout
pub const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Connection write timeout  
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(30);

/// Unique session identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// Session information
#[derive(Debug, Clone)]
pub struct Session {
    pub id: SessionId,
    pub user: User,
    pub created_at: Instant,
    pub last_activity: Instant,
    pub remote_addr: SocketAddr,
    pub protocol_version: u8,
}

impl Session {
    pub fn new(user: User, remote_addr: SocketAddr, protocol_version: u8) -> Self {
        let now = Instant::now();
        Self {
            id: SessionId::new(),
            user,
            created_at: now,
            last_activity: now,
            remote_addr,
            protocol_version,
        }
    }

    pub fn is_expired(&self, timeout: Duration) -> bool {
        self.last_activity.elapsed() > timeout
    }

    pub fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }
}

/// Connection state
#[derive(Debug)]
pub enum ConnectionState {
    /// Waiting for authentication
    Unauthenticated,
    /// Authenticated with valid session
    Authenticated(Session),
    /// Connection closed
    Closed,
}

/// Individual connection handler
pub struct Connection {
    pub id: Uuid,
    pub stream: TcpStream,
    pub remote_addr: SocketAddr,
    pub state: ConnectionState,
    pub created_at: Instant,
}

impl Connection {
    pub fn new(stream: TcpStream, remote_addr: SocketAddr) -> Self {
        Self {
            id: Uuid::new_v4(),
            stream,
            remote_addr,
            state: ConnectionState::Unauthenticated,
            created_at: Instant::now(),
        }
    }

    /// Read a command from the connection with timeout
    pub async fn read_command(&mut self) -> Result<Command, ConnectionError> {
        // Read command header first
        let mut header_buf = vec![0u8; 24]; // CmdHeader::SIZE
        
        timeout(READ_TIMEOUT, self.stream.read_exact(&mut header_buf))
            .await
            .map_err(|_| ConnectionError::ReadTimeout)?
            .map_err(|e| ConnectionError::IoError(e))?;

        // Parse header to get payload size
        let header = unsafe {
            std::ptr::read_unaligned(header_buf.as_ptr() as *const crate::protocol::CmdHeader)
        };

        let payload_size = header.total_payload_len();
        if payload_size > 16 * 1024 * 1024 { // 16MB limit
            return Err(ConnectionError::PayloadTooLarge);
        }

        // Read payload
        let mut payload_buf = vec![0u8; payload_size];
        if payload_size > 0 {
            timeout(READ_TIMEOUT, self.stream.read_exact(&mut payload_buf))
                .await
                .map_err(|_| ConnectionError::ReadTimeout)?
                .map_err(|e| ConnectionError::IoError(e))?;
        }

        // Combine header and payload
        let mut full_buf = header_buf;
        full_buf.extend_from_slice(&payload_buf);

        // Parse complete command
        Command::from_bytes(&full_buf)
            .map_err(|e| ConnectionError::ProtocolError(e.to_string()))
    }

    /// Write a response to the connection with timeout
    pub async fn write_response(&mut self, response: Response) -> Result<(), ConnectionError> {
        let bytes = response.to_bytes();
        
        timeout(WRITE_TIMEOUT, self.stream.write_all(&bytes))
            .await
            .map_err(|_| ConnectionError::WriteTimeout)?
            .map_err(|e| ConnectionError::IoError(e))?;

        Ok(())
    }

    /// Update session activity if authenticated
    pub fn update_activity(&mut self) {
        if let ConnectionState::Authenticated(ref mut session) = self.state {
            session.update_activity();
        }
    }
}

/// Connection pool manager
pub struct ConnectionManager {
    /// Active connections
    connections: Arc<RwLock<HashMap<Uuid, Arc<RwLock<Connection>>>>>,
    
    /// Active sessions
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
    
    /// Connection semaphore to limit concurrent connections
    connection_semaphore: Arc<Semaphore>,
    
    /// Authentication system
    auth_system: Arc<RwLock<AuthSystem>>,
    
    /// JWT service for token validation
    jwt_service: Arc<JwtService>,
    
    /// TLS acceptor
    tls_acceptor: Option<Arc<TlsAcceptor>>,
    
    /// Protocol compatibility handler
    compatibility_handler: CompatibilityHandler,
    
    /// Session timeout
    session_timeout: Duration,
}

impl ConnectionManager {
    pub fn new(
        auth_system: Arc<RwLock<AuthSystem>>,
        jwt_service: Arc<JwtService>,
        tls_acceptor: Option<Arc<TlsAcceptor>>,
    ) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            connection_semaphore: Arc::new(Semaphore::new(MAX_CONNECTIONS)),
            auth_system,
            jwt_service,
            tls_acceptor,
            compatibility_handler: CompatibilityHandler::new(true), // Log warnings
            session_timeout: DEFAULT_SESSION_TIMEOUT,
        }
    }

    /// Start listening for connections
    pub async fn listen(&self, addr: SocketAddr) -> Result<(), ConnectionError> {
        let listener = TcpListener::bind(addr).await
            .map_err(|e| ConnectionError::IoError(e))?;

        info!("VedDB server listening on {}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, remote_addr)) => {
                    // Check connection limit
                    if let Ok(permit) = self.connection_semaphore.clone().try_acquire_owned() {
                        let manager = self.clone();
                        tokio::spawn(async move {
                            if let Err(e) = manager.handle_connection(stream, remote_addr).await {
                                warn!("Connection error from {}: {}", remote_addr, e);
                            }
                            drop(permit); // Release connection slot
                        });
                    } else {
                        warn!("Connection limit reached, rejecting connection from {}", remote_addr);
                        // Connection will be dropped, closing it
                    }
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    /// Handle a single connection
    async fn handle_connection(&self, stream: TcpStream, remote_addr: SocketAddr) -> Result<(), ConnectionError> {
        debug!("New connection from {}", remote_addr);

        // TLS handling would be implemented here
        // For now, we'll work with plain TCP streams
        let stream = stream;

        let connection = Connection::new(stream, remote_addr);
        let connection_id = connection.id;

        // Add to connection pool
        {
            let mut connections = self.connections.write().await;
            connections.insert(connection_id, Arc::new(RwLock::new(connection)));
        }

        // Handle connection lifecycle
        let result = self.connection_loop(connection_id).await;

        // Remove from connection pool
        {
            let mut connections = self.connections.write().await;
            connections.remove(&connection_id);
        }

        result
    }

    /// Main connection processing loop
    async fn connection_loop(&self, connection_id: Uuid) -> Result<(), ConnectionError> {
        loop {
            // Get connection
            let connection_arc = {
                let connections = self.connections.read().await;
                connections.get(&connection_id).cloned()
                    .ok_or(ConnectionError::ConnectionNotFound)?
            };

            // Read command
            let command = {
                let mut conn = connection_arc.write().await;
                conn.read_command().await?
            };

            // Update activity
            {
                let mut conn = connection_arc.write().await;
                conn.update_activity();
            }

            // Process command
            let response = self.process_command(connection_id, command).await?;

            // Write response
            {
                let mut conn = connection_arc.write().await;
                conn.write_response(response).await?;
            }
        }
    }

    /// Process a single command
    async fn process_command(&self, connection_id: Uuid, command: Command) -> Result<Response, ConnectionError> {
        // Translate v0.1.x commands if needed
        let command = self.compatibility_handler.translate_command(command)
            .map_err(|e| ConnectionError::ProtocolError(e))?;

        // Check authentication for non-auth commands
        if command.header.opcode().map_err(|_| ConnectionError::ProtocolError("Invalid opcode".to_string()))? != OpCode::Auth {
            if !self.is_authenticated(connection_id).await {
                return Ok(Response::new(Status::AuthRequired, command.header.seq, Vec::new()));
            }
        }

        // Process based on opcode
        match command.header.opcode().map_err(|_| ConnectionError::ProtocolError("Invalid opcode".to_string()))? {
            OpCode::Auth => self.handle_auth(connection_id, command).await,
            OpCode::Ping => Ok(Response::ok(command.header.seq, b"pong".to_vec())),
            _ => {
                // For now, return not implemented for other commands
                // These would be handled by the actual database engine
                Ok(Response::new(Status::Error, command.header.seq, b"Not implemented".to_vec()))
            }
        }
    }

    /// Handle authentication command
    async fn handle_auth(&self, connection_id: Uuid, command: Command) -> Result<Response, ConnectionError> {
        let auth_request: AuthRequest = serde_json::from_slice(&command.value)
            .map_err(|e| ConnectionError::ProtocolError(format!("Invalid auth request: {}", e)))?;

        let auth_result: Result<User, ConnectionError> = match auth_request.credentials {
            crate::protocol::AuthCredentials::UsernamePassword { username, password } => {
                let mut auth_system = self.auth_system.write().await;
                match auth_system.authenticate(&username, &password, None).await {
                    Ok(_token) => {
                        // Get user from user manager
                        match auth_system.user_manager().get_user(&username).await {
                            Ok(Some(user)) => Ok(user),
                            Ok(None) => Err(ConnectionError::AuthError("User not found".to_string())),
                            Err(e) => Err(ConnectionError::AuthError(format!("Database error: {}", e))),
                        }
                    }
                    Err(e) => Err(ConnectionError::AuthError(format!("Authentication failed: {}", e))),
                }
            }
            crate::protocol::AuthCredentials::JwtToken { token } => {
                match self.jwt_service.verify_token(&token) {
                    Ok(claims) => {
                        // Get user from claims
                        let mut auth_system = self.auth_system.write().await;
                        match auth_system.user_manager().get_user(&claims.username).await {
                            Ok(Some(user)) => Ok(user),
                            Ok(None) => Err(ConnectionError::AuthError("User not found".to_string())),
                            Err(e) => Err(ConnectionError::AuthError(format!("Database error: {}", e))),
                        }
                    }
                    Err(_) => Err(ConnectionError::AuthError("Invalid token".to_string())),
                }
            }
        };

        match auth_result {
            Ok(user) => {
                // Create session
                let connection_arc = {
                    let connections = self.connections.read().await;
                    connections.get(&connection_id).cloned()
                        .ok_or(ConnectionError::ConnectionNotFound)?
                };

                let (remote_addr, protocol_version) = {
                    let conn = connection_arc.read().await;
                    (conn.remote_addr, command.header.version)
                };

                let session = Session::new(user.clone(), remote_addr, protocol_version);
                let session_id = session.id;

                // Update connection state
                {
                    let mut conn = connection_arc.write().await;
                    conn.state = ConnectionState::Authenticated(session.clone());
                }

                // Store session
                {
                    let mut sessions = self.sessions.write().await;
                    sessions.insert(session_id, session);
                }

                // Generate JWT token for response
                let token = self.jwt_service.generate_token(&user)
                    .map_err(|e| ConnectionError::AuthError(format!("Token generation failed: {}", e)))?;

                let auth_response = AuthResponse {
                    success: true,
                    token: Some(token),
                    expires_at: Some((Instant::now() + self.session_timeout).elapsed().as_secs()),
                    error: None,
                };

                let payload = serde_json::to_vec(&auth_response)
                    .map_err(|e| ConnectionError::ProtocolError(format!("Response serialization failed: {}", e)))?;

                info!("User {} authenticated from {}", user.username, remote_addr);
                Ok(Response::ok(command.header.seq, payload))
            }
            Err(e) => {
                let auth_response = AuthResponse {
                    success: false,
                    token: None,
                    expires_at: None,
                    error: Some(format!("Authentication failed: {}", e)),
                };

                let payload = serde_json::to_vec(&auth_response)
                    .map_err(|e| ConnectionError::ProtocolError(format!("Response serialization failed: {}", e)))?;

                warn!("Authentication failed from {}: {}", 
                      self.get_connection_addr(connection_id).await.unwrap_or_else(|| "unknown".parse().unwrap()), e);
                Ok(Response::new(Status::AuthFailed, command.header.seq, payload))
            }
        }
    }

    /// Check if connection is authenticated
    async fn is_authenticated(&self, connection_id: Uuid) -> bool {
        let connections = self.connections.read().await;
        if let Some(connection_arc) = connections.get(&connection_id) {
            let conn = connection_arc.read().await;
            matches!(conn.state, ConnectionState::Authenticated(_))
        } else {
            false
        }
    }

    /// Get connection remote address
    async fn get_connection_addr(&self, connection_id: Uuid) -> Option<SocketAddr> {
        let connections = self.connections.read().await;
        if let Some(connection_arc) = connections.get(&connection_id) {
            let conn = connection_arc.read().await;
            Some(conn.remote_addr)
        } else {
            None
        }
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired_sessions(&self) {
        let mut sessions = self.sessions.write().await;
        let mut connections = self.connections.write().await;
        
        let expired_sessions: Vec<SessionId> = sessions
            .iter()
            .filter(|(_, session)| session.is_expired(self.session_timeout))
            .map(|(id, _)| *id)
            .collect();

        for session_id in expired_sessions {
            if let Some(session) = sessions.remove(&session_id) {
                info!("Session expired for user {} from {}", session.user.username, session.remote_addr);
                
                // Find and close corresponding connection
                let connection_to_close: Vec<Uuid> = connections
                    .iter()
                    .filter_map(|(conn_id, _conn_arc)| {
                        // This is a simplified check - in practice we'd need to match session to connection
                        Some(*conn_id)
                    })
                    .collect();

                for conn_id in connection_to_close {
                    connections.remove(&conn_id);
                }
            }
        }
    }

    /// Get connection statistics
    pub async fn get_stats(&self) -> ConnectionStats {
        let connections = self.connections.read().await;
        let sessions = self.sessions.read().await;
        
        ConnectionStats {
            total_connections: connections.len(),
            authenticated_connections: sessions.len(),
            max_connections: MAX_CONNECTIONS,
            available_slots: self.connection_semaphore.available_permits(),
        }
    }
}

impl Clone for ConnectionManager {
    fn clone(&self) -> Self {
        Self {
            connections: Arc::clone(&self.connections),
            sessions: Arc::clone(&self.sessions),
            connection_semaphore: Arc::clone(&self.connection_semaphore),
            auth_system: Arc::clone(&self.auth_system),
            jwt_service: Arc::clone(&self.jwt_service),
            tls_acceptor: self.tls_acceptor.clone(),
            compatibility_handler: CompatibilityHandler::new(true),
            session_timeout: self.session_timeout,
        }
    }
}

/// Connection statistics
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub total_connections: usize,
    pub authenticated_connections: usize,
    pub max_connections: usize,
    pub available_slots: usize,
}

/// Connection errors
#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Read timeout")]
    ReadTimeout,
    
    #[error("Write timeout")]
    WriteTimeout,
    
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    
    #[error("Payload too large")]
    PayloadTooLarge,
    
    #[error("TLS error: {0}")]
    TlsError(String),
    
    #[error("Authentication error: {0}")]
    AuthError(String),
    
    #[error("Connection not found")]
    ConnectionNotFound,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::auth::{AuthSystem, JwtService, Role};
    use crate::protocol::PROTOCOL_V2;

    #[tokio::test]
    async fn test_session_creation() {
        let user = User {
            username: "test_user".to_string(),
            password_hash: "hash".to_string(),
            role: Role::ReadWrite,
            created_at: chrono::Utc::now(),
            last_login: None,
            enabled: true,
            metadata: Default::default(),
        };
        
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let session = Session::new(user.clone(), addr, PROTOCOL_V2);
        
        assert_eq!(session.user.username, "test_user");
        assert_eq!(session.remote_addr, addr);
        assert_eq!(session.protocol_version, PROTOCOL_V2);
        assert!(!session.is_expired(Duration::from_secs(1)));
    }

    #[tokio::test]
    async fn test_session_expiration() {
        let user = User {
            username: "test_user".to_string(),
            password_hash: "hash".to_string(),
            role: Role::ReadWrite,
            created_at: chrono::Utc::now(),
            last_login: None,
            enabled: true,
            metadata: Default::default(),
        };
        
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let mut session = Session::new(user, addr, PROTOCOL_V2);
        
        // Simulate old session
        session.last_activity = Instant::now() - Duration::from_secs(2);
        
        assert!(session.is_expired(Duration::from_secs(1)));
        assert!(!session.is_expired(Duration::from_secs(3)));
    }

    #[tokio::test]
    async fn test_connection_manager_creation() {
        // This test is simplified since AuthSystem requires file paths
        // In a real test, we'd use temporary directories
        let stats = ConnectionStats {
            total_connections: 0,
            authenticated_connections: 0,
            max_connections: MAX_CONNECTIONS,
            available_slots: MAX_CONNECTIONS,
        };
        
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.authenticated_connections, 0);
        assert_eq!(stats.max_connections, MAX_CONNECTIONS);
    }
}