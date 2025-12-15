// Test script to verify Compass connection functionality
// This tests the connection flow without needing the full Tauri app

use std::net::SocketAddr;
use veddb_client::{Client, TlsConfig, AuthConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== VedDB Compass Connection Test ===\n");
    
    // Test configuration matching the default server setup
    let host = "127.0.0.1";
    let port = 50051;
    let username = "admin";
    let password = "admin123";
    
    println!("Test Configuration:");
    println!("  Host: {}", host);
    println!("  Port: {}", port);
    println!("  Username: {}", username);
    println!("  Password: {}", password);
    println!();
    
    // Step 1: Resolve hostname to IP address
    println!("Step 1: Resolving hostname...");
    let addr_string = format!("{}:{}", host, port);
    let socket_addr: SocketAddr = match tokio::net::lookup_host(&addr_string).await {
        Ok(mut addrs) => {
            match addrs.next() {
                Some(addr) => {
                    println!("  ✓ Resolved {} to {}", host, addr);
                    addr
                }
                None => {
                    println!("  ✗ No addresses found for {}", host);
                    return Err("No addresses found".into());
                }
            }
        }
        Err(e) => {
            println!("  ✗ Failed to resolve hostname: {}", e);
            return Err(e.into());
        }
    };
    println!();
    
    // Step 2: Create authentication config
    println!("Step 2: Creating authentication config...");
    let auth_config = AuthConfig::username_password(username, password);
    println!("  ✓ Auth config created");
    println!();
    
    // Step 3: Establish TCP connection and authenticate
    println!("Step 3: Connecting to server...");
    let client = match Client::connect_with_auth(socket_addr, None, auth_config).await {
        Ok(client) => {
            println!("  ✓ Connected successfully!");
            client
        }
        Err(e) => {
            println!("  ✗ Connection failed: {}", e);
            return Err(e.into());
        }
    };
    println!();
    
    // Step 4: Test the connection with a ping
    println!("Step 4: Testing connection with ping...");
    match client.ping().await {
        Ok(_) => {
            println!("  ✓ Ping successful!");
        }
        Err(e) => {
            println!("  ✗ Ping failed: {}", e);
            return Err(e.into());
        }
    };
    println!();
    
    // Step 5: Test a simple operation (list collections or similar)
    println!("Step 5: Testing basic operation...");
    // Since we don't have a list_collections command, we'll just do another ping
    match client.ping().await {
        Ok(_) => {
            println!("  ✓ Basic operation successful!");
        }
        Err(e) => {
            println!("  ✗ Basic operation failed: {}", e);
            return Err(e.into());
        }
    };
    println!();
    
    println!("=== All Tests Passed! ===");
    println!();
    println!("Summary:");
    println!("  ✓ Hostname resolution works");
    println!("  ✓ TCP connection established");
    println!("  ✓ Authentication successful");
    println!("  ✓ Connection status: Connected");
    println!("  ✓ Server responds to commands");
    println!();
    println!("The Compass connection flow is working correctly!");
    
    Ok(())
}
