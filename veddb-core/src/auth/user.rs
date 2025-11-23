
//! User management with bcrypt password hashing and RocksDB storage

use anyhow::{anyhow, Result};
use bcrypt::{hash, verify};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[cfg(feature = "rocksdb-storage")]
use rocksdb::{ColumnFamily, DB};

/// User role defining access permissions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    /// Full administrative access
    Admin,
    /// Read and write operations
    ReadWrite,
    /// Read-only access
    ReadOnly,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::ReadWrite => "read_write",
            Role::ReadOnly => "read_only",
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "admin" => Ok(Role::Admin),
            "read_write" => Ok(Role::ReadWrite),
            "read_only" => Ok(Role::ReadOnly),
            _ => Err(anyhow!("Invalid role: {}", s)),
        }
    }
}

/// User account information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password_hash: String,
    pub role: Role,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub enabled: bool,
    pub metadata: UserMetadata,
}

/// Additional user metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMetadata {
    pub full_name: Option<String>,
    pub email: Option<String>,
    pub description: Option<String>,
    pub created_by: Option<String>,
}

impl Default for UserMetadata {
    fn default() -> Self {
        Self {
            full_name: None,
            email: None,
            description: None,
            created_by: None,
        }
    }
}

/// User management system with RocksDB storage
#[cfg(feature = "rocksdb-storage")]
pub struct UserManager {
    db: DB,
}

/// Mock user manager for when RocksDB is not available
#[cfg(not(feature = "rocksdb-storage"))]
pub struct UserManager {
    users: std::collections::HashMap<String, User>,
}

#[cfg(feature = "rocksdb-storage")]
impl UserManager {
    /// Create a new user manager with RocksDB storage
    pub fn new(storage_path: &str) -> Result<Self> {
        let path = Path::new(storage_path).join("users");
        
        let mut opts = rocksdb::Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        
        let cf_descriptors = vec![
            rocksdb::ColumnFamilyDescriptor::new("users", rocksdb::Options::default()),
            rocksdb::ColumnFamilyDescriptor::new("sessions", rocksdb::Options::default()),
        ];
        
        let db = DB::open_cf_descriptors(&opts, path, cf_descriptors)?;
        
        Ok(Self { db })
    }

    /// Create the default admin user if no users exist
    pub async fn create_default_admin(&mut self) -> Result<()> {
        // Check if any users exist
        let users_cf = self.get_users_cf()?;
        let mut iter = self.db.iterator_cf(users_cf, rocksdb::IteratorMode::Start);
        
        if iter.next().is_none() {
            // No users exist, create default admin
            let admin_user = User {
                username: "admin".to_string(),
                password_hash: self.hash_password("admin123")?,
                role: Role::Admin,
                created_at: Utc::now(),
                last_login: None,
                enabled: true,
                metadata: UserMetadata {
                    full_name: Some("Default Administrator".to_string()),
                    description: Some("Default admin user created on first startup".to_string()),
                    created_by: Some("system".to_string()),
                    ..Default::default()
                },
            };
            
            self.create_user(admin_user).await?;
            log::info!("Created default admin user with username 'admin' and password 'admin123'");
            log::warn!("Please change the default admin password immediately!");
        }
        
        Ok(())
    }

    /// Create a new user
    pub async fn create_user(&mut self, user: User) -> Result<()> {
        let users_cf = self.get_users_cf()?;
        
        // Check if user already exists
        if self.db.get_cf(users_cf, &user.username)?.is_some() {
            return Err(anyhow!("User '{}' already exists", user.username));
        }
        
        // Serialize and store user
        let user_data = serde_json::to_vec(&user)?;
        self.db.put_cf(users_cf, &user.username, user_data)?;
        
        log::info!("Created user '{}' with role '{}'", user.username, user.role.as_str());
        Ok(())
    }

    /// Get user by username
    pub async fn get_user(&self, username: &str) -> Result<Option<User>> {
        let users_cf = self.get_users_cf()?;
        
        match self.db.get_cf(users_cf, username)? {
            Some(data) => {
                let user: User = serde_json::from_slice(data.as_ref())?;
                Ok(Some(user))
            }
            None => Ok(None),
        }
    }

    /// Update user information
    pub async fn update_user(&mut self, user: User) -> Result<()> {
        let users_cf = self.get_users_cf()?;
        
        // Check if user exists
        if self.db.get_cf(users_cf, &user.username)?.is_none() {
            return Err(anyhow!("User '{}' does not exist", user.username));
        }
        
        // Serialize and store updated user
        let user_data = serde_json::to_vec(&user)?;
        self.db.put_cf(users_cf, &user.username, user_data)?;
        
        log::info!("Updated user '{}'", user.username);
        Ok(())
    }

    /// Delete user
    pub async fn delete_user(&mut self, username: &str) -> Result<()> {
        let users_cf = self.get_users_cf()?;
        
        // Check if user exists
        if self.db.get_cf(users_cf, username)?.is_none() {
            return Err(anyhow!("User '{}' does not exist", username));
        }
        
        // Don't allow deleting the last admin user
        if let Some(user) = self.get_user(username).await? {
            if user.role == Role::Admin {
                let admin_count = self.count_users_by_role(Role::Admin).await?;
                if admin_count <= 1 {
                    return Err(anyhow!("Cannot delete the last admin user"));
                }
            }
        }
        
        self.db.delete_cf(users_cf, username)?;
        log::info!("Deleted user '{}'", username);
        Ok(())
    }

