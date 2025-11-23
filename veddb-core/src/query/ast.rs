//! Query Abstract Syntax Tree (AST) definitions
//!
//! Defines the structure for MongoDB-compatible queries

use crate::document::Value;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Query structure with filter, projection, sort, skip, limit
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Query {
    /// Filter conditions
    pub filter: Filter,
    /// Projection (fields to include/exclude)
    pub projection: Option<Projection>,
    /// Sort specification
    pub sort: Option<Sort>,
    /// Number of documents to skip
    pub skip: Option<u64>,
    /// Maximum number of documents to return
    pub limit: Option<u64>,
}

impl Query {
    /// Create a new empty query (matches all documents)
    pub fn new() -> Self {
        Self {
            filter: Filter::Empty,
            projection: None,
            sort: None,
            skip: None,
            limit: None,
        }
    }

    /// Create a query with a filter
    pub fn with_filter(filter: Filter) -> Self {
        Self {
            filter,
            projection: None,
            sort: None,
            skip: None,
            limit: None,
        }
    }

    /// Set projection
    pub fn projection(mut self, projection: Projection) -> Self {
        self.projection = Some(projection);
        self
    }

    /// Set sort
    pub fn sort(mut self, sort: Sort) -> Self {
        self.sort = Some(sort);
        self
    }

    /// Set skip
    pub fn skip(mut self, skip: u64) -> Self {
        self.skip = Some(skip);
        self
    }

    /// Set limit
    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Check if this is a simple key lookup (single equality filter)
    pub fn is_simple_key_lookup(&self) -> bool {
        matches!(self.filter, Filter::Eq { .. })
            && self.projection.is_none()
            && self.sort.is_none()
            && self.skip.is_none()
            && self.limit == Some(1)
    }

    /// Get the field being queried in a simple lookup
    pub fn get_lookup_field(&self) -> Option<&str> {
        match &self.filter {
            Filter::Eq { field, .. } => Some(field.as_str()),
            _ => None,
        }
    }
}

impl Default for Query {
    fn default() -> Self {
        Self::new()
    }
}

/// Filter conditions for queries
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", content = "args")]
pub enum Filter {
    /// Empty filter (matches all documents)
    Empty,

    /// Equality: field == value
    Eq {
        field: String,
        value: Value,
    },

    /// Not equal: field != value
    Ne {
        field: String,
        value: Value,
    },

    /// Greater than: field > value
    Gt {
        field: String,
        value: Value,
    },

    /// Greater than or equal: field >= value
    Gte {
        field: String,
        value: Value,
    },

    /// Less than: field < value
    Lt {
        field: String,
        value: Value,
    },

    /// Less than or equal: field <= value
    Lte {
        field: String,
        value: Value,
    },

    /// In: field in [values]
    In {
        field: String,
        values: Vec<Value>,
    },

    /// Not in: field not in [values]
    Nin {
        field: String,
        values: Vec<Value>,
    },

    /// Exists: field exists (or not)
    Exists {
        field: String,
        exists: bool,
    },

    /// Regex: field matches pattern
    Regex {
        field: String,
        pattern: String,
        options: Option<String>,
    },

    /// Logical AND: all conditions must match
    And(Vec<Filter>),

    /// Logical OR: at least one condition must match
    Or(Vec<Filter>),

    /// Logical NOT: condition must not match
    Not(Box<Filter>),
}

impl Filter {
    /// Create an equality filter
    pub fn eq(field: impl Into<String>, value: impl Into<Value>) -> Self {
        Self::Eq {
            field: field.into(),
            value: value.into(),
        }
    }

    /// Create a not-equal filter
    pub fn ne(field: impl Into<String>, value: impl Into<Value>) -> Self {
        Self::Ne {
            field: field.into(),
            value: value.into(),
        }
    }

    /// Create a greater-than filter
    pub fn gt(field: impl Into<String>, value: impl Into<Value>) -> Self {
        Self::Gt {
            field: field.into(),
            value: value.into(),
        }
    }

    /// Create a greater-than-or-equal filter
    pub fn gte(field: impl Into<String>, value: impl Into<Value>) -> Self {
        Self::Gte {
            field: field.into(),
            value: value.into(),
        }
    }

