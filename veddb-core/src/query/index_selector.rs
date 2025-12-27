//! Index selector for choosing the best index for a query
//!
//! Analyzes queries and selects the most appropriate index

use super::ast::{Filter, Query, Sort};
use crate::schema::IndexDefinition;

/// Index selector for choosing optimal indexes
pub struct IndexSelector {
    /// Available indexes (will be populated from actual collection indexes in task 4.3)
    available_indexes: Vec<IndexCandidate>,
}

impl IndexSelector {
    /// Create a new index selector
    pub fn new() -> Self {
        Self {
            available_indexes: Vec::new(),
        }
    }

    /// Create index selector with available indexes
    pub fn with_indexes(indexes: Vec<IndexDefinition>) -> Self {
        let candidates = indexes
            .into_iter()
            .map(IndexCandidate::from_definition)
            .collect();

        Self {
            available_indexes: candidates,
        }
    }

    /// Select the best index for a query
    pub fn select_index(&self, query: &Query) -> Result<Option<String>, IndexSelectionError> {
        if self.available_indexes.is_empty() {
            return Ok(None);
        }

        let mut candidates = Vec::new();

        // Analyze filter for index candidates
        self.analyze_filter(&query.filter, &mut candidates)?;

        // Score candidates based on query characteristics
        for candidate in &mut candidates {
            candidate.score = self.calculate_score(candidate, query)?;
        }

        // Sort by score (higher is better)
        candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Return the best candidate
        Ok(candidates.first().map(|c| c.name.clone()))
    }

    /// Analyze filter to find potential index candidates
    fn analyze_filter(
        &self,
        filter: &Filter,
        candidates: &mut Vec<IndexCandidate>,
    ) -> Result<(), IndexSelectionError> {
        match filter {
            Filter::Empty => {}
            
            // Single field filters
            Filter::Eq { field, .. } |
            Filter::Ne { field, .. } |
            Filter::Gt { field, .. } |
            Filter::Gte { field, .. } |
            Filter::Lt { field, .. } |
            Filter::Lte { field, .. } |
            Filter::In { field, .. } |
            Filter::Nin { field, .. } |
            Filter::Exists { field, .. } => {
                self.find_indexes_for_field(field, candidates);
            }
            
            Filter::Regex { field, .. } => {
                // Text indexes are preferred for regex
                self.find_text_indexes_for_field(field, candidates);
                // Fall back to regular indexes
                self.find_indexes_for_field(field, candidates);
            }
            
            // Logical operators
            Filter::And(filters) => {
                for f in filters {
                    self.analyze_filter(f, candidates)?;
                }
                // Also look for compound indexes that cover multiple fields
                self.find_compound_indexes(filters, candidates);
            }
            
            Filter::Or(filters) => {
                // OR queries are harder to optimize with indexes
                // Look for indexes on individual branches
                for f in filters {
                    self.analyze_filter(f, candidates)?;
                }
            }
            
            Filter::Not(filter) => {
                // NOT queries typically can't use indexes effectively
                self.analyze_filter(filter, candidates)?;
            }
        }

        Ok(())
    }

    /// Find indexes that can be used for a specific field
    pub fn find_indexes_for_field(&self, field: &str, candidates: &mut Vec<IndexCandidate>) {
        for index in &self.available_indexes {
            if index.can_use_for_field(field) {
                if !candidates.iter().any(|c| c.name == index.name) {
                    candidates.push(index.clone());
                }
            }
        }
    }

    /// Find text indexes for a field (for regex queries)
    fn find_text_indexes_for_field(&self, field: &str, candidates: &mut Vec<IndexCandidate>) {
        for index in &self.available_indexes {
            if matches!(index.index_type, IndexType::Text { .. }) && index.can_use_for_field(field) {
                if !candidates.iter().any(|c| c.name == index.name) {
                    let mut candidate = index.clone();
                    candidate.score += 5.0; // Bonus for text indexes on regex
                    candidates.push(candidate);
                }
            }
        }
    }

    /// Find compound indexes that cover multiple fields in an AND query
    fn find_compound_indexes(&self, filters: &[Filter], candidates: &mut Vec<IndexCandidate>) {
        let fields: Vec<String> = filters
            .iter()
            .filter_map(|f| self.get_filter_field(f))
            .collect();

        for index in &self.available_indexes {
            if let IndexType::Compound { fields: index_fields } = &index.index_type {
                // Check if the index covers the query fields
                let covered_fields = fields
                    .iter()
                    .filter(|f| index_fields.contains(f))
                    .count();

                if covered_fields > 1 {
                    if !candidates.iter().any(|c| c.name == index.name) {
                        let mut candidate = index.clone();
                        candidate.score += covered_fields as f64 * 2.0; // Bonus for compound coverage
                        candidates.push(candidate);
                    }
                }
            }
        }
    }

    /// Get the field name from a filter
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

    /// Calculate score for an index candidate
    fn calculate_score(
        &self,
        candidate: &IndexCandidate,
        query: &Query,
    ) -> Result<f64, IndexSelectionError> {
        let mut score = candidate.score;

        // Bonus for unique indexes on equality queries
        if candidate.unique {
            if self.has_equality_filter(&query.filter) {
                score += 10.0;
            }
        }

        // Bonus if index can help with sorting
        if let Some(ref sort) = query.sort {
            if self.can_help_with_sort(candidate, sort) {
                score += 5.0;
            }
        }

        // Penalty for sparse indexes if query doesn't check existence
        if candidate.sparse && !self.has_exists_filter(&query.filter) {
            score -= 2.0;
        }

        Ok(score)
    }

