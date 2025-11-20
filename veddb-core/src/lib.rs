//! VedDB Core - Shared memory primitives and data structures
//!
//! This crate provides the foundational components for VedDB:
//! - Shared memory management
//! - Ring buffer implementations (SPSC and MPMC)
//! - Arena allocation
//! - Command protocol definitions

pub mod arena;
pub mod cache;
pub mod core;
pub mod document;
pub mod kv;
pub mod memory;
pub mod protocol;
pub mod pubsub;
pub mod ring;
pub mod schema;
pub mod session;
pub mod simple_kv;
pub mod snapshot;
pub mod storage;
pub mod wal;

pub use arena::*;
pub use cache::*;
pub use core::*;
pub use document::*;
pub use memory::*;
pub use protocol::*;
pub use ring::*;
pub use schema::*;
pub use simple_kv::SimpleKvStore;
pub use storage::*;