    /// Create a less-than filter
    pub fn lt(field: impl Into<String>, value: impl Into<Value>) -> Self {
        Self::Lt {
            field: field.into(),
            value: value.into(),
        }
    }

    /// Create a less-than-or-equal filter
    pub fn lte(field: impl Into<String>, value: impl Into<Value>) -> Self {
        Self::Lte {
            field: field.into(),
            value: value.into(),
        }
    }

    /// Create an in filter
    pub fn in_values(field: impl Into<String>, values: Vec<Value>) -> Self {
        Self::In {
            field: field.into(),
            values,
        }
    }

    /// Create a not-in filter
    pub fn nin(field: impl Into<String>, values: Vec<Value>) -> Self {
        Self::Nin {
            field: field.into(),
            values,
        }
    }

    /// Create an exists filter
    pub fn exists(field: impl Into<String>, exists: bool) -> Self {
        Self::Exists {
            field: field.into(),
            exists,
        }
    }

    /// Create a regex filter
    pub fn regex(field: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self::Regex {
            field: field.into(),
            pattern: pattern.into(),
            options: None,
        }
    }

    /// Create an AND filter
    pub fn and(filters: Vec<Filter>) -> Self {
        Self::And(filters)
    }

    /// Create an OR filter
    pub fn or(filters: Vec<Filter>) -> Self {
        Self::Or(filters)
    }

    /// Create a NOT filter
    pub fn not(filter: Filter) -> Self {
        Self::Not(Box::new(filter))
    }

    /// Get all fields referenced in this filter
    pub fn get_fields(&self) -> Vec<String> {
        let mut fields = Vec::new();
        self.collect_fields(&mut fields);
        fields.sort();
        fields.dedup();
        fields
    }

    fn collect_fields(&self, fields: &mut Vec<String>) {
        match self {
            Filter::Empty => {}
            Filter::Eq { field, .. }
            | Filter::Ne { field, .. }
            | Filter::Gt { field, .. }
            | Filter::Gte { field, .. }
            | Filter::Lt { field, .. }
            | Filter::Lte { field, .. }
            | Filter::In { field, .. }
            | Filter::Nin { field, .. }
            | Filter::Exists { field, .. }
            | Filter::Regex { field, .. } => {
                fields.push(field.clone());
            }
            Filter::And(filters) | Filter::Or(filters) => {
                for f in filters {
                    f.collect_fields(fields);
                }
            }
            Filter::Not(filter) => {
                filter.collect_fields(fields);
            }
        }
    }

    /// Check if this filter is empty (matches all)
    pub fn is_empty(&self) -> bool {
        matches!(self, Filter::Empty)
    }

    /// Check if this filter can use an index on the given field
    pub fn can_use_index(&self, field: &str) -> bool {
        match self {
            Filter::Empty => false,
            Filter::Eq { field: f, .. }
            | Filter::Ne { field: f, .. }
            | Filter::Gt { field: f, .. }
            | Filter::Gte { field: f, .. }
            | Filter::Lt { field: f, .. }
            | Filter::Lte { field: f, .. }
            | Filter::In { field: f, .. }
            | Filter::Nin { field: f, .. } => f == field,
            Filter::Exists { field: f, .. } => f == field,
            Filter::Regex { field: f, .. } => f == field,
            Filter::And(filters) => filters.iter().any(|f| f.can_use_index(field)),
            Filter::Or(filters) => filters.iter().all(|f| f.can_use_index(field)),
            Filter::Not(filter) => filter.can_use_index(field),
        }
    }
}

/// Projection specification (fields to include/exclude)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Projection {
    /// Fields to include (if Some) or exclude (if None)
    pub fields: BTreeMap<String, ProjectionType>,
}

impl Projection {
    /// Create a new empty projection
    pub fn new() -> Self {
        Self {
            fields: BTreeMap::new(),
        }
    }

    /// Include a field
    pub fn include(mut self, field: impl Into<String>) -> Self {
        self.fields.insert(field.into(), ProjectionType::Include);
        self
    }

    /// Exclude a field
    pub fn exclude(mut self, field: impl Into<String>) -> Self {
        self.fields.insert(field.into(), ProjectionType::Exclude);
        self
    }

