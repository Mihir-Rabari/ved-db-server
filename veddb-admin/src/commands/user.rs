use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use crate::client::AdminClient;

#[derive(Subcommand)]
pub enum UserCommands {
    /// List all users
    List,
    /// Create a new user
    Create {
        /// Username
        username: String,
        /// User role (admin, read-write, read-only)
        #[arg(short, long, default_value = "read-only")]
        role: String,
        /// User password (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Delete a user
    Delete {
        /// Username to delete
        username: String,
        /// Force deletion without confirmation
        #[arg(short, long)]
        force: bool,
    },
    /// Update user role
    UpdateRole {
        /// Username
        username: String,
        /// New role (admin, read-write, read-only)
        role: String,
    },
    /// Enable or disable a user
    SetEnabled {
        /// Username
        username: String,
        /// Enable (true) or disable (false) the user
        enabled: bool,
    },
    /// Change user password
    ChangePassword {
        /// Username
        username: String,
        /// New password (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
}

pub async fn execute_user_command(client: &mut AdminClient, command: UserCommands) -> Result<()> {
    match command {
        UserCommands::List => {
            println!("Fetching user list...");
            let response = client.execute_command("user.list", json!({})).await?;
            
            if let Some(users) = response.get("users").and_then(|u| u.as_array()) {
                println!("\n{:<15} {:<12} {:<8}", "USERNAME", "ROLE", "ENABLED");
                println!("{}", "-".repeat(40));
                
                for user in users {
                    let username = user.get("username").and_then(|u| u.as_str()).unwrap_or("N/A");
                    let role = user.get("role").and_then(|r| r.as_str()).unwrap_or("N/A");
                    let enabled = user.get("enabled").and_then(|e| e.as_bool()).unwrap_or(false);
                    
                    println!("{:<15} {:<12} {:<8}", username, role, if enabled { "Yes" } else { "No" });
                }
            }
        },
        
        UserCommands::Create { username, role, password } => {
            let password = match password {
                Some(p) => p,
                None => {
                    print!("Enter password for user '{}': ", username);
                    rpassword::read_password()?
                }
            };
            
            // Validate role
            if !["admin", "read-write", "read-only"].contains(&role.as_str()) {
                return Err(anyhow::anyhow!("Invalid role. Must be one of: admin, read-write, read-only"));
            }
            
            println!("Creating user '{}'...", username);
            let response = client.execute_command("user.create", json!({
                "username": username,
                "role": role,
                "password": password
            })).await?;
            
            if let Some(message) = response.get("message").and_then(|m| m.as_str()) {
                println!("✓ {}", message);
            }
        },
        
        UserCommands::Delete { username, force } => {
            if !force {
                print!("Are you sure you want to delete user '{}'? (y/N): ", username);
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().to_lowercase().starts_with('y') {
                    println!("Operation cancelled.");
                    return Ok(());
                }
            }
            
            println!("Deleting user '{}'...", username);
            let response = client.execute_command("user.delete", json!({
                "username": username
            })).await?;
            
            if let Some(message) = response.get("message").and_then(|m| m.as_str()) {
                println!("✓ {}", message);
            }
        },
        
        UserCommands::UpdateRole { username, role } => {
            // Validate role
            if !["admin", "read-write", "read-only"].contains(&role.as_str()) {
                return Err(anyhow::anyhow!("Invalid role. Must be one of: admin, read-write, read-only"));
            }
            
            println!("Updating role for user '{}'...", username);
            let response = client.execute_command("user.update_role", json!({
                "username": username,
                "role": role
            })).await?;
            
            if let Some(message) = response.get("message").and_then(|m| m.as_str()) {
                println!("✓ {}", message);
            }
        },
        
        UserCommands::SetEnabled { username, enabled } => {
            let action = if enabled { "Enabling" } else { "Disabling" };
            println!("{} user '{}'...", action, username);
            
            let response = client.execute_command("user.set_enabled", json!({
                "username": username,
                "enabled": enabled
            })).await?;
            
            if let Some(message) = response.get("message").and_then(|m| m.as_str()) {
                println!("✓ {}", message);
            }
        },
        
        UserCommands::ChangePassword { username, password } => {
            let password = match password {
                Some(p) => p,
                None => {
                    print!("Enter new password for user '{}': ", username);
                    rpassword::read_password()?
                }
            };
            
            println!("Changing password for user '{}'...", username);
            let response = client.execute_command("user.change_password", json!({
                "username": username,
                "password": password
            })).await?;
            
            if let Some(message) = response.get("message").and_then(|m| m.as_str()) {
                println!("✓ {}", message);
            }
        },
    }
    
    Ok(())
}