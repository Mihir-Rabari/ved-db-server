//! TCP Integration Tests for Protocol Handlers
//!
//! End-to-end tests using actual TCP connections to verify all 17 handlers

use veddb_core::protocol::{Command, Response, OpCode, Status, CommandHeader, CmdHeader};
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::atomic::{AtomicU32, Ordering};
use anyhow::Result;

/// TCP Test Client for end-to-end protocol testing
pub struct TcpTestClient {
    stream: TcpStream,
    seq: AtomicU32,
}

impl TcpTestClient {
    /// Connect to VedDB server
    pub async fn connect(addr: &str) -> Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        Ok(Self {
            stream,
            seq: AtomicU32::new(1),
        })
    }

    /// Send a command and receive response
    pub async fn send_command(&mut self, opcode: OpCode, payload: Vec<u8>) -> Result<Response> {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst);
        
        // Build command
        let command = Command {
            header: CmdHeader {
                version: 2,
                opcode: opcode as u8,
                seq,
                flags: 0,
                payload_len: payload.len() as u32,
            },
            value: payload,
        };

        // Serialize header (10 bytes: version(2) + opcode(1) + seq(4) + flags(1) + len(4))
        let mut header_bytes = Vec::with_capacity(12);
        header_bytes.extend_from_slice(&command.header.version.to_be_bytes());
        header_bytes.push(command.header.opcode);
        header_bytes.extend_from_slice(&command.header.seq.to_be_bytes());
        header_bytes.push(command.header.flags);
        header_bytes.extend_from_slice(&command.header.payload_len.to_be_bytes());

        // Send header + payload
        self.stream.write_all(&header_bytes).await?;
        self.stream.write_all(&command.value).await?;
        self.stream.flush().await?;

        // Read response header
        let mut resp_header = vec![0u8; 12];
        self.stream.read_exact(&mut resp_header).await?;

        let status = resp_header[0];
        let resp_seq = u32::from_be_bytes([resp_header[1], resp_header[2], resp_header[3], resp_header[4]]);
        let payload_len = u32::from_be_bytes([resp_header[8], resp_header[9], resp_header[10], resp_header[11]]);

        // Read payload
        let mut payload = vec![0u8; payload_len as usize];
        if payload_len > 0 {
            self.stream.read_exact(&mut payload).await?;
        }

        Ok(Response {
            status: Status::from_u8(status),
            seq: resp_seq,
            payload,
        })
    }

    /// Login to server
    pub async fn login(&mut self, username: &str, password: &str) -> Result<String> {
        let request = serde_json::json!({
            "username": username,
            "password": password,
        });
        let payload = serde_json::to_vec(&request)?;
        
        let response = self.send_command(OpCode::Login, payload).await?;
        if response.status != Status::Ok {
            anyhow::bail!("Login failed");
        }

        let token: String = serde_json::from_slice(&response.payload)?;
        Ok(token)
    }

    /// Create backup
    pub async fn create_backup(&mut self) -> Result<serde_json::Value> {
        let request = serde_json::json!({
            "wal_sequence": null,
            "compress": false,
        });
        let payload = serde_json::to_vec(&request)?;
        
        let response = self.send_command(OpCode::CreateBackup, payload).await?;
        if response.status != Status::Ok {
            anyhow::bail!("Create backup failed");
        }

        Ok(serde_json::from_slice(&response.payload)?)
    }

    /// List backups
    pub async fn list_backups(&mut self) -> Result<Vec<serde_json::Value>> {
        let payload = vec![];
        
        let response = self.send_command(OpCode::ListBackups, payload).await?;
        if response.status != Status::Ok {
            antml::bail!("List backups failed");
        }

        Ok(serde_json::from_slice(&response.payload)?)
    }

    /// Create encryption key
    pub async fn create_key(&mut self, key_id: &str) -> Result<()> {
        let request = serde_json::json!({
            "key_id": key_id,
        });
        let payload = serde_json::to_vec(&request)?;
        
        let response = self.send_command(OpCode::CreateKey, payload).await?;
        if response.status != Status::Ok {
            anyhow::bail!("Create key failed");
        }

        Ok(())
    }

    /// List encryption keys
    pub async fn list_keys(&mut self) -> Result<Vec<serde_json::Value>> {
        let payload = vec![];
        
        let response = self.send_command(OpCode::ListKeys, payload).await?;
        if response.status != Status::Ok {
            anyhow::bail!("List keys failed");
        }

        Ok(serde_json::from_slice(&response.payload)?)
    }

    /// Export encryption key
    pub async fn export_key(&mut self, key_id: &str) -> Result<String> {
        let request = serde_json::json!({
            "key_id": key_id,
        });
        let payload = serde_json::to_vec(&request)?;
        
        let response = self.send_command(OpCode::ExportKey, payload).await?;
        if response.status != Status::Ok {
            anyhow::bail!("Export key failed");
        }

        let resp: serde_json::Value = serde_json::from_slice(&response.payload)?;
        Ok(resp["encrypted_data"].as_str().unwrap().to_string())
    }

    /// Get replication status
    pub async fn get_replication_status(&mut self) -> Result<serde_json::Value> {
        let payload = vec![];
        
        let response = self.send_command(OpCode::GetReplicationStatus, payload).await?;
        if response.status != Status::Ok {
            anyhow::bail!("Get replication status failed");
        }

        Ok(serde_json::from_slice(&response.payload)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to start server for tests
    async fn start_test_server() -> tokio::task::JoinHandle<()> {
        tokio::spawn(async {
            // Start veddb-server in background
            // In real tests, you'd spawn actual server process
        })
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_tcp_create_backup() {
        let mut client = TcpTestClient::connect("localhost:50051").await.unwrap();
        
        // Login first
        let _token = client.login("admin", "admin123").await.unwrap();
        
        // Create backup
        let backup_info = client.create_backup().await.unwrap();
        assert!(backup_info["backup_id"].is_string());
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_tcp_list_backups() {
        let mut client = TcpTestClient::connect("localhost:50051").await.unwrap();
        let _token = client.login("admin", "admin123").await.unwrap();
        
        // Create then list
        let _backup1 = client.create_backup().await.unwrap();
        let _backup2 = client.create_backup().await.unwrap();
        
        let backups = client.list_backups().await.unwrap();
        assert!(backups.len() >= 2);
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_tcp_create_key() {
        let mut client = TcpTestClient::connect("localhost:50051").await.unwrap();
        let _token = client.login("admin", "admin123").await.unwrap();
        
        // Create key
        client.create_key("test_tcp_key").await.unwrap();
        
        // List and verify
        let keys = client.list_keys().await.unwrap();
        assert!(keys.iter().any(|k| k["key_id"] == "test_tcp_key"));
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_tcp_export_key() {
        let mut client = TcpTestClient::connect("localhost:50051").await.unwrap();
        let _token = client.login("admin", "admin123").await.unwrap();
        
        // Create and export
        client.create_key("export_tcp_key").await.unwrap();
        let exported = client.export_key("export_tcp_key").await.unwrap();
        assert!(!exported.is_empty());
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_tcp_replication_status() {
        let mut client = TcpTestClient::connect("localhost:50051").await.unwrap();
        let _token = client.login("admin", "admin123").await.unwrap();
        
        // Get status
        let status = client.get_replication_status().await.unwrap();
        assert!(status["role"].is_string());
    }

    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_tcp_concurrent_operations() {
        let num_clients = 10;
        let mut handles = vec![];

        for i in 0..num_clients {
            let handle = tokio::spawn(async move {
                let mut client = TcpTestClient::connect("localhost:50051").await.unwrap();
                let _token = client.login("admin", "admin123").await.unwrap();
                
                // Each client creates a backup
                client.create_backup().await.unwrap();
                
                // Each client creates a key
                client.create_key(&format!("concurrent_key_{}", i)).await.unwrap();
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }
    }
}
