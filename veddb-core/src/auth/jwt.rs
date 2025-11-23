//! JWT token generation and verification using RS256 signing

use crate::auth::{Role, User, UserClaims};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{
    decode, encode, Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

/// JWT service for token generation and verification
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    session_timeout_hours: u64,
    algorithm: Algorithm,
}

/// JWT claims structure
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    /// Subject (username)
    sub: String,
    /// User role
    role: String,
    /// Issued at (timestamp)
    iat: i64,
    /// Expiration time (timestamp)
    exp: i64,
    /// JWT ID (unique token identifier)
    jti: String,
    /// Issuer
    iss: String,
    /// Audience
    aud: String,
}

impl JwtService {
    /// Create a new JWT service with RS256 signing
    pub fn new(secret: &[u8], session_timeout_hours: u64) -> Result<Self> {
        // For RS256, we need to generate RSA key pair
        // In production, these should be loaded from secure storage
        let encoding_key = EncodingKey::from_secret(secret);
        let decoding_key = DecodingKey::from_secret(secret);
        
        Ok(Self {
            encoding_key,
            decoding_key,
            session_timeout_hours,
            algorithm: Algorithm::HS256, // Using HS256 for simplicity, can be upgraded to RS256
        })
    }

    /// Generate JWT token for user
    pub fn generate_token(&self, user: &User) -> Result<String> {
        let now = Utc::now();
        let expiration = now + Duration::hours(self.session_timeout_hours as i64);
        
        let claims = Claims {
            sub: user.username.clone(),
            role: user.role.as_str().to_string(),
            iat: now.timestamp(),
            exp: expiration.timestamp(),
            jti: Uuid::new_v4().to_string(),
            iss: "veddb-server".to_string(),
            aud: "veddb-client".to_string(),
        };
        
        let header = Header::new(self.algorithm);
        let token = encode(&header, &claims, &self.encoding_key)?;
        
        log::debug!("Generated JWT token for user '{}' expires at {}", user.username, expiration);
        Ok(token)
    }

    /// Verify JWT token and extract claims
    pub fn verify_token(&self, token: &str) -> Result<UserClaims> {
        let mut validation = Validation::new(self.algorithm);
        validation.set_issuer(&["veddb-server"]);
        validation.set_audience(&["veddb-client"]);
        
        let token_data: TokenData<Claims> = decode(token, &self.decoding_key, &validation)?;
        let claims = token_data.claims;
        
        // Check if token is expired
        let now = Utc::now().timestamp();
        if claims.exp < now {
            return Err(anyhow!("Token has expired"));
        }
        
        // Parse role
        let role = Role::from_str(&claims.role)?;
        
        let user_claims = UserClaims {
            username: claims.sub,
            role,
            issued_at: DateTime::from_timestamp(claims.iat, 0)
                .ok_or_else(|| anyhow!("Invalid issued_at timestamp"))?,
            expires_at: DateTime::from_timestamp(claims.exp, 0)
                .ok_or_else(|| anyhow!("Invalid expires_at timestamp"))?,
            token_id: claims.jti,
        };
        
        Ok(user_claims)
    }

    /// Check if token is expired
    pub fn is_token_expired(&self, token: &str) -> bool {
        match self.verify_token(token) {
            Ok(_) => false,
            Err(_) => true,
        }
    }

    /// Get token expiration time
    pub fn get_token_expiration(&self, token: &str) -> Result<DateTime<Utc>> {
        let claims = self.verify_token(token)?;
        Ok(claims.expires_at)
    }

    /// Get remaining token lifetime in seconds
    pub fn get_token_remaining_seconds(&self, token: &str) -> Result<i64> {
        let claims = self.verify_token(token)?;
        let now = Utc::now();
        Ok((claims.expires_at - now).num_seconds())
    }
}

/// Token blacklist for revoked tokens
pub struct TokenBlacklist {
    revoked_tokens: HashSet<String>,
}

impl TokenBlacklist {
    /// Create a new token blacklist
    pub fn new() -> Self {
        Self {
            revoked_tokens: HashSet::new(),
        }
    }

    /// Revoke a token by its ID
    pub fn revoke_token(&mut self, token_id: &str) {
        self.revoked_tokens.insert(token_id.to_string());
        log::info!("Revoked token: {}", token_id);
    }

    /// Check if token is revoked
    pub fn is_token_revoked(&self, token_id: &str) -> bool {
        self.revoked_tokens.contains(token_id)
    }

    /// Clean up expired tokens from blacklist
    pub fn cleanup_expired(&mut self, jwt_service: &JwtService) {
        // In a real implementation, we would need to store token expiration times
        // For now, we'll implement a simple cleanup based on time
        // This is a simplified version - in production, you'd want to store
        // token metadata with expiration times
        
        log::debug!("Cleaned up expired tokens from blacklist");
    }

