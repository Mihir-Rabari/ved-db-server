use veddb_client::{Client, CreateCollectionRequest};
use veddb_client::{CreateIndexRequest, IndexField};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    const HOST: &str = "127.0.0.1";
    const PORT: u16 = 50051;
    
    println!("Connecting to VedDB at {}:{}...", HOST, PORT);
    
    let addr: std::net::SocketAddr = format!("{}:{}", HOST, PORT).parse()?;
    
    // Connect using basic auth
    let client = Client::connect_with_auth(
        addr,
        None,
        veddb_client::AuthConfig::username_password("admin", "admin123")
    ).await?;
    
    println!("Connected successfully!");
    
    // Test 1: List Collections (Initial)
    println!("\n--- Test 1: List Collections (Initial) ---");
    // Client::list_collections() takes no args
    let initial_cols = client.list_collections().await?;
    println!("Current collections: {:?}", initial_cols);
    
    // Test 2: Create Collection
    let test_col = "compass_test_collection";
    println!("\n--- Test 2: Create Collection '{}' ---", test_col);
    
    // Clean up if exists from previous run (ignore error)
    // Client::drop_collection takes name: impl Into<String>
    let _ = client.drop_collection(test_col).await;
    
    let create_req = CreateCollectionRequest {
        name: test_col.to_string(),
        schema: None,
    };
    client.create_collection(create_req).await?;
    println!("Collection created!");
    
    // Verify creation
    let cols_after_create = client.list_collections().await?;
    println!("Collections after create: {:?}", cols_after_create);
    assert!(cols_after_create.contains(&test_col.to_string()), "Collection should exist");
    
    // Test 3: List Indexes (Initial)
    println!("\n--- Test 3: List Indexes (Initial) ---");
    // Client::list_indexes takes collection: impl Into<String>
    let initial_indexes = client.list_indexes(test_col).await?;
    println!("Indexes on '{}': {:?}", test_col, initial_indexes);
    
    // Test 4: Create Index
    let index_name = "test_idx_1";
    println!("\n--- Test 4: Create Index '{}' ---", index_name);
    let create_idx_req = CreateIndexRequest {
        collection: test_col.to_string(),
        name: index_name.to_string(),
        fields: vec![IndexField { field: "field1".to_string(), direction: 1 }],
        unique: false,
    };
    client.create_index(create_idx_req).await?;
    println!("Index created!");
    
    // Verify index creation
    let indexes_after_create = client.list_indexes(test_col).await?;
    println!("Indexes after create: {:?}", indexes_after_create);
    
    // Test 5: Drop Index
    println!("\n--- Test 5: Drop Index '{}' ---", index_name);
    // Client::drop_index takes collection: impl Into<String>, name: impl Into<String>
    client.drop_index(test_col, index_name).await?; 
    println!("Index dropped!");

    let indexes_after_drop = client.list_indexes(test_col).await?;
    println!("Indexes after drop: {:?}", indexes_after_drop);
    
    // Test 6: Drop Collection
    println!("\n--- Test 6: Drop Collection '{}' ---", test_col);
    client.drop_collection(test_col).await?;
    println!("Collection dropped!");
    
    // Verify drop
    let cols_after_drop = client.list_collections().await?;
    println!("Collections after drop: {:?}", cols_after_drop);
    assert!(!cols_after_drop.contains(&test_col.to_string()), "Collection should NOT exist");
    
    println!("\nAll Compass Operations tests passed successfully!");
    
    Ok(())
}
