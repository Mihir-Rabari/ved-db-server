//! Persistence layer for rotation state
//!
//! CRITICAL: State is stored in encryption metadata directory, NOT generic storage path.
//! This ensures state survives storage churn/wipes while remaining crypto-bound.

use super::rotation_state::KeyRotationState;
use anyhow::Result;
use std::fs;
use std::path::Path;

/// State file name in encryption directory
const STATE_FILE: &str = "rotation_state.json";

/// Save rotation state to encryption metadata directory
///
/// CRITICAL: encryption_path must be the encryption metadata directory,
/// NOT the generic storage path. State must survive storage operations.
pub fn save_rotation_state(
    encryption_path: &Path,
    state: &KeyRotationState,
) -> Result<()> {
    // Ensure encryption directory exists
    fs::create_dir_all(encryption_path)?;
    
    let state_path = encryption_path.join(STATE_FILE);
    
    // Serialize state to JSON (pretty for debugging)
    let json = serde_json::to_string_pretty(state)?;
    
    // Atomic write: write to temp file, then rename
    let temp_path = state_path.with_extension("tmp");
    fs::write(&temp_path, json)?;
    fs::rename(&temp_path, &state_path)?;
    
    log::debug!("Saved rotation state: {:?}", state);
    Ok(())
}

/// Load rotation state from encryption metadata directory
///
/// Returns Idle state if no state file exists (clean start).
pub fn load_rotation_state(encryption_path: &Path) -> Result<KeyRotationState> {
    let state_path = encryption_path.join(STATE_FILE);
    
    if !state_path.exists() {
        log::debug!("No rotation state file found, returning Idle");
        return Ok(KeyRotationState::Idle);
    }
    
    let json = fs::read_to_string(&state_path)?;
    let state: KeyRotationState = serde_json::from_str(&json)?;
    
    log::debug!("Loaded rotation state: {:?}", state);
    Ok(state)
}

/// Delete rotation state file (used after successful completion or manual reset)
pub fn clear_rotation_state(encryption_path: &Path) -> Result<()> {
    let state_path = encryption_path.join(STATE_FILE);
    
    if state_path.exists() {
        fs::remove_file(&state_path)?;
        log::info!("Cleared rotation state file");
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::TempDir;
    
    #[test]
    fn test_save_and_load_state() {
        let temp_dir = TempDir::new().unwrap();
        let encryption_path = temp_dir.path();
        
        let state = KeyRotationState::ReEncrypting {
            key_id: "test_key".to_string(),
            started_at: Utc::now(),
            processed: 42,
            total: 100,
            last_checkpoint: Some(("collection".to_string(), "doc_id".to_string())),
        };
        
        // Save
        save_rotation_state(encryption_path, &state).unwrap();
        
        // Load
        let loaded = load_rotation_state(encryption_path).unwrap();
        assert_eq!(state, loaded);
    }
    
    #[test]
    fn test_load_nonexistent_returns_idle() {
        let temp_dir = TempDir::new().unwrap();
        let encryption_path = temp_dir.path();
        
        let state = load_rotation_state(encryption_path).unwrap();
        assert!(state.is_idle());
    }
    
    #[test]
    fn test_clear_state() {
        let temp_dir = TempDir::new().unwrap();
        let encryption_path = temp_dir.path();
        
        let state = KeyRotationState::Idle;
        save_rotation_state(encryption_path, &state).unwrap();
        
        // Verify exists
        assert!(encryption_path.join(STATE_FILE).exists());
        
        // Clear
        clear_rotation_state(encryption_path).unwrap();
        
        // Verify deleted
        assert!(!encryption_path.join(STATE_FILE).exists());
    }
}