    /// List all users
    pub async fn list_users(&self) -> Result<Vec<User>> {
        let users_cf = self.get_users_cf()?;
        let mut users = Vec::new();
        
        let iter = self.db.iterator_cf(users_cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (_, value) = item?;
            let user: User = serde_json::from_slice(value.as_ref())?;
            users.push(user);
        }
        
        Ok(users)
    }

    /// Verify user password and return user if valid
    pub async fn verify_password(&mut self, username: &str, password: &str) -> Result<User> {
        let mut user = self
            .get_user(username)
            .await?
            .ok_or_else(|| anyhow!("User '{}' not found", username))?;
        
        if !user.enabled {
            return Err(anyhow!("User '{}' is disabled", username));
        }
        
        if !verify(password, &user.password_hash)? {
            return Err(anyhow!("Invalid password for user '{}'", username));
        }
        
        // Update last login time
        user.last_login = Some(Utc::now());
        self.update_user(user.clone()).await?;
        
        Ok(user)
    }

    /// Change user password
    pub async fn change_password(&mut self, username: &str, new_password: &str) -> Result<()> {
        let mut user = self
            .get_user(username)
            .await?
            .ok_or_else(|| anyhow!("User '{}' not found", username))?;
        
        user.password_hash = self.hash_password(new_password)?;
        self.update_user(user).await?;
        
        log::info!("Changed password for user '{}'", username);
        Ok(())
    }

    /// Enable or disable user
    pub async fn set_user_enabled(&mut self, username: &str, enabled: bool) -> Result<()> {
        let mut user = self
            .get_user(username)
            .await?
            .ok_or_else(|| anyhow!("User '{}' not found", username))?;
        
        user.enabled = enabled;
        self.update_user(user).await?;
        
        log::info!("Set user '{}' enabled: {}", username, enabled);
        Ok(())
    }

    /// Count users by role
    pub async fn count_users_by_role(&self, role: Role) -> Result<usize> {
        let users = self.list_users().await?;
        Ok(users.iter().filter(|u| u.role == role).count())
    }

    /// Hash password using bcrypt with cost factor 12
    fn hash_password(&self, password: &str) -> Result<String> {
        const BCRYPT_COST: u32 = 12;
        hash(password, BCRYPT_COST).map_err(|e| anyhow!("Failed to hash password: {}", e))
    }

    /// Get users column family
    fn get_users_cf(&self) -> Result<&ColumnFamily> {
        self.db
            .cf_handle("users")
            .ok_or_else(|| anyhow!("Users column family not found"))
    }

    /// Get sessions column family
    fn get_sessions_cf(&self) -> Result<&ColumnFamily> {
        self.db
            .cf_handle("sessions")
            .ok_or_else(|| anyhow!("Sessions column family not found"))
    }
}

#[cfg(not(feature = "rocksdb-storage"))]
impl UserManager {
    /// Create a new user manager with in-memory storage (mock implementation)
    pub fn new(_storage_path: &str) -> Result<Self> {
        Ok(Self {
            users: std::collections::HashMap::new(),
        })
    }

    /// Create the default admin user if no users exist
    pub async fn create_default_admin(&mut self) -> Result<()> {
        if self.users.is_empty() {
            let admin_user = User {
                username: "admin".to_string(),
                password_hash: self.hash_password("admin123")?,
                role: Role::Admin,
                created_at: Utc::now(),
                last_login: None,
                enabled: true,
                metadata: UserMetadata {
                    full_name: Some("Default Administrator".to_string()),
                    description: Some("Default admin user created on first startup".to_string()),
                    created_by: Some("system".to_string()),
                    ..Default::default()
                },
            };
            
            self.create_user(admin_user).await?;
            log::info!("Created default admin user with username 'admin' and password 'admin123'");
            log::warn!("Please change the default admin password immediately!");
        }
        
        Ok(())
    }

    /// Create a new user
    pub async fn create_user(&mut self, user: User) -> Result<()> {
        if self.users.contains_key(&user.username) {
            return Err(anyhow!("User '{}' already exists", user.username));
        }
        
        self.users.insert(user.username.clone(), user.clone());
        log::info!("Created user '{}' with role '{}'", user.username, user.role.as_str());
        Ok(())
    }

    /// Get user by username
    pub async fn get_user(&self, username: &str) -> Result<Option<User>> {
        Ok(self.users.get(username).cloned())
    }

    /// Update user information
    pub async fn update_user(&mut self, user: User) -> Result<()> {
        if !self.users.contains_key(&user.username) {
            return Err(anyhow!("User '{}' does not exist", user.username));
        }
        
        self.users.insert(user.username.clone(), user.clone());
        log::info!("Updated user '{}'", user.username);
        Ok(())
    }

