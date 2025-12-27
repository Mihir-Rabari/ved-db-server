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
    
    /// Execute the aggregation pipeline on a collection of documents
    /// This implementation uses streaming to maintain bounded memory usage
    /// as required by the memory safety guardrail.
    pub fn execute(&self, documents: Vec<crate::document::Document>) ->  Result<Vec<crate::document::Document>, AggregationError>
    {
        // Start with document iterator
        let mut current: Box<dyn Iterator<Item = crate::document::Document>> = Box::new(documents.into_iter());
        
        // Apply each stage in sequence
        for stage in &self.stages {
            current = match stage {
                PipelineStage::Match { filter } => {
                    Box::new(Self::apply_match(current, filter.clone())?)
                }
                PipelineStage::Project { fields } => {
                    Box::new(Self::apply_project(current, fields.clone()))
                }
                PipelineStage::Sort { fields } => {
                    // Note: Sort requires materialization but we limit it with MAX_SORT_DOCS
                    Box::new(Self::apply_sort(current, fields.clone())?)
                }
                PipelineStage::Limit { count } => {
                    Box::new(current.take(*count))
                }
                PipelineStage::Skip { count } => {
                    Box::new(current.skip(*count))
                }
                PipelineStage::Group { _id, fields } => {
                    // Group requires materialization but we enforce MAX_GROUP_SIZE
                    Box::new(Self::apply_group(current, _id.clone(), fields.clone())?)
                }
            };
        }
        
        // Collect results with memory limit
        const MAX_RESULT_DOCS: usize = 100_000; // 100k documents max in memory
        let results: Vec<_> = current.take(MAX_RESULT_DOCS).collect();
        
        Ok(results)
    }
    
    /// Apply $match stage - filter documents
    fn apply_match(
        docs: Box<dyn Iterator<Item = crate::document::Document>>,
        filter: serde_json::Value,
    ) -> Result<impl Iterator<Item = crate::document::Document>, AggregationError> {
        // Convert serde_json::Value filter to crate::document::Value
        let filter_doc = Self::json_to_doc_value(&filter);
        
        Ok(docs.filter(move |doc| {
            Self::matches_filter(doc, &filter_doc)
        }))
    }
    
    /// Apply $project stage - select/transform fields
    fn apply_project(
        docs: Box<dyn Iterator<Item = crate::document::Document>>,
        fields: std::collections::HashMap<String, i32>,
    ) -> impl Iterator<Item = crate::document::Document> {
        docs.map(move |mut doc| {
            if fields.is_empty() {
                return doc;
            }
            
            let mut projected = crate::document::Document::new();
            
            // Always include _id unless explicitly excluded
            if !fields.contains_key("_id") || *fields.get("_id").unwrap() != 0 {
                if let Some(id_val) = doc.get("_id") {
                    projected.insert("_id".to_string(), id_val.clone());
                }
            }
            
            // Project requested fields
            for (field, include) in &fields {
                if *include != 0 && field != "_id" {
                    if let Some(val) = doc.get(field) {
                        projected.insert(field.clone(), val.clone());
                    }
                }
            }
            
            projected
        })
    }
    
    /// Apply $sort stage - order results
    /// MEMORY SAFETY: Limits sort to MAX_SORT_DOCS to prevent unbounded memory usage
    fn apply_sort(
        docs: Box<dyn Iterator<Item = crate::document::Document>>,
        fields: std::collections::HashMap<String, i32>,
    ) -> Result<impl Iterator<Item = crate::document::Document>, AggregationError> {
        const MAX_SORT_DOCS: usize = 1_000_000; // 1M docs max for sorting
        
        let mut collected: Vec<_> = docs.take(MAX_SORT_DOCS).collect();
        
        collected.sort_by(|a, b| {
            for (field, direction) in &fields {
                let a_val = a.get(field);
                let b_val = b.get(field);
                
                let cmp = Self::compare_values(a_val, b_val);
                let ordered_cmp = if *direction < 0 {
                    cmp.reverse()
                } else {
                    cmp
                };
                
                if ordered_cmp != std::cmp::Ordering::Equal {
                    return ordered_cmp;
                }
            }
            std::cmp::Ordering::Equal
        });
        
        Ok(collected.into_iter())
    }
    
    /// Apply $group stage - group and aggregate
    /// MEMORY SAFETY: Enforces MAX_GROUP_SIZE to prevent unbounded memory usage
    fn apply_group(
        docs: Box<dyn Iterator<Item = crate::document::Document>>,
        group_by: serde_json::Value,
        accumulators: std::collections::HashMap<String, AggregateOp>,
    ) -> Result<impl Iterator<Item = crate::document::Document>, AggregationError> {
        use std::collections::HashMap;
        
        const MAX_GROUP_SIZE: usize = 100_000; // 100k groups max
        
        let mut groups: HashMap<String, GroupAccumulator> = HashMap::new();
        
        for doc in docs {
            // Extract group key
            let group_key = Self::extract_group_key(&doc, &group_by);
            
            // Check group size limit
            if !groups.contains_key(&group_key) && groups.len() >= MAX_GROUP_SIZE {
                return Err(AggregationError::ExecutionError(
                    format!("Group size limit exceeded (max: {})", MAX_GROUP_SIZE)
                ));
            }
            
            // Get or create accumulator for this group
            let acc = groups.entry(group_key.clone()).or_insert_with(|| {
                GroupAccumulator::new(group_key.clone())
            });
            
            // Apply accumulators
            for (field, op) in &accumulators {
                acc.accumulate(field, op, &doc)?;
            }
        }
        
        // Convert groups to documents
        let results: Vec<crate::document::Document> = groups.into_iter().map(|(key, acc)| {
            acc.to_document()
        }).collect();
        
        Ok(results.into_iter())
    }
    
    /// Helper: Check if document matches filter
    fn matches_filter(doc: &crate::document::Document, filter: &crate::document::Value) -> bool {
        if let Some(filter_obj) = filter.as_object() {
            for (key, expected) in filter_obj {
                let actual = doc.get(key);
                if !Self::value_matches(actual, expected) {
                    return false;
                }
            }
            true
        } else {
            false
        }
    }
    
    /// Helper: Check if value matches expected value/operator
    fn value_matches(actual: Option<&crate::document::Value>, expected: &crate::document::Value) -> bool {
        match (actual, expected) {
            (Some(a), e) => {
                // Check for operators
                if let Some(obj) = e.as_object() {
                    for (op, val) in obj {
                        match op.as_str() {
                            "$eq" => return a == val,
                            "$ne" => return a != val,
                            "$gt" => return Self::compare_values(Some(a), Some(val)) == std::cmp::Ordering::Greater,
                            "$gte" => {
                                let cmp = Self::compare_values(Some(a), Some(val));
                                return cmp == std::cmp::Ordering::Greater || cmp == std::cmp::Ordering::Equal;
                            }
                            "$lt" => return Self::compare_values(Some(a), Some(val)) == std::cmp::Ordering::Less,
                            "$lte" => {
                                let cmp = Self::compare_values(Some(a), Some(val));
                                return cmp == std::cmp::Ordering::Less || cmp == std::cmp::Ordering::Equal;
                            }
                            "$in" => {
                                if let Some(arr) = val.as_array() {
                                    return arr.contains(a);
                                }
                                return false;
                            }
                            _ => {}
                        }
                    }
                }
                // Direct equality
                a == e
            }
            (None, _) => false,
        }
    }
    
    /// Helper: Compare two values
    fn compare_values(
        a: Option<&crate::document::Value>,
        b: Option<&crate::document::Value>,
    ) -> std::cmp::Ordering {
        use crate::document::Value;
        
        match (a, b) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (Some(Value::Int32(a)), Some(Value::Int32(b))) => a.cmp(b),
            (Some(Value::Int64(a)), Some(Value::Int64(b))) => a.cmp(b),
            (Some(Value::Float64(a)), Some(Value::Float64(b))) => {
                a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
            }
            (Some(Value::String(a)), Some(Value::String(b))) => a.cmp(b),
            (Some(Value::Bool(a)), Some(Value::Bool(b))) => a.cmp(b),
            _ => std::cmp::Ordering::Equal,
        }
    }
    
    /// Helper: Extract group key from document
    fn extract_group_key(doc: &crate::document::Document, group_by: &serde_json::Value) -> String {
        if let Some(s) = group_by.as_str() {
            // Simple field reference like "_id": "$category"
            if let Some(field) = s.strip_prefix('$') {
                if let Some(val) = doc.get(field) {
                    return format!("{:?}", val);
                }
            }
            return s.to_string();
        }
        format!("{:?}", group_by)
    }
    
    /// Helper: Convert serde_json::Value to crate::document::Value
    fn json_to_doc_value(v: &serde_json::Value) -> crate::document::Value {
        use crate::document::Value;
        use std::collections::BTreeMap;
        
        match v {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Value::Int64(i)
                } else if let Some(f) = n.as_f64() {
                    Value::Float64(f)
                } else {
                    Value::Int64(0)
                }
            }
            serde_json::Value::String(s) => Value::String(s.clone()),
            serde_json::Value::Array(arr) => {
                Value::Array(arr.iter().map(Self::json_to_doc_value).collect())
            }
            serde_json::Value::Object(obj) => {
                let mut map = BTreeMap::new();
                for (k, v) in obj {
                    map.insert(k.clone(), Self::json_to_doc_value(v));
                }
                Value::Object(map)
            }
        }
    }
}

/// Group accumulator for $group stage
struct GroupAccumulator {
    group_key: String,
    fields: std::collections::HashMap<String, AccumulatorState>,
}

impl GroupAccumulator {
    fn new(group_key: String) -> Self {
        Self {
            group_key,
            fields: std::collections::HashMap::new(),
        }
    }
    
    fn accumulate(
        &mut self,
        field: &str,
        op: &AggregateOp,
        doc: &crate::document::Document,
    ) -> Result<(), AggregationError> {
        let state = self.fields.entry(field.to_string()).or_insert_with(|| {
            AccumulatorState::new(op.clone())
        });
        
        state.add(doc)?;
        Ok(())
    }
    
    fn to_document(self) -> crate::document::Document {
        use crate::document::Value;
        
        let mut doc = crate::document::Document::new();
        doc.insert("_id".to_string(), Value::String(self.group_key));
        
        for (field, state) in self.fields {
            doc.insert(field, state.value());
        }
        
        doc
    }
}

/// Accumulator state for aggregation operations
struct AccumulatorState {
    op: AggregateOp,
    sum: f64,
    count: usize,
    min: Option<f64>,
    max: Option<f64>,
}

impl AccumulatorState {
    fn new(op: AggregateOp) -> Self {
        Self {
            op,
            sum: 0.0,
            count: 0,
            min: None,
            max: None,
        }
    }
    
    fn add(&mut self, doc: &crate::document::Document) -> Result<(), AggregationError> {
        use crate::document::Value;
        
        match &self.op {
            AggregateOp::Sum(field_ref) => {
                if let Some(s) = field_ref.as_str() {
                    if let Some(field) = s.strip_prefix('$') {
                        if let Some(val) = doc.get(field) {
                            self.sum += Self::to_number(val);
                        }
                    }
                }
            }
            AggregateOp::Count {} => {
                self.count += 1;
            }
            AggregateOp::Avg(field) => {
                if let Some(val) = doc.get(field) {
                    self.sum += Self::to_number(val);
                    self.count += 1;
                }
            }
            AggregateOp::Min(field) => {
                if let Some(val) = doc.get(field) {
                    let num = Self::to_number(val);
                    self.min = Some(self.min.map_or(num, |m| m.min(num)));
                }
            }
            AggregateOp::Max(field) => {
                if let Some(val) = doc.get(field) {
                    let num = Self::to_number(val);
                    self.max = Some(self.max.map_or(num, |m| m.max(num)));
                }
            }
        }
        
        Ok(())
    }
    
    fn value(self) -> crate::document::Value {
        use crate::document::Value;
        
        match self.op {
            AggregateOp::Sum(_) => Value::Float64(self.sum),
            AggregateOp::Count {} => Value::Int64(self.count as i64),
            AggregateOp::Avg(_) => {
                if self.count > 0 {
                    Value::Float64(self.sum / self.count as f64)
                } else {
                    Value::Float64(0.0)
                }
            }
            AggregateOp::Min(_) => Value::Float64(self.min.unwrap_or(0.0)),
            AggregateOp::Max(_) => Value::Float64(self.max.unwrap_or(0.0)),
        }
    }
    
    fn to_number(val: &crate::document::Value) -> f64 {
        use crate::document::Value;
        
        match val {
            Value::Int32(i) => *i as f64,
            Value::Int64(i) => *i as f64,
            Value::Float64(f) => *f,
            _ => 0.0,
        }
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
