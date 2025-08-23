//! VedDB core structure that ties all components together
//! 
//! Provides the main VedDB instance with shared memory layout,
//! component initialization, and high-level operations.

use crate::{
    arena::Arena,
    kv::{KvStore, KvConfig, KvError},
    session::{SessionRegistry, SessionManager, SessionError, SessionId},
    pubsub::{TopicRegistry, PubSubConfig, PubSubError},
    protocol::{Command, Response, OpCode, Status},
    ring::Slot,
    memory::SharedMemory,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// VedDB configuration
#[derive(Debug, Clone)]
pub struct VedDbConfig {
    pub memory_size: usize,
    pub kv_config: KvConfig,
    pub pubsub_config: PubSubConfig,
    pub max_sessions: usize,
    pub session_ring_capacity: u64,
    pub session_timeout_secs: u64,
}

impl Default for VedDbConfig {
    fn default() -> Self {
        Self {
            memory_size: 64 * 1024 * 1024, // 64MB
            kv_config: KvConfig::default(),
            pubsub_config: PubSubConfig::default(),
            max_sessions: 1000,
            session_ring_capacity: 1024,
            session_timeout_secs: 300, // 5 minutes
        }
    }
}

/// VedDB header in shared memory
#[repr(C)]
pub struct VedDbHeader {
    /// Magic number for validation
    pub magic: u64,
    /// Version number
    pub version: u32,
    /// Header size
    pub header_size: u32,
    /// Total memory size
    pub memory_size: u64,
    /// Creation timestamp
    pub created_at: u64,
    /// Last access timestamp
    pub last_access: AtomicU64,
    /// Global sequence counter
    pub sequence: AtomicU64,
    
    /// Component offsets in shared memory
    pub arena_offset: u64,
    pub arena_size: u64,
    pub kv_store_offset: u64,
    pub kv_store_size: u64,
    pub session_registry_offset: u64,
    pub session_registry_size: u64,
    pub topic_registry_offset: u64,
    pub topic_registry_size: u64,
    
    /// Configuration
    pub config: VedDbConfig,
    
    /// Statistics
    pub total_operations: AtomicU64,
    pub uptime_start: u64,
}

impl VedDbHeader {
    const MAGIC: u64 = 0x5645444200000001; // "VEDB" + version
    const VERSION: u32 = 1;
    
    pub fn new(config: VedDbConfig) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            magic: Self::MAGIC,
            version: Self::VERSION,
            header_size: std::mem::size_of::<Self>() as u32,
            memory_size: config.memory_size as u64,
            created_at: now,
            last_access: AtomicU64::new(now),
            sequence: AtomicU64::new(0),
            arena_offset: 0,
            arena_size: 0,
            kv_store_offset: 0,
            kv_store_size: 0,
            session_registry_offset: 0,
            session_registry_size: 0,
            topic_registry_offset: 0,
            topic_registry_size: 0,
            config,
            total_operations: AtomicU64::new(0),
            uptime_start: now,
        }
    }
    
    pub fn is_valid(&self) -> bool {
        self.magic == Self::MAGIC && self.version == Self::VERSION
    }
    
    pub fn touch(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.last_access.store(now, Ordering::Relaxed);
    }
    
    pub fn next_sequence(&self) -> u64 {
        self.sequence.fetch_add(1, Ordering::Relaxed)
    }
}

/// Main VedDB instance
pub struct VedDb {
    shared_memory: SharedMemory,
    header: *mut VedDbHeader,
    arena: *mut Arena,
    kv_store: *mut KvStore,
    session_manager: SessionManager,
    topic_registry: *mut TopicRegistry,
}

impl VedDb {
    /// Create a new VedDB instance
    pub fn create(name: &str, config: VedDbConfig) -> Result<Self, VedDbError> {
        let shared_memory = SharedMemory::create(name, config.memory_size)
            .map_err(VedDbError::Memory)?;
        
        unsafe { Self::init_new(shared_memory, config) }
    }
    
    /// Open an existing VedDB instance
    pub fn open(name: &str) -> Result<Self, VedDbError> {
        let shared_memory = SharedMemory::open(name)
            .map_err(VedDbError::Memory)?;
        
        unsafe { Self::init_existing(shared_memory) }
    }
    
