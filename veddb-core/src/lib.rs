//! VedDB Core - Shared memory primitives and data structures
//!
//! This crate provides the foundational components for VedDB:
//! - Shared memory management
//! - Ring buffer implementations (SPSC and MPMC)
//! - Arena allocation
//! - Command protocol definitions

pub mod arena;
pub mod auth;
pub mod backup;
pub mod cache;
pub mod config;
pub mod core;
pub mod document;
pub mod encryption;
pub mod index;
pub mod kv;
pub mod memory;
pub mod monitoring;
pub mod protocol;
pub mod pubsub;
pub mod query;
pub mod replication;
pub mod ring;
pub mod schema;
pub mod session;
pub mod simple_kv;
pub mod snapshot;
pub mod storage;
pub mod wal;

pub use arena::*;
pub use auth::*;
pub use backup::*;
pub use cache::*;
pub use config::*;
pub use core::*;
pub use document::*;
pub use encryption::*;
pub use index::*;
pub use memory::*;
pub use monitoring::*;
pub use protocol::*;
pub use pubsub::*;
pub use query::*;
pub use replication::*;
pub use ring::*;
pub use schema::*;
pub use simple_kv::SimpleKvStore;
pub use snapshot::*;
pub use storage::*;
pub use wal::*;
