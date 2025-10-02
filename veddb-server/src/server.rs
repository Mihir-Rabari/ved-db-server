//! TCP server implementation for remote VedDB access
//!
//! Provides binary protocol interface for remote clients to interact
//! with VedDB over the network.

use anyhow::Result;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{error, info, warn};
use veddb_core::{VedDb, SimpleKvStore};

/// VedDB TCP server
pub struct VedDbServer {
    veddb: Arc<VedDb>,
    kv_store: SimpleKvStore,
}

impl VedDbServer {
    pub fn new(veddb: Arc<VedDb>) -> Self {
        Self { 
            veddb,
            kv_store: SimpleKvStore::new(),
        }
    }

    /// Start the gRPC server on the specified port
    pub async fn serve(self, port: u16) -> Result<()> {
        let addr = format!("0.0.0.0:{}", port);
        info!("Starting gRPC server on {}", addr);

        // For now, implement a simple TCP server
        // In production, you'd use tonic for proper gRPC
        let listener = TcpListener::bind(&addr).await?;
        info!("gRPC server listening on {}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New client connection from {}", addr);
                    let veddb = self.veddb.clone();
                    let kv_store = self.kv_store.clone();

                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client(veddb, kv_store, stream).await {
                            warn!("Client {} error: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    /// Handle a single client connection
    async fn handle_client(veddb: Arc<VedDb>, kv_store: SimpleKvStore, mut stream: tokio::net::TcpStream) -> Result<()> {
        // Attach a session for this client
        let session_id = veddb.attach_session(std::process::id())?;
        info!("Attached session {} for remote client", session_id);

        loop {
            // Read command header (24 bytes)
            let mut header_buf = [0u8; 24];
            match stream.read_exact(&mut header_buf).await {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    info!("Client disconnected");
                    break;
                }
                Err(e) => {
                    warn!("Failed to read header: {}", e);
                    break;
                }
            }

            // Parse header (little-endian)
            let opcode = header_buf[0];
            let seq = u32::from_le_bytes([header_buf[4], header_buf[5], header_buf[6], header_buf[7]]);
            let key_len = u32::from_le_bytes([header_buf[8], header_buf[9], header_buf[10], header_buf[11]]) as usize;
            let val_len = u32::from_le_bytes([header_buf[12], header_buf[13], header_buf[14], header_buf[15]]) as usize;
            
            info!("Command: opcode={}, seq={}, key_len={}, val_len={}", opcode, seq, key_len, val_len);

            // Read key
            let mut key = vec![0u8; key_len];
            if key_len > 0 {
                stream.read_exact(&mut key).await?;
            }

            // Read value
            let mut value = vec![0u8; val_len];
            if val_len > 0 {
                stream.read_exact(&mut value).await?;
            }

            // Process command based on opcode
            // Status codes: 0x00=Ok, 0x01=NotFound, 0x04=InternalError
            let (status, payload) = match opcode {
                0x01 => {
                    // Ping
                    info!("PING");
                    (0x00, b"pong".to_vec())
                }
                0x02 => {
                    // Set
                    info!("SET key={:?} value={:?}", String::from_utf8_lossy(&key), String::from_utf8_lossy(&value));
                    match kv_store.set(&key, &value) {
                        Ok(()) => {
                            info!("SET successful, total keys: {}", kv_store.len());
                            (0x00, Vec::new())
                        }
                        Err(e) => {
                            warn!("SET error: {}", e);
                            (0x04, b"error".to_vec()) // InternalError
                        }
                    }
                }
                0x03 => {
                    // Get
                    info!("GET key={:?}", String::from_utf8_lossy(&key));
                    match kv_store.get(&key) {
                        Some(v) => {
                            info!("GET found value: {} bytes", v.len());
                            (0x00, v)
                        }
                        None => {
                            info!("GET not found");
                            (0x01, Vec::new()) // NotFound = 0x01
                        }
                    }
                }
                0x04 => {
                    // Delete
                    info!("DELETE key={:?}", String::from_utf8_lossy(&key));
                    if kv_store.delete(&key) {
                        info!("DELETE successful, total keys: {}", kv_store.len());
                        (0x00, Vec::new())
                    } else {
                        info!("DELETE not found");
                        (0x01, Vec::new()) // NotFound = 0x01
                    }
                }
                0x09 => {
                    // List keys (Fetch opcode)
                    info!("LIST keys");
                    let keys = kv_store.keys();
                    info!("Found {} keys", keys.len());
                    
                    // Serialize keys as newline-separated list
                    let mut payload = Vec::new();
                    for key in keys {
                        payload.extend_from_slice(&key);
                        payload.push(b'\n');
                    }
                    
                    (0x00, payload)
                }
                _ => {
                    warn!("Unknown opcode: {}", opcode);
                    (0x04, b"unknown command".to_vec()) // InternalError
                }
            };

            // Build response (20 bytes header + payload)
            let mut response = Vec::with_capacity(20 + payload.len());
            response.push(status); // status
            response.push(0); // flags
            response.extend_from_slice(&0u16.to_le_bytes()); // reserved
            response.extend_from_slice(&seq.to_le_bytes()); // seq
            response.extend_from_slice(&(payload.len() as u32).to_le_bytes()); // payload_len
            response.extend_from_slice(&0u64.to_le_bytes()); // extra
            response.extend_from_slice(&payload);

            info!("Sending response: status={}, payload_len={}", status, payload.len());
            
            // Send response
            stream.write_all(&response).await?;
            stream.flush().await?;
            
            info!("Response sent successfully");
        }

        // Cleanup session
        if let Err(e) = veddb.detach_session(session_id) {
            warn!("Failed to detach session {}: {}", session_id, e);
        }

        Ok(())
    }
}
