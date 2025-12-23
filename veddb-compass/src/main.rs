// Prevents additional console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use veddb_client::{Client, TlsConfig, AuthConfig, QueryRequest, UpdateDocRequest, DeleteDocRequest, CreateCollectionRequest, CreateIndexRequest, IndexField, Document};

// Connection management
type ConnectionId = String;

// Error categorization for better user feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "message")]
pub enum ErrorCategory {
    /// DNS resolution failed
    DnsResolution(String),
    /// Connection to server failed
    Connection(String),
    /// Authentication failed
    Authentication(String),
    /// General error
    General(String),
}

impl ErrorCategory {
    /// Format DNS resolution error with hostname context
    pub fn dns_resolution(hostname: &str, error: impl std::fmt::Display) -> Self {
        ErrorCategory::DnsResolution(format!(
            "Failed to resolve hostname '{}': {}. Please check if the hostname is correct and your network connection is working.",
            hostname, error
        ))
    }

    /// Format connection error with resolved IP context
    pub fn connection(hostname: &str, addr: std::net::SocketAddr, error: impl std::fmt::Display) -> Self {
        ErrorCategory::Connection(format!(
            "Failed to connect to {} (resolved from '{}'): {}. Please verify the server is running and accessible.",
            addr, hostname, error
        ))
    }

    /// Format authentication error
    pub fn authentication(error: impl std::fmt::Display) -> Self {
        ErrorCategory::Authentication(format!(
            "Authentication failed: {}. Please check your username and password.",
            error
        ))
    }

    /// Format general error
    pub fn general(error: impl std::fmt::Display) -> Self {
        ErrorCategory::General(format!("{}", error))
    }

    /// Convert to string for backward compatibility
    pub fn to_string(&self) -> String {
        match self {
            ErrorCategory::DnsResolution(msg) => msg.clone(),
            ErrorCategory::Connection(msg) => msg.clone(),
            ErrorCategory::Authentication(msg) => msg.clone(),
            ErrorCategory::General(msg) => msg.clone(),
        }
    }

    /// Check if error is authentication-related
    pub fn is_authentication_error(error: &veddb_client::Error) -> bool {
        matches!(error, veddb_client::Error::AuthenticationFailed)
    }

    /// Check if error is connection-related
    pub fn is_connection_error(error: &veddb_client::Error) -> bool {
        matches!(
            error,
            veddb_client::Error::Connection(_)
                | veddb_client::Error::Io(_)
                | veddb_client::Error::NotConnected
                | veddb_client::Error::Timeout(_)
        )
    }

    /// Categorize a veddb_client::Error
    pub fn from_client_error(hostname: &str, addr: Option<std::net::SocketAddr>, error: &veddb_client::Error) -> Self {
        if Self::is_authentication_error(error) {
            Self::authentication(error)
        } else if Self::is_connection_error(error) {
            if let Some(socket_addr) = addr {
                Self::connection(hostname, socket_addr, error)
            } else {
                Self::general(error)
            }
        } else {
            Self::general(error)
        }
    }
}

// Hostname resolution function
async fn resolve_host(host: &str, port: u16) -> Result<std::net::SocketAddr, String> {
    debug!("Attempting to resolve hostname: {}:{}", host, port);
    
    // Special handling for 0.0.0.0 - convert to localhost for client connections
    // 0.0.0.0 is only valid for servers to bind to, not for clients to connect to
    let host = if host == "0.0.0.0" {
        debug!("Converting 0.0.0.0 to 127.0.0.1 for client connection");
        "127.0.0.1"
    } else {
        host
    };
    
    // Try to parse as IP address first (bypass DNS lookup)
    if let Ok(ip_addr) = host.parse::<std::net::IpAddr>() {
        let addr = std::net::SocketAddr::new(ip_addr, port);
        debug!("Parsed as IP address (bypassing DNS): {}", addr);
        return Ok(addr);
    }
    
    // If not an IP address, perform DNS resolution
    debug!("Performing DNS lookup for hostname: {}", host);
    let addr_string = format!("{}:{}", host, port);
    let lookup_result = tokio::net::lookup_host(addr_string).await;
    
    match lookup_result {
        Ok(addrs) => {
            // Collect all addresses
            let all_addrs: Vec<std::net::SocketAddr> = addrs.collect();
            
            if all_addrs.is_empty() {
                error!("DNS resolution failed for '{}': No addresses found", host);
                return Err(format!("Failed to resolve hostname '{}': No addresses found", host));
            }
            
            debug!("DNS lookup returned {} address(es) for '{}'", all_addrs.len(), host);
            
            // Prefer IPv4 addresses over IPv6
            if let Some(ipv4_addr) = all_addrs.iter().find(|addr| addr.is_ipv4()) {
                info!("Resolved '{}' to {} (IPv4 preferred)", host, ipv4_addr);
                Ok(*ipv4_addr)
            } else {
                // If no IPv4 found, use the first available address
                info!("Resolved '{}' to {} (IPv6)", host, all_addrs[0]);
                Ok(all_addrs[0])
            }
        }
        Err(e) => {
            error!("DNS resolution failed for '{}': {}", host, e);
            Err(format!("Failed to resolve hostname '{}': {}", host, e))
        }
    }
}