    /// Check if this is an inclusion projection
    pub fn is_inclusion(&self) -> bool {
        self.fields.values().any(|t| matches!(t, ProjectionType::Include))
    }

    /// Check if this is an exclusion projection
    pub fn is_exclusion(&self) -> bool {
        self.fields.values().any(|t| matches!(t, ProjectionType::Exclude))
    }

    /// Check if a field should be included
    pub fn should_include(&self, field: &str) -> bool {
        if self.is_inclusion() {
            // Inclusion mode: only include specified fields (and _id by default)
            field == "_id" || self.fields.get(field) == Some(&ProjectionType::Include)
        } else {
            // Exclusion mode: include all except excluded fields
            self.fields.get(field) != Some(&ProjectionType::Exclude)
        }
    }
}

impl Default for Projection {
    fn default() -> Self {
        Self::new()
    }
}

/// Projection type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProjectionType {
    /// Include the field
    Include,
    /// Exclude the field
    Exclude,
}

/// Sort specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Sort {
    /// Fields to sort by with their order
    pub fields: Vec<(String, SortOrder)>,
}

impl Sort {
    /// Create a new empty sort
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    /// Add a field to sort by
    pub fn add(mut self, field: impl Into<String>, order: SortOrder) -> Self {
        self.fields.push((field.into(), order));
        self
    }

    /// Sort by field in ascending order
    pub fn asc(self, field: impl Into<String>) -> Self {
        self.add(field, SortOrder::Ascending)
    }

    /// Sort by field in descending order
    pub fn desc(self, field: impl Into<String>) -> Self {
        self.add(field, SortOrder::Descending)
    }

    /// Get the first sort field
    pub fn first_field(&self) -> Option<&str> {
        self.fields.first().map(|(f, _)| f.as_str())
    }
}

impl Default for Sort {
    fn default() -> Self {
        Self::new()
    }
}