    /// Initialize a new VedDB instance
    unsafe fn init_new(shared_memory: SharedMemory, config: VedDbConfig) -> Result<Self, VedDbError> {
        let base_ptr = shared_memory.as_ptr();
        let mut offset = 0;
        
        // Initialize header
        let header = base_ptr as *mut VedDbHeader;
        std::ptr::write(header, VedDbHeader::new(config.clone()));
        offset += std::mem::size_of::<VedDbHeader>();
        
        // Calculate component sizes
        let arena_size = config.memory_size / 2; // Use half for arena
        let kv_size = KvStore::size_for_config(&config.kv_config);
        let session_size = SessionRegistry::size_for_max_sessions(config.max_sessions);
        let topic_size = TopicRegistry::size_for_max_topics(config.pubsub_config.max_topics);
        
        // Initialize arena
        let arena_ptr = base_ptr.add(offset);
        let arena = Arena::init(arena_ptr, arena_size);
        (*header).arena_offset = offset as u64;
        (*header).arena_size = arena_size as u64;
        offset += arena_size;
        
        // Initialize KV store
        let kv_ptr = base_ptr.add(offset);
        let kv_store = KvStore::init(kv_ptr, &config.kv_config, arena);
        (*header).kv_store_offset = offset as u64;
        (*header).kv_store_size = kv_size as u64;
        offset += kv_size;
        
        // Initialize session registry
        let session_ptr = base_ptr.add(offset);
        let session_registry = SessionRegistry::init(session_ptr, config.max_sessions);
        (*header).session_registry_offset = offset as u64;
        (*header).session_registry_size = session_size as u64;
        offset += session_size;
        
        // Initialize topic registry
        let topic_ptr = base_ptr.add(offset);
        let topic_registry = TopicRegistry::init(topic_ptr, config.pubsub_config.max_topics);
        (*header).topic_registry_offset = offset as u64;
        (*header).topic_registry_size = topic_size as u64;
        
        // Create session manager
        let session_manager = SessionManager::new(
            session_registry,
            arena,
            config.session_ring_capacity,
        );
        
        Ok(Self {
            shared_memory,
            header,
            arena,
            kv_store,
            session_manager,
            topic_registry,
        })
    }
    
    /// Initialize from existing shared memory
    unsafe fn init_existing(shared_memory: SharedMemory) -> Result<Self, VedDbError> {
        let base_ptr = shared_memory.as_ptr();
        let header = base_ptr as *mut VedDbHeader;
        
        if !(*header).is_valid() {
            return Err(VedDbError::InvalidHeader);
        }
        
        // Get component pointers from offsets
        let arena = base_ptr.add((*header).arena_offset as usize) as *mut Arena;
        let kv_store = base_ptr.add((*header).kv_store_offset as usize) as *mut KvStore;
        let session_registry = base_ptr.add((*header).session_registry_offset as usize) as *mut SessionRegistry;
        let topic_registry = base_ptr.add((*header).topic_registry_offset as usize) as *mut TopicRegistry;
        
        let session_manager = SessionManager::new(
            session_registry,
            arena,
            (*header).config.session_ring_capacity,
        );
        
        Ok(Self {
            shared_memory,
            header,
            arena,
            kv_store,
            session_manager,
            topic_registry,
        })
    }
    
    /// Process a command and return response
    pub fn process_command(&self, command: Command) -> Response {
        self.header().touch();
        self.header().total_operations.fetch_add(1, Ordering::Relaxed);
        
        match command.header.opcode() {
            Ok(OpCode::Ping) => self.handle_ping(command),
            Ok(OpCode::Set) => self.handle_set(command),
            Ok(OpCode::Get) => self.handle_get(command),
            Ok(OpCode::Del) => self.handle_delete(command),
            Ok(OpCode::Cas) => self.handle_cas(command),
            Ok(OpCode::Subscribe) => self.handle_subscribe(command),
            Ok(OpCode::Unsubscribe) => self.handle_unsubscribe(command),
            Ok(OpCode::Publish) => self.handle_publish(command),
            Ok(OpCode::Info) => self.handle_info(command),
            _ => Response::error(command.header.seq),
        }
    }
    
