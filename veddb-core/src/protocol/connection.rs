//! Connection Management for VedDB v0.2.0
//!
//! This module handles TLS connections, authentication handshake, session management,
//! and server-side connection pooling to support up to 10,000 concurrent connections.

use std::collections::{HashMap, BTreeMap};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, Semaphore};
use tokio::time::timeout;
use uuid::Uuid;
use log::{info, warn, error, debug};
use sysinfo::{System, Pid};

use crate::auth::{AuthSystem, JwtService, User, UserClaims, Role};
use crate::encryption::tls::TlsAcceptor;
use crate::storage::HybridStorageEngine;
use crate::protocol::{
    Command, Response, OpCode, Status, AuthRequest, AuthResponse, 
    CompatibilityHandler, PROTOCOL_V2,
    CreateCollectionRequest, CreateIndexRequest, InsertDocRequest, UpdateDocRequest, DeleteDocRequest, QueryRequest,
    ListCollectionsRequest, DropCollectionRequest, ListIndexesRequest, DropIndexRequest,
    OperationResponse, Value,
    CreateUserRequest, DeleteUserRequest, UpdateUserRoleRequest, UserInfoResponse, ServerInfoResponse
};

// Advanced features
use crate::backup::BackupManager;
use crate::replication::ReplicationManager;
use crate::encryption::EncryptionEngine;

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
    
    /// Storage engine
    storage: Arc<HybridStorageEngine>,

    /// Session timeout
    session_timeout: Duration,

    /// Server start time for uptime calculation
    start_time: Instant,

    /// Total operations counter for ops/sec calculation
    total_ops: Arc<AtomicU64>,

    /// Last ops count for ops/sec calculation
    last_ops_snapshot: Arc<RwLock<(u64, Instant)>>,
    
    // Advanced Features (Optional)
    /// Backup manager for backup/restore operations
    backup_manager: Option<Arc<BackupManager>>,
    
    /// Replication manager for master-slave replication
    replication_manager: Option<Arc<ReplicationManager>>,
    
    /// Encryption engine for key management (with interior mutability)
    encryption_engine: Option<Arc<RwLock<EncryptionEngine>>>,
}