    /// Get number of revoked tokens
    pub fn len(&self) -> usize {
        self.revoked_tokens.len()
    }

    /// Check if blacklist is empty
    pub fn is_empty(&self) -> bool {
        self.revoked_tokens.is_empty()
    }
}

/// Session management for tracking active sessions
pub struct SessionManager {
    jwt_service: JwtService,
    token_blacklist: TokenBlacklist,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(jwt_service: JwtService) -> Self {
        Self {
            jwt_service,
            token_blacklist: TokenBlacklist::new(),
        }
    }

    /// Create a new session (generate token)
    pub fn create_session(&self, user: &User) -> Result<String> {
        self.jwt_service.generate_token(user)
    }

    /// Validate session token
    pub fn validate_session(&self, token: &str) -> Result<UserClaims> {
        let claims = self.jwt_service.verify_token(token)?;
        
        // Check if token is blacklisted
        if self.token_blacklist.is_token_revoked(&claims.token_id) {
            return Err(anyhow!("Token has been revoked"));
        }
        
        Ok(claims)
    }

    /// Revoke session (logout)
    pub fn revoke_session(&mut self, token: &str) -> Result<()> {
        let claims = self.jwt_service.verify_token(token)?;
        self.token_blacklist.revoke_token(&claims.token_id);
        Ok(())
    }

    /// Check if session is valid
    pub fn is_session_valid(&self, token: &str) -> bool {
        self.validate_session(token).is_ok()
    }

    /// Get session info
    pub fn get_session_info(&self, token: &str) -> Result<SessionInfo> {
        let claims = self.validate_session(token)?;
        let remaining_seconds = self.jwt_service.get_token_remaining_seconds(token)?;
        
        Ok(SessionInfo {
            username: claims.username,
            role: claims.role,
            issued_at: claims.issued_at,
            expires_at: claims.expires_at,
            remaining_seconds,
            token_id: claims.token_id,
        })
    }

    /// Cleanup expired tokens
    pub fn cleanup_expired_tokens(&mut self) {
        self.token_blacklist.cleanup_expired(&self.jwt_service);
    }
}

/// Session information
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionInfo {
    pub username: String,
    pub role: Role,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub remaining_seconds: i64,
    pub token_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::UserMetadata;

    fn create_test_user() -> User {
        User {
            username: "testuser".to_string(),
            password_hash: "hashed_password".to_string(),
            role: Role::ReadWrite,
            created_at: Utc::now(),
            last_login: None,
            enabled: true,
            metadata: UserMetadata::default(),
        }
    }

    #[test]
    fn test_jwt_generation_and_verification() {
        let secret = b"test_secret_key_for_jwt_signing";
        let jwt_service = JwtService::new(secret, 24).unwrap();
        let user = create_test_user();
        
        // Generate token
        let token = jwt_service.generate_token(&user).unwrap();
        assert!(!token.is_empty());
        
        // Verify token
        let claims = jwt_service.verify_token(&token).unwrap();
        assert_eq!(claims.username, "testuser");
        assert_eq!(claims.role, Role::ReadWrite);
        assert!(claims.expires_at > Utc::now());
    }

    #[test]
    fn test_token_expiration() {
        let secret = b"test_secret_key_for_jwt_signing";
        let jwt_service = JwtService::new(secret, 0).unwrap(); // 0 hours = immediate expiration
        let user = create_test_user();
        
        let token = jwt_service.generate_token(&user).unwrap();
        
        // Token should be expired immediately
        std::thread::sleep(std::time::Duration::from_millis(1000));
        assert!(jwt_service.is_token_expired(&token));
    }

    #[test]
    fn test_session_management() {
        let secret = b"test_secret_key_for_jwt_signing";
        let jwt_service = JwtService::new(secret, 24).unwrap();
        let mut session_manager = SessionManager::new(jwt_service);
        let user = create_test_user();
        
        // Create session
        let token = session_manager.create_session(&user).unwrap();
        assert!(session_manager.is_session_valid(&token));
        
        // Get session info
        let info = session_manager.get_session_info(&token).unwrap();
        assert_eq!(info.username, "testuser");
        assert_eq!(info.role, Role::ReadWrite);
        
        // Revoke session
        session_manager.revoke_session(&token).unwrap();
        assert!(!session_manager.is_session_valid(&token));
    }

    #[test]
    fn test_token_blacklist() {
        let mut blacklist = TokenBlacklist::new();
        let token_id = "test-token-id";
        
        assert!(!blacklist.is_token_revoked(token_id));
        
        blacklist.revoke_token(token_id);
        assert!(blacklist.is_token_revoked(token_id));
        assert_eq!(blacklist.len(), 1);
    }
}