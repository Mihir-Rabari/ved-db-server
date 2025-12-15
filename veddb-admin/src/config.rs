use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminConfig {
    pub server: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub tls: Option<bool>,
    pub timeout_seconds: Option<u64>,
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            server: Some("127.0.0.1:50051".to_string()),
            username: None,
            password: None,
            tls: Some(true),
            timeout_seconds: Some(30),
        }
    }
}

impl AdminConfig {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        
        let content = std::fs::read_to_string(path)?;
        let config: AdminConfig = toml::from_str(&content)?;
        Ok(config)
    }
    
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}