    /// Delete user
    pub async fn delete_user(&mut self, username: &str) -> Result<()> {
        if !self.users.contains_key(username) {
            return Err(anyhow!("User '{}' does not exist", username));
        }
        
        // Don't allow deleting the last admin user
        if let Some(user) = self.users.get(username) {
            if user.role == Role::Admin {
                let admin_count = self.count_users_by_role(Role::Admin).await?;
                if admin_count <= 1 {
                    return Err(anyhow!("Cannot delete the last admin user"));
                }
            }
        }
        
        self.users.remove(username);
        log::info!("Deleted user '{}'", username);
        Ok(())
    }

    /// List all users
    pub async fn list_users(&self) -> Result<Vec<User>> {
        Ok(self.users.values().cloned().collect())
    }

    /// Verify user password and return user if valid
    pub async fn verify_password(&mut self, username: &str, password: &str) -> Result<User> {
        let mut user = self
            .get_user(username)
            .await?
            .ok_or_else(|| anyhow!("User '{}' not found", username))?;
        
        if !user.enabled {
            return Err(anyhow!("User '{}' is disabled", username));
        }
        
        if !verify(password, &user.password_hash)? {
            return Err(anyhow!("Invalid password for user '{}'", username));
        }
        
        // Update last login time
        user.last_login = Some(Utc::now());
        self.update_user(user.clone()).await?;
        
        Ok(user)
    }

    /// Change user password
    pub async fn change_password(&mut self, username: &str, new_password: &str) -> Result<()> {
        let mut user = self
            .get_user(username)
            .await?
            .ok_or_else(|| anyhow!("User '{}' not found", username))?;
        
        user.password_hash = self.hash_password(new_password)?;
        self.update_user(user).await?;
        
        log::info!("Changed password for user '{}'", username);
        Ok(())
    }

    /// Enable or disable user
    pub async fn set_user_enabled(&mut self, username: &str, enabled: bool) -> Result<()> {
        let mut user = self
            .get_user(username)
            .await?
            .ok_or_else(|| anyhow!("User '{}' not found", username))?;
        
        user.enabled = enabled;
        self.update_user(user).await?;
        
        log::info!("Set user '{}' enabled: {}", username, enabled);
        Ok(())
    }

    /// Count users by role
    pub async fn count_users_by_role(&self, role: Role) -> Result<usize> {
        Ok(self.users.values().filter(|u| u.role == role).count())
    }

    /// Hash password using bcrypt with cost factor 12
    fn hash_password(&self, password: &str) -> Result<String> {
        const BCRYPT_COST: u32 = 12;
        hash(password, BCRYPT_COST).map_err(|e| anyhow!("Failed to hash password: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_user_manager() -> (UserManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let manager = UserManager::new(temp_dir.path().to_str().unwrap()).unwrap();
        (manager, temp_dir)
    }

    #[tokio::test]
    async fn test_create_default_admin() {
        let (mut manager, _temp_dir) = create_test_user_manager().await;
        
        manager.create_default_admin().await.unwrap();
        
        let admin = manager.get_user("admin").await.unwrap().unwrap();
        assert_eq!(admin.username, "admin");
        assert_eq!(admin.role, Role::Admin);
        assert!(admin.enabled);
    }

    #[tokio::test]
    async fn test_user_crud() {
        let (mut manager, _temp_dir) = create_test_user_manager().await;
        
        let user = User {
            username: "testuser".to_string(),
            password_hash: manager.hash_password("password123").unwrap(),
            role: Role::ReadWrite,
            created_at: Utc::now(),
            last_login: None,
            enabled: true,
            metadata: UserMetadata::default(),
        };
        
        // Create user
        manager.create_user(user.clone()).await.unwrap();
        
        // Get user
        let retrieved = manager.get_user("testuser").await.unwrap().unwrap();
        assert_eq!(retrieved.username, "testuser");
        assert_eq!(retrieved.role, Role::ReadWrite);
        
        // Update user
        let mut updated = retrieved.clone();
        updated.role = Role::ReadOnly;
        manager.update_user(updated).await.unwrap();
        
        let retrieved = manager.get_user("testuser").await.unwrap().unwrap();
        assert_eq!(retrieved.role, Role::ReadOnly);
        
        // Delete user
        manager.delete_user("testuser").await.unwrap();
        assert!(manager.get_user("testuser").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_password_verification() {
        let (mut manager, _temp_dir) = create_test_user_manager().await;
        
        let user = User {
            username: "testuser".to_string(),
            password_hash: manager.hash_password("password123").unwrap(),
            role: Role::ReadWrite,
            created_at: Utc::now(),
            last_login: None,
            enabled: true,
            metadata: UserMetadata::default(),
        };
        
        manager.create_user(user).await.unwrap();
        
        // Valid password
        let verified = manager.verify_password("testuser", "password123").await.unwrap();
        assert_eq!(verified.username, "testuser");
        assert!(verified.last_login.is_some());
        
        // Invalid password
        assert!(manager.verify_password("testuser", "wrongpassword").await.is_err());
        
        // Disabled user
        manager.set_user_enabled("testuser", false).await.unwrap();
        assert!(manager.verify_password("testuser", "password123").await.is_err());
    }
}