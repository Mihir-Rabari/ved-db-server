//! Worker thread pool for processing VedDB commands
//!
//! Each worker thread is pinned to a CPU core and processes commands
//! from client sessions using SPSC rings.

use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use veddb_core::session::SessionId;
use veddb_core::{Command, VedDb};

/// Worker pool that manages command processing threads
pub struct WorkerPool {
    workers: Vec<Worker>,
    shutdown_tx: mpsc::Sender<()>,
}

impl WorkerPool {
    /// Create a new worker pool with the specified number of workers
    pub async fn new(veddb: Arc<VedDb>, num_workers: usize) -> Result<Self> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        let mut workers = Vec::new();

        for worker_id in 0..num_workers {
            let worker = Worker::new(worker_id, veddb.clone()).await?;
            workers.push(worker);
        }

        // Spawn coordinator task to handle shutdown
        let workers_for_shutdown = workers.clone();
        tokio::spawn(async move {
            let _ = shutdown_rx.recv().await;
            for worker in workers_for_shutdown {
                worker.shutdown().await;
            }
        });

        Ok(Self {
            workers,
            shutdown_tx,
        })
    }

    /// Shutdown all workers gracefully
    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(()).await;

        // Wait a bit for graceful shutdown
        tokio::time::sleep(Duration::from_millis(100)).await;

        for worker in self.workers {
            worker.join().await;
        }
    }
}

/// Individual worker thread
#[derive(Clone)]
pub struct Worker {
    id: usize,
    handle: Arc<tokio::task::JoinHandle<()>>,
    shutdown_tx: mpsc::Sender<()>,
}

impl Worker {
    /// Create a new worker thread
    async fn new(id: usize, veddb: Arc<VedDb>) -> Result<Self> {
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        let handle = tokio::spawn(async move {
            if let Err(e) = Self::run_worker(id, veddb, shutdown_rx).await {
                error!("Worker {} error: {}", id, e);
            }
        });

        Ok(Self {
            id,
            handle: Arc::new(handle),
            shutdown_tx,
        })
    }

    /// Main worker loop
    async fn run_worker(
        id: usize,
        veddb: Arc<VedDb>,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) -> Result<()> {
        info!("Worker {} started", id);

        // Try to pin to CPU core (best effort)
        #[cfg(target_os = "linux")]
        {
            let core_id = id % num_cpus::get();
            if let Err(e) = Self::pin_to_core(core_id) {
                warn!("Worker {} failed to pin to core {}: {}", id, core_id, e);
            } else {
                debug!("Worker {} pinned to core {}", id, core_id);
            }
        }

        let mut processed_count = 0u64;
        let mut last_report = std::time::Instant::now();

        loop {
            // Check for shutdown signal
            if shutdown_rx.try_recv().is_ok() {
                info!("Worker {} received shutdown signal", id);
                break;
            }

            // Process commands from all sessions
            // This is a simplified approach - in production you'd want better load balancing
            let mut had_work = false;

            // Get session stats to iterate over active sessions
            let stats = veddb.get_stats();
            if stats.active_sessions > 0 {
                // In a real implementation, you'd maintain a list of active sessions
                // and distribute them across workers. For now, we'll simulate work.
                had_work = Self::process_session_commands(veddb.clone(), id).await?;
            }

            if had_work {
                processed_count += 1;
            } else {
                // No work available, sleep briefly to avoid busy waiting
                tokio::time::sleep(Duration::from_micros(100)).await;
            }

            // Report stats periodically
            if last_report.elapsed() > Duration::from_secs(60) {
                info!(
                    "Worker {} processed {} commands in last minute",
                    id, processed_count
                );
                processed_count = 0;
                last_report = std::time::Instant::now();
            }
        }

        Ok(())
    }

    /// Process commands from sessions assigned to this worker
    async fn process_session_commands(veddb: Arc<VedDb>, worker_id: usize) -> Result<bool> {
        // Create command processor for this worker
        let processor = CommandProcessor::new(veddb.clone());
        
        // This is a simplified implementation
        // In production, you'd have a more sophisticated session assignment
        // For now, we'll check if there are any pending commands
        
        // Check for sessions with pending commands
        let sessions = veddb.get_active_sessions();
        let mut processed_any = false;
        
        for session_id in sessions {
            // Try to get a command from this session
            if let Ok(Some(command)) = veddb.try_get_command(session_id) {
                // Process the command
                if let Err(e) = processor.process_command(command, session_id).await {
                    warn!("Worker {} failed to process command for session {}: {}", 
                          worker_id, session_id, e);
                }
                processed_any = true;
            }
        }
        
        Ok(processed_any)
    }

    /// Pin worker thread to specific CPU core (Linux only)
    #[cfg(target_os = "linux")]
    fn pin_to_core(core_id: usize) -> Result<()> {
        use libc::{cpu_set_t, sched_setaffinity, CPU_SET, CPU_ZERO};
        use std::mem;

        unsafe {
            let mut cpuset: cpu_set_t = mem::zeroed();
            CPU_ZERO(&mut cpuset);
            CPU_SET(core_id, &mut cpuset);

            let result = sched_setaffinity(
                0, // current thread
                mem::size_of::<cpu_set_t>(),
                &cpuset,
            );

            if result != 0 {
                return Err(anyhow::anyhow!("sched_setaffinity failed"));
            }
        }

        Ok(())
    }

    /// Signal worker to shutdown
    async fn shutdown(&self) {
        let _ = self.shutdown_tx.send(()).await;
    }

    /// Wait for worker to complete
    async fn join(self) {
        if let Ok(handle) = Arc::try_unwrap(self.handle) {
            let _ = handle.await;
        }
    }
}

/// Command processor that handles individual commands
pub struct CommandProcessor {
    veddb: Arc<VedDb>,
}

impl CommandProcessor {
    pub fn new(veddb: Arc<VedDb>) -> Self {
        Self { veddb }
    }

    /// Process a single command and return response
    pub async fn process_command(&self, command: Command, session_id: SessionId) -> Result<()> {
        let response = self.veddb.process_command(command);

        // Send response back to client
        if let Err(e) = self.veddb.send_response(session_id, response) {
            warn!("Failed to send response to session {}: {}", session_id, e);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use veddb_core::VedDbConfig;

    #[tokio::test]
    async fn test_worker_pool_creation() {
        let config = VedDbConfig::default();
        let veddb = Arc::new(VedDb::create("test_workers", config).unwrap());

        let pool = WorkerPool::new(veddb, 2).await.unwrap();

        // Let it run briefly
        tokio::time::sleep(Duration::from_millis(100)).await;

        pool.shutdown().await;
    }
}
