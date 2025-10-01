//! Session management for client connections
//!
//! Handles client attachment, SPSC ring allocation, and eventfd-based notifications.

use crate::arena::Arena;
use crate::protocol::Command;
use crate::ring::{Slot, SpscRing};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

#[cfg(target_os = "linux")]
use nix::sys::eventfd::{eventfd, EfdFlags};
#[cfg(target_os = "linux")]
use std::os::unix::io::RawFd;

/// Session identifier
pub type SessionId = u64;

/// Session descriptor stored in shared memory
#[repr(C)]
pub struct SessionDesc {
    /// Session ID
    pub id: SessionId,
    /// Client process ID (for cleanup detection)
    pub pid: u32,
    /// Command ring offset in shared memory
    pub cmd_ring_offset: u64,
    /// Response ring offset in shared memory
    pub resp_ring_offset: u64,
    /// Ring capacity
    pub ring_capacity: u64,
    /// Eventfd for notifications (Linux only)
    #[cfg(target_os = "linux")]
    pub eventfd: RawFd,
    /// Pending notification counter
    pub pending_notifications: AtomicU32,
    /// Session flags
    pub flags: u32,
    /// Last activity timestamp
    pub last_activity: AtomicU64,
}

impl SessionDesc {
    pub fn new(id: SessionId, pid: u32) -> Self {
        Self {
            id,
            pid,
            cmd_ring_offset: 0,
            resp_ring_offset: 0,
            ring_capacity: 0,
            #[cfg(target_os = "linux")]
            eventfd: -1,
            pending_notifications: AtomicU32::new(0),
            flags: 0,
            last_activity: AtomicU64::new(0),
        }
    }

    /// Update last activity timestamp
    pub fn touch(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.last_activity.store(now, Ordering::Relaxed);
    }

    /// Check if session is stale (no activity for timeout_secs)
    pub fn is_stale(&self, timeout_secs: u64) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let last = self.last_activity.load(Ordering::Relaxed);
        now.saturating_sub(last) > timeout_secs
    }
}

/// Session registry in shared memory
#[repr(C)]
pub struct SessionRegistry {
    /// Next session ID to assign
    next_session_id: AtomicU64,
    /// Maximum number of sessions
    max_sessions: u64,
    /// Current number of active sessions
    active_sessions: AtomicU64,
    // Session descriptors follow this structure
}

impl SessionRegistry {
    /// Calculate size needed for session registry
    pub fn size_for_max_sessions(max_sessions: usize) -> usize {
        std::mem::size_of::<Self>() + max_sessions * std::mem::size_of::<SessionDesc>()
    }

    /// Initialize session registry in shared memory
    ///
    /// # Safety
    /// - ptr must point to valid memory of sufficient size
    pub unsafe fn init(ptr: *mut u8, max_sessions: usize) -> *mut Self {
        let registry = ptr as *mut Self;

        std::ptr::write(
            registry,
            Self {
                next_session_id: AtomicU64::new(1),
                max_sessions: max_sessions as u64,
                active_sessions: AtomicU64::new(0),
            },
        );

        // Initialize session descriptors
        let sessions_ptr = ptr.add(std::mem::size_of::<Self>()) as *mut SessionDesc;
        for i in 0..max_sessions {
            std::ptr::write(sessions_ptr.add(i), SessionDesc::new(0, 0));
        }

        registry
    }

    /// Get pointer to session descriptors array
    unsafe fn sessions_ptr(&self) -> *mut SessionDesc {
        let self_ptr = self as *const Self as *mut u8;
        self_ptr.add(std::mem::size_of::<Self>()) as *mut SessionDesc
    }

    /// Get session descriptor at index
    unsafe fn session_at(&self, index: usize) -> &mut SessionDesc {
        let sessions = self.sessions_ptr();
        &mut *sessions.add(index)
    }

    /// Get list of active session IDs
    pub fn get_active_sessions(&self) -> Vec<SessionId> {
        let mut sessions = Vec::new();
        unsafe {
            for i in 0..self.max_sessions as usize {
                let s = &*self.sessions_ptr().add(i);
                if s.id != 0 {
                    sessions.push(s.id);
                }
            }
        }
        sessions
    }