    fn handle_ping(&self, command: Command) -> Response {
        Response::ok(command.header.seq, b"pong".to_vec())
    }
    
    fn handle_set(&self, command: Command) -> Response {
        let kv_store = unsafe { &*self.kv_store };
        match kv_store.set(&command.key, &command.value) {
            Ok(()) => Response::ok(command.header.seq, Vec::new()),
            Err(KvError::KeyTooLarge) => Response::error(command.header.seq),
            Err(KvError::ValueTooLarge) => Response::error(command.header.seq),
            Err(KvError::OutOfMemory) => Response::error(command.header.seq),
            _ => Response::error(command.header.seq),
        }
    }
    
    fn handle_get(&self, command: Command) -> Response {
        let kv_store = unsafe { &*self.kv_store };
        match kv_store.get(&command.key) {
            Some(value) => Response::ok(command.header.seq, value),
            None => Response::not_found(command.header.seq),
        }
    }
    
    fn handle_delete(&self, command: Command) -> Response {
        let kv_store = unsafe { &*self.kv_store };
        if kv_store.delete(&command.key) {
            Response::ok(command.header.seq, Vec::new())
        } else {
            Response::not_found(command.header.seq)
        }
    }
    
    fn handle_cas(&self, command: Command) -> Response {
        let kv_store = unsafe { &*self.kv_store };
        let expected_version = command.header.extra;
        
        match kv_store.cas(&command.key, expected_version, &command.value) {
            Ok(new_version) => {
                let mut resp = Response::ok(command.header.seq, Vec::new());
                resp.header.extra = new_version;
                resp
            }
            Err(KvError::NotFound) => Response::not_found(command.header.seq),
            Err(KvError::VersionMismatch) => {
                let mut resp = Response::new(Status::VersionMismatch, command.header.seq, Vec::new());
                resp
            }
            _ => Response::error(command.header.seq),
        }
    }
    
    fn handle_subscribe(&self, _command: Command) -> Response {
        // TODO: Implement subscription logic
        Response::error(_command.header.seq)
    }
    
    fn handle_unsubscribe(&self, _command: Command) -> Response {
        // TODO: Implement unsubscription logic
        Response::error(_command.header.seq)
    }
    
    fn handle_publish(&self, _command: Command) -> Response {
        // TODO: Implement publish logic
        Response::error(_command.header.seq)
    }
    
    fn handle_info(&self, command: Command) -> Response {
        let stats = self.get_stats();
        let info_json = serde_json::to_vec(&stats).unwrap_or_default();
        Response::ok(command.header.seq, info_json)
    }
    
    /// Attach a new session
    pub fn attach_session(&self, pid: u32) -> Result<SessionId, SessionError> {
        self.session_manager.attach_session(pid)
    }
    
    /// Detach a session
    pub fn detach_session(&self, session_id: SessionId) -> Result<(), SessionError> {
        self.session_manager.detach_session(session_id)
    }
    
    /// Send response to a session
    pub fn send_response(&self, session_id: SessionId, response: Response) -> Result<(), SessionError> {
        let response_bytes = response.to_bytes();
        let slot = if response_bytes.len() <= 8 {
            Slot::inline_data(&response_bytes).unwrap()
        } else {
            // Allocate in arena for larger responses
            let arena = unsafe { &*self.arena };
            let offset = arena.allocate(response_bytes.len(), 1);
            if offset == 0 {
                return Err(SessionError::OutOfMemory);
            }
            unsafe {
                let ptr = arena.offset_to_ptr(offset);
                std::ptr::copy_nonoverlapping(response_bytes.as_ptr(), ptr, response_bytes.len());
            }
            Slot::arena_offset(response_bytes.len() as u32, offset)
        };
        
        self.session_manager.send_response(session_id, slot)
    }
    
