//! Write-Ahead Log (WAL) implementation for VedDB v0.2.0
//!
//! This module provides durability through write-ahead logging with:
//! - Binary serialization with CRC32 checksums
//! - Configurable fsync policies
//! - WAL file rotation and compaction
//! - Crash recovery through WAL replay

pub mod entry;
pub mod writer;
pub mod reader;
pub mod replay;

pub use entry::*;
pub use writer::*;
pub use reader::*;
pub use replay::*;