    /// Find free session slot
    fn find_free_slot(&self) -> Option<usize> {
        unsafe {
            for i in 0..self.max_sessions as usize {
                let session = self.session_at(i);
                if session.id == 0 {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Create a new session
    pub fn create_session(&self, pid: u32, ring_capacity: u64) -> Option<SessionId> {
        if self.active_sessions.load(Ordering::Relaxed) >= self.max_sessions {
            return None;
        }

        let slot = self.find_free_slot()?;
        let session_id = self.next_session_id.fetch_add(1, Ordering::Relaxed);

        unsafe {
            let session = self.session_at(slot);
            session.id = session_id;
            session.pid = pid;
            session.ring_capacity = ring_capacity;
            session.touch();

            #[cfg(target_os = "linux")]
            {
                // Create eventfd for notifications
                match eventfd(0, EfdFlags::EFD_CLOEXEC) {
                    Ok(fd) => session.eventfd = fd,
                    Err(_) => session.eventfd = -1,
                }
            }
        }

        self.active_sessions.fetch_add(1, Ordering::Relaxed);
        Some(session_id)
    }

    /// Get session descriptor by ID
    pub fn get_session(&self, session_id: SessionId) -> Option<&SessionDesc> {
        unsafe {
            for i in 0..self.max_sessions as usize {
                let session = &*self.sessions_ptr().add(i);
                if session.id == session_id {
                    return Some(session);
                }
            }
        }
        None
    }

    /// Get mutable session descriptor by ID
    pub fn get_session_mut(&self, session_id: SessionId) -> Option<&mut SessionDesc> {
        unsafe {
            for i in 0..self.max_sessions as usize {
                let session = &mut *self.sessions_ptr().add(i);
                if session.id == session_id {
                    return Some(session);
                }
            }
        }
        None
    }

    /// Remove a session
    pub fn remove_session(&self, session_id: SessionId) -> bool {
        unsafe {
            for i in 0..self.max_sessions as usize {
                let session = self.session_at(i);
                if session.id == session_id {
                    #[cfg(target_os = "linux")]
                    {
                        if session.eventfd != -1 {
                            let _ = nix::unistd::close(session.eventfd);
                        }
                    }

                    session.id = 0;
                    session.pid = 0;
                    self.active_sessions.fetch_sub(1, Ordering::Relaxed);
                    return true;
                }
            }
        }
        false
    }

    /// Clean up stale sessions
    pub fn cleanup_stale_sessions(&self, timeout_secs: u64) -> usize {
        let mut cleaned = 0;

        unsafe {
            for i in 0..self.max_sessions as usize {
                let session = self.session_at(i);
                if session.id != 0 && session.is_stale(timeout_secs) {
                    #[cfg(target_os = "linux")]
                    {
                        if session.eventfd != -1 {
                            let _ = nix::unistd::close(session.eventfd);
                        }
                    }

                    session.id = 0;
                    session.pid = 0;
                    cleaned += 1;
                }
            }
        }

        self.active_sessions.fetch_sub(cleaned, Ordering::Relaxed);
        cleaned as usize
    }

    /// Get session statistics
    pub fn stats(&self) -> SessionStats {
        SessionStats {
            active_sessions: self.active_sessions.load(Ordering::Relaxed),
            max_sessions: self.max_sessions,
            next_session_id: self.next_session_id.load(Ordering::Relaxed),
        }
    }
}

/// Session manager that coordinates ring allocation and notifications
pub struct SessionManager {
    registry: *mut SessionRegistry,
    arena: *mut Arena,
    ring_capacity: u64,
}

impl SessionManager {
    /// Create a new session manager
    ///
    /// # Safety
    /// - registry and arena must be valid pointers to initialized structures
    pub unsafe fn new(
        registry: *mut SessionRegistry,
        arena: *mut Arena,
        ring_capacity: u64,
    ) -> Self {
        Self {
            registry,
            arena,
            ring_capacity,
        }
    }

    /// Attach a new client session
    pub fn attach_session(&self, pid: u32) -> Result<SessionId, SessionError> {
        let registry = unsafe { &*self.registry };
        let arena = unsafe { &*self.arena };

        // Create session entry
        let session_id = registry
            .create_session(pid, self.ring_capacity)
            .ok_or(SessionError::TooManySessions)?;

        // Allocate command and response rings
        let cmd_ring_size = SpscRing::size_for_capacity(self.ring_capacity);
        let resp_ring_size = SpscRing::size_for_capacity(self.ring_capacity);

        let cmd_ring_offset = arena.allocate(cmd_ring_size, 64);
        let resp_ring_offset = arena.allocate(resp_ring_size, 64);

        if cmd_ring_offset == 0 || resp_ring_offset == 0 {
            registry.remove_session(session_id);
            return Err(SessionError::OutOfMemory);
        }

        // Initialize rings
        unsafe {
            let cmd_ring_ptr = arena.offset_to_ptr(cmd_ring_offset);
            let resp_ring_ptr = arena.offset_to_ptr(resp_ring_offset);

            SpscRing::init(cmd_ring_ptr, self.ring_capacity);
            SpscRing::init(resp_ring_ptr, self.ring_capacity);
        }

        // Update session descriptor
        if let Some(session) = registry.get_session_mut(session_id) {
            session.cmd_ring_offset = cmd_ring_offset;
            session.resp_ring_offset = resp_ring_offset;
            session.touch();
        }

        Ok(session_id)
    }

    /// Detach a client session
    pub fn detach_session(&self, session_id: SessionId) -> Result<(), SessionError> {
        let registry = unsafe { &*self.registry };
        let arena = unsafe { &*self.arena };

        // Get session info before removing
        let (cmd_ring_offset, resp_ring_offset) = {
            let session = registry
                .get_session(session_id)
                .ok_or(SessionError::SessionNotFound)?;
            (session.cmd_ring_offset, session.resp_ring_offset)
        };

        // Remove session
        if !registry.remove_session(session_id) {
            return Err(SessionError::SessionNotFound);
        }

        // Free ring memory
        unsafe {
            let ring_size = SpscRing::size_for_capacity(self.ring_capacity);
            arena.free(cmd_ring_offset, ring_size);
            arena.free(resp_ring_offset, ring_size);
        }

        Ok(())
    }

    /// Get command ring for a session
    pub fn get_cmd_ring(&self, session_id: SessionId) -> Result<&SpscRing, SessionError> {
        let registry = unsafe { &*self.registry };
        let arena = unsafe { &*self.arena };

        let session = registry
            .get_session(session_id)
            .ok_or(SessionError::SessionNotFound)?;

        unsafe {
            let ring_ptr = arena.offset_to_ptr(session.cmd_ring_offset);
            Ok(&*(ring_ptr as *const SpscRing))
        }
    }

    /// Get response ring for a session
    pub fn get_resp_ring(&self, session_id: SessionId) -> Result<&SpscRing, SessionError> {
        let registry = unsafe { &*self.registry };
        let arena = unsafe { &*self.arena };

        let session = registry
            .get_session(session_id)
            .ok_or(SessionError::SessionNotFound)?;

        unsafe {
            let ring_ptr = arena.offset_to_ptr(session.resp_ring_offset);
            Ok(&*(ring_ptr as *const SpscRing))
        }
    }

    /// Send response to a session with batched notifications
    pub fn send_response(&self, session_id: SessionId, slot: Slot) -> Result<(), SessionError> {
        let registry = unsafe { &*self.registry };
        let resp_ring = self.get_resp_ring(session_id)?;

        // Push response to ring
        if !resp_ring.try_push(slot) {
            return Err(SessionError::RingFull);
        }

        // Handle notifications with batching
        let session = registry
            .get_session(session_id)
            .ok_or(SessionError::SessionNotFound)?;

        let prev_pending = session
            .pending_notifications
            .fetch_add(1, Ordering::Relaxed);

        // Only write to eventfd if this is the first pending notification
        // This batches multiple responses into a single wake event
        if prev_pending == 0 {
            #[cfg(target_os = "linux")]
            {
                if session.eventfd != -1 {
                    let _ = nix::sys::eventfd::eventfd_write(session.eventfd, 1);
                }
            }
        }

        session.touch();
        Ok(())
    }

    /// Client acknowledges notifications (resets counter)
    pub fn ack_notifications(&self, session_id: SessionId) -> Result<(), SessionError> {
        let registry = unsafe { &*self.registry };
        let session = registry
            .get_session(session_id)
            .ok_or(SessionError::SessionNotFound)?;

        session.pending_notifications.store(0, Ordering::Relaxed);
        session.touch();
        Ok(())
    }

    /// Clean up stale sessions
    pub fn cleanup_stale_sessions(&self, timeout_secs: u64) -> usize {
        let registry = unsafe { &*self.registry };
        registry.cleanup_stale_sessions(timeout_secs)
    }

    /// Get list of active session IDs
    pub fn get_active_sessions(&self) -> Vec<SessionId> {
        let registry = unsafe { &*self.registry };
        registry.get_active_sessions()
    }

    /// Try to get the next command from a session's command ring
    pub fn try_get_command(&self, session_id: SessionId) -> Result<Option<Command>, SessionError> {
        let arena = unsafe { &*self.arena };
        let cmd_ring = self.get_cmd_ring(session_id)?;

        if let Some(slot) = cmd_ring.try_pop() {
            // Extract bytes from slot
            let bytes: Vec<u8> = if let Some(inline) = slot.get_inline_data() {
                inline.to_vec()
            } else if let Some(offset) = slot.get_arena_offset() {
                let len = slot.len as usize;
                unsafe {
                    let ptr = arena.offset_to_ptr(offset);
                    std::slice::from_raw_parts(ptr, len).to_vec()
                }
            } else {
                Vec::new()
            };

            match Command::from_bytes(&bytes) {
                Ok(cmd) => Ok(Some(cmd)),
                Err(_) => Err(SessionError::InvalidSession),
            }
        } else {
            Ok(None)
        }
    }
}

unsafe impl Send for SessionManager {}
unsafe impl Sync for SessionManager {}

/// Session statistics
#[derive(Debug, Clone)]
pub struct SessionStats {
    pub active_sessions: u64,
    pub max_sessions: u64,
    pub next_session_id: u64,
}

/// Session management errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    TooManySessions,
    OutOfMemory,
    SessionNotFound,
    RingFull,
    InvalidSession,
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::TooManySessions => write!(f, "Too many sessions"),
            SessionError::OutOfMemory => write!(f, "Out of memory"),
            SessionError::SessionNotFound => write!(f, "Session not found"),
            SessionError::RingFull => write!(f, "Ring buffer full"),
            SessionError::InvalidSession => write!(f, "Invalid session"),
        }
    }
}

impl std::error::Error for SessionError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::ArenaBuffer;

    #[test]
    fn test_session_registry() {
        let registry_size = SessionRegistry::size_for_max_sessions(10);
        // Allocate aligned memory for the registry
        let layout = std::alloc::Layout::from_size_align(registry_size, 8).unwrap();
        let registry_memory = unsafe { std::alloc::alloc(layout) };
        
        let registry = unsafe { SessionRegistry::init(registry_memory, 10) };

        let registry_ref = unsafe { &*registry };

        // Create sessions
        let session1 = registry_ref.create_session(1234, 256).unwrap();
        let session2 = registry_ref.create_session(5678, 256).unwrap();

        assert_ne!(session1, session2);
        assert_eq!(registry_ref.stats().active_sessions, 2);

        // Get sessions
        let desc1 = registry_ref.get_session(session1).unwrap();
        assert_eq!(desc1.pid, 1234);

        // Remove session
        assert!(registry_ref.remove_session(session1));
        assert!(registry_ref.get_session(session1).is_none());
        assert_eq!(registry_ref.stats().active_sessions, 1);
        
        // Clean up allocated memory
        unsafe {
            std::alloc::dealloc(registry_memory, layout);
        }
    }

    #[test]
    fn test_session_manager() {
        let arena_buf = ArenaBuffer::new(65536);
        let arena = arena_buf.arena() as *const Arena as *mut Arena;

        let registry_size = SessionRegistry::size_for_max_sessions(10);
        // Allocate aligned memory for the registry
        let layout = std::alloc::Layout::from_size_align(registry_size, 8).unwrap();
        let registry_memory = unsafe { std::alloc::alloc(layout) };
        let registry = unsafe { SessionRegistry::init(registry_memory, 10) };

        let manager = unsafe { SessionManager::new(registry, arena, 256) };

        // Attach session
        let session_id = manager.attach_session(1234).unwrap();

        // Get rings
        let cmd_ring = manager.get_cmd_ring(session_id).unwrap();
        let resp_ring = manager.get_resp_ring(session_id).unwrap();

        assert_eq!(cmd_ring.capacity(), 256);
        assert_eq!(resp_ring.capacity(), 256);

        // Send response
        let slot = Slot::inline_data(b"response").unwrap();
        manager.send_response(session_id, slot).unwrap();

        // Check response was queued
        assert_eq!(resp_ring.len(), 1);

        // Detach session
        manager.detach_session(session_id).unwrap();
        assert!(manager.get_cmd_ring(session_id).is_err());
        
        // Clean up allocated memory
        unsafe {
            std::alloc::dealloc(registry_memory, layout);
        }
    }
}
