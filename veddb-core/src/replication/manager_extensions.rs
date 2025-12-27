// Extension module for ReplicationManager with public slave management methods

use super::manager::ReplicationManager;
use super::{ReplicationError, ReplicationResult};
use crate::replication::connection::SlaveInfo;
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

        // Note: Actual connection happens in start_master or when listener accepts connections
        // This is a simplified implementation that just validates the address
        Ok(slave_id)
    }

    /// Remove a slave from the cluster (master only)
    pub async fn remove_slave(&self, slave_id: &str) -> ReplicationResult<()> {
        info!("Removing slave {}", slave_id);
        
        let mut slave_manager = self.slave_manager.lock().await;
        if let Some(manager) = slave_manager.as_mut() {
            if manager.disconnect_slave(slave_id).await {
                info!("Successfully removed slave {}", slave_id);
                Ok(())
            } else {
                Err(ReplicationError::ConnectionError(format!("Slave not found: {}", slave_id)))
            }
        } else {
            Err(ReplicationError::NotMaster)
        }
    }

    /// List all connected slaves (master only)
    pub async fn list_slaves(&self) -> Vec<SlaveInfo> {
        let slave_manager = self.slave_manager.lock().await;
        if let Some(manager) = slave_manager.as_ref() {
            manager.get_slave_info()
        } else {
            vec![]
        }
    }

    /// Force synchronization with all slaves (master only)
    pub async fn force_sync(&self) -> ReplicationResult<usize> {
        info!("Forcing synchronization with all slaves");
        
        let slave_manager = self.slave_manager.lock().await;
        if let Some(manager) = slave_manager.as_ref() {
            let synced_count = manager.force_sync().await;
            info!("Force sync sent to {} slaves", synced_count);
            Ok(synced_count)
        } else {
            Err(ReplicationError::NotMaster)
        }
    }
}
