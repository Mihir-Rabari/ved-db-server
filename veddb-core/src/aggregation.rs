//! Aggregation Pipeline for VedDB v0.2.0
//!
//! Provides MongoDB-style aggregation with pipeline operators:
//! - $match: Filter documents
//! - $project: Select/transform fields
//! - $sort: Order results
//! - $limit: Limit results  
//! - $skip: Skip documents
//! - $group: Group and aggregate

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Pipeline stage in aggregation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "$")]
pub enum PipelineStage {
    #[serde(rename = "match")]
    Match {
        filter: serde_json::Value,
    },
    
    #[serde(rename = "project")]
    Project {
        fields: HashMap<String, i32>,
    },
    
    #[serde(rename = "sort")]
    Sort {
        fields: HashMap<String, i32>, // 1 for asc, -1 for desc
    },
    
    #[serde(rename = "limit")]
    Limit {
        count: usize,
    },
    
    #[serde(rename = "skip")]
    Skip {
        count: usize,
    },
    
    #[serde(rename = "group")]
    Group {
        _id: serde_json::Value,
        fields: HashMap<String, AggregateOp>,
    },
}

/// Aggregate operations for $group
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "$")]
pub enum AggregateOp {
    #[serde(rename = "sum")]
    Sum(serde_json::Value),
    
    #[serde(rename = "count")]
    Count {},
    
    #[serde(rename = "avg")]
    Avg(String),
    
    #[serde(rename = "min")]
    Min(String),
    
    #[serde(rename = "max")]
    Max(String),
}

/// Aggregation pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    pub stages: Vec<PipelineStage>,
}

impl Pipeline {
    pub fn new(stages: Vec<PipelineStage>) -> Self {
        Self { stages }
    }
}

/// Aggregation error
#[derive(Debug, thiserror::Error)]
pub enum AggregationError {
    #[error("Invalid stage: {0}")]
    InvalidStage(String),
    
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    
    #[error("Execution error: {0}")]
    ExecutionError(String),
}
