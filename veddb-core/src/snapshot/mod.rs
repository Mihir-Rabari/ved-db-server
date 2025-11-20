//! Snapshot creation and loading for VedDB v0.2.0
//!
//! This module provides point-in-time snapshots of the database state:
//! - Snapshot file format with header, metadata, collections, footer
//! - SnapshotWriter for creating snapshots
//! - SnapshotReader for loading snapshots
//! - Periodic snapshot creation
//! - Manual snapshot trigger

pub mod format;
pub mod writer;
pub mod reader;

pub use format::*;
pub use writer::*;
pub use reader::*;
