//! Query planner for optimizing query execution
//!
//! Creates execution plans with cost estimation and index selection

use super::ast::{Filter, Query};
use super::index_selector::IndexSelector;

/// Query planner for creating optimized execution plans
pub struct QueryPlanner {
    index_selector: IndexSelector,
}

impl QueryPlanner {
    /// Create a new query planner
    pub fn new() -> Self {
        Self {
            index_selector: IndexSelector::new(),
        }
    }

    /// Create an execution plan for a query
    pub fn create_plan(&self, query: &Query) -> Result<QueryPlan, QueryPlanError> {
        let mut plan = QueryPlan::new();

        // Analyze filter for index usage
        let index_candidate = self.analyze_filter_for_index(&query.filter)?;
        plan.use_index = index_candidate;

        // Estimate costs
        plan.estimated_cost = self.estimate_cost(query, &plan)?;

        // Determine execution strategy
        plan.execution_strategy = if plan.use_index.is_some() {
            ExecutionStrategy::IndexScan
        } else {
            ExecutionStrategy::CollectionScan
        };

        // Plan post-processing steps
        plan.needs_sort = query.sort.is_some();
        plan.needs_projection = query.projection.is_some();
        plan.has_skip = query.skip.is_some() && query.skip.unwrap() > 0;
        plan.has_limit = query.limit.is_some();

        Ok(plan)
    }

    /// Analyze filter to determine if an index can be used
    fn analyze_filter_for_index(&self, filter: &Filter) -> Result<Option<String>, QueryPlanError> {
        match filter {
            Filter::Empty => Ok(None),
            
            // Single field filters that can use indexes
            Filter::Eq { field, .. } |
            Filter::Gt { field, .. } |
            Filter::Gte { field, .. } |
            Filter::Lt { field, .. } |
            Filter::Lte { field, .. } |
            Filter::In { field, .. } => {
                // For now, assume we have an index on any field
                // In task 4.3, this will check actual available indexes
                Ok(Some(format!("idx_{}", field)))
            }
            
            // Range queries can use indexes
            Filter::And(filters) => {
                // Look for range queries on the same field
                let mut field_filters: std::collections::HashMap<String, Vec<&Filter>> = 
                    std::collections::HashMap::new();
                
                for f in filters {
                    if let Some(field) = self.get_filter_field(f) {
                        field_filters.entry(field).or_insert_with(Vec::new).push(f);
                    }
                }
                
                // Find the best field for index usage
                for (field, field_filters) in field_filters {
                    if field_filters.len() >= 1 {
                        return Ok(Some(format!("idx_{}", field)));
                    }
                }
                
                Ok(None)
            }
            
            _ => Ok(None),
        }
    }

    /// Get the field name from a filter if it's a single-field filter
    fn get_filter_field(&self, filter: &Filter) -> Option<String> {
        match filter {
            Filter::Eq { field, .. } |
            Filter::Ne { field, .. } |
            Filter::Gt { field, .. } |
            Filter::Gte { field, .. } |
            Filter::Lt { field, .. } |
            Filter::Lte { field, .. } |
            Filter::In { field, .. } |
            Filter::Nin { field, .. } |
            Filter::Exists { field, .. } |
            Filter::Regex { field, .. } => Some(field.clone()),
            _ => None,
        }
    }

    /// Estimate the cost of executing a query
    fn estimate_cost(&self, query: &Query, plan: &QueryPlan) -> Result<f64, QueryPlanError> {
        let mut cost = 0.0;

        // Base scan cost
        cost += if plan.use_index.is_some() {
            // Index scan cost (estimated)
            100.0
        } else {
            // Collection scan cost (higher)
            1000.0
        };

        // Filter complexity cost
        cost += self.estimate_filter_cost(&query.filter) * 10.0;

        // Sort cost
        if query.sort.is_some() {
            cost += if plan.use_index.is_some() {
                // Index might provide sorted results
                50.0
            } else {
                // Need to sort in memory
                500.0
            };
        }

        // Projection cost (minimal)
        if query.projection.is_some() {
            cost += 10.0;
        }

        Ok(cost)
    }

    /// Estimate the cost of a filter
    fn estimate_filter_cost(&self, filter: &Filter) -> f64 {
        match filter {
            Filter::Empty => 0.0,
            Filter::Eq { .. } => 1.0,
            Filter::Ne { .. } => 1.5,
            Filter::Gt { .. } | Filter::Gte { .. } | Filter::Lt { .. } | Filter::Lte { .. } => 2.0,
            Filter::In { values, .. } => values.len() as f64 * 0.5,
            Filter::Nin { values, .. } => values.len() as f64 * 0.7,
            Filter::Exists { .. } => 1.0,
            Filter::Regex { .. } => 10.0, // Regex is expensive
            Filter::And(filters) => filters.iter().map(|f| self.estimate_filter_cost(f)).sum(),
            Filter::Or(filters) => filters.iter().map(|f| self.estimate_filter_cost(f)).sum::<f64>() * 1.5,
            Filter::Not(filter) => self.estimate_filter_cost(filter) * 1.2,
        }
    }
}