    /// Get current statistics
    pub fn get_stats(&self) -> VedDbStats {
        let header = self.header();
        // Compute uptime
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let uptime_secs = now.saturating_sub(header.uptime_start);

        // Arena stats for memory usage
        let arena_stats = unsafe { &*self.arena }.stats();

        // KV stats
        let kv_stats = unsafe { &*self.kv_store }.stats();

        // Active sessions via session manager
        let active_sessions = self.get_active_sessions().len() as u64;

        // Topic registry stats
        let active_topics = unsafe { &*self.topic_registry }.stats().active_topics;

        VedDbStats {
            uptime_secs,
            total_operations: header.total_operations.load(Ordering::Relaxed),
            memory_size: header.memory_size,
            memory_used: arena_stats.allocated_bytes,
            kv_operations: kv_stats.total_operations,
            kv_keys: kv_stats.total_keys,
            active_sessions,
            active_topics,
        }
    }
    
    /// Get list of active session IDs
    pub fn get_active_sessions(&self) -> Vec<SessionId> {
        self.session_manager.get_active_sessions()
    }
    
    /// Try to get a command from a session's command ring
    pub fn try_get_command(&self, session_id: SessionId) -> Result<Option<Command>, VedDbError> {
        Ok(self.session_manager.try_get_command(session_id)?)
    }
    
    /// Clean up stale sessions
    pub fn cleanup_stale_sessions(&self) -> usize {
        self.session_manager.cleanup_stale_sessions(self.header().config.session_timeout_secs)
    }
    
    fn header(&self) -> &VedDbHeader {
        unsafe { &*self.header }
    }
}

unsafe impl Send for VedDb {}
unsafe impl Sync for VedDb {}

/// VedDB statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct VedDbStats {
    pub uptime_secs: u64,
    pub total_operations: u64,
    pub memory_size: u64,
    pub memory_used: u64,
    pub kv_operations: u64,
    pub kv_keys: u64,
    pub active_sessions: u64,
    pub active_topics: u64,
}

/// VedDB errors
#[derive(Debug, thiserror::Error)]
pub enum VedDbError {
    #[error("Memory error: {0}")]
    Memory(#[from] anyhow::Error),
    #[error("Invalid header")]
    InvalidHeader,
    #[error("KV error: {0}")]
    Kv(#[from] KvError),
    #[error("Session error: {0}")]
    Session(#[from] SessionError),
    #[error("PubSub error: {0}")]
    PubSub(#[from] PubSubError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_veddb_creation() {
        let config = VedDbConfig::default();
        let veddb = VedDb::create("test_veddb", config).unwrap();
        
        let stats = veddb.get_stats();
        assert_eq!(stats.kv_keys, 0);
        assert_eq!(stats.active_sessions, 0);
    }
    
    #[test]
    fn test_veddb_kv_operations() {
        let config = VedDbConfig::default();
        let veddb = VedDb::create("test_kv", config).unwrap();
        
        // Test SET
        let set_cmd = Command::set(1, b"key1".to_vec(), b"value1".to_vec());
        let resp = veddb.process_command(set_cmd);
        assert_eq!(resp.header.status().unwrap(), Status::Ok);
        
        // Test GET
        let get_cmd = Command::get(2, b"key1".to_vec());
        let resp = veddb.process_command(get_cmd);
        assert_eq!(resp.header.status().unwrap(), Status::Ok);
        assert_eq!(resp.payload, b"value1");
        
        // Test DELETE
        let del_cmd = Command::del(3, b"key1".to_vec());
        let resp = veddb.process_command(del_cmd);
        assert_eq!(resp.header.status().unwrap(), Status::Ok);
        
        // Test GET after delete
        let get_cmd = Command::get(4, b"key1".to_vec());
        let resp = veddb.process_command(get_cmd);
        assert_eq!(resp.header.status().unwrap(), Status::NotFound);
    }
    
    #[test]
    fn test_veddb_sessions() {
        let config = VedDbConfig::default();
        let veddb = VedDb::create("test_sessions", config).unwrap();
        
        // Attach session
        let session_id = veddb.attach_session(1234).unwrap();
        assert!(session_id > 0);
        
        // Send response
        let response = Response::ok(1, b"test".to_vec());
        veddb.send_response(session_id, response).unwrap();
        
        // Detach session
        veddb.detach_session(session_id).unwrap();
    }
}