    /// Check if filter has equality conditions
    fn has_equality_filter(&self, filter: &Filter) -> bool {
        match filter {
            Filter::Eq { .. } => true,
            Filter::And(filters) | Filter::Or(filters) => {
                filters.iter().any(|f| self.has_equality_filter(f))
            }
            Filter::Not(filter) => self.has_equality_filter(filter),
            _ => false,
        }
    }

    /// Check if filter has exists conditions
    fn has_exists_filter(&self, filter: &Filter) -> bool {
        match filter {
            Filter::Exists { .. } => true,
            Filter::And(filters) | Filter::Or(filters) => {
                filters.iter().any(|f| self.has_exists_filter(f))
            }
            Filter::Not(filter) => self.has_exists_filter(filter),
            _ => false,
        }
    }

    /// Check if index can help with sorting
    fn can_help_with_sort(&self, candidate: &IndexCandidate, sort: &Sort) -> bool {
        if let Some(first_sort_field) = sort.first_field() {
            match &candidate.index_type {
                IndexType::Single { field } => field == first_sort_field,
                IndexType::Compound { fields } => {
                    fields.first().map(|f| f == first_sort_field).unwrap_or(false)
                }
                IndexType::Text { .. } => false, // Text indexes don't help with sorting
            }
        } else {
            false
        }
    }
}

impl Default for IndexSelector {
    fn default() -> Self {
        Self::new()
    }
}

///Index candidate for selection
#[derive(Debug, Clone)]
pub struct IndexCandidate {
    /// Index name
    pub name: String,
    /// Index type
    index_type: IndexType,
    /// Whether the index is unique
    unique: bool,
    /// Whether the index is sparse
    sparse: bool,
    /// Selection score (higher is better)
    score: f64,
}

impl IndexCandidate {
    /// Create from index definition
    fn from_definition(def: IndexDefinition) -> Self {
        let index_type = match def.index_type {
            crate::schema::IndexType::Single { field } => IndexType::Single { field },
            crate::schema::IndexType::Compound { fields } => IndexType::Compound { fields },
            crate::schema::IndexType::Text { field } => IndexType::Text { field },
            crate::schema::IndexType::Geospatial { field } => IndexType::Single { field }, // Treat as single for now
        };

        Self {
            name: def.name,
            index_type,
            unique: def.unique,
            sparse: def.sparse,
            score: 0.0,
        }
    }

    /// Check if this index can be used for a field
    fn can_use_for_field(&self, field: &str) -> bool {
        match &self.index_type {
            IndexType::Single { field: index_field } => index_field == field,
            IndexType::Compound { fields } => fields.contains(&field.to_string()),
            IndexType::Text { field: index_field } => index_field == field,
        }
    }
}

/// Internal index type representation
#[derive(Debug, Clone)]
enum IndexType {
    /// Single field index
    Single { field: String },
    /// Compound index on multiple fields
    Compound { fields: Vec<String> },
    /// Text index for full-text search
    Text { field: String },
}

/// Index selection errors
#[derive(Debug, thiserror::Error)]
pub enum IndexSelectionError {
    #[error("Selection error: {0}")]
    SelectionError(String),

    #[error("Invalid index configuration: {0}")]
    InvalidIndex(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::IndexDefinition;

    fn create_test_indexes() -> Vec<IndexDefinition> {
        vec![
            IndexDefinition::single("name".to_string()),
            IndexDefinition::single("age".to_string()),
            IndexDefinition::compound(vec!["name".to_string(), "age".to_string()]),
        ]
    }

    #[test]
    fn test_select_index_for_equality() {
        let indexes = create_test_indexes();
        let selector = IndexSelector::with_indexes(indexes);
        
        let query = Query::with_filter(Filter::eq("name", "John"));
        let selected = selector.select_index(&query).unwrap();
        
        assert!(selected.is_some());
        let index_name = selected.unwrap();
        assert!(index_name.contains("name") || index_name.contains("compound"));
    }

    #[test]
    fn test_select_index_for_range() {
        let indexes = create_test_indexes();
        let selector = IndexSelector::with_indexes(indexes);
        
        let query = Query::with_filter(Filter::and(vec![
            Filter::gte("age", 18i32),
            Filter::lte("age", 65i32),
        ]));
        let selected = selector.select_index(&query).unwrap();
        
        assert!(selected.is_some());
    }

    #[test]
    fn test_select_compound_index() {
        let indexes = create_test_indexes();
        let selector = IndexSelector::with_indexes(indexes);
        
        let query = Query::with_filter(Filter::and(vec![
            Filter::eq("name", "John"),
            Filter::eq("age", 30i32),
        ]));
        let selected = selector.select_index(&query).unwrap();
        
        assert!(selected.is_some());
        // Should prefer compound index for multi-field queries
    }

    #[test]
    fn test_no_index_available() {
        let selector = IndexSelector::new();
        
        let query = Query::with_filter(Filter::eq("name", "John"));
        let selected = selector.select_index(&query).unwrap();
        
        assert!(selected.is_none());
    }

    #[test]
    fn test_index_with_sort() {
        let indexes = create_test_indexes();
        let selector = IndexSelector::with_indexes(indexes);
        
        let query = Query::with_filter(Filter::eq("name", "John"))
            .sort(crate::query::ast::Sort::new().asc("name"));
        let selected = selector.select_index(&query).unwrap();
        
        assert!(selected.is_some());
    }
}