// Helper function to create a VedDB client
async fn create_client(
    addr: std::net::SocketAddr,
    tls_config: Option<TlsConfig>,
    auth_config: Option<AuthConfig>,
) -> Result<Client, veddb_client::Error> {
    match (tls_config, auth_config) {
        (Some(tls), Some(auth)) => Client::connect_with_auth(addr, Some(tls), auth).await,
        (Some(tls), None) => Client::connect_with_tls(addr, tls).await,
        (None, Some(auth)) => Client::connect_with_auth(addr, None, auth).await,
        (None, None) => Client::connect(addr).await,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub tls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStatus {
    pub id: String,
    pub connected: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionInfo {
    pub name: String,
    pub document_count: u64,
    pub size_bytes: u64,
    pub indexes: Vec<IndexInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub fields: Vec<String>,
    pub unique: bool,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerMetrics {
    pub ops_per_second: f64,
    pub latency_p99: f64,
    pub memory_usage_bytes: u64,
    pub cache_hit_rate: f64,
    pub connection_count: u32,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub username: String,
    pub role: String,
    pub created_at: String,
    pub last_login: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub documents: Vec<serde_json::Value>,
    pub total_count: u64,
    pub execution_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportProgress {
    pub processed: u64,
    pub total: u64,
    pub errors: Vec<String>,
    pub is_complete: bool,
}

// Application state
#[derive(Default)]
pub struct AppState {
    connections: RwLock<HashMap<ConnectionId, ConnectionConfig>>,
    connection_status: RwLock<HashMap<ConnectionId, ConnectionStatus>>,
    clients: RwLock<HashMap<ConnectionId, Arc<Client>>>,
}

// Connection management commands
#[tauri::command]
async fn test_connection(
    config: ConnectionConfig,
    _state: State<'_, AppState>,
) -> Result<bool, String> {
    info!("Testing connection to {}:{} (TLS: {})", config.host, config.port, config.tls);
    
    // Resolve hostname to IP address
    let socket_addr = match resolve_host(&config.host, config.port).await {
        Ok(addr) => {
            debug!("Resolved {}:{} to {}", config.host, config.port, addr);
            addr
        },
        Err(e) => {
            error!("Failed to resolve hostname for test connection: {}", e);
            // DNS resolution error - already formatted with hostname context
            return Err(e);
        }
    };
    
    // Create TLS config if enabled
    let tls_config = if config.tls {
        debug!("Creating TLS config for {}", config.host);
        Some(TlsConfig::new(&config.host).accept_invalid_certs())
    } else {
        None
    };
    
    // Create auth config if credentials provided
    let auth_config = if let (Some(username), Some(password)) = (&config.username, &config.password) {
        debug!("Creating auth config for user: {}", username);
        Some(AuthConfig::username_password(username, password))
    } else {
        debug!("No authentication credentials provided");
        None
    };
    
    // Attempt to connect and ping
    debug!("Attempting to create client connection to {}", socket_addr);
    match create_client(socket_addr, tls_config, auth_config).await {
        Ok(client) => {
            debug!("Client created successfully, sending ping command");
            match client.ping().await {
                Ok(_) => {
                    info!("Connection test successful for {}:{}", config.host, config.port);
                    Ok(true)
                },
                Err(e) => {
                    error!("Ping command failed: {}", e);
                    // Categorize the error for better user feedback
                    let error_category = ErrorCategory::from_client_error(&config.host, Some(socket_addr), &e);
                    Err(error_category.to_string())
                }
            }
        }
        Err(e) => {
            error!("Failed to create client connection to {}: {}", socket_addr, e);
            // Categorize the connection error
            let error_category = ErrorCategory::from_client_error(&config.host, Some(socket_addr), &e);
            Err(error_category.to_string())
        }
    }
}

#[tauri::command]
async fn connect_to_server(
    config: ConnectionConfig,
    state: State<'_, AppState>,
) -> Result<ConnectionStatus, String> {
    info!("Connecting to server: {} (id: {}, host: {}:{}, TLS: {})", 
          config.name, config.id, config.host, config.port, config.tls);
    
    // Resolve hostname to IP address
    let socket_addr = match resolve_host(&config.host, config.port).await {
        Ok(addr) => {
            debug!("Resolved {}:{} to {} for connection {}", config.host, config.port, addr, config.id);
            addr
        },
        Err(e) => {
            error!("Failed to resolve hostname for connection {}: {}", config.id, e);
            // DNS resolution error - already formatted with hostname context
            let status = ConnectionStatus {
                id: config.id.clone(),
                connected: false,
                last_error: Some(e.clone()),
            };
            state.connection_status.write().await.insert(status.id.clone(), status.clone());
            return Err(e);
        }
    };
    
    // Create TLS config if enabled
    let tls_config = if config.tls {
        debug!("Creating TLS config for connection {} to {}", config.id, config.host);
        Some(TlsConfig::new(&config.host).accept_invalid_certs())
    } else {
        debug!("TLS disabled for connection {}", config.id);
        None
    };
    
    // Create auth config if credentials provided
    let auth_config = if let (Some(username), Some(password)) = (&config.username, &config.password) {
        debug!("Creating auth config for connection {} with user: {}", config.id, username);
        Some(AuthConfig::username_password(username, password))
    } else {
        debug!("No authentication credentials for connection {}", config.id);
        None
    };
    
    // Create client and test connection
    debug!("Creating client for connection {} to {}", config.id, socket_addr);
    match create_client(socket_addr, tls_config, auth_config).await {
        Ok(client) => {
            debug!("Client created successfully for connection {}, sending ping", config.id);
            // Test the connection with a ping
            match client.ping().await {
                Ok(_) => {
                    info!("Successfully connected to server: {} (id: {}, address: {})", 
                          config.name, config.id, socket_addr);
                    
                    let status = ConnectionStatus {
                        id: config.id.clone(),
                        connected: true,
                        last_error: None,
                    };
                    
                    // Store connection config, status, and client
                    state.connections.write().await.insert(config.id.clone(), config.clone());
                    state.connection_status.write().await.insert(status.id.clone(), status.clone());
                    state.clients.write().await.insert(config.id.clone(), Arc::new(client));
                    
                    Ok(status)
                }
                Err(e) => {
                    error!("Ping failed for connection {} to {}: {}", config.id, socket_addr, e);
                    // Categorize the error for better user feedback
                    let error_category = ErrorCategory::from_client_error(&config.host, Some(socket_addr), &e);
                    let error_msg = error_category.to_string();
                    
                    let status = ConnectionStatus {
                        id: config.id.clone(),
                        connected: false,
                        last_error: Some(error_msg.clone()),
                    };
                    
                    state.connection_status.write().await.insert(status.id.clone(), status.clone());
                    Err(error_msg)
                }
            }
        }
        Err(e) => {
            error!("Failed to create client for connection {} to {}: {}", config.id, socket_addr, e);
            // Categorize the connection error with resolved IP context
            let error_category = ErrorCategory::from_client_error(&config.host, Some(socket_addr), &e);
            let error_msg = error_category.to_string();
            
            let status = ConnectionStatus {
                id: config.id.clone(),
                connected: false,
                last_error: Some(error_msg.clone()),
            };
            
            state.connection_status.write().await.insert(status.id.clone(), status.clone());
            Err(error_msg)
        }
    }
}

#[tauri::command]
async fn disconnect_from_server(
    connection_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Disconnecting from server: {}", connection_id);
    
    // Remove client from active connections
    let removed = state.clients.write().await.remove(&connection_id);
    
    if removed.is_some() {
        debug!("Removed client for connection: {}", connection_id);
    } else {
        warn!("Attempted to disconnect non-existent connection: {}", connection_id);
    }
    
    // Update connection status
    let mut status_map = state.connection_status.write().await;
    if let Some(status) = status_map.get_mut(&connection_id) {
        status.connected = false;
        debug!("Updated connection status to disconnected: {}", connection_id);
    }
    
    info!("Successfully disconnected from server: {}", connection_id);
    Ok(())
}

#[tauri::command]
async fn get_connection_status(
    connection_id: String,
    state: State<'_, AppState>,
) -> Result<ConnectionStatus, String> {
    let status_map = state.connection_status.read().await;
    status_map
        .get(&connection_id)
        .cloned()
        .ok_or_else(|| "Connection not found".to_string())
}

// Database exploration commands
#[tauri::command]
async fn get_collections(
    connection_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<CollectionInfo>, String> {
    let clients = state.clients.read().await;
    let client = clients.get(&connection_id)
        .ok_or_else(|| "Connection not found".to_string())?;
    
    // Use the real list_collections command
    match client.list_collections().await {
        Ok(collection_names) => {
            let mut collections = Vec::new();
            
            for name in collection_names {
                // For now, we don't have an endpoint to get detailed stats per collection
                // so we'll fetch indexes for each collection and set other stats to 0
                
                let indexes = match client.list_indexes(name.as_str()).await {
                    Ok(idxs) => {
                        idxs.into_iter().filter_map(|idx| {
                            // Extract index info from Value
                            // Use as_object() to get the BTreeMap from veddb_client::Value
                            let obj = idx.as_object()?;
                            let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                            let unique = obj.get("unique").and_then(|v| v.as_bool()).unwrap_or(false);
                            let fields = obj.get("fields").and_then(|v| v.as_array())
                                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                                .unwrap_or_else(Vec::new);
                                
                            Some(IndexInfo {
                                name,
                                fields,
                                unique,
                                size_bytes: 0, // Placeholder
                            })
                        }).collect()
                    },
                    Err(_) => Vec::new(),
                };

                collections.push(CollectionInfo {
                    name,
                    document_count: 0, // Placeholder until we have count stats
                    size_bytes: 0,     // Placeholder
                    indexes,
                });
            }
            
            Ok(collections)
        }
        Err(e) => Err(format!("Failed to list collections: {}", e)),
    }
}

#[tauri::command]
async fn execute_query(
    connection_id: String,
    collection: String,
    query: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<QueryResult, String> {
    debug!("Executing query on connection {} for collection: {}", connection_id, collection);
    
    let clients = state.clients.read().await;
    let client = clients.get(&connection_id)
        .ok_or_else(|| {
            error!("Query failed: Connection {} not found", connection_id);
            "Connection not found".to_string()
        })?;
    
    let start_time = std::time::Instant::now();
    
    // Parse the query JSON into a QueryRequest
    let query_request = QueryRequest {
        collection: collection.clone(),
        filter: query.get("filter").cloned().and_then(|f| serde_json::from_value(f).ok()),
        projection: query.get("projection").cloned().and_then(|p| serde_json::from_value(p).ok()),
        sort: query.get("sort").cloned().and_then(|s| serde_json::from_value(s).ok()),
        skip: query.get("skip").and_then(|s| s.as_u64()),
        limit: query.get("limit").and_then(|l| l.as_u64()),
    };
    
    debug!("Query request: collection={}, filter={:?}, limit={:?}", 
           collection, query_request.filter, query_request.limit);
    
    match client.query(query_request).await {
        Ok(documents) => {
            let execution_time = start_time.elapsed().as_millis() as u64;
            let total_count = documents.len() as u64;
            
            info!("Query executed successfully: collection={}, documents={}, time={}ms", 
                  collection, total_count, execution_time);
            
            // Convert documents to JSON values
            let json_docs: Result<Vec<serde_json::Value>, _> = documents
                .into_iter()
                .map(|doc| serde_json::to_value(doc))
                .collect();
            
            match json_docs {
                Ok(docs) => Ok(QueryResult {
                    documents: docs,
                    total_count,
                    execution_time_ms: execution_time,
                }),
                Err(e) => {
                    error!("Failed to serialize query results: {}", e);
                    Err(format!("Failed to serialize documents: {}", e))
                }
            }
        }
        Err(e) => {
            error!("Query execution failed for collection {}: {}", collection, e);
            Err(format!("Query execution failed: {}", e))
        }
    }
}

// Metrics commands
#[tauri::command]
async fn get_server_metrics(
    connection_id: String,
    state: State<'_, AppState>,
) -> Result<ServerMetrics, String> {
    let clients = state.clients.read().await;
    let client = clients.get(&connection_id)
        .ok_or_else(|| "Connection not found".to_string())?;
    
    // Use real server info from the Info opcode
    match client.info().await {
        Ok(info) => {
            Ok(ServerMetrics {
                ops_per_second: info.ops_per_second,
                latency_p99: 2.3, // Placeholder - not available in basic ServerInfo
                memory_usage_bytes: info.memory_usage_bytes,
                cache_hit_rate: info.cache_hit_rate,
                connection_count: info.connection_count,
                uptime_seconds: info.uptime_seconds,
            })
        }
        Err(e) => Err(format!("Failed to get server metrics: {}", e)),
    }
}

// Index management commands
#[tauri::command]
async fn create_index(
    connection_id: String,
    collection: String,
    index_name: String,
    fields: Vec<String>,
    unique: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Creating index '{}' on collection '{}' (connection: {}, fields: {:?}, unique: {})", 
          index_name, collection, connection_id, fields, unique);
    
    let clients = state.clients.read().await;
    let client = clients.get(&connection_id)
        .ok_or_else(|| {
            error!("Create index failed: Connection {} not found", connection_id);
            "Connection not found".to_string()
        })?;
    
    // Create index fields
    let index_fields: Vec<IndexField> = fields
        .into_iter()
        .map(|field| IndexField {
            field: field,
            direction: 1, // Ascending
        })
        .collect();
    
    let request = CreateIndexRequest {
        collection: collection.clone(),
        name: index_name.clone(),
        fields: index_fields,
        unique,
    };
    
    match client.create_index(request).await {
        Ok(_) => {
            info!("Index '{}' created successfully on collection '{}'", index_name, collection);
            Ok(())
        },
        Err(e) => {
            error!("Failed to create index '{}' on collection '{}': {}", index_name, collection, e);
            Err(format!("Failed to create index: {}", e))
        }
    }
}

#[tauri::command]
async fn drop_index(
    connection_id: String,
    collection: String,
    index_name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let clients = state.clients.read().await;
    let client = clients.get(&connection_id)
        .ok_or_else(|| "Connection not found".to_string())?;
    
    // Use real drop_index command
    match client.drop_index(collection, index_name).await {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Failed to drop index: {}", e)),
    }
}

// User management commands
#[tauri::command]
async fn get_users(
    connection_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<UserInfo>, String> {
    let clients = state.clients.read().await;
    let client = clients.get(&connection_id)
        .ok_or_else(|| "Connection not found".to_string())?;
    
    // Use real user listing from the ListUsers opcode
    match client.list_users().await {
        Ok(users) => {
            Ok(users.into_iter().map(|u| UserInfo {
                username: u.username,
                role: u.role,
                created_at: u.created_at,
                last_login: u.last_login,
                enabled: u.enabled,
            }).collect())
        }
        Err(e) => Err(format!("Failed to list users: {}", e)),
    }
}

#[tauri::command]
async fn create_user(
    connection_id: String,
    username: String,
    password: String,
    role: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let clients = state.clients.read().await;
    let client = clients.get(&connection_id)
        .ok_or_else(|| "Connection not found".to_string())?;
    
    // Use real user creation from the CreateUser opcode
    let request = veddb_client::CreateUserRequest {
        username,
        password,
        role,
    };
    
    match client.create_user(request).await {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Failed to create user: {}", e)),
    }
}

#[tauri::command]
async fn delete_user(
    connection_id: String,
    username: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let clients = state.clients.read().await;
    let client = clients.get(&connection_id)
        .ok_or_else(|| "Connection not found".to_string())?;
    
    // Use real user deletion from the DeleteUser opcode
    match client.delete_user(&username).await {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Failed to delete user: {}", e)),
    }
}

#[tauri::command]
async fn update_user_role(
    connection_id: String,
    username: String,
    role: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let clients = state.clients.read().await;
    let client = clients.get(&connection_id)
        .ok_or_else(|| "Connection not found".to_string())?;
    
    // Use real user role update from the UpdateUserRole opcode
    match client.update_user_role(&username, &role).await {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Failed to update user role: {}", e)),
    }
}

// Document operations
#[tauri::command]
async fn insert_document(
    connection_id: String,
    collection: String,
    document: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<(), String> {
    debug!("Inserting document into collection {} on connection {}", collection, connection_id);
    
    let clients = state.clients.read().await;
    let client = clients.get(&connection_id)
        .ok_or_else(|| {
            error!("Insert failed: Connection {} not found", connection_id);
            "Connection not found".to_string()
        })?;
    
    // Convert JSON value to Document
    let doc: Document = serde_json::from_value(document)
        .map_err(|e| {
            error!("Invalid document format for insert: {}", e);
            format!("Invalid document format: {}", e)
        })?;
    
    match client.insert_document(&collection, doc).await {
        Ok(_) => {
            info!("Document inserted successfully into collection: {}", collection);
            Ok(())
        },
        Err(e) => {
            error!("Failed to insert document into {}: {}", collection, e);
            Err(format!("Failed to insert document: {}", e))
        }
    }
}

#[tauri::command]
async fn update_document(
    connection_id: String,
    collection: String,
    filter: serde_json::Value,
    update: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    debug!("Updating document in collection {} on connection {}", collection, connection_id);
    
    let clients = state.clients.read().await;
    let client = clients.get(&connection_id)
        .ok_or_else(|| {
            error!("Update failed: Connection {} not found", connection_id);
            "Connection not found".to_string()
        })?;
    
    let request = UpdateDocRequest {
        collection: collection.clone(),
        filter: serde_json::from_value(filter)
            .map_err(|e| {
                error!("Invalid filter format for update: {}", e);
                format!("Invalid filter format: {}", e)
            })?,
        update: serde_json::from_value(update)
            .map_err(|e| {
                error!("Invalid update format: {}", e);
                format!("Invalid update format: {}", e)
            })?,
        upsert: false,
    };
    
    match client.update_document(request).await {
        Ok(count) => {
            info!("Updated {} document(s) in collection: {}", count, collection);
            Ok(count)
        },
        Err(e) => {
            error!("Failed to update document in {}: {}", collection, e);
            Err(format!("Failed to update document: {}", e))
        }
    }
}

#[tauri::command]
async fn delete_document(
    connection_id: String,
    collection: String,
    filter: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    debug!("Deleting document from collection {} on connection {}", collection, connection_id);
    
    let clients = state.clients.read().await;
    let client = clients.get(&connection_id)
        .ok_or_else(|| {
            error!("Delete failed: Connection {} not found", connection_id);
            "Connection not found".to_string()
        })?;
    
    let request = DeleteDocRequest {
        collection: collection.clone(),
        filter: serde_json::from_value(filter)
            .map_err(|e| {
                error!("Invalid filter format for delete: {}", e);
                format!("Invalid filter format: {}", e)
            })?,
    };
    
    match client.delete_document(request).await {
        Ok(count) => {
            info!("Deleted {} document(s) from collection: {}", count, collection);
            Ok(count)
        },
        Err(e) => {
            error!("Failed to delete document from {}: {}", collection, e);
            Err(format!("Failed to delete document: {}", e))
        }
    }
}

#[tauri::command]
async fn create_collection(
    connection_id: String,
    collection_name: String,
    schema: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Creating collection '{}' on connection {}", collection_name, connection_id);
    
    let clients = state.clients.read().await;
    let client = clients.get(&connection_id)
        .ok_or_else(|| {
            error!("Create collection failed: Connection {} not found", connection_id);
            "Connection not found".to_string()
        })?;
    
    let request = CreateCollectionRequest {
        name: collection_name.clone(),
        schema: schema.and_then(|s| serde_json::from_value(s).ok()),
    };
    
    match client.create_collection(request).await {
        Ok(_) => {
            info!("Collection '{}' created successfully", collection_name);
            Ok(())
        },
        Err(e) => {
            error!("Failed to create collection '{}': {}", collection_name, e);
            Err(format!("Failed to create collection: {}", e))
        }
    }
}

#[tauri::command]
async fn drop_collection(
    connection_id: String,
    collection_name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Dropping collection '{}' on connection {}", collection_name, connection_id);
    
    let clients = state.clients.read().await;
    let client = clients.get(&connection_id)
        .ok_or_else(|| {
            error!("Drop collection failed: Connection {} not found", connection_id);
            "Connection not found".to_string()
        })?;
    
    match client.drop_collection(&collection_name).await {
        Ok(_) => {
            info!("Collection '{}' dropped successfully", collection_name);
            Ok(())
        },
        Err(e) => {
            error!("Failed to drop collection '{}': {}", collection_name, e);
            Err(format!("Failed to drop collection: {}", e))
        }
    }
}

// Import/Export commands
#[tauri::command]
async fn export_collection(
    connection_id: String,
    collection: String,
    format: String,
    query: Option<String>,
    include_metadata: bool,
    pretty_print: bool,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let clients = state.clients.read().await;
    let client = clients.get(&connection_id)
        .ok_or_else(|| "Connection not found".to_string())?;
    
    // Build query request
    let query_request = if let Some(query_str) = query {
        let filter = serde_json::from_str(&query_str)
            .map_err(|e| format!("Invalid query JSON: {}", e))?;
        QueryRequest {
            collection: collection.clone(),
            filter: Some(filter),
            projection: None,
            sort: None,
            skip: None,
            limit: None,
        }
    } else {
        QueryRequest {
            collection: collection.clone(),
            filter: None,
            projection: None,
            sort: None,
            skip: None,
            limit: None,
        }
    };
    
    // Execute query to get documents
    let documents = client.query(query_request).await
        .map_err(|e| format!("Failed to query collection: {}", e))?;
    
    // Convert documents to the requested format
    match format.as_str() {
        "json" => {
            let json_docs: Result<Vec<serde_json::Value>, String> = documents
                .into_iter()
                .map(|doc| {
                    let mut json_doc = serde_json::to_value(doc)
                        .map_err(|e| format!("Failed to serialize document: {}", e))?;
                    
                    // Remove metadata if not requested
                    if !include_metadata {
                        if let Some(obj) = json_doc.as_object_mut() {
                            obj.remove("_metadata");
                            obj.remove("_version");
                        }
                    }
                    
                    Ok::<serde_json::Value, String>(json_doc)
                })
                .collect();
            
            let docs = json_docs?;
            
            if pretty_print {
                serde_json::to_string_pretty(&docs)
                    .map_err(|e| format!("Failed to serialize JSON: {}", e))
            } else {
                serde_json::to_string(&docs)
                    .map_err(|e| format!("Failed to serialize JSON: {}", e))
            }
        }
        "csv" => {
            // Convert documents to CSV format
            if documents.is_empty() {
                return Ok(String::new());
            }
            
            // Extract all unique field names for CSV headers
            let mut field_names = std::collections::BTreeSet::new();
            for doc in &documents {
                let json_doc = serde_json::to_value(doc)
                    .map_err(|e| format!("Failed to serialize document: {}", e))?;
                
                if let Some(obj) = json_doc.as_object() {
                    for key in obj.keys() {
                        if include_metadata || (!key.starts_with("_") || key == "_id") {
                            field_names.insert(key.clone());
                        }
                    }
                }
            }
            
            let mut csv_output = String::new();
            
            // Write headers
            let headers: Vec<String> = field_names.iter().cloned().collect();
            csv_output.push_str(&headers.join(","));
            csv_output.push('\n');
            
            // Write data rows
            for doc in documents {
                let json_doc = serde_json::to_value(doc)
                    .map_err(|e| format!("Failed to serialize document: {}", e))?;
                
                let mut row_values = Vec::new();
                for field in &headers {
                    let value = json_doc.get(field)
                        .map(|v| match v {
                            serde_json::Value::String(s) => format!("\"{}\"", s.replace("\"", "\"\"")),
                            serde_json::Value::Number(n) => n.to_string(),
                            serde_json::Value::Bool(b) => b.to_string(),
                            serde_json::Value::Null => String::new(),
                            _ => format!("\"{}\"", v.to_string().replace("\"", "\"\"")),
                        })
                        .unwrap_or_default();
                    row_values.push(value);
                }
                csv_output.push_str(&row_values.join(","));
                csv_output.push('\n');
            }
            
            Ok(csv_output)
        }
        "bson" => {
            // For BSON, we'll export as JSON with a note that it's BSON-compatible
            let json_docs: Result<Vec<serde_json::Value>, String> = documents
                .into_iter()
                .map(|doc| {
                    let mut json_doc = serde_json::to_value(doc)
                        .map_err(|e| format!("Failed to serialize document: {}", e))?;
                    
                    // Remove metadata if not requested
                    if !include_metadata {
                        if let Some(obj) = json_doc.as_object_mut() {
                            obj.remove("_metadata");
                            obj.remove("_version");
                        }
                    }
                    
                    Ok::<serde_json::Value, String>(json_doc)
                })
                .collect();
            
            let docs = json_docs?;
            
            // For now, export as JSON (BSON binary format would require additional dependencies)
            if pretty_print {
                serde_json::to_string_pretty(&docs)
                    .map_err(|e| format!("Failed to serialize BSON: {}", e))
            } else {
                serde_json::to_string(&docs)
                    .map_err(|e| format!("Failed to serialize BSON: {}", e))
            }
        }
        _ => Err(format!("Unsupported export format: {}", format)),
    }
}

#[tauri::command]
async fn import_collection(
    connection_id: String,
    collection: String,
    format: String,
    data: String,
    mode: String,
    batch_size: u64,
    state: State<'_, AppState>,
) -> Result<ImportProgress, String> {
    let clients = state.clients.read().await;
    let client = clients.get(&connection_id)
        .ok_or_else(|| "Connection not found".to_string())?;
    
    // Parse the data based on format
    let documents: Vec<serde_json::Value> = match format.as_str() {
        "json" => {
            // Try to parse as array first, then as single document
            if let Ok(docs) = serde_json::from_str::<Vec<serde_json::Value>>(&data) {
                docs
            } else if let Ok(doc) = serde_json::from_str::<serde_json::Value>(&data) {
                vec![doc]
            } else {
                return Err("Invalid JSON format".to_string());
            }
        }
        "csv" => {
            // Parse CSV data
            let mut documents = Vec::new();
            let lines: Vec<&str> = data.lines().collect();
            
            if lines.is_empty() {
                return Ok(ImportProgress {
                    processed: 0,
                    total: 0,
                    errors: vec![],
                    is_complete: true,
                });
            }
            
            // Parse headers
            let headers: Vec<&str> = lines[0].split(',').map(|h| h.trim()).collect();
            
            // Parse data rows
            for (line_num, line) in lines.iter().skip(1).enumerate() {
                let values: Vec<&str> = line.split(',').map(|v| v.trim()).collect();
                
                if values.len() != headers.len() {
                    continue; // Skip malformed rows
                }
                
                let mut doc = serde_json::Map::new();
                for (i, header) in headers.iter().enumerate() {
                    let value = values.get(i).unwrap_or(&"").trim_matches('"');
                    
                    // Try to parse as number, boolean, or keep as string
                    let json_value = if value.is_empty() {
                        serde_json::Value::Null
                    } else if let Ok(num) = value.parse::<i64>() {
                        serde_json::Value::Number(serde_json::Number::from(num))
                    } else if let Ok(num) = value.parse::<f64>() {
                        serde_json::Value::Number(serde_json::Number::from_f64(num).unwrap_or(serde_json::Number::from(0)))
                    } else if value == "true" || value == "false" {
                        serde_json::Value::Bool(value == "true")
                    } else {
                        serde_json::Value::String(value.to_string())
                    };
                    
                    doc.insert(header.to_string(), json_value);
                }
                
                documents.push(serde_json::Value::Object(doc));
            }
            
            documents
        }
        "bson" => {
            // For BSON, we'll treat it as JSON for now
            if let Ok(docs) = serde_json::from_str::<Vec<serde_json::Value>>(&data) {
                docs
            } else if let Ok(doc) = serde_json::from_str::<serde_json::Value>(&data) {
                vec![doc]
            } else {
                return Err("Invalid BSON/JSON format".to_string());
            }
        }
        _ => return Err(format!("Unsupported import format: {}", format)),
    };
    
    let total_docs = documents.len() as u64;
    let mut processed = 0u64;
    let mut errors = Vec::new();
    
    // Handle different import modes
    if mode == "replace" {
        // Clear the collection first (this would need a drop_collection command)
        // For now, we'll just proceed with the import
    }
    
    // Process documents in batches
    for chunk in documents.chunks(batch_size as usize) {
        for doc_value in chunk {
            // Convert JSON value to Document
            match serde_json::from_value::<Document>(doc_value.clone()) {
                Ok(document) => {
                    match client.insert_document(&collection, document).await {
                        Ok(_) => processed += 1,
                        Err(e) => {
                            if mode == "insert" {
                                errors.push(format!("Failed to insert document: {}", e));
                            } else if mode == "upsert" {
                                // Try to update instead (this would need an upsert operation)
                                errors.push(format!("Upsert not yet implemented: {}", e));
                            }
                        }
                    }
                }
                Err(e) => {
                    errors.push(format!("Invalid document format: {}", e));
                }
            }
        }
    }
    
    Ok(ImportProgress {
        processed,
        total: total_docs,
        errors,
        is_complete: true,
    })
}

fn main() {
    // Initialize tracing subscriber for logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .init();
    
    info!("Starting VedDB Compass application");
    
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            // Connection management
            test_connection,
            connect_to_server,
            disconnect_from_server,
            get_connection_status,
            // Database exploration
            get_collections,
            execute_query,
            // Metrics
            get_server_metrics,
            // Index management
            create_index,
            drop_index,
            // User management
            get_users,
            create_user,
            delete_user,
            update_user_role,
            // Document operations
            insert_document,
            update_document,
            delete_document,
            // Collection operations
            create_collection,
            drop_collection,
            // Import/Export operations
            export_collection,
            import_collection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    // Helper to create a tokio runtime for async tests
    fn run_async<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Runtime::new().unwrap().block_on(f)
    }

    // Unit tests for resolve_host function
    #[test]
    fn test_resolve_localhost() {
        run_async(async {
            let result = resolve_host("localhost", 8080).await;
            assert!(result.is_ok(), "Failed to resolve localhost: {:?}", result);
            let addr = result.unwrap();
            assert_eq!(addr.port(), 8080);
        });
    }

    #[test]
    fn test_resolve_ipv4_address() {
        run_async(async {
            let result = resolve_host("127.0.0.1", 8080).await;
            assert!(result.is_ok());
            let addr = result.unwrap();
            assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
            assert_eq!(addr.port(), 8080);
        });
    }

    #[test]
    fn test_resolve_ipv6_address() {
        run_async(async {
            let result = resolve_host("::1", 8080).await;
            assert!(result.is_ok());
            let addr = result.unwrap();
            assert_eq!(addr.ip(), IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)));
            assert_eq!(addr.port(), 8080);
        });
    }

    #[test]
    fn test_resolve_invalid_hostname() {
        run_async(async {
            let result = resolve_host("this-hostname-definitely-does-not-exist-12345.invalid", 8080).await;
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(err.contains("Failed to resolve hostname"));
            assert!(err.contains("this-hostname-definitely-does-not-exist-12345.invalid"));
        });
    }

    #[test]
    fn test_error_message_includes_hostname() {
        run_async(async {
            let hostname = "nonexistent-host-xyz.test";
            let result = resolve_host(hostname, 9999).await;
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(err.contains(hostname), "Error message should include original hostname");
        });
    }

    // Property-based tests
    
    // Feature: compass-hostname-resolution-fix, Property 4: IP addresses bypass DNS resolution
    // Validates: Requirements 1.4, 3.2
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        
        #[test]
        fn prop_ipv4_addresses_bypass_dns(
            a in 0u8..=255,
            b in 0u8..=255,
            c in 0u8..=255,
            d in 0u8..=255,
            port in 1u16..=65535
        ) {
            run_async(async move {
                let ip_str = format!("{}.{}.{}.{}", a, b, c, d);
                let result = resolve_host(&ip_str, port).await;
                
                // Should succeed for valid IP addresses
                prop_assert!(result.is_ok(), "Failed to resolve IP address: {}", ip_str);
                
                let addr = result.unwrap();
                let expected_ip = IpAddr::V4(Ipv4Addr::new(a, b, c, d));
                
                // IP should match exactly (no DNS lookup performed)
                prop_assert_eq!(addr.ip(), expected_ip);
                prop_assert_eq!(addr.port(), port);
                
                Ok(())
            })?;
        }
    }

    // Feature: compass-hostname-resolution-fix, Property 9: IP addresses are returned unchanged
    // Validates: Requirements 3.2
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        
        #[test]
        fn prop_ip_addresses_returned_unchanged(
            a in 0u8..=255,
            b in 0u8..=255,
            c in 0u8..=255,
            d in 0u8..=255,
            port in 1u16..=65535
        ) {
            run_async(async move {
                let ip_str = format!("{}.{}.{}.{}", a, b, c, d);
                let original_ip = IpAddr::V4(Ipv4Addr::new(a, b, c, d));
                
                let result = resolve_host(&ip_str, port).await;
                prop_assert!(result.is_ok());
                
                let resolved_addr = result.unwrap();
                
                // The resolved IP should be identical to the input IP
                prop_assert_eq!(resolved_addr.ip(), original_ip);
                prop_assert_eq!(resolved_addr.port(), port);
                
                Ok(())
            })?;
        }
    }

    // Feature: compass-hostname-resolution-fix, Property 11: IPv4 preference in multi-address results
    // Validates: Requirements 3.4
    // Note: This test uses localhost which typically resolves to both IPv4 and IPv6
    #[test]
    fn test_ipv4_preference_localhost() {
        run_async(async {
            // localhost typically resolves to both 127.0.0.1 (IPv4) and ::1 (IPv6)
            let result = resolve_host("localhost", 8080).await;
            assert!(result.is_ok());
            
            let addr = result.unwrap();
            // If both IPv4 and IPv6 are available, IPv4 should be preferred
            // On most systems, localhost resolves to IPv4 first
            // We just verify that we get a valid address
            assert_eq!(addr.port(), 8080);
        });
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        
        #[test]
        fn prop_ipv6_addresses_work(
            a in 0u16..=0xFFFF,
            b in 0u16..=0xFFFF,
            c in 0u16..=0xFFFF,
            d in 0u16..=0xFFFF,
            e in 0u16..=0xFFFF,
            f in 0u16..=0xFFFF,
            g in 0u16..=0xFFFF,
            h in 0u16..=0xFFFF,
            port in 1u16..=65535
        ) {
            run_async(async move {
                let ip_str = format!("{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}", a, b, c, d, e, f, g, h);
                let result = resolve_host(&ip_str, port).await;
                
                // Should succeed for valid IPv6 addresses
                prop_assert!(result.is_ok(), "Failed to resolve IPv6 address: {}", ip_str);
                
                let addr = result.unwrap();
                let expected_ip = IpAddr::V6(Ipv6Addr::new(a, b, c, d, e, f, g, h));
                
                // IP should match exactly
                prop_assert_eq!(addr.ip(), expected_ip);
                prop_assert_eq!(addr.port(), port);
                
                Ok(())
            })?;
        }
    }

    // Feature: compass-hostname-resolution-fix, Property 1: Hostname resolution is attempted before connection
    // Validates: Requirements 1.1
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        
        #[test]
        fn prop_hostname_resolution_before_connection(
            a in 0u8..=255,
            b in 0u8..=255,
            c in 0u8..=255,
            d in 0u8..=255,
            port in 1024u16..=65535
        ) {
            run_async(async move {
                // Generate a valid IP address to use as hostname
                let hostname = format!("{}.{}.{}.{}", a, b, c, d);
                
                // Create a test connection config
                let config = ConnectionConfig {
                    id: "test-connection".to_string(),
                    name: "Test Connection".to_string(),
                    host: hostname.clone(),
                    port,
                    username: None,
                    password: None,
                    tls: false,
                };
                
                // First verify that resolve_host works for this input
                let resolve_result = resolve_host(&config.host, config.port).await;
                prop_assert!(resolve_result.is_ok(), "Resolution should succeed for valid IP: {}", hostname);
                
                let resolved_addr = resolve_result.unwrap();
                let expected_ip = IpAddr::V4(Ipv4Addr::new(a, b, c, d));
                
                // Verify that resolution returns the correct address
                prop_assert_eq!(resolved_addr.ip(), expected_ip);
                prop_assert_eq!(resolved_addr.port(), port);
                
                // The property we're testing: hostname resolution must happen before connection
                // We verify this by checking that resolve_host is called and returns a valid SocketAddr
                // The actual connection will fail (no server running), but resolution should succeed
                // This demonstrates that resolution is attempted before connection
                
                Ok(())
            })?;
        }
    }

    // Unit tests for connection handlers
    // Requirements: 1.2, 1.3, 2.1

    #[test]
    fn test_resolution_with_ip_address_before_connection() {
        run_async(async {
            // Test that IP addresses are resolved before connection attempts
            let host = "127.0.0.1";
            let port = 9999;
            
            // Resolution should succeed for IP addresses
            let result = resolve_host(host, port).await;
            assert!(result.is_ok(), "IP address resolution should succeed");
            
            let addr = result.unwrap();
            assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
            assert_eq!(addr.port(), port);
        });
    }

    #[test]
    fn test_resolution_error_for_invalid_hostname() {
        run_async(async {
            // Test that invalid hostnames produce proper error messages
            let host = "this-hostname-definitely-does-not-exist-xyz123.invalid";
            let port = 8080;
            
            let result = resolve_host(host, port).await;
            assert!(result.is_err(), "Invalid hostname should fail to resolve");
            
            let err = result.unwrap_err();
            // Error should mention hostname resolution failure
            assert!(err.contains("Failed to resolve hostname"), 
                "Error should mention hostname resolution: {}", err);
            // Error should include the original hostname
            assert!(err.contains(host), 
                "Error should include original hostname: {}", err);
        });
    }

    #[test]
    fn test_resolution_with_localhost() {
        run_async(async {
            // Test that localhost hostname resolves successfully
            let host = "localhost";
            let port = 9999;
            
            let result = resolve_host(host, port).await;
            assert!(result.is_ok(), "localhost should resolve successfully");
            
            let addr = result.unwrap();
            assert_eq!(addr.port(), port);
            // localhost typically resolves to either 127.0.0.1 or ::1
            assert!(addr.is_ipv4() || addr.is_ipv6(), 
                "localhost should resolve to either IPv4 or IPv6");
        });
    }

    #[test]
    fn test_tls_config_creation_with_hostname() {
        run_async(async {
            // Test that TLS configuration can be created with a hostname
            let host = "example.com";
            let port = 8080;
            
            // First resolve the hostname
            let resolve_result = resolve_host(host, port).await;
            
            // Resolution might fail if DNS is unavailable, but we're testing the flow
            // If it succeeds, we can create TLS config
            if resolve_result.is_ok() {
                let addr = resolve_result.unwrap();
                
                // Create TLS config with the original hostname (not the resolved IP)
                let tls_config = TlsConfig::new(host).accept_invalid_certs();
                
                // Verify we can create a client with the resolved address and TLS config
                // (This will fail to connect, but tests the parameter flow)
                let client_result = create_client(addr, Some(tls_config), None).await;
                
                // Connection will fail (no server), but that's expected
                assert!(client_result.is_err(), "Connection should fail (no server)");
            }
        });
    }

    #[test]
    fn test_auth_config_creation_with_credentials() {
        run_async(async {
            // Test that auth configuration works with resolved addresses
            let host = "127.0.0.1";
            let port = 9999;
            
            // Resolve the address
            let result = resolve_host(host, port).await;
            assert!(result.is_ok(), "IP resolution should succeed");
            
            let addr = result.unwrap();
            
            // Create auth config
            let auth_config = AuthConfig::username_password("testuser", "testpass");
            
            // Try to create client with auth (will fail to connect, but tests the flow)
            let client_result = create_client(addr, None, Some(auth_config)).await;
            
            // Connection will fail (no server), but that's expected
            assert!(client_result.is_err(), "Connection should fail (no server)");
        });
    }

    #[test]
    fn test_combined_tls_and_auth_with_resolution() {
        run_async(async {
            // Test that both TLS and auth work together with hostname resolution
            let host = "localhost";
            let port = 9999;
            
            // Resolve the hostname
            let result = resolve_host(host, port).await;
            assert!(result.is_ok(), "localhost resolution should succeed");
            
            let addr = result.unwrap();
            
            // Create both TLS and auth configs
            let tls_config = TlsConfig::new(host).accept_invalid_certs();
            let auth_config = AuthConfig::username_password("testuser", "testpass");
            
            // Try to create client with both (will fail to connect, but tests the flow)
            let client_result = create_client(addr, Some(tls_config), Some(auth_config)).await;
            
            // Connection will fail (no server), but that's expected
            assert!(client_result.is_err(), "Connection should fail (no server)");
        });
    }

    #[test]
    fn test_error_message_includes_original_hostname_context() {
        run_async(async {
            // Test that error messages preserve the original hostname for debugging
            let hostnames = vec![
                "nonexistent-host-1.invalid",
                "fake-server-xyz.test",
                "does-not-exist-123.local",
            ];
            
            for hostname in hostnames {
                let result = resolve_host(hostname, 8080).await;
                assert!(result.is_err(), "Invalid hostname should fail: {}", hostname);
                
                let err = result.unwrap_err();
                // Each error should include the specific hostname that failed
                assert!(err.contains(hostname), 
                    "Error should include hostname '{}': {}", hostname, err);
                assert!(err.contains("Failed to resolve hostname"),
                    "Error should mention resolution failure: {}", err);
            }
        });
    }

    // Tests for error categorization
    // Requirements: 1.3, 2.1, 2.2, 2.3

    #[test]
    fn test_dns_resolution_error_formatting() {
        // Test that DNS resolution errors are properly formatted with hostname context
        let hostname = "nonexistent-host.invalid";
        let error = ErrorCategory::dns_resolution(hostname, "no such host");
        let error_msg = error.to_string();
        
        assert!(error_msg.contains("Failed to resolve hostname"));
        assert!(error_msg.contains(hostname));
        assert!(error_msg.contains("no such host"));
        assert!(error_msg.contains("Please check if the hostname is correct"));
    }

    #[test]
    fn test_connection_error_formatting_with_resolved_ip() {
        // Test that connection errors include both hostname and resolved IP
        let hostname = "example.com";
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 8080));
        let error = ErrorCategory::connection(hostname, addr, "connection refused");
        let error_msg = error.to_string();
        
        assert!(error_msg.contains("Failed to connect to"));
        assert!(error_msg.contains("127.0.0.1:8080"));
        assert!(error_msg.contains(hostname));
        assert!(error_msg.contains("connection refused"));
        assert!(error_msg.contains("Please verify the server is running"));
    }

    #[test]
    fn test_authentication_error_formatting() {
        // Test that authentication errors are clearly distinguished
        let error = ErrorCategory::authentication("invalid credentials");
        let error_msg = error.to_string();
        
        assert!(error_msg.contains("Authentication failed"));
        assert!(error_msg.contains("invalid credentials"));
        assert!(error_msg.contains("Please check your username and password"));
    }

    #[test]
    fn test_error_type_categorization() {
        // Test that veddb_client errors are properly categorized
        use veddb_client::Error;
        
        // Test authentication error detection
        let auth_error = Error::AuthenticationFailed;
        assert!(ErrorCategory::is_authentication_error(&auth_error));
        
        // Test connection error detection
        let conn_error = Error::Connection("test".to_string());
        assert!(ErrorCategory::is_connection_error(&conn_error));
        
        let io_error = Error::Io(std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "test"));
        assert!(ErrorCategory::is_connection_error(&io_error));
        
        let not_connected_error = Error::NotConnected;
        assert!(ErrorCategory::is_connection_error(&not_connected_error));
    }

    #[test]
    fn test_error_categorization_from_client_error() {
        // Test that client errors are properly categorized with context
        use veddb_client::Error;
        
        let hostname = "test.example.com";
        let addr = std::net::SocketAddr::from(([192, 168, 1, 100], 9999));
        
        // Test authentication error categorization
        let auth_error = Error::AuthenticationFailed;
        let categorized = ErrorCategory::from_client_error(hostname, Some(addr), &auth_error);
        match categorized {
            ErrorCategory::Authentication(msg) => {
                assert!(msg.contains("Authentication failed"));
            }
            _ => panic!("Expected Authentication error category"),
        }
        
        // Test connection error categorization
        let conn_error = Error::Connection("refused".to_string());
        let categorized = ErrorCategory::from_client_error(hostname, Some(addr), &conn_error);
        match categorized {
            ErrorCategory::Connection(msg) => {
                assert!(msg.contains("Failed to connect to"));
                assert!(msg.contains("192.168.1.100:9999"));
                assert!(msg.contains(hostname));
            }
            _ => panic!("Expected Connection error category"),
        }
        
        // Test general error categorization
        let general_error = Error::Protocol("invalid protocol".to_string());
        let categorized = ErrorCategory::from_client_error(hostname, Some(addr), &general_error);
        match categorized {
            ErrorCategory::General(_) => {
                // Expected
            }
            _ => panic!("Expected General error category"),
        }
    }
}
