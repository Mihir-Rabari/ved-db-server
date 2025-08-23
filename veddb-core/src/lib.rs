//! VedDB Core - Shared memory primitives and data structures
//! 
//! This crate provides the foundational components for VedDB:
//! - Shared memory management
//! - Ring buffer implementations (SPSC and MPMC)
//! - Arena allocation
//! - Command protocol definitions

pub mod memory;
pub mod ring;
pub mod arena;
pub mod protocol;
pub mod kv;
pub mod session;
pub mod pubsub;
pub mod core;

pub use memory::*;
pub use ring::*;
pub use arena::*;
pub use protocol::*;
pub use core::*;