impl Default for QueryPlanner {
    fn default() -> Self {
        Self::new()
    }
}

/// Query execution plan
#[derive(Debug, Clone)]
pub struct QueryPlan {
    /// Index to use (if any)
    pub use_index: Option<String>,
    /// Execution strategy
    pub execution_strategy: ExecutionStrategy,
    /// Estimated cost
    pub estimated_cost: f64,
    /// Whether sorting is needed
    pub needs_sort: bool,
    /// Whether projection is needed
    pub needs_projection: bool,
    /// Whether skip is used
    pub has_skip: bool,
    /// Whether limit is used
    pub has_limit: bool,
}

impl QueryPlan {
    /// Create a new empty query plan
    pub fn new() -> Self {
        Self {
            use_index: None,
            execution_strategy: ExecutionStrategy::CollectionScan,
            estimated_cost: 0.0,
            needs_sort: false,
            needs_projection: false,
            has_skip: false,
            has_limit: false,
        }
    }
}

impl Default for QueryPlan {
    fn default() -> Self {
        Self::new()
    }
}

/// Execution strategy
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionStrategy {
    /// Scan the entire collection
    CollectionScan,
    /// Use an index for scanning
    IndexScan,
}

/// Query planning errors
#[derive(Debug, thiserror::Error)]
pub enum QueryPlanError {
    #[error("Planning error: {0}")]
    PlanningError(String),

    #[error("Invalid query structure: {0}")]
    InvalidQuery(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Value;

    #[test]
    fn test_create_plan_empty_query() {
        let planner = QueryPlanner::new();
        let query = Query::new();
        
        let plan = planner.create_plan(&query).unwrap();
        assert_eq!(plan.execution_strategy, ExecutionStrategy::CollectionScan);
        assert!(plan.use_index.is_none());
        assert!(!plan.needs_sort);
        assert!(!plan.needs_projection);
    }

    #[test]
    fn test_create_plan_with_equality_filter() {
        let planner = QueryPlanner::new();
        let query = Query::with_filter(Filter::eq("name", "John"));
        
        let plan = planner.create_plan(&query).unwrap();
        assert_eq!(plan.execution_strategy, ExecutionStrategy::IndexScan);
        assert_eq!(plan.use_index, Some("idx_name".to_string()));
    }

    #[test]
    fn test_create_plan_with_range_filter() {
        let planner = QueryPlanner::new();
        let query = Query::with_filter(Filter::and(vec![
            Filter::gte("age", 18i32),
            Filter::lte("age", 65i32),
        ]));
        
        let plan = planner.create_plan(&query).unwrap();
        assert_eq!(plan.execution_strategy, ExecutionStrategy::IndexScan);
        assert_eq!(plan.use_index, Some("idx_age".to_string()));
    }

    #[test]
    fn test_create_plan_with_sort() {
        let planner = QueryPlanner::new();
        let query = Query::new().sort(crate::query::ast::Sort::new().asc("name"));
        
        let plan = planner.create_plan(&query).unwrap();
        assert!(plan.needs_sort);
    }

    #[test]
    fn test_estimate_filter_cost() {
        let planner = QueryPlanner::new();
        
        let eq_cost = planner.estimate_filter_cost(&Filter::eq("name", "John"));
        assert_eq!(eq_cost, 1.0);
        
        let regex_cost = planner.estimate_filter_cost(&Filter::regex("email", r".*@example\.com"));
        assert_eq!(regex_cost, 10.0);
        
        let and_cost = planner.estimate_filter_cost(&Filter::and(vec![
            Filter::eq("name", "John"),
            Filter::gt("age", 18i32),
        ]));
        assert_eq!(and_cost, 3.0); // 1.0 + 2.0
    }

    #[test]
    fn test_estimate_cost_with_index() {
        let planner = QueryPlanner::new();
        let query = Query::with_filter(Filter::eq("name", "John"));
        
        let plan = planner.create_plan(&query).unwrap();
        assert!(plan.estimated_cost > 0.0);
        assert!(plan.estimated_cost < 200.0); // Should be relatively low with index
    }

    #[test]
    fn test_estimate_cost_without_index() {
        let planner = QueryPlanner::new();
        let query = Query::with_filter(Filter::regex("description", "complex.*pattern"));
        
        let plan = planner.create_plan(&query).unwrap();
        assert!(plan.estimated_cost > 1000.0); // Should be high without index
    }
}
