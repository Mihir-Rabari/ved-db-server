//! Query engine for VedDB v0.2.0
//!
//! This module provides MongoDB-compatible query parsing and execution

pub mod ast;
pub mod parser;
pub mod executor;
pub mod planner;
pub mod index_selector;

pub use ast::{Query, Filter, Projection, Sort, SortOrder};
pub use parser::QueryParser;
pub use executor::QueryExecutor;
pub use planner::QueryPlanner;
pub use index_selector::IndexSelector;