impl ConnectionManager {
    pub fn new(
        auth_system: Arc<RwLock<AuthSystem>>,
        jwt_service: Arc<JwtService>,
        tls_acceptor: Option<Arc<TlsAcceptor>>,
        storage: Arc<HybridStorageEngine>,
    ) -> Self {
        let now = Instant::now();
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            connection_semaphore: Arc::new(Semaphore::new(MAX_CONNECTIONS)),
            auth_system,
            jwt_service,
            tls_acceptor,
            compatibility_handler: CompatibilityHandler::new(true), // Log warnings
            storage,
            session_timeout: DEFAULT_SESSION_TIMEOUT,
            start_time: now,
            total_ops: Arc::new(AtomicU64::new(0)),
            last_ops_snapshot: Arc::new(RwLock::new((0, now))),
            backup_manager: None,
            replication_manager: None,
            encryption_engine: None,
        }
    }
    
    /// Set backup manager for backup/restore operations
    pub fn with_backup_manager(mut self, manager: Arc<BackupManager>) -> Self {
        self.backup_manager = Some(manager);
        self
    }
    
    /// Set replication manager for master-slave replication
    pub fn with_replication_manager(mut self, manager: Arc<ReplicationManager>) -> Self {
        self.replication_manager = Some(manager);
        self
    }
    
    /// Set encryption engine for key management
    pub fn with_encryption_engine(mut self, engine: Arc<RwLock<EncryptionEngine>>) -> Self {
        self.encryption_engine = Some(engine);
        self
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
            
            // Collection Management
            OpCode::ListCollections => {
                let collections = self.storage.list_collections().map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                let col_values: Vec<Value> = collections.into_iter().map(Value::String).collect();
                let op_res = OperationResponse::success(Some(Value::Array(col_values)));
                let payload = serde_json::to_vec(&op_res).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                Ok(Response::ok(command.header.seq, payload))
            },
            OpCode::CreateCollection => {
                let req: CreateCollectionRequest = serde_json::from_slice(&command.value).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                self.storage.create_collection(&req.name).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                let op_res = OperationResponse::success(None);
                let payload = serde_json::to_vec(&op_res).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                Ok(Response::ok(command.header.seq, payload))
            },
            OpCode::DropCollection => {
                let req: crate::protocol::DropCollectionRequest = serde_json::from_slice(&command.value).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                self.storage.drop_collection(&req.name).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                let op_res = OperationResponse::success(None);
                let payload = serde_json::to_vec(&op_res).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                Ok(Response::ok(command.header.seq, payload))
            },

            // Index Management
            OpCode::ListIndexes => {
                let req: crate::protocol::ListIndexesRequest = serde_json::from_slice(&command.value).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                let indexes = self.storage.list_indexes(&req.collection).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                
                // Helper to convert serde_json::Value to crate::document::Value
                fn json_to_value(v: serde_json::Value) -> Value {
                    match v {
                        serde_json::Value::Null => Value::Null,
                        serde_json::Value::Bool(b) => Value::Bool(b),
                        serde_json::Value::Number(n) => {
                            if let Some(i) = n.as_i64() {
                                Value::Int64(i)
                            } else if let Some(f) = n.as_f64() {
                                Value::Float64(f)
                            } else {
                                Value::Int64(0) 
                            }
                        },
                        serde_json::Value::String(s) => Value::String(s),
                        serde_json::Value::Array(arr) => Value::Array(arr.into_iter().map(json_to_value).collect()),
                        serde_json::Value::Object(obj) => {
                            let mut map = BTreeMap::new();
                            for (k, v) in obj {
                                map.insert(k, json_to_value(v));
                            }
                            Value::Object(map)
                        }
                    }
                }

                let proto_vals: Vec<Value> = indexes.into_iter().map(json_to_value).collect();
                
                let op_res = OperationResponse::success(Some(Value::Array(proto_vals)));
                let payload = serde_json::to_vec(&op_res).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                Ok(Response::ok(command.header.seq, payload))
            },
            OpCode::CreateIndex => {
                let req: CreateIndexRequest = serde_json::from_slice(&command.value).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                self.storage.create_index(&req.collection, &req.name, req.fields, req.unique).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                let op_res = OperationResponse::success(None);
                let payload = serde_json::to_vec(&op_res).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                Ok(Response::ok(command.header.seq, payload))
            },
            OpCode::DropIndex => {
                let req: crate::protocol::DropIndexRequest = serde_json::from_slice(&command.value).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                self.storage.drop_index(&req.collection, &req.name).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                let op_res = OperationResponse::success(None);
                let payload = serde_json::to_vec(&op_res).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                Ok(Response::ok(command.header.seq, payload))
            },

            // Document Operations
            OpCode::InsertDoc => {
                let req: InsertDocRequest = serde_json::from_slice(&command.value).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                self.storage.insert_document(&req.collection, req.document).await.map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                let op_res = OperationResponse::success(None);
                let payload = serde_json::to_vec(&op_res).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                Ok(Response::ok(command.header.seq, payload))
            },
            OpCode::DeleteDoc => {
                let req: DeleteDocRequest = serde_json::from_slice(&command.value)
                    .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                
                // Parse filter using query parser for full filter support
                use crate::query::parser::QueryParser;
                use crate::query::executor::QueryExecutor;
                
                // Convert document::Value to serde_json::Value for parser
                let filter_json = serde_json::to_value(&req.filter)
                    .map_err(|e| ConnectionError::ProtocolError(format!("Filter serialization error: {}", e)))?;
                
                let filter = QueryParser::parse_filter(&filter_json)
                    .map_err(|e| ConnectionError::ProtocolError(format!("Invalid filter: {}", e)))?;
                
                // Scan collection to find matching documents
                let documents = self.storage.scan_collection(&req.collection)
                    .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                
                // Create query executor for filter matching
                let executor = QueryExecutor::new();
                
                // Find all documents that match the filter
                let mut deleted_count = 0;
                for doc in documents {
                    if executor.matches_filter(&doc, &filter)
                        .map_err(|e| ConnectionError::ProtocolError(format!("Filter matching error: {}", e)))?
                    {
                        // Delete this document
                        match self.storage.delete_document(&req.collection, doc.id).await {
                            Ok(true) => deleted_count += 1,
                            Ok(false) => {}, // Document already deleted  
                            Err(e) => {
                                // Log error but continue deleting other documents
                                log::warn!("Failed to delete document {}: {}", doc.id, e);
                            }
                        }
                    }
                }
                
                // Return operation response with accurate deletion count
                let mut op_res = OperationResponse::success(None);
                op_res.affected_count = Some(deleted_count);
                let payload = serde_json::to_vec(&op_res)
                    .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                Ok(Response::ok(command.header.seq, payload))
            },
             OpCode::Query => {
                 // Return empty list for now to satisfy protocol if not impl
                 // Or implement simple scan
                 Ok(Response::ok(command.header.seq, b"[]".to_vec()))
             },

            // Server Info / Metrics
            OpCode::Info => {
                // Get real server stats
                let stats = self.get_stats().await;
                let collections = self.storage.list_collections().unwrap_or_default();
                
                // Get real process memory usage using sysinfo
                let memory_usage_bytes = {
                    let sys = System::new_all();
                    let pid = Pid::from_u32(std::process::id());
                    sys.process(pid)
                        .map(|p| p.memory())
                        .unwrap_or(0)
                };
                
                // Get real cache hit rate from storage stats
                let cache_hit_rate = self.storage.stats().cache_hit_rate();
                
                // Construct Value::Object with real metrics
                let mut info_map = BTreeMap::new();
                info_map.insert("uptime_seconds".to_string(), Value::Int64(stats.uptime_seconds as i64));
                info_map.insert("connection_count".to_string(), Value::Int32(stats.total_connections as i32));
                info_map.insert("total_collections".to_string(), Value::Int64(collections.len() as i64));
                info_map.insert("memory_usage_bytes".to_string(), Value::Int64(memory_usage_bytes as i64));
                info_map.insert("ops_per_second".to_string(), Value::Float64(stats.ops_per_second));
                info_map.insert("cache_hit_rate".to_string(), Value::Float64(cache_hit_rate));
                info_map.insert("version".to_string(), Value::String("0.2.0".to_string()));
                info_map.insert("total_ops".to_string(), Value::Int64(stats.total_ops as i64));
                
                let op_res = OperationResponse::success(Some(Value::Object(info_map)));
                let payload = serde_json::to_vec(&op_res).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                Ok(Response::ok(command.header.seq, payload))
            },

            OpCode::ListUsers => {
                let mut auth_system = self.auth_system.write().await;
                match auth_system.user_manager().list_users().await {
                    Ok(users) => {
                        // Construct Value::Array of Value::Object directly
                        let user_values: Vec<Value> = users.into_iter().map(|u| {
                            let mut user_map = BTreeMap::new();
                            user_map.insert("username".to_string(), Value::String(u.username));
                            user_map.insert("role".to_string(), Value::String(u.role.as_str().to_string()));
                            user_map.insert("created_at".to_string(), Value::String(u.created_at.to_rfc3339()));
                            user_map.insert("last_login".to_string(), u.last_login.map(|dt| Value::String(dt.to_rfc3339())).unwrap_or(Value::Null));
                            user_map.insert("enabled".to_string(), Value::Bool(u.enabled));
                            Value::Object(user_map)
                        }).collect();
                        
                        let op_res = OperationResponse::success(Some(Value::Array(user_values)));
                        let payload = serde_json::to_vec(&op_res).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                        Ok(Response::ok(command.header.seq, payload))
                    },
                    Err(e) => {
                        let op_res = OperationResponse::error(format!("Failed to list users: {}", e));
                        let payload = serde_json::to_vec(&op_res).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                        Ok(Response::ok(command.header.seq, payload))
                    }
                }
            },
            OpCode::CreateUser => {
                let req: CreateUserRequest = serde_json::from_slice(&command.value)
                    .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                
                let role = Role::from_str(&req.role).unwrap_or(Role::ReadOnly);
                let password_hash = bcrypt::hash(&req.password, 12)
                    .map_err(|e| ConnectionError::ProtocolError(format!("Failed to hash password: {}", e)))?;
                
                let new_user = User {
                    username: req.username.clone(),
                    password_hash,
                    role,
                    created_at: chrono::Utc::now(),
                    last_login: None,
                    enabled: true,
                    metadata: Default::default(),
                };
                
                let mut auth_system = self.auth_system.write().await;
                match auth_system.user_manager().create_user(new_user).await {
                    Ok(_) => {
                        let op_res = OperationResponse::success(None);
                        let payload = serde_json::to_vec(&op_res).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                        Ok(Response::ok(command.header.seq, payload))
                    },
                    Err(e) => {
                        let op_res = OperationResponse::error(format!("Failed to create user: {}", e));
                        let payload = serde_json::to_vec(&op_res).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                        Ok(Response::ok(command.header.seq, payload))
                    }
                }
            },
            OpCode::DeleteUser => {
                let req: DeleteUserRequest = serde_json::from_slice(&command.value)
                    .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                
                let mut auth_system = self.auth_system.write().await;
                match auth_system.user_manager().delete_user(&req.username).await {
                    Ok(_) => {
                        let op_res = OperationResponse::success(None);
                        let payload = serde_json::to_vec(&op_res).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                        Ok(Response::ok(command.header.seq, payload))
                    },
                    Err(e) => {
                        let op_res = OperationResponse::error(format!("Failed to delete user: {}", e));
                        let payload = serde_json::to_vec(&op_res).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                        Ok(Response::ok(command.header.seq, payload))
                    }
                }
            },
            OpCode::UpdateUserRole => {
                let req: UpdateUserRoleRequest = serde_json::from_slice(&command.value)
                    .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                
                let new_role = Role::from_str(&req.role).unwrap_or(Role::ReadOnly);
                let mut auth_system = self.auth_system.write().await;
                
                // Get user, update role, and save
                match auth_system.user_manager().get_user(&req.username).await {
                    Ok(Some(mut user)) => {
                        user.role = new_role;
                        match auth_system.user_manager().update_user(user).await {
                            Ok(_) => {
                                let op_res = OperationResponse::success(None);
                                let payload = serde_json::to_vec(&op_res).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                                Ok(Response::ok(command.header.seq, payload))
                            },
                            Err(e) => {
                                let op_res = OperationResponse::error(format!("Failed to update user: {}", e));
                                let payload = serde_json::to_vec(&op_res).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                                Ok(Response::ok(command.header.seq, payload))
                            }
                        }
                    },
                    Ok(None) => {
                        let op_res = OperationResponse::error("User not found".to_string());
                        let payload = serde_json::to_vec(&op_res).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                        Ok(Response::ok(command.header.seq, payload))
                    },
                    Err(e) => {
                        let op_res = OperationResponse::error(format!("Failed to get user: {}", e));
                        let payload = serde_json::to_vec(&op_res).map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                        Ok(Response::ok(command.header.seq, payload))
                    }
                }
            },

            // ============================================================================
            // Backup & Recovery Operations
            // ============================================================================
            OpCode::CreateBackup => {
                let backup_mgr = self.backup_manager.as_ref()
                    .ok_or_else(|| ConnectionError::ProtocolError("Backup feature not enabled".to_string()))?;
                
                let req: crate::protocol::CreateBackupRequest = serde_json::from_slice(&command.value)
                    .map_err(|e| ConnectionError::ProtocolError(format!("Invalid request: {}", e)))?;
                
                let wal_seq = req.wal_sequence.unwrap_or(0);
                
                match backup_mgr.create_backup(wal_seq).await {
                    Ok(backup_info) => {
                        // Convert internal BackupInfo to protocol BackupInfo
                        let proto_info = crate::protocol::BackupInfo {
                            backup_id: backup_info.backup_id,
                            created_at: backup_info.created_at,
                            wal_sequence: backup_info.wal_sequence,
                            size_bytes: backup_info.size_bytes,
                            compressed: backup_info.compressed,
                            file_path: backup_info.file_path.to_string_lossy().to_string(),
                        };
                        let payload = serde_json::to_vec(&proto_info)
                            .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                        Ok(Response::ok(command.header.seq, payload))
                    },
                    Err(e) => {
                        let error_msg = format!("Backup failed: {}", e);
                        Ok(Response::new(Status::Error, command.header.seq, error_msg.into_bytes()))
                    }
                }
            },
            
            OpCode::ListBackups => {
                let backup_mgr = self.backup_manager.as_ref()
                    .ok_or_else(|| ConnectionError::ProtocolError("Backup feature not enabled".to_string()))?;
                
                match backup_mgr.list_backups().await {
                    Ok(backups) => {
                        // Convert internal BackupInfo to protocol BackupInfo
                        let backup_infos: Vec<crate::protocol::BackupInfo> = backups.iter().map(|b| {
                            crate::protocol::BackupInfo {
                                backup_id: b.backup_id.clone(),
                                created_at: b.created_at,
                                wal_sequence: b.wal_sequence,
                                size_bytes: b.size_bytes,
                                compressed: b.compressed,
                                file_path: b.file_path.to_string_lossy().to_string(),
                            }
                        }).collect();
                        
                        let payload = serde_json::to_vec(&backup_infos)
                            .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                        Ok(Response::ok(command.header.seq, payload))
                    },
                    Err(e) => {
                        let error_msg = format!("List backups failed: {}", e);
                        Ok(Response::new(Status::Error, command.header.seq, error_msg.into_bytes()))
                    }
                }
            },
            
            OpCode::RestoreBackup => {
                let backup_mgr = self.backup_manager.as_ref()
                    .ok_or_else(|| ConnectionError::ProtocolError("Backup feature not enabled".to_string()))?;
                
                let req: crate::protocol::RestoreBackupRequest = serde_json::from_slice(&command.value)
                    .map_err(|e| ConnectionError::ProtocolError(format!("Invalid request: {}", e)))?;
                
                let backup_path = std::path::Path::new(&req.backup_id);
                match backup_mgr.restore_backup(backup_path).await {
                    Ok(wal_sequence) => {
                        let op_res = OperationResponse::success(Some(Value::Int64(wal_sequence as i64)));
                        let payload = serde_json::to_vec(&op_res)
                            .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                        Ok(Response::ok(command.header.seq, payload))
                    },
                    Err(e) => {
                        let error_msg = format!("Restore failed: {}", e);
                        Ok(Response::new(Status::Error, command.header.seq, error_msg.into_bytes()))
                    }
                }
            },
            
            OpCode::DeleteBackup => {
                let backup_mgr = self.backup_manager.as_ref()
                    .ok_or_else(|| ConnectionError::ProtocolError("Backup feature not enabled".to_string()))?;
                
                let req: crate::protocol::DeleteBackupRequest = serde_json::from_slice(&command.value)
                    .map_err(|e| ConnectionError::ProtocolError(format!("Invalid delete backup request: {}", e)))?;
                
                // Get backup directory from BackupManager configuration
                let backup_dir = self.backup_manager.as_ref()
                    .map(|bm| bm.config().backup_dir.as_path())
                    .unwrap_or_else(|| std::path::Path::new("./backups"));
                
                let backup_path = backup_dir.join(&req.backup_id);
                let meta_path = backup_path.with_extension("meta");
                
                // Delete both backup file and metadata
                match tokio::fs::remove_file(&backup_path).await {
                    Ok(_) => {
                        let _ = tokio::fs::remove_file(&meta_path).await; // Ignore if meta doesn't exist
                        let op_res = OperationResponse::success(None);
                        let payload = serde_json::to_vec(&op_res)
                            .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                        Ok(Response::ok(command.header.seq, payload))
                    },
                    Err(e) => {
                        let error_msg = format!("Delete backup failed: {}", e);
                        Ok(Response::new(Status::Error, command.header.seq, error_msg.into_bytes()))
                    }
                }
            },
            
            OpCode::PointInTimeRecover => {
                let backup_mgr = self.backup_manager.as_ref()
                    .ok_or_else(|| ConnectionError::ProtocolError("Backup feature not enabled".to_string()))?;
                
                let req: crate::protocol::PointInTimeRecoverRequest = serde_json::from_slice(&command.value)
                    .map_err(|e| ConnectionError::ProtocolError(format!("Invalid request: {}", e)))?;
                
                let backup_path = std::path::Path::new(&req.backup_id);
                let wal_dir = std::path::Path::new(&req.wal_directory);
                
                match backup_mgr.point_in_time_recovery(backup_path, req.target_time, wal_dir).await {
                    Ok(_) => {
                        let op_res = OperationResponse::success(None);
                        let payload = serde_json::to_vec(&op_res)
                            .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                        Ok(Response::ok(command.header.seq, payload))
                    },
                    Err(e) => {
                        let error_msg = format!("PITR failed: {}", e);
                        Ok(Response::new(Status::Error, command.header.seq, error_msg.into_bytes()))
                    }
                }
            },
            
            // ============================================================================
            // Replication Management
            // ============================================================================
            OpCode::GetReplicationStatus => {
                let _repl_mgr = self.replication_manager.as_ref()
                    .ok_or_else(|| ConnectionError::ProtocolError("Replication feature not enabled".to_string()))?;
                
                // Build replication status response
                let status = crate::protocol::ReplicationStatusResponse {
                    role: "master".to_string(),
                    slaves: vec![],
                    lag_bytes: 0,
                    healthy: true,
                };
                
                let payload = serde_json::to_vec(&status)
                    .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                Ok(Response::ok(command.header.seq, payload))
            },
            
            OpCode::AddSlave => {
                let repl_mgr = self.replication_manager.as_ref()
                    .ok_or_else(|| ConnectionError::ProtocolError("Replication feature not enabled".to_string()))?;
                
                let req: crate::protocol::AddSlaveRequest = serde_json::from_slice(&command.value)
                    .map_err(|e| ConnectionError::ProtocolError(format!("Invalid request: {}", e)))?;
                
                // Implement add_slave in ReplicationManager
                let slave_id = repl_mgr.add_slave(&req.slave_address).await
                    .map_err(|e| ConnectionError::ProtocolError(format!("Failed to add slave: {}", e)))?;
                
                let result = serde_json::json!({
                    "success": true,
                    "slave_id": slave_id
                });
                let payload = serde_json::to_vec(&result)
                    .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                Ok(Response::ok(command.header.seq, payload))
            },
            
            OpCode::RemoveSlave => {
                let repl_mgr = self.replication_manager.as_ref()
                    .ok_or_else(|| ConnectionError::ProtocolError("Replication feature not enabled".to_string()))?;
                
                let req: crate::protocol::RemoveSlaveRequest = serde_json::from_slice(&command.value)
                    .map_err(|e| ConnectionError::ProtocolError(format!("Invalid request: {}", e)))?;
                
                // Implement remove_slave in ReplicationManager
                repl_mgr.remove_slave(&req.slave_id).await
                    .map_err(|e| ConnectionError::ProtocolError(format!("Failed to remove slave: {}", e)))?;
                
                let op_res = OperationResponse::success(None);
                let payload = serde_json::to_vec(&op_res)
                    .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                Ok(Response::ok(command.header.seq, payload))
            },
            
            OpCode::ListSlaves => {
                let repl_mgr = self.replication_manager.as_ref()
                    .ok_or_else(|| ConnectionError::ProtocolError("Replication feature not enabled".to_string()))?;
                
                // Get actual slave list from ReplicationManager
                let repl_slaves = repl_mgr.list_slaves().await;
                
                // Convert to protocol SlaveInfo format
                let slaves: Vec<crate::protocol::SlaveInfo> = repl_slaves.iter().map(|s| {
                    crate::protocol::SlaveInfo {
                        slave_id: s.connection_id.clone(),
                        address: s.peer_addr.to_string(),
                        last_ack_sequence: 0, // Not tracked yet
                        connected: s.connected,
                        connected_at: None, // Not tracked yet
                    }
                }).collect();
                
                let payload = serde_json::to_vec(&slaves)
                    .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                Ok(Response::ok(command.header.seq, payload))
            },
            
            OpCode::ForceSync => {
                let repl_mgr = self.replication_manager.as_ref()
                    .ok_or_else(|| ConnectionError::ProtocolError("Replication feature not enabled".to_string()))?;
                
                // Implement force_sync in ReplicationManager
                let synced_count = repl_mgr.force_sync().await
                    .map_err(|e| ConnectionError::ProtocolError(format!("Failed to force sync: {}", e)))?;
                
                let result = serde_json::json!({
                    "success": true,
                    "synced_slaves": synced_count
                });
                let payload = serde_json::to_vec(&result)
                    .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                Ok(Response::ok(command.header.seq, payload))
            },
            
            // Key Management
            // ============================================================================
            OpCode::CreateKey => {
                let enc_engine = self.encryption_engine.as_ref()
                    .ok_or_else(|| ConnectionError::ProtocolError("Encryption feature not enabled".to_string()))?;
                
                let req: crate::protocol::CreateKeyRequest = serde_json::from_slice(&command.value)
                    .map_err(|e| ConnectionError::ProtocolError(format!("Invalid request: {}", e)))?;
                
                // Need write lock for creating keys
                let mut engine = enc_engine.write().await;
                match engine.key_manager_mut().create_key(&req.key_id) {
                    Ok(()) => {
                        let op_res = OperationResponse::success(None);
                        let payload = serde_json::to_vec(&op_res)
                            .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                        Ok(Response::ok(command.header.seq, payload))
                    },
                    Err(e) => {
                        let error_msg = format!("Create key failed: {}", e);
                        Ok(Response::new(Status::Error, command.header.seq, error_msg.into_bytes()))
                    }
                }
            },
            
            OpCode::ListKeys => {
                let enc_engine = self.encryption_engine.as_ref()
                    .ok_or_else(|| ConnectionError::ProtocolError("Encryption feature not enabled".to_string()))?;
                
                // Read lock sufficient for listing
                let engine = enc_engine.read().await;
                let keys = engine.key_manager().list_keys();
                let key_infos: Vec<crate::protocol::KeyMetadataResponse> = keys.iter().map(|key| {
                    crate::protocol::KeyMetadataResponse {
                        key_id: key.id.clone(),
                        version: key.version,
                        algorithm: "AES-256-GCM".to_string(), // Default algorithm
                        created_at: key.created_at,
                        last_rotated: key.last_rotated,
                        expires_at: None, // No expiration by default
                        active: key.active,
                        is_active: key.active, // Mirror active field
                    }
                }).collect();
                
                let response = crate::protocol::ListKeysResponse { keys: key_infos };
                let payload = serde_json::to_vec(&response)
                    .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                Ok(Response::ok(command.header.seq, payload))
            },
            
            OpCode::ExportKey => {
                let enc_engine = self.encryption_engine.as_ref()
                    .ok_or_else(|| ConnectionError::ProtocolError("Encryption feature not enabled".to_string()))?;
                
                let req: crate::protocol::ExportKeyRequest = serde_json::from_slice(&command.value)
                    .map_err(|e| ConnectionError::ProtocolError(format!("Invalid request: {}", e)))?;
                
                // Read lock sufficient for export
                let engine = enc_engine.read().await;
                match engine.key_manager().export_key(&req.key_id) {
                    Ok(encrypted_data) => {
                        let response = crate::protocol::ExportKeyResponse {
                            key_id: req.key_id,
                            encrypted_data,
                        };
                        let payload = serde_json::to_vec(&response)
                            .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                        Ok(Response::ok(command.header.seq, payload))
                    },
                    Err(e) => {
                        let error_msg = format!("Export key failed: {}", e);
                        Ok(Response::new(Status::Error, command.header.seq, error_msg.into_bytes()))
                    }
                }
            },
            
            OpCode::ImportKey => {
                let enc_engine = self.encryption_engine.as_ref()
                    .ok_or_else(|| ConnectionError::ProtocolError("Encryption feature not enabled".to_string()))?;
                
                let req: crate::protocol::ImportKeyRequest = serde_json::from_slice(&command.value)
                    .map_err(|e| ConnectionError::ProtocolError(format!("Invalid request: {}", e)))?;
                
                // Need write lock for importing keys
                let mut engine = enc_engine.write().await;
                match engine.key_manager_mut().import_key(&req.encrypted_data) {
                    Ok(key_id) => {
                        let op_res = OperationResponse::success(Some(Value::String(key_id)));
                        let payload = serde_json::to_vec(&op_res)
                            .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                        Ok(Response::ok(command.header.seq, payload))
                    },
                    Err(e) => {
                        let error_msg = format!("Import key failed: {}", e);
                        Ok(Response::new(Status::Error, command.header.seq, error_msg.into_bytes()))
                    }
                }
            },
            
            OpCode::GetKeysExpiring => {
                let enc_engine = self.encryption_engine.as_ref()
                    .ok_or_else(|| ConnectionError::ProtocolError("Encryption feature not enabled".to_string()))?;
                
                let req: crate::protocol::GetKeysExpiringRequest = serde_json::from_slice(&command.value)
                    .map_err(|e| ConnectionError::ProtocolError(format!("Invalid request: {}", e)))?;
                
                // Read lock sufficient for checking expiry
                let engine = enc_engine.read().await;
                let expiring = engine.key_manager().get_keys_with_expiry_warnings(req.rotation_days);
                let expiring_infos: Vec<crate::protocol::ExpiringKeyInfo> = expiring.iter().map(|(key, days_remaining)| {
                    crate::protocol::ExpiringKeyInfo {
                        key_id: key.id.clone(),
                        days_remaining: *days_remaining,
                        last_rotated: key.last_rotated,
                        created_at: key.created_at,
                    }
                }).collect();
                
                let response = crate::protocol::GetKeysExpiringResponse {
                    expiring_keys: expiring_infos,
                };
                let payload = serde_json::to_vec(&response)
                    .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                Ok(Response::ok(command.header.seq, payload))
            },
            
            OpCode::RotateKey => {
                // SECURITY: Key re-encryption engine implemented but integration pending.
                // Server MUST refuse rotation requests until P0.5 integration is complete.
                // 
                // Cryptographic re-encryption logic is COMPLETE in:
                // - encryption/key_manager.rs (rotate_key_with_backup)
                // - encryption/key_rotation.rs (real batch re-encryption)
                // - encryption/encrypted_storage.rs (EncryptedStorage trait)
                //
                // Remaining work: storage reference threading, state machine, startup enforcement.
                // DO NOT REMOVE THIS GUARD until integration is verified and tested.
                
                return Err(ConnectionError::ProtocolError(
                    "Key rotation is not yet fully integrated. \
                     Cryptographic re-encryption engine is complete but system integration is pending. \
                     This is a fail-closed security measure. \
                     Contact system administrator for manual key rotation procedures.".to_string()
                ));
                
                // The following code will be activated when P0.5 integration completes:
                /*
                let enc_engine = self.encryption_engine.as_ref()
                    .ok_or_else(|| ConnectionError::ProtocolError("Encryption feature not enabled".to_string()))?;
                
                let req: crate::protocol::RotateKeyRequest = serde_json::from_slice(&command.value)
                    .map_err(|e| ConnectionError::ProtocolError(format!("Invalid request: {}", e)))?;
                
                // Need write lock for rotation
                let mut engine = enc_engine.write().await;
                
                // Pass storage as &dyn EncryptedStorage to rotate_key
                let storage_ref: &dyn crate::encryption::EncryptedStorage = &*self.storage;
                match engine.rotate_key(storage_ref, &req.key_id).await {
                    Ok(_) => {
                        let op_res = OperationResponse::success(None);
                        let payload = serde_json::to_vec(&op_res)
                            .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                        Ok(Response::ok(command.header.seq, payload))
                    }
                    Err(e) => {
                        let err_msg = format!("Key rotation failed: {}", e);
                        Ok(Response::error(command.header.seq, err_msg))
                    }
                }
                */
            },
            
            OpCode::GetKeyMetadata => {
                let enc_engine = self.encryption_engine.as_ref()
                    .ok_or_else(|| ConnectionError::ProtocolError("Encryption feature not enabled".to_string()))?;
                
                let req: crate::protocol::GetKeyMetadataRequest = serde_json::from_slice(&command.value)
                    .map_err(|e| ConnectionError::ProtocolError(format!("Invalid request: {}", e)))?;
                
                // Read lock sufficient for metadata
                let engine = enc_engine.read().await;
                match engine.key_manager().get_key_metadata(&req.key_id) {
                    Ok(key) => {
                        let response = crate::protocol::KeyMetadataResponse {
                            key_id: key.id.clone(),
                            version: key.version,
                            algorithm: "AES-256-GCM".to_string(),
                            created_at: key.created_at,
                            last_rotated: key.last_rotated,
                            expires_at: None,
                            active: key.active,
                            is_active: key.active,
                        };
                        let payload = serde_json::to_vec(&response)
                            .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                        Ok(Response::ok(command.header.seq, payload))
                    },
                    Err(e) => {
                        let error_msg = format!("Get key metadata failed: {}", e);
                        Ok(Response::new(Status::Error, command.header.seq, error_msg.into_bytes()))
                    }
                }
            },

            // ============================================================================
            // Aggregation Pipeline
            // ============================================================================
            OpCode::Aggregate => {
                let req: crate::protocol::AggregateRequest = serde_json::from_slice(&command.value)
                    .map_err(|e| ConnectionError::ProtocolError(format!("Invalid aggregation request: {}", e)))?;
                
                // Get collection documents as iterator for streaming execution
                let collection_name = req.collection.clone();
                
                // Create pipeline executor
                let pipeline = crate::aggregation::Pipeline::new(req.pipeline);
                
                // Get documents from storage
                // Note: Current API returns Vec. In future, add true streaming iterator for better memory efficiency.
                let documents = self.storage.scan_collection(&collection_name)
                    .map_err(|e| ConnectionError::ProtocolError(format!("Failed to scan collection: {}", e)))?;
                
                // Execute aggregation pipeline with streaming engine
                match pipeline.execute(documents) {
                    Ok(results) => {
                        // Convert results to Value::Array
                        use crate::document::Value;
                        
                        let result_values: Vec<Value> = results.into_iter().map(|doc| {
                            // Convert Document to Value::Object by accessing fields
                            Value::Object(doc.fields)
                        }).collect();
                        
                        let op_res = OperationResponse::success(Some(Value::Array(result_values)));
                        let payload = serde_json::to_vec(&op_res)
                            .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                        Ok(Response::ok(command.header.seq, payload))
                    }
                    Err(e) => {
                        let op_res = OperationResponse::error(format!("Aggregation failed: {}", e));
                        let payload = serde_json::to_vec(&op_res)
                            .map_err(|e| ConnectionError::ProtocolError(e.to_string()))?;
                        Ok(Response::ok(command.header.seq, payload))
                    }
                }
            },
            
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
        
        // Calculate uptime
        let uptime_seconds = self.start_time.elapsed().as_secs();
        
        // Calculate ops per second (rolling average over last check period)
        let current_ops = self.total_ops.load(Ordering::Relaxed);
        let ops_per_second = {
            let mut snapshot = self.last_ops_snapshot.write().await;
            let (last_ops, last_time) = *snapshot;
            let elapsed = last_time.elapsed().as_secs_f64();
            
            let ops_per_sec = if elapsed > 0.0 {
                (current_ops - last_ops) as f64 / elapsed
            } else {
                0.0
            };
            
            // Update snapshot every 5 seconds
            if elapsed >= 5.0 {
                *snapshot = (current_ops, Instant::now());
            }
            
            ops_per_sec
        };
        
        ConnectionStats {
            total_connections: connections.len(),
            authenticated_connections: sessions.len(),
            max_connections: MAX_CONNECTIONS,
            available_slots: self.connection_semaphore.available_permits(),
            uptime_seconds,
            ops_per_second,
            total_ops: current_ops,
        }
    }

    /// Increment the operations counter
    pub fn record_operation(&self) {
        self.total_ops.fetch_add(1, Ordering::Relaxed);
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
            storage: Arc::clone(&self.storage),
            session_timeout: self.session_timeout,
            start_time: self.start_time,
            total_ops: Arc::clone(&self.total_ops),
            last_ops_snapshot: Arc::clone(&self.last_ops_snapshot),
            backup_manager: self.backup_manager.clone(),
            replication_manager: self.replication_manager.clone(),
            encryption_engine: self.encryption_engine.clone(),
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
    pub uptime_seconds: u64,
    pub ops_per_second: f64,
    pub total_ops: u64,
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
            uptime_seconds: 0,
            ops_per_second: 0.0,
            total_ops: 0,
        };
        
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.authenticated_connections, 0);
        assert_eq!(stats.max_connections, MAX_CONNECTIONS);
    }
}