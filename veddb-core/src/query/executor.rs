//! Query executor for executing queries against collections
//!
//! Executes queries with filtering, projection, sorting, skip, and limit

use super::ast::{Filter, Projection, Query, Sort, SortOrder};
use super::planner::{QueryPlanner, QueryPlanError};
use crate::document::{Document, Value};
use crate::index::manager::IndexManager; // Import IndexManager
use regex::Regex;
use std::cmp::Ordering as CmpOrdering;
use std::sync::Arc;

/// Query executor
pub struct QueryExecutor {
    planner: QueryPlanner,
    index_manager: Option<Arc<IndexManager>>,
}

impl QueryExecutor {
    /// Create a new query executor
    pub fn new() -> Self {
        Self {
            planner: QueryPlanner::new(),
            index_manager: None,
        }
    }

    /// Set index manager for index scan optimization
    pub fn set_index_manager(&mut self, index_manager: Arc<IndexManager>) {
        self.index_manager = Some(index_manager);
    }

    /// Execute a query against a collection
    pub fn execute(
        &self,
        documents: Vec<Document>,
        query: &Query,
    ) -> Result<Vec<Document>, QueryExecutionError> {
        // Create query plan
        let plan = self.planner.create_plan(query)?;

        // Execute based on plan
        let mut results = if plan.use_index.is_some() {
            // Index scan (to be implemented with actual indexes in task 4.3)
            self.execute_index_scan(documents, query)?
        } else {
            // Collection scan
            self.execute_collection_scan(documents, query)?
        };

        // Apply post-processing
        results = self.apply_post_processing(results, query)?;

        Ok(results)
    }

    /// Execute a collection scan
    fn execute_collection_scan(
        &self,
        documents: Vec<Document>,
        query: &Query,
    ) -> Result<Vec<Document>, QueryExecutionError> {
        let mut results = Vec::new();

        for doc in documents {
            if self.matches_filter(&doc, &query.filter)? {
                results.push(doc);
            }
        }

        Ok(results)
    }

    /// Execute an index scan using IndexManager
    fn execute_index_scan(
        &self,
        documents: Vec<Document>,
        query: &Query,
    ) -> Result<Vec<Document>, QueryExecutionError> {
        use crate::index::btree::IndexKey;
        use std::collections::HashMap;

        // If no index manager, fall back to collection scan
        let index_manager = match &self.index_manager {
            Some(mgr) => mgr,
            None => return self.execute_collection_scan(documents, query),
        };

        // Get execution plan to determine which index to use
        let plan = self.planner.create_plan(query)?;
        let index_name = match &plan.use_index {
            Some(name) => name,
            None => return self.execute_collection_scan(documents, query),
        };

        // Build document map for fast lookup by ID
        let doc_map: HashMap<crate::document::DocumentId, &Document> = documents
            .iter()
            .map(|d| (d.id, d))
            .collect();

        // Get document IDs from index based on filter type
        let doc_ids = self.get_doc_ids_from_index(index_manager, index_name, &query.filter)?;

        // Fetch documents by ID
        let mut results: Vec<Document> = doc_ids
            .into_iter()
            .filter_map(|doc_id| doc_map.get(&doc_id).map(|d| (*d).clone()))
            .collect();

        // Apply remaining filters (in case of complex AND/OR conditions)
        results.retain(|doc| self.matches_filter(doc, &query.filter).unwrap_or(false));

        Ok(results)
    }

