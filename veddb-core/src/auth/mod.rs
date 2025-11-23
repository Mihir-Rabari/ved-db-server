//! Authentication and Authorization module for VedDB
//!
//! This module provides:
//! - User management with bcrypt password hashing
//! - JWT token generation and verification
//! - Role-based access control (RBAC)
//! - Audit logging for security events

pub mod audit;
pub mod jwt;
pub mod rbac;
pub mod user;

pub use audit::*;
pub use jwt::*;
pub use rbac::*;
pub use user::*;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Authentication system managing users, tokens, and permissions
pub struct AuthSystem {
    user_manager: UserManager,
    jwt_service: JwtService,
    rbac: RbacSystem,
    audit_logger: AuditLogger,
}

impl AuthSystem {
    /// Create a new authentication system
    pub fn new(
        storage_path: &str,
        jwt_secret: &[u8],
        session_timeout_hours: u64,
    ) -> Result<Self> {
        let user_manager = UserManager::new(storage_path)?;
        let jwt_service = JwtService::new(jwt_secret, session_timeout_hours)?;
        let rbac = RbacSystem::new();
        let audit_logger = AuditLogger::new(storage_path)?;

        Ok(Self {
            user_manager,
            jwt_service,
            rbac,
            audit_logger,
        })
    }

    /// Initialize the system with default admin user
    pub async fn initialize(&mut self) -> Result<()> {
        self.user_manager.create_default_admin().await?;
        Ok(())
    }

    /// Authenticate user with username and password
    pub async fn authenticate(
        &mut self,
        username: &str,
        password: &str,
        client_ip: Option<&str>,
    ) -> Result<String> {
        match self.user_manager.verify_password(username, password).await {
            Ok(user) => {
                let token = self.jwt_service.generate_token(&user)?;
                self.audit_logger
                    .log_auth_success(username, client_ip)
                    .await?;
                Ok(token)
            }
            Err(e) => {
                self.audit_logger
                    .log_auth_failure(username, client_ip, &e.to_string())
                    .await?;
                Err(e)
            }
        }
    }

    /// Verify JWT token and return user claims
    pub fn verify_token(&self, token: &str) -> Result<UserClaims> {
        self.jwt_service.verify_token(token)
    }

    /// Check if user has permission for operation
    pub async fn authorize(
        &mut self,
        token: &str,
        operation: Operation,
        resource: Option<&str>,
    ) -> Result<()> {
        let claims = self.verify_token(token)?;
        
        match self.rbac.check_permission(claims.role, operation) {
            Ok(()) => Ok(()),
            Err(e) => {
                self.audit_logger
                    .log_authorization_failure(&claims.username, operation, resource)
                    .await?;
                Err(e)
            }
        }
    }

    /// Get user manager for admin operations
    pub fn user_manager(&mut self) -> &mut UserManager {
        &mut self.user_manager
    }

    /// Get audit logger
    pub fn audit_logger(&mut self) -> &mut AuditLogger {
        &mut self.audit_logger
    }
}

/// User claims extracted from JWT token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserClaims {
    pub username: String,
    pub role: Role,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub token_id: String,
}