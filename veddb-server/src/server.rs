//! gRPC server implementation for remote VedDB access
//! 
//! Provides streaming RPC interface for remote clients to interact
//! with VedDB over the network.

use anyhow::Result;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tracing::{info, warn, error};
use veddb_core::{VedDb, Command, Response};

/// VedDB gRPC server
pub struct VedDbServer {
    veddb: Arc<VedDb>,
}

impl VedDbServer {
    pub fn new(veddb: Arc<VedDb>) -> Self {
        Self { veddb }
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
                    
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client(veddb, stream).await {
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
    async fn handle_client(
        veddb: Arc<VedDb>,
        mut stream: tokio::net::TcpStream,
    ) -> Result<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        
        // Attach a session for this client
        let session_id = veddb.attach_session(std::process::id())?;
        info!("Attached session {} for remote client", session_id);
        
        let mut buffer = vec![0u8; 8192];
        
        loop {
            // Read command from client
            match stream.read(&mut buffer).await {
                Ok(0) => {
                    info!("Client disconnected");
                    break;
                }
                Ok(n) => {
                    // Parse command
                    match Command::from_bytes(&buffer[..n]) {
                        Ok(command) => {
                            // Process command
                            let response = veddb.process_command(command);
                            
                            // Send response back
                            let response_bytes = response.to_bytes();
                            if let Err(e) = stream.write_all(&response_bytes).await {
                                warn!("Failed to send response: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            warn!("Failed to parse command: {}", e);
                            // Send error response
                            let error_response = Response::error(0);
                            let response_bytes = error_response.to_bytes();
                            let _ = stream.write_all(&response_bytes).await;
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to read from client: {}", e);
                    break;
                }
            }
        }
        
        // Cleanup session
        if let Err(e) = veddb.detach_session(session_id) {
            warn!("Failed to detach session {}: {}", session_id, e);
        }
        
        Ok(())
    }
}
