//! Key rotation state machine
//!
//! Tracks rotation progress with persistent state to enable:
//! - Crash recovery from checkpoints
//! - Prevention of concurrent rotations
//! - Clear rotation lifecycle management

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Key rotation state machine
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "state", content = "data")]
pub enum KeyRotationState {
    /// No rotation in progress
    Idle,
    
    /// Re-encryption in progress
    ReEncrypting {
        /// Key being rotated
        key_id: String,
        /// When rotation started
        started_at: DateTime<Utc>,
        /// Documents processed so far
        processed: u64,
        /// Total documents to process
        total: u64,
        /// Last checkpoint for resume (collection_name, document_id)
        /// CRITICAL: Tuple ensures deterministic resume across storage reordering
        last_checkpoint: Option<(String, String)>,
    },
    
    /// Rotation completed successfully
    Completed {
        /// Key that was rotated
        key_id: String,
        /// When rotation completed
        completed_at: DateTime<Utc>,
        /// Total documents re-encrypted
        documents_processed: u64,
    },
    
    /// Rotation failed
    Failed {
        /// Key that failed
        key_id: String,
        /// Failure reason
        reason: String,
        /// When failure occurred
        failed_at: DateTime<Utc>,
    },
}

impl KeyRotationState {
    /// Check if state is idle
    pub fn is_idle(&self) -> bool {
        matches!(self, KeyRotationState::Idle)
    }
    
    /// Check if state is completed
    pub fn is_completed(&self) -> bool {
        matches!(self, KeyRotationState::Completed { .. })
    }
    
    /// Check if rotation is in progress
    pub fn is_in_progress(&self) -> bool {
        matches!(self, KeyRotationState::ReEncrypting { .. })
    }
    
    /// Check if state is failed
    pub fn is_failed(&self) -> bool {
        matches!(self, KeyRotationState::Failed { .. })
    }
    
    /// Check if a new rotation can start
    /// Only Idle and Completed states allow new rotations
    pub fn can_start_rotation(&self) -> bool {
        matches!(
            self,
            KeyRotationState::Idle | KeyRotationState::Completed { .. }
        )
    }
    
    /// Get the key ID if rotation is in progress or failed
    pub fn active_key_id(&self) -> Option<&str> {
        match self {
            KeyRotationState::ReEncrypting { key_id, .. } => Some(key_id),
            KeyRotationState::Failed { key_id, .. } => Some(key_id),
            _ => None,
        }
    }
    
    /// Get progress information if in progress
    pub fn progress(&self) -> Option<(u64, u64)> {
        match self {
            KeyRotationState::ReEncrypting { processed, total, .. } => {
                Some((*processed, *total))
            }
            _ => None,
        }
    }
}

impl Default for KeyRotationState {
    fn default() -> Self {
        KeyRotationState::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_state_transitions() {
        let idle = KeyRotationState::Idle;
        assert!(idle.can_start_rotation());
        assert!(idle.is_idle());
        
        let encrypting = KeyRotationState::ReEncrypting {
            key_id: "test".to_string(),
            started_at: Utc::now(),
            processed: 10,
            total: 100,
            last_checkpoint: Some(("col".to_string(), "doc1".to_string())),
        };
        assert!(!encrypting.can_start_rotation());
        assert!(encrypting.is_in_progress());
        assert_eq!(encrypting.progress(), Some((10, 100)));
        
        let completed = KeyRotationState::Completed {
            key_id: "test".to_string(),
            completed_at: Utc::now(),
            documents_processed: 100,
        };
        assert!(completed.can_start_rotation());
        assert!(completed.is_completed());
    }
    
    #[test]
    fn test_serialization() {
        let state = KeyRotationState::ReEncrypting {
            key_id: "master".to_string(),
            started_at: Utc::now(),
            processed: 50,
            total: 200,
            last_checkpoint: Some(("users".to_string(), "abc123".to_string())),
        };
        
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: KeyRotationState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }
}