    /// Get document IDs from index based on filter
    fn get_doc_ids_from_index(
        &self,
        index_manager: &IndexManager,
        index_name: &str,
        filter: &Filter,
    ) -> Result<Vec<crate::document::DocumentId>, QueryExecutionError> {
        use crate::index::btree::IndexKey;

        match filter {
            // Exact match
            Filter::Eq { field: _, value } => {
                let key = IndexKey::from_values(vec![value.clone()])
                    .map_err(|e| QueryExecutionError::ExecutionError(format!("Index key error: {}", e)))?;
                
                index_manager.find_with_index(index_name, &key)
                    .map_err(|e| QueryExecutionError::ExecutionError(format!("Index lookup error: {}", e)))
            }

            // Greater than
            Filter::Gt { field: _, value } => {
                let start_key = IndexKey::from_values(vec![value.clone()])
                    .map_err(|e| QueryExecutionError::ExecutionError(format!("Index key error: {}", e)))?;
                
                index_manager.find_range_with_index(
                    index_name,
                    Some(&start_key),
                    None,
                    false, // exclude start
                    false,
                )
                .map_err(|e| QueryExecutionError::ExecutionError(format!("Index range error: {}", e)))
            }

            // Greater than or equal
            Filter::Gte { field: _, value } => {
                let start_key = IndexKey::from_values(vec![value.clone()])
                    .map_err(|e| QueryExecutionError::ExecutionError(format!("Index key error: {}", e)))?;
                
                index_manager.find_range_with_index(
                    index_name,
                    Some(&start_key),
                    None,
                    true, // include start
                    false,
                )
                .map_err(|e| QueryExecutionError::ExecutionError(format!("Index range error: {}", e)))
            }

            // Less than
            Filter::Lt { field: _, value } => {
                let end_key = IndexKey::from_values(vec![value.clone()])
                    .map_err(|e| QueryExecutionError::ExecutionError(format!("Index key error: {}", e)))?;
                
                index_manager.find_range_with_index(
                    index_name,
                    None,
                    Some(&end_key),
                    false,
                    false, // exclude end
                )
                .map_err(|e| QueryExecutionError::ExecutionError(format!("Index range error: {}", e)))
            }

            // Less than or equal
            Filter::Lte { field: _, value } => {
                let end_key = IndexKey::from_values(vec![value.clone()])
                    .map_err(|e| QueryExecutionError::ExecutionError(format!("Index key error: {}", e)))?;
                
                index_manager.find_range_with_index(
                    index_name,
                    None,
                    Some(&end_key),
                    false,
                    true, // include end
                )
                .map_err(|e| QueryExecutionError::ExecutionError(format!("Index range error: {}", e)))
            }

            // AND: Extract first indexable sub-filter
            Filter::And(filters) => {
                for f in filters {
                    if matches!(f, Filter::Eq { .. } | Filter::Gt { .. } | Filter::Gte { .. } | Filter::Lt { .. } | Filter::Lte { .. }) {
                        return self.get_doc_ids_from_index(index_manager, index_name, f);
                    }
                }
                // No indexable sub-filter, return empty
                Ok(Vec::new())
            }

            // Other filters: not optimized with index
            _ => Ok(Vec::new()),
        }
    }

