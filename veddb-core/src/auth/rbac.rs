//! Role-Based Access Control (RBAC) system

use crate::auth::Role;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Database operations that can be authorized
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Operation {
    // Data operations
    Read,
    Write,
    Delete,
    
    // Collection operations
    CreateCollection,
    DropCollection,
    ListCollections,
    
    // Index operations
    CreateIndex,
    DropIndex,
    ListIndexes,
    
    // User management operations
    CreateUser,
    UpdateUser,
    DeleteUser,
    ListUsers,
    ChangePassword,
    
    // Administrative operations
    Backup,
    Restore,
    ViewMetrics,
    ViewLogs,
    ConfigureServer,
    
    // Pub/Sub operations
    Subscribe,
    Publish,
    
    // Cache operations
    FlushCache,
    ViewCacheStats,
    
    // Replication operations
    ConfigureReplication,
    ViewReplicationStatus,
}

impl Operation {
    /// Get string representation of operation
    pub fn as_str(&self) -> &'static str {
        match self {
            Operation::Read => "read",
            Operation::Write => "write",
            Operation::Delete => "delete",
            Operation::CreateCollection => "create_collection",
            Operation::DropCollection => "drop_collection",
            Operation::ListCollections => "list_collections",
            Operation::CreateIndex => "create_index",
            Operation::DropIndex => "drop_index",
            Operation::ListIndexes => "list_indexes",
            Operation::CreateUser => "create_user",
            Operation::UpdateUser => "update_user",
            Operation::DeleteUser => "delete_user",
            Operation::ListUsers => "list_users",
            Operation::ChangePassword => "change_password",
            Operation::Backup => "backup",
            Operation::Restore => "restore",
            Operation::ViewMetrics => "view_metrics",
            Operation::ViewLogs => "view_logs",
            Operation::ConfigureServer => "configure_server",
            Operation::Subscribe => "subscribe",
            Operation::Publish => "publish",
            Operation::FlushCache => "flush_cache",
            Operation::ViewCacheStats => "view_cache_stats",
            Operation::ConfigureReplication => "configure_replication",
            Operation::ViewReplicationStatus => "view_replication_status",
        }
    }

    /// Parse operation from string
    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "read" => Ok(Operation::Read),
            "write" => Ok(Operation::Write),
            "delete" => Ok(Operation::Delete),
            "create_collection" => Ok(Operation::CreateCollection),
            "drop_collection" => Ok(Operation::DropCollection),
            "list_collections" => Ok(Operation::ListCollections),
            "create_index" => Ok(Operation::CreateIndex),
            "drop_index" => Ok(Operation::DropIndex),
            "list_indexes" => Ok(Operation::ListIndexes),
            "create_user" => Ok(Operation::CreateUser),
            "update_user" => Ok(Operation::UpdateUser),
            "delete_user" => Ok(Operation::DeleteUser),
            "list_users" => Ok(Operation::ListUsers),
            "change_password" => Ok(Operation::ChangePassword),
            "backup" => Ok(Operation::Backup),
            "restore" => Ok(Operation::Restore),
            "view_metrics" => Ok(Operation::ViewMetrics),
            "view_logs" => Ok(Operation::ViewLogs),
            "configure_server" => Ok(Operation::ConfigureServer),
            "subscribe" => Ok(Operation::Subscribe),
            "publish" => Ok(Operation::Publish),
            "flush_cache" => Ok(Operation::FlushCache),
            "view_cache_stats" => Ok(Operation::ViewCacheStats),
            "configure_replication" => Ok(Operation::ConfigureReplication),
            "view_replication_status" => Ok(Operation::ViewReplicationStatus),
            _ => Err(anyhow!("Unknown operation: {}", s)),
        }
    }
}

/// Resource identifier for resource-level access control
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Resource {
    pub resource_type: ResourceType,
    pub resource_id: String,
}

