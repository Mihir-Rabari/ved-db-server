// Manual test for verifying the full connection workflow
use veddb_client::{Client, AuthConfig};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Manual Connection Test ===\n");

    // Test 1: Connect with authentication
    println!("Test 1: Connecting with authentication...");
    let auth_config = AuthConfig::username_password("admin", "admin123");
    
    match Client::connect_with_auth(
        ([127, 0, 0, 1], 50051),
        None,
        auth_config,
    ).await {
        Ok(client) => {
            println!("✓ Connected successfully");
            
            // Test 2: Ping
            println!("\nTest 2: Sending ping...");
            match client.ping().await {
                Ok(_) => println!("✓ Ping successful"),
                Err(e) => println!("✗ Ping failed: {}", e),
            }
            
            // Test 3: Set a value
            println!("\nTest 3: Setting a value...");
            match client.set("test_key", "test_value").await {
                Ok(_) => println!("✓ Set successful"),
                Err(e) => println!("✗ Set failed: {}", e),
            }
            
            // Test 4: Get the value
            println!("\nTest 4: Getting the value...");
            match client.get("test_key").await {
                Ok(value) => {
                    let value_str = String::from_utf8_lossy(&value);
                    println!("✓ Get successful: {}", value_str);
                }
                Err(e) => println!("✗ Get failed: {}", e),
            }
            
            // Test 5: Delete the value
            println!("\nTest 5: Deleting the value...");
            match client.delete("test_key").await {
                Ok(_) => println!("✓ Delete successful"),
                Err(e) => println!("✗ Delete failed: {}", e),
            }
            
            println!("\n=== All tests completed ===");
        }
        Err(e) => {
            println!("✗ Connection failed: {}", e);
            return Err(e.into());
        }
    }
    
    // Give the server time to process
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    Ok(())
}