/// Sort order
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SortOrder {
    /// Ascending order (1)
    Ascending,
    /// Descending order (-1)
    Descending,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_creation() {
        let query = Query::new();
        assert!(query.filter.is_empty());
        assert!(query.projection.is_none());
        assert!(query.sort.is_none());
        assert!(query.skip.is_none());
        assert!(query.limit.is_none());
    }

    #[test]
    fn test_query_builder() {
        let query = Query::with_filter(Filter::eq("age", 30i32))
            .projection(Projection::new().include("name").include("age"))
            .sort(Sort::new().asc("name"))
            .skip(10)
            .limit(20);

        assert!(!query.filter.is_empty());
        assert!(query.projection.is_some());
        assert!(query.sort.is_some());
        assert_eq!(query.skip, Some(10));
        assert_eq!(query.limit, Some(20));
    }

    #[test]
    fn test_filter_eq() {
        let filter = Filter::eq("name", "John");
        match filter {
            Filter::Eq { field, value } => {
                assert_eq!(field, "name");
                assert_eq!(value.as_str(), Some("John"));
            }
            _ => panic!("Expected Eq filter"),
        }
    }

    #[test]
    fn test_filter_comparison() {
        let filter_gt = Filter::gt("age", 18i32);
        let filter_gte = Filter::gte("age", 18i32);
        let filter_lt = Filter::lt("age", 65i32);
        let filter_lte = Filter::lte("age", 65i32);

        assert!(matches!(filter_gt, Filter::Gt { .. }));
        assert!(matches!(filter_gte, Filter::Gte { .. }));
        assert!(matches!(filter_lt, Filter::Lt { .. }));
        assert!(matches!(filter_lte, Filter::Lte { .. }));
    }

    #[test]
    fn test_filter_in() {
        let filter = Filter::in_values(
            "status",
            vec![
                Value::String("active".to_string()),
                Value::String("pending".to_string()),
            ],
        );

        match filter {
            Filter::In { field, values } => {
                assert_eq!(field, "status");
                assert_eq!(values.len(), 2);
            }
            _ => panic!("Expected In filter"),
        }
    }

    #[test]
    fn test_filter_logical() {
        let filter = Filter::and(vec![
            Filter::eq("status", "active"),
            Filter::gt("age", 18i32),
        ]);

        match filter {
            Filter::And(filters) => {
                assert_eq!(filters.len(), 2);
            }
            _ => panic!("Expected And filter"),
        }

        let filter_or = Filter::or(vec![
            Filter::eq("role", "admin"),
            Filter::eq("role", "moderator"),
        ]);

        assert!(matches!(filter_or, Filter::Or(_)));
    }

    #[test]
    fn test_filter_not() {
        let filter = Filter::not(Filter::eq("deleted", true));
        assert!(matches!(filter, Filter::Not(_)));
    }

    #[test]
    fn test_filter_exists() {
        let filter = Filter::exists("email", true);
        match filter {
            Filter::Exists { field, exists } => {
                assert_eq!(field, "email");
                assert!(exists);
            }
            _ => panic!("Expected Exists filter"),
        }
    }

    #[test]
    fn test_filter_regex() {
        let filter = Filter::regex("email", r".*@example\.com$");
        match filter {
            Filter::Regex { field, pattern, .. } => {
                assert_eq!(field, "email");
                assert_eq!(pattern, r".*@example\.com$");
            }
            _ => panic!("Expected Regex filter"),
        }
    }

    #[test]
    fn test_filter_get_fields() {
        let filter = Filter::and(vec![
            Filter::eq("name", "John"),
            Filter::gt("age", 18i32),
            Filter::exists("email", true),
        ]);

        let fields = filter.get_fields();
        assert_eq!(fields, vec!["age", "email", "name"]);
    }

    #[test]
    fn test_filter_can_use_index() {
        let filter = Filter::eq("email", "test@example.com");
        assert!(filter.can_use_index("email"));
        assert!(!filter.can_use_index("name"));

        let filter_and = Filter::and(vec![
            Filter::eq("status", "active"),
            Filter::gt("age", 18i32),
        ]);
        assert!(filter_and.can_use_index("status"));
        assert!(filter_and.can_use_index("age"));
    }

    #[test]
    fn test_projection_include() {
        let proj = Projection::new().include("name").include("age");
        assert!(proj.is_inclusion());
        assert!(!proj.is_exclusion());
        assert!(proj.should_include("name"));
        assert!(proj.should_include("age"));
        assert!(proj.should_include("_id")); // _id included by default
        assert!(!proj.should_include("email"));
    }

    #[test]
    fn test_projection_exclude() {
        let proj = Projection::new().exclude("password").exclude("secret");
        assert!(!proj.is_inclusion());
        assert!(proj.is_exclusion());
        assert!(proj.should_include("name"));
        assert!(proj.should_include("age"));
        assert!(!proj.should_include("password"));
        assert!(!proj.should_include("secret"));
    }

    #[test]
    fn test_sort_creation() {
        let sort = Sort::new().asc("name").desc("age");
        assert_eq!(sort.fields.len(), 2);
        assert_eq!(sort.fields[0].0, "name");
        assert_eq!(sort.fields[0].1, SortOrder::Ascending);
        assert_eq!(sort.fields[1].0, "age");
        assert_eq!(sort.fields[1].1, SortOrder::Descending);
    }

    #[test]
    fn test_sort_first_field() {
        let sort = Sort::new().asc("name").desc("age");
        assert_eq!(sort.first_field(), Some("name"));

        let empty_sort = Sort::new();
        assert_eq!(empty_sort.first_field(), None);
    }

    #[test]
    fn test_query_is_simple_key_lookup() {
        let query = Query::with_filter(Filter::eq("_id", "123")).limit(1);
        assert!(query.is_simple_key_lookup());

        let query_complex = Query::with_filter(Filter::and(vec![
            Filter::eq("name", "John"),
            Filter::gt("age", 18i32),
        ]));
        assert!(!query_complex.is_simple_key_lookup());
    }

    #[test]
    fn test_query_get_lookup_field() {
        let query = Query::with_filter(Filter::eq("email", "test@example.com"));
        assert_eq!(query.get_lookup_field(), Some("email"));

        let query_complex = Query::with_filter(Filter::and(vec![
            Filter::eq("name", "John"),
        ]));
        assert_eq!(query_complex.get_lookup_field(), None);
    }
}