/// Types of resources that can be protected
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    Collection,
    Index,
    User,
    Database,
    Channel, // For pub/sub
}

impl Resource {
    /// Create a collection resource
    pub fn collection(name: &str) -> Self {
        Self {
            resource_type: ResourceType::Collection,
            resource_id: name.to_string(),
        }
    }

    /// Create an index resource
    pub fn index(collection: &str, index_name: &str) -> Self {
        Self {
            resource_type: ResourceType::Index,
            resource_id: format!("{}:{}", collection, index_name),
        }
    }

    /// Create a user resource
    pub fn user(username: &str) -> Self {
        Self {
            resource_type: ResourceType::User,
            resource_id: username.to_string(),
        }
    }

    /// Create a database resource
    pub fn database(name: &str) -> Self {
        Self {
            resource_type: ResourceType::Database,
            resource_id: name.to_string(),
        }
    }

    /// Create a channel resource
    pub fn channel(name: &str) -> Self {
        Self {
            resource_type: ResourceType::Channel,
            resource_id: name.to_string(),
        }
    }
}

/// Permission definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    pub operation: Operation,
    pub resource: Option<Resource>,
    pub conditions: Vec<String>, // Future: conditions like time-based access
}

/// Access Control List entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclEntry {
    pub principal: String, // username or role
    pub permissions: Vec<Permission>,
    pub deny: bool, // true for deny rules, false for allow rules
}

/// Role-Based Access Control system
pub struct RbacSystem {
    role_permissions: HashMap<Role, HashSet<Operation>>,
    resource_acls: HashMap<Resource, Vec<AclEntry>>,
}

impl RbacSystem {
    /// Create a new RBAC system with default permissions
    pub fn new() -> Self {
        let mut system = Self {
            role_permissions: HashMap::new(),
            resource_acls: HashMap::new(),
        };
        
        system.initialize_default_permissions();
        system
    }

    /// Initialize default role permissions
    fn initialize_default_permissions(&mut self) {
        // Admin role - full access
        let admin_permissions = vec![
            Operation::Read,
            Operation::Write,
            Operation::Delete,
            Operation::CreateCollection,
            Operation::DropCollection,
            Operation::ListCollections,
            Operation::CreateIndex,
            Operation::DropIndex,
            Operation::ListIndexes,
            Operation::CreateUser,
            Operation::UpdateUser,
            Operation::DeleteUser,
            Operation::ListUsers,
            Operation::ChangePassword,
            Operation::Backup,
            Operation::Restore,
            Operation::ViewMetrics,
            Operation::ViewLogs,
            Operation::ConfigureServer,
            Operation::Subscribe,
            Operation::Publish,
            Operation::FlushCache,
            Operation::ViewCacheStats,
            Operation::ConfigureReplication,
            Operation::ViewReplicationStatus,
        ].into_iter().collect();
        
        // ReadWrite role - data operations and basic management
        let readwrite_permissions = vec![
            Operation::Read,
            Operation::Write,
            Operation::Delete,
            Operation::ListCollections,
            Operation::ListIndexes,
            Operation::ChangePassword, // Users can change their own password
            Operation::Subscribe,
            Operation::Publish,
            Operation::ViewCacheStats,
            Operation::ViewReplicationStatus,
        ].into_iter().collect();
        
        // ReadOnly role - read operations only
        let readonly_permissions = vec![
            Operation::Read,
            Operation::ListCollections,
            Operation::ListIndexes,
            Operation::ChangePassword, // Users can change their own password
            Operation::Subscribe,
            Operation::ViewCacheStats,
            Operation::ViewReplicationStatus,
        ].into_iter().collect();
        
        self.role_permissions.insert(Role::Admin, admin_permissions);
        self.role_permissions.insert(Role::ReadWrite, readwrite_permissions);
        self.role_permissions.insert(Role::ReadOnly, readonly_permissions);
    }

