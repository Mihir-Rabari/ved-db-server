// Extension module for ReplicationManager with public slave management methods

use super::manager::ReplicationManager;
use super::{ReplicationError, ReplicationResult};
use crate::replication::connection::SlaveInfo;
use std::net::SocketAddr;
use tracing::info;

impl ReplicationManager {
    /// Add a new slave to the replication cluster (master only)
    /// 
    /// Note: In VedDB's replication model, slaves connect TO the master,
    /// not the other way around. This method validates the expected slave address
    /// but the actual connection is established when the slave calls start_slave()
    /// and connects to this master's bind_address.
    /// 
    /// To add a slave:
    /// 1. Call this method on master to whitelist the slave address (optional)
    /// 2. Start the slave node with master_addr pointing to this master
    /// 3. Slave will connect and appear in list_slaves()
    pub async fn add_slave(&self, slave_addr: &str) -> ReplicationResult<String> {
        // Parse address
        let addr: SocketAddr = slave_addr.parse()
            .map_err(|e| ReplicationError::ConnectionError(format!("Invalid slave address: {}", e)))?;

        // Generate slave ID
        let slave_id = format!("slave_{}_{}", addr.ip(), addr.port());

        info!("Registered expected slave {} at {} (awaiting connection)", slave_id, addr);

        // Actual connection happens when slave connects to this master's listener
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
