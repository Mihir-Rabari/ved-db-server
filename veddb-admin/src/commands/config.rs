use anyhow::Result;
use clap::Subcommand;
use serde_json::{json, Value};
use crate::client::AdminClient;

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Get configuration value(s)
    Get {
        /// Configuration key (e.g., "server.port" or "storage.data_dir")
        /// If not provided, shows all configuration
        key: Option<String>,
    },
    /// Set configuration value
    Set {
        /// Configuration key (e.g., "server.log_level")
        key: String,
        /// Configuration value
        value: String,
    },
    /// Reload configuration from file
    Reload,
    /// Show current configuration file path
    Path,
    /// Validate configuration
    Validate,
}

pub async fn execute_config_command(client: &mut AdminClient, command: ConfigCommands) -> Result<()> {
    match command {
        ConfigCommands::Get { key } => {
            println!("Fetching configuration...");
            
            let response = client.execute_command("config.get", json!({})).await?;
            
            match key {
                Some(key_path) => {
                    // Get specific key
                    let value = get_nested_value(&response, &key_path);
                    match value {
                        Some(val) => {
                            println!("{} = {}", key_path, format_config_value(val));
                        },
                        None => {
                            println!("Configuration key '{}' not found", key_path);
                        }
                    }
                },
                None => {
                    // Show all configuration
                    println!("\nCurrent Configuration:");
                    println!("{}", "=".repeat(50));
                    print_config_section("server", response.get("server"));
                    print_config_section("storage", response.get("storage"));
                    print_config_section("cache", response.get("cache"));
                }
            }
        },
        
        ConfigCommands::Set { key, value } => {
            println!("Setting configuration: {} = {}", key, value);
            
            let response = client.execute_command("config.set", json!({
                "key": key,
                "value": parse_config_value(&value)
            })).await?;
            
            if let Some(message) = response.get("message").and_then(|m| m.as_str()) {
                println!("✓ {}", message);
            }
            
            // Check if restart is required
            if is_restart_required(&key) {
                println!("⚠️  Note: Server restart required for this change to take effect");
            } else {
                println!("✓ Configuration updated and applied immediately");
            }
        },
        
        ConfigCommands::Reload => {
            println!("Reloading configuration from file...");
            
            let response = client.execute_command("config.reload", json!({})).await?;
            
            if let Some(message) = response.get("message").and_then(|m| m.as_str()) {
                println!("✓ {}", message);
            }
        },
        
        ConfigCommands::Path => {
            println!("Configuration file path: /etc/veddb/veddb.toml");
            println!("User config path: ~/.config/veddb/veddb.toml");
        },
        
        ConfigCommands::Validate => {
            println!("Validating configuration...");
            
            let response = client.execute_command("config.validate", json!({})).await?;
            
            if let Some(valid) = response.get("valid").and_then(|v| v.as_bool()) {
                if valid {
                    println!("✓ Configuration is valid");
                } else {
                    println!("✗ Configuration has errors:");
                    if let Some(errors) = response.get("errors").and_then(|e| e.as_array()) {
                        for error in errors {
                            if let Some(err_msg) = error.as_str() {
                                println!("  - {}", err_msg);
                            }
                        }
                    }
                }
            }
        },
    }
    
    Ok(())
}

fn get_nested_value<'a>(obj: &'a Value, key_path: &str) -> Option<&'a Value> {
    let parts: Vec<&str> = key_path.split('.').collect();
    let mut current = obj;
    
    for part in parts {
        current = current.get(part)?;
    }
    
    Some(current)
}

fn format_config_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(arr) => format!("[{}]", arr.len()),
        Value::Object(obj) => format!("{{{}}} keys", obj.len()),
        Value::Null => "null".to_string(),
    }
}

fn parse_config_value(value_str: &str) -> Value {
    // Try to parse as different types
    if let Ok(b) = value_str.parse::<bool>() {
        return json!(b);
    }
    
    if let Ok(i) = value_str.parse::<i64>() {
        return json!(i);
    }
    
    if let Ok(f) = value_str.parse::<f64>() {
        return json!(f);
    }
    
    // Default to string
    json!(value_str)
}

fn print_config_section(name: &str, section: Option<&Value>) {
    if let Some(obj) = section.and_then(|v| v.as_object()) {
        println!("\n[{}]", name);
        for (key, value) in obj {
            println!("  {} = {}", key, format_config_value(value));
        }
    }
}

fn is_restart_required(key: &str) -> bool {
    // Configuration keys that require server restart
    let restart_keys = [
        "server.port",
        "server.tls_cert_path",
        "server.tls_key_path",
        "storage.data_dir",
        "storage.rocksdb_options",
    ];
    
    restart_keys.iter().any(|&restart_key| key.starts_with(restart_key))
}