    /// Check if role has permission for operation
    pub fn check_permission(&self, role: Role, operation: Operation) -> Result<()> {
        let permissions = self.role_permissions.get(&role)
            .ok_or_else(|| anyhow!("Unknown role: {:?}", role))?;
        
        if permissions.contains(&operation) {
            Ok(())
        } else {
            Err(anyhow!(
                "Role '{}' does not have permission for operation '{}'",
                role.as_str(),
                operation.as_str()
            ))
        }
    }

    /// Check if user has permission for operation on specific resource
    pub fn check_resource_permission(
        &self,
        username: &str,
        role: Role,
        operation: Operation,
        resource: &Resource,
    ) -> Result<()> {
        // First check role-based permissions
        self.check_permission(role, operation)?;
        
        // Then check resource-specific ACLs
        if let Some(acl_entries) = self.resource_acls.get(resource) {
            let mut allowed = false;
            let mut denied = false;
            let mut has_matching_entry = false;
            
            // Process ACL entries in order
            for entry in acl_entries {
                let matches = entry.principal == username || entry.principal == role.as_str();
                
                if matches {
                    has_matching_entry = true;
                    for permission in &entry.permissions {
                        if permission.operation == operation {
                            if entry.deny {
                                denied = true;
                            } else {
                                allowed = true;
                            }
                        }
                    }
                }
            }
            
            // Deny rules take precedence
            if denied {
                return Err(anyhow!(
                    "Access denied by ACL for user '{}' on resource '{:?}'",
                    username,
                    resource
                ));
            }
            
            // If there are matching ACL entries but no explicit allow, deny access
            // If no matching entries, fall back to role-based permissions (already checked above)
            if has_matching_entry && !allowed {
                return Err(anyhow!(
                    "No explicit permission in ACL for user '{}' on resource '{:?}'",
                    username,
                    resource
                ));
            }
        }
        
        Ok(())
    }

    /// Add permission to role
    pub fn add_role_permission(&mut self, role: Role, operation: Operation) {
        self.role_permissions
            .entry(role)
            .or_insert_with(HashSet::new)
            .insert(operation);
    }

    /// Remove permission from role
    pub fn remove_role_permission(&mut self, role: Role, operation: Operation) {
        if let Some(permissions) = self.role_permissions.get_mut(&role) {
            permissions.remove(&operation);
        }
    }

    /// Get all permissions for role
    pub fn get_role_permissions(&self, role: Role) -> Option<&HashSet<Operation>> {
        self.role_permissions.get(&role)
    }

    /// Add ACL entry for resource
    pub fn add_acl_entry(&mut self, resource: Resource, entry: AclEntry) {
        self.resource_acls
            .entry(resource)
            .or_insert_with(Vec::new)
            .push(entry);
    }

    /// Remove ACL entries for resource
    pub fn remove_acl_entries(&mut self, resource: &Resource) {
        self.resource_acls.remove(resource);
    }

    /// Get ACL entries for resource
    pub fn get_acl_entries(&self, resource: &Resource) -> Option<&Vec<AclEntry>> {
        self.resource_acls.get(resource)
    }

    /// Check if user can perform operation (convenience method)
    pub fn authorize(
        &self,
        username: &str,
        role: Role,
        operation: Operation,
        resource: Option<&Resource>,
    ) -> Result<()> {
        match resource {
            Some(res) => self.check_resource_permission(username, role, operation, res),
            None => self.check_permission(role, operation),
        }
    }

    /// Get all operations a role can perform
    pub fn get_allowed_operations(&self, role: Role) -> Vec<Operation> {
        self.role_permissions
            .get(&role)
            .map(|perms| perms.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Check if operation requires admin privileges
    pub fn is_admin_operation(&self, operation: Operation) -> bool {
        matches!(
            operation,
            Operation::CreateUser
                | Operation::UpdateUser
                | Operation::DeleteUser
                | Operation::ListUsers
                | Operation::Backup
                | Operation::Restore
                | Operation::ViewLogs
                | Operation::ConfigureServer
                | Operation::FlushCache
                | Operation::ConfigureReplication
        )
    }
}

impl Default for RbacSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_permissions() {
        let rbac = RbacSystem::new();
        
        // Admin should have all permissions
        assert!(rbac.check_permission(Role::Admin, Operation::Read).is_ok());
        assert!(rbac.check_permission(Role::Admin, Operation::CreateUser).is_ok());
        assert!(rbac.check_permission(Role::Admin, Operation::ConfigureServer).is_ok());
        
        // ReadWrite should have data operations
        assert!(rbac.check_permission(Role::ReadWrite, Operation::Read).is_ok());
        assert!(rbac.check_permission(Role::ReadWrite, Operation::Write).is_ok());
        assert!(rbac.check_permission(Role::ReadWrite, Operation::Delete).is_ok());
        
        // ReadWrite should not have admin operations
        assert!(rbac.check_permission(Role::ReadWrite, Operation::CreateUser).is_err());
        assert!(rbac.check_permission(Role::ReadWrite, Operation::ConfigureServer).is_err());
        
        // ReadOnly should only have read operations
        assert!(rbac.check_permission(Role::ReadOnly, Operation::Read).is_ok());
        assert!(rbac.check_permission(Role::ReadOnly, Operation::Write).is_err());
        assert!(rbac.check_permission(Role::ReadOnly, Operation::Delete).is_err());
    }

    #[test]
    fn test_resource_acl() {
        let mut rbac = RbacSystem::new();
        let resource = Resource::collection("test_collection");
        
        // Add ACL entry denying write access to specific user
        let acl_entry = AclEntry {
            principal: "testuser".to_string(),
            permissions: vec![Permission {
                operation: Operation::Write,
                resource: None,
                conditions: vec![],
            }],
            deny: true,
        };
        
        rbac.add_acl_entry(resource.clone(), acl_entry);
        
        // User should be denied write access even with ReadWrite role
        assert!(rbac
            .check_resource_permission("testuser", Role::ReadWrite, Operation::Write, &resource)
            .is_err());
        
        // Other users with ReadWrite role should still have access
        assert!(rbac
            .check_resource_permission("otheruser", Role::ReadWrite, Operation::Write, &resource)
            .is_ok());
    }

    #[test]
    fn test_operation_string_conversion() {
        assert_eq!(Operation::Read.as_str(), "read");
        assert_eq!(Operation::CreateUser.as_str(), "create_user");
        
        assert_eq!(Operation::from_str("read").unwrap(), Operation::Read);
        assert_eq!(Operation::from_str("create_user").unwrap(), Operation::CreateUser);
        assert!(Operation::from_str("invalid_operation").is_err());
    }

    #[test]
    fn test_resource_creation() {
        let collection_resource = Resource::collection("my_collection");
        assert_eq!(collection_resource.resource_type, ResourceType::Collection);
        assert_eq!(collection_resource.resource_id, "my_collection");
        
        let index_resource = Resource::index("my_collection", "my_index");
        assert_eq!(index_resource.resource_type, ResourceType::Index);
        assert_eq!(index_resource.resource_id, "my_collection:my_index");
        
        let user_resource = Resource::user("testuser");
        assert_eq!(user_resource.resource_type, ResourceType::User);
        assert_eq!(user_resource.resource_id, "testuser");
    }

    #[test]
    fn test_admin_operations() {
        let rbac = RbacSystem::new();
        
        assert!(rbac.is_admin_operation(Operation::CreateUser));
        assert!(rbac.is_admin_operation(Operation::ConfigureServer));
        assert!(rbac.is_admin_operation(Operation::Backup));
        
        assert!(!rbac.is_admin_operation(Operation::Read));
        assert!(!rbac.is_admin_operation(Operation::Write));
        assert!(!rbac.is_admin_operation(Operation::Subscribe));
    }
}