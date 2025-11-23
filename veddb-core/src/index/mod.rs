//! Index management for VedDB v0.2.0
//!
//! This module provides B-tree based indexing with support for:
//! - Single field indexes
//! - Compound indexes
//! - Unique indexes
//! - Background index building

pub mod btree;
pub mod manager;
pub mod builder;
pub mod statistics;

pub use btree::{BTreeIndex, IndexEntry, IndexKey};
pub use manager::IndexManager;
pub use builder::IndexBuilder;
pub use statistics::IndexStatistics;