    /// Check if a document matches a filter
    pub fn matches_filter(&self, doc: &Document, filter: &Filter) -> Result<bool, QueryExecutionError> {
        match filter {
            Filter::Empty => Ok(true),
            
            Filter::Eq { field, value } => {
                let doc_value = doc.get_by_path(field);
                Ok(doc_value == Some(value))
            }
            
            Filter::Ne { field, value } => {
                let doc_value = doc.get_by_path(field);
                Ok(doc_value != Some(value))
            }
            
            Filter::Gt { field, value } => {
                let doc_value = doc.get_by_path(field);
                Ok(doc_value.map(|v| self.compare_values(v, value) == CmpOrdering::Greater).unwrap_or(false))
            }
            
            Filter::Gte { field, value } => {
                let doc_value = doc.get_by_path(field);
                Ok(doc_value.map(|v| {
                    let cmp = self.compare_values(v, value);
                    cmp == CmpOrdering::Greater || cmp == CmpOrdering::Equal
                }).unwrap_or(false))
            }
            
            Filter::Lt { field, value } => {
                let doc_value = doc.get_by_path(field);
                Ok(doc_value.map(|v| self.compare_values(v, value) == CmpOrdering::Less).unwrap_or(false))
            }
            
            Filter::Lte { field, value } => {
                let doc_value = doc.get_by_path(field);
                Ok(doc_value.map(|v| {
                    let cmp = self.compare_values(v, value);
                    cmp == CmpOrdering::Less || cmp == CmpOrdering::Equal
                }).unwrap_or(false))
            }
            
            Filter::In { field, values } => {
                let doc_value = doc.get_by_path(field);
                Ok(doc_value.map(|v| values.contains(v)).unwrap_or(false))
            }
            
            Filter::Nin { field, values } => {
                let doc_value = doc.get_by_path(field);
                Ok(doc_value.map(|v| !values.contains(v)).unwrap_or(true))
            }
            
            Filter::Exists { field, exists } => {
                let has_field = doc.get_by_path(field).is_some();
                Ok(has_field == *exists)
            }
            
            Filter::Regex { field, pattern, options } => {
                let doc_value = doc.get_by_path(field);
                if let Some(Value::String(s)) = doc_value {
                    let regex = if let Some(opts) = options {
                        // Parse regex options (i = case insensitive, m = multiline, etc.)
                        let case_insensitive = opts.contains('i');
                        if case_insensitive {
                            Regex::new(&format!("(?i){}", pattern))
                        } else {
                            Regex::new(pattern)
                        }
                    } else {
                        Regex::new(pattern)
                    };
                    
                    match regex {
                        Ok(re) => Ok(re.is_match(s)),
                        Err(e) => Err(QueryExecutionError::InvalidRegex(e.to_string())),
                    }
                } else {
                    Ok(false)
                }
            }
            
            Filter::And(filters) => {
                for f in filters {
                    if !self.matches_filter(doc, f)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            
            Filter::Or(filters) => {
                for f in filters {
                    if self.matches_filter(doc, f)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            
            Filter::Not(filter) => {
                Ok(!self.matches_filter(doc, filter)?)
            }
        }
    }

    /// Compare two values
    fn compare_values(&self, a: &Value, b: &Value) -> CmpOrdering {
        match (a, b) {
            (Value::Null, Value::Null) => CmpOrdering::Equal,
            (Value::Null, _) => CmpOrdering::Less,
            (_, Value::Null) => CmpOrdering::Greater,
            
            (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
            
            (Value::Int32(a), Value::Int32(b)) => a.cmp(b),
            (Value::Int64(a), Value::Int64(b)) => a.cmp(b),
            (Value::Int32(a), Value::Int64(b)) => (*a as i64).cmp(b),
            (Value::Int64(a), Value::Int32(b)) => a.cmp(&(*b as i64)),
            
            (Value::Float64(a), Value::Float64(b)) => {
                a.partial_cmp(b).unwrap_or(CmpOrdering::Equal)
            }
            (Value::Int32(a), Value::Float64(b)) => {
                (*a as f64).partial_cmp(b).unwrap_or(CmpOrdering::Equal)
            }
            (Value::Int64(a), Value::Float64(b)) => {
                (*a as f64).partial_cmp(b).unwrap_or(CmpOrdering::Equal)
            }
            (Value::Float64(a), Value::Int32(b)) => {
                a.partial_cmp(&(*b as f64)).unwrap_or(CmpOrdering::Equal)
            }
            (Value::Float64(a), Value::Int64(b)) => {
                a.partial_cmp(&(*b as f64)).unwrap_or(CmpOrdering::Equal)
            }
            
            (Value::String(a), Value::String(b)) => a.cmp(b),
            
            (Value::DateTime(a), Value::DateTime(b)) => a.cmp(b),
            
            (Value::ObjectId(a), Value::ObjectId(b)) => a.cmp(b),
            
            _ => CmpOrdering::Equal,
        }
    }

    /// Apply post-processing (projection, sort, skip, limit)
    fn apply_post_processing(
        &self,
        mut documents: Vec<Document>,
        query: &Query,
    ) -> Result<Vec<Document>, QueryExecutionError> {
        // Apply sort
        if let Some(ref sort) = query.sort {
            self.apply_sort(&mut documents, sort)?;
        }

        // Apply skip
        if let Some(skip) = query.skip {
            if skip > 0 {
                documents = documents.into_iter().skip(skip as usize).collect();
            }
        }

        // Apply limit
        if let Some(limit) = query.limit {
            documents.truncate(limit as usize);
        }

        // Apply projection
        if let Some(ref projection) = query.projection {
            documents = self.apply_projection(documents, projection)?;
        }

        Ok(documents)
    }

    /// Apply sorting to documents
    fn apply_sort(
        &self,
        documents: &mut Vec<Document>,
        sort: &Sort,
    ) -> Result<(), QueryExecutionError> {
        documents.sort_by(|a, b| {
            for (field, order) in &sort.fields {
                let a_val = a.get_by_path(field);
                let b_val = b.get_by_path(field);

                let cmp = match (a_val, b_val) {
                    (Some(av), Some(bv)) => self.compare_values(av, bv),
                    (Some(_), None) => CmpOrdering::Greater,
                    (None, Some(_)) => CmpOrdering::Less,
                    (None, None) => CmpOrdering::Equal,
                };

                let cmp = match order {
                    SortOrder::Ascending => cmp,
                    SortOrder::Descending => cmp.reverse(),
                };

                if cmp != CmpOrdering::Equal {
                    return cmp;
                }
            }
            CmpOrdering::Equal
        });

        Ok(())
    }

    /// Apply projection to documents
    fn apply_projection(
        &self,
        documents: Vec<Document>,
        projection: &Projection,
    ) -> Result<Vec<Document>, QueryExecutionError> {
        let mut result = Vec::new();

        for doc in documents {
            let mut new_doc = Document::with_id(doc.id);

            for (field, value) in &doc.fields {
                if projection.should_include(field) {
                    new_doc.insert(field.clone(), value.clone());
                }
            }

            result.push(new_doc);
        }

        Ok(result)
    }
}

impl Default for QueryExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Query execution errors
#[derive(Debug, thiserror::Error)]
pub enum QueryExecutionError {
    #[error("Invalid regex pattern: {0}")]
    InvalidRegex(String),

    #[error("Query planning error: {0}")]
    PlanningError(#[from] QueryPlanError),

    #[error("Execution error: {0}")]
    ExecutionError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::DocumentId;

    fn create_test_documents() -> Vec<Document> {
        let mut docs = Vec::new();

        for i in 0..10 {
            let mut doc = Document::with_id(DocumentId::new());
            doc.insert("name".to_string(), Value::String(format!("User{}", i)));
            doc.insert("age".to_string(), Value::Int32(20 + i));
            doc.insert("active".to_string(), Value::Bool(i % 2 == 0));
            docs.push(doc);
        }

        docs
    }

    #[test]
    fn test_execute_empty_filter() {
        let executor = QueryExecutor::new();
        let docs = create_test_documents();
        let query = Query::new();

        let results = executor.execute(docs.clone(), &query).unwrap();
        assert_eq!(results.len(), 10);
    }

    #[test]
    fn test_execute_eq_filter() {
        let executor = QueryExecutor::new();
        let docs = create_test_documents();
        let query = Query::with_filter(Filter::eq("name", "User5"));

        let results = executor.execute(docs, &query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].get("name").unwrap().as_str(), Some("User5"));
    }

    #[test]
    fn test_execute_gt_filter() {
        let executor = QueryExecutor::new();
        let docs = create_test_documents();
        let query = Query::with_filter(Filter::gt("age", 25i32));

        let results = executor.execute(docs, &query).unwrap();
        assert_eq!(results.len(), 4); // ages 26, 27, 28, 29
    }

    #[test]
    fn test_execute_in_filter() {
        let executor = QueryExecutor::new();
        let docs = create_test_documents();
        let query = Query::with_filter(Filter::in_values(
            "name",
            vec![
                Value::String("User1".to_string()),
                Value::String("User3".to_string()),
                Value::String("User5".to_string()),
            ],
        ));

        let results = executor.execute(docs, &query).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_execute_exists_filter() {
        let executor = QueryExecutor::new();
        let docs = create_test_documents();
        let query = Query::with_filter(Filter::exists("age", true));

        let results = executor.execute(docs, &query).unwrap();
        assert_eq!(results.len(), 10);

        let query_not_exists = Query::with_filter(Filter::exists("email", true));
        let results = executor.execute(create_test_documents(), &query_not_exists).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_execute_and_filter() {
        let executor = QueryExecutor::new();
        let docs = create_test_documents();
        let query = Query::with_filter(Filter::and(vec![
            Filter::gt("age", 23i32),
            Filter::eq("active", true),
        ]));

        let results = executor.execute(docs, &query).unwrap();
        // ages 24, 26, 28 (even numbers > 23)
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_execute_or_filter() {
        let executor = QueryExecutor::new();
        let docs = create_test_documents();
        let query = Query::with_filter(Filter::or(vec![
            Filter::eq("name", "User1"),
            Filter::eq("name", "User2"),
        ]));

        let results = executor.execute(docs, &query).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_execute_not_filter() {
        let executor = QueryExecutor::new();
        let docs = create_test_documents();
        let query = Query::with_filter(Filter::not(Filter::eq("active", true)));

        let results = executor.execute(docs, &query).unwrap();
        assert_eq!(results.len(), 5); // odd numbers
    }

    #[test]
    fn test_execute_with_sort() {
        let executor = QueryExecutor::new();
        let docs = create_test_documents();
        let query = Query::new().sort(Sort::new().desc("age"));

        let results = executor.execute(docs, &query).unwrap();
        assert_eq!(results.len(), 10);
        assert_eq!(results[0].get("age").unwrap().as_i64(), Some(29));
        assert_eq!(results[9].get("age").unwrap().as_i64(), Some(20));
    }

    #[test]
    fn test_execute_with_skip_limit() {
        let executor = QueryExecutor::new();
        let docs = create_test_documents();
        let query = Query::new()
            .sort(Sort::new().asc("age"))
            .skip(3)
            .limit(4);

        let results = executor.execute(docs, &query).unwrap();
        assert_eq!(results.len(), 4);
        assert_eq!(results[0].get("age").unwrap().as_i64(), Some(23));
        assert_eq!(results[3].get("age").unwrap().as_i64(), Some(26));
    }

    #[test]
    fn test_execute_with_projection() {
        let executor = QueryExecutor::new();
        let docs = create_test_documents();
        let query = Query::new().projection(
            Projection::new().include("name").include("age")
        );

        let results = executor.execute(docs, &query).unwrap();
        assert_eq!(results.len(), 10);
        assert!(results[0].contains_key("name"));
        assert!(results[0].contains_key("age"));
        assert!(!results[0].contains_key("active"));
    }

    #[test]
    fn test_execute_complex_query() {
        let executor = QueryExecutor::new();
        let docs = create_test_documents();
        let query = Query::with_filter(Filter::and(vec![
            Filter::gte("age", 22i32),
            Filter::lte("age", 27i32),
            Filter::eq("active", true),
        ]))
        .projection(Projection::new().include("name").include("age"))
        .sort(Sort::new().desc("age"))
        .limit(2);

        let results = executor.execute(docs, &query).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].get("age").unwrap().as_i64(), Some(26));
        assert_eq!(results[1].get("age").unwrap().as_i64(), Some(24));
    }

    #[test]
    fn test_regex_filter() {
        let executor = QueryExecutor::new();
        let docs = create_test_documents();
        let query = Query::with_filter(Filter::regex("name", r"^User[0-4]$"));

        let results = executor.execute(docs, &query).unwrap();
        assert_eq!(results.len(), 5);
    }
}

