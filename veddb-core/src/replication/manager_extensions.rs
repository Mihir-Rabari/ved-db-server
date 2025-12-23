// Extension module for ReplicationManager with public slave management methods

use super::manager::ReplicationManager;
use super::{ReplicationError, ReplicationResult};
use crate::protocol::SlaveInfo;
use std::net::SocketAddr;
use tracing::info;

impl ReplicationManager {
    /// Add a new slave to the replication cluster (master only)
    pub async fn add_slave(&self, slave_addr: &str) -> ReplicationResult<String> {
        // Parse address
        let addr: SocketAddr = slave_addr.parse()
            .map_err(|e| ReplicationError::ConnectionError(format!("Invalid slave address: {}", e)))?;

        // Generate slave ID
        let slave_id = format!("slave_{}_{}", addr.ip(), addr.port());

        info!("Adding slave {} at {}", slave_id, addr);

        // Simplified implementation - actual connection happens in start_master
        Ok(slave_id)
    }

    /// Remove a slave from the cluster (master only)
    pub async fn remove_slave(&self, slave_id: &str) -> ReplicationResult<()> {
        info!("Removing slave {}", slave_id);
        
        // TODO: Implement when SlaveConnectionManager exposes remove method
        Ok(())
    }

    /// List all connected slaves (master only)
    pub async fn list_slaves(&self) -> Vec<SlaveInfo> {
        vec![]
        // TODO: Expose public method on SlaveConnectionManager to list slaves
    }

    /// Force synchronization with all slaves (master only)
    pub async fn force_sync(&self) -> ReplicationResult<()> {
        info!("Forcing synchronization with all slaves");
        
        // TODO: Implement by triggering sync through slave_manager
        Ok(())
    }
}

