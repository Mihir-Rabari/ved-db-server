//! VedDB Core - Shared memory primitives and data structures
//!
//! This crate provides the foundational components for VedDB:
//! - Shared memory management
//! - Ring buffer implementations (SPSC and MPMC)
//! - Arena allocation
//! - Command protocol definitions

pub mod arena;
pub mod core;
pub mod kv;
pub mod memory;
pub mod protocol;
pub mod pubsub;
pub mod ring;
pub mod session;

pub use arena::*;
pub use core::*;
pub use memory::*;
pub use protocol::*;
pub use ring::*;
