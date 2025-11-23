//! Query parser for converting JSON to Query AST
//!
//! Parses MongoDB-style query JSON into internal Query structures

use super::ast::{Filter, Projection, ProjectionType, Query, Sort, SortOrder};
use crate::document::Value;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;

/// Query parser for JSON queries
pub struct QueryParser;

impl QueryParser {
    /// Parse a query from JSON
    pub fn parse(json: &str) -> Result<Query, QueryParseError> {
        let value: JsonValue = serde_json::from_str(json)
            .map_err(|e| QueryParseError::InvalidJson(e.to_string()))?;

        Self::parse_from_value(&value)
    }

    /// Parse a query from a JSON value
    pub fn parse_from_value(value: &JsonValue) -> Result<Query, QueryParseError> {
        let obj = value
            .as_object()
            .ok_or_else(|| QueryParseError::InvalidFormat("Query must be an object".to_string()))?;

        let mut query = Query::new();

        // Parse filter
        if let Some(filter_value) = obj.get("filter") {
            query.filter = Self::parse_filter(filter_value)?;
        }

        // Parse projection
        if let Some(proj_value) = obj.get("projection") {
            query.projection = Some(Self::parse_projection(proj_value)?);
        }

        // Parse sort
        if let Some(sort_value) = obj.get("sort") {
            query.sort = Some(Self::parse_sort(sort_value)?);
        }

        // Parse skip
        if let Some(skip_value) = obj.get("skip") {
            query.skip = Some(
                skip_value
                    .as_u64()
                    .ok_or_else(|| QueryParseError::InvalidFormat("skip must be a number".to_string()))?,
            );
        }

        // Parse limit
        if let Some(limit_value) = obj.get("limit") {
            query.limit = Some(
                limit_value
                    .as_u64()
                    .ok_or_else(|| QueryParseError::InvalidFormat("limit must be a number".to_string()))?,
            );
        }

        Ok(query)
    }

    /// Parse a filter from JSON
    pub fn parse_filter(value: &JsonValue) -> Result<Filter, QueryParseError> {
        match value {
            JsonValue::Object(obj) => {
                if obj.is_empty() {
                    return Ok(Filter::Empty);
                }

                let mut filters = Vec::new();

                for (key, val) in obj {
                    if key.starts_with('$') {
                        // Logical operator
                        match key.as_str() {
                            "$and" => {
                                let arr = val.as_array().ok_or_else(|| {
                                    QueryParseError::InvalidFormat("$and must be an array".to_string())
                                })?;
                                let sub_filters: Result<Vec<_>, _> =
                                    arr.iter().map(Self::parse_filter).collect();
                                return Ok(Filter::And(sub_filters?));
                            }
                            "$or" => {
                                let arr = val.as_array().ok_or_else(|| {
                                    QueryParseError::InvalidFormat("$or must be an array".to_string())
                                })?;
                                let sub_filters: Result<Vec<_>, _> =
                                    arr.iter().map(Self::parse_filter).collect();
                                return Ok(Filter::Or(sub_filters?));
                            }
                            "$not" => {
                                let sub_filter = Self::parse_filter(val)?;
                                return Ok(Filter::Not(Box::new(sub_filter)));
                            }
                            _ => {
                                return Err(QueryParseError::UnsupportedOperator(key.clone()));
                            }
                        }
                    } else {
                        // Field condition
                        let filter = Self::parse_field_condition(key, val)?;
                        filters.push(filter);
                    }
                }

                // If multiple filters, combine with AND
                if filters.len() == 1 {
                    Ok(filters.into_iter().next().unwrap())
                } else {
                    Ok(Filter::And(filters))
                }
            }
            _ => Err(QueryParseError::InvalidFormat(
                "Filter must be an object".to_string(),
            )),
        }
    }

    /// Parse a field condition
    fn parse_field_condition(field: &str, value: &JsonValue) -> Result<Filter, QueryParseError> {
        match value {
            JsonValue::Object(obj) => {
                // Check for operators
                if obj.is_empty() {
                    return Ok(Filter::eq(field, Self::json_to_value(value)?));
                }

                let mut filters = Vec::new();

                for (op, val) in obj {
                    let filter = match op.as_str() {
                        "$eq" => Filter::eq(field, Self::json_to_value(val)?),
                        "$ne" => Filter::ne(field, Self::json_to_value(val)?),
                        "$gt" => Filter::gt(field, Self::json_to_value(val)?),
                        "$gte" => Filter::gte(field, Self::json_to_value(val)?),
                        "$lt" => Filter::lt(field, Self::json_to_value(val)?),
                        "$lte" => Filter::lte(field, Self::json_to_value(val)?),
                        "$in" => {
                            let arr = val.as_array().ok_or_else(|| {
                                QueryParseError::InvalidFormat("$in must be an array".to_string())
                            })?;
                            let values: Result<Vec<_>, _> =
                                arr.iter().map(Self::json_to_value).collect();
                            Filter::in_values(field, values?)
                        }
                        "$nin" => {
                            let arr = val.as_array().ok_or_else(|| {
                                QueryParseError::InvalidFormat("$nin must be an array".to_string())
                            })?;
                            let values: Result<Vec<_>, _> =
                                arr.iter().map(Self::json_to_value).collect();
                            Filter::nin(field, values?)
                        }
                        "$exists" => {
                            let exists = val.as_bool().ok_or_else(|| {
                                QueryParseError::InvalidFormat("$exists must be a boolean".to_string())
                            })?;
                            Filter::exists(field, exists)
                        }
                        "$regex" => {
                            let pattern = val.as_str().ok_or_else(|| {
                                QueryParseError::InvalidFormat("$regex must be a string".to_string())
                            })?;
                            Filter::Regex {
                                field: field.to_string(),
                                pattern: pattern.to_string(),
                                options: obj.get("$options").and_then(|v| v.as_str()).map(String::from),
                            }
                        }
                        _ => {
                            return Err(QueryParseError::UnsupportedOperator(op.clone()));
                        }
                    };
                    filters.push(filter);
                }

                if filters.len() == 1 {
                    Ok(filters.into_iter().next().unwrap())
                } else {
                    Ok(Filter::And(filters))
                }
            }
            _ => {
                // Direct value comparison (equality)
                Ok(Filter::eq(field, Self::json_to_value(value)?))
            }
        }
    }

    /// Parse projection from JSON
    fn parse_projection(value: &JsonValue) -> Result<Projection, QueryParseError> {
        let obj = value.as_object().ok_or_else(|| {
            QueryParseError::InvalidFormat("Projection must be an object".to_string())
        })?;

        let mut projection = Projection::new();

        for (field, val) in obj {
            let include = match val {
                JsonValue::Number(n) => {
                    let num = n.as_i64().ok_or_else(|| {
                        QueryParseError::InvalidFormat("Projection value must be 0 or 1".to_string())
                    })?;
                    match num {
                        0 => false,
                        1 => true,
                        _ => {
                            return Err(QueryParseError::InvalidFormat(
                                "Projection value must be 0 or 1".to_string(),
                            ))
                        }
                    }
                }
                JsonValue::Bool(b) => *b,
                _ => {
                    return Err(QueryParseError::InvalidFormat(
                        "Projection value must be 0, 1, true, or false".to_string(),
                    ))
                }
            };

            if include {
                projection = projection.include(field);
            } else {
                projection = projection.exclude(field);
            }
        }

        Ok(projection)
    }

    /// Parse sort from JSON
    fn parse_sort(value: &JsonValue) -> Result<Sort, QueryParseError> {
        let obj = value.as_object().ok_or_else(|| {
            QueryParseError::InvalidFormat("Sort must be an object".to_string())
        })?;

        let mut sort = Sort::new();

        for (field, val) in obj {
            let order = match val {
                JsonValue::Number(n) => {
                    let num = n.as_i64().ok_or_else(|| {
                        QueryParseError::InvalidFormat("Sort value must be 1 or -1".to_string())
                    })?;
                    match num {
                        1 => SortOrder::Ascending,
                        -1 => SortOrder::Descending,
                        _ => {
                            return Err(QueryParseError::InvalidFormat(
                                "Sort value must be 1 or -1".to_string(),
                            ))
                        }
                    }
                }
                JsonValue::String(s) => match s.as_str() {
                    "asc" | "ascending" => SortOrder::Ascending,
                    "desc" | "descending" => SortOrder::Descending,
                    _ => {
                        return Err(QueryParseError::InvalidFormat(
                            "Sort value must be 'asc' or 'desc'".to_string(),
                        ))
                    }
                },
                _ => {
                    return Err(QueryParseError::InvalidFormat(
                        "Sort value must be 1, -1, 'asc', or 'desc'".to_string(),
                    ))
                }
            };

            sort = sort.add(field, order);
        }

        Ok(sort)
    }

    /// Convert JSON value to internal Value
    fn json_to_value(json: &JsonValue) -> Result<Value, QueryParseError> {
        match json {
            JsonValue::Null => Ok(Value::Null),
            JsonValue::Bool(b) => Ok(Value::Bool(*b)),
            JsonValue::Number(n) => {
                if let Some(i) = n.as_i64() {
                    if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                        Ok(Value::Int32(i as i32))
                    } else {
                        Ok(Value::Int64(i))
                    }
                } else if let Some(f) = n.as_f64() {
                    Ok(Value::Float64(f))
                } else {
                    Err(QueryParseError::InvalidFormat(
                        "Invalid number format".to_string(),
                    ))
                }
            }
            JsonValue::String(s) => Ok(Value::String(s.clone())),
            JsonValue::Array(arr) => {
                let values: Result<Vec<_>, _> = arr.iter().map(Self::json_to_value).collect();
                Ok(Value::Array(values?))
            }
            JsonValue::Object(obj) => {
                let mut map = BTreeMap::new();
                for (k, v) in obj {
                    map.insert(k.clone(), Self::json_to_value(v)?);
                }
                Ok(Value::Object(map))
            }
        }
    }

    /// Validate a query
    pub fn validate(query: &Query) -> Result<(), QueryParseError> {
        // Check for invalid combinations
        if let Some(skip) = query.skip {
            if skip > 1_000_000 {
                return Err(QueryParseError::ValidationError(
                    "Skip value too large (max: 1,000,000)".to_string(),
                ));
            }
        }

        if let Some(limit) = query.limit {
            if limit > 100_000 {
                return Err(QueryParseError::ValidationError(
                    "Limit value too large (max: 100,000)".to_string(),
                ));
            }
        }

        // Validate projection
        if let Some(ref projection) = query.projection {
            if projection.is_inclusion() && projection.is_exclusion() {
                // Check if mixing inclusion and exclusion (not allowed except for _id)
                let has_non_id_inclusion = projection
                    .fields
                    .iter()
                    .any(|(k, v)| k != "_id" && *v == ProjectionType::Include);
                let has_exclusion = projection
                    .fields
                    .values()
                    .any(|v| *v == ProjectionType::Exclude);

                if has_non_id_inclusion && has_exclusion {
                    return Err(QueryParseError::ValidationError(
                        "Cannot mix inclusion and exclusion in projection (except _id)".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }
}

/// Query parsing errors
#[derive(Debug, thiserror::Error)]
pub enum QueryParseError {
    #[error("Invalid JSON: {0}")]
    InvalidJson(String),

    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    #[error("Unsupported operator: {0}")]
    UnsupportedOperator(String),

    #[error("Validation error: {0}")]
    ValidationError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_query() {
        let json = r#"{"filter": {}}"#;
        let query = QueryParser::parse(json).unwrap();
        assert!(query.filter.is_empty());
    }

    #[test]
    fn test_parse_simple_equality() {
        let json = r#"{"filter": {"name": "John"}}"#;
        let query = QueryParser::parse(json).unwrap();

        match query.filter {
            Filter::Eq { field, value } => {
                assert_eq!(field, "name");
                assert_eq!(value.as_str(), Some("John"));
            }
            _ => panic!("Expected Eq filter"),
        }
    }

    #[test]
    fn test_parse_comparison_operators() {
        let json = r#"{"filter": {"age": {"$gt": 18}}}"#;
        let query = QueryParser::parse(json).unwrap();

        match query.filter {
            Filter::Gt { field, value } => {
                assert_eq!(field, "age");
                assert_eq!(value.as_i64(), Some(18));
            }
            _ => panic!("Expected Gt filter"),
        }
    }

    #[test]
    fn test_parse_in_operator() {
        let json = r#"{"filter": {"status": {"$in": ["active", "pending"]}}}"#;
        let query = QueryParser::parse(json).unwrap();

        match query.filter {
            Filter::In { field, values } => {
                assert_eq!(field, "status");
                assert_eq!(values.len(), 2);
            }
            _ => panic!("Expected In filter"),
        }
    }

    #[test]
    fn test_parse_exists_operator() {
        let json = r#"{"filter": {"email": {"$exists": true}}}"#;
        let query = QueryParser::parse(json).unwrap();

        match query.filter {
            Filter::Exists { field, exists } => {
                assert_eq!(field, "email");
                assert!(exists);
            }
            _ => panic!("Expected Exists filter"),
        }
    }

    #[test]
    fn test_parse_regex_operator() {
        let json = r#"{"filter": {"email": {"$regex": ".*@example\\.com$"}}}"#;
        let query = QueryParser::parse(json).unwrap();

        match query.filter {
            Filter::Regex { field, pattern, .. } => {
                assert_eq!(field, "email");
                assert_eq!(pattern, r".*@example\.com$");
            }
            _ => panic!("Expected Regex filter"),
        }
    }

    #[test]
    fn test_parse_and_operator() {
        let json = r#"{"filter": {"$and": [{"name": "John"}, {"age": {"$gt": 18}}]}}"#;
        let query = QueryParser::parse(json).unwrap();

        match query.filter {
            Filter::And(filters) => {
                assert_eq!(filters.len(), 2);
            }
            _ => panic!("Expected And filter"),
        }
    }

    #[test]
    fn test_parse_or_operator() {
        let json = r#"{"filter": {"$or": [{"role": "admin"}, {"role": "moderator"}]}}"#;
        let query = QueryParser::parse(json).unwrap();

        match query.filter {
            Filter::Or(filters) => {
                assert_eq!(filters.len(), 2);
            }
            _ => panic!("Expected Or filter"),
        }
    }

    #[test]
    fn test_parse_not_operator() {
        let json = r#"{"filter": {"$not": {"deleted": true}}}"#;
        let query = QueryParser::parse(json).unwrap();

        match query.filter {
            Filter::Not(filter) => {
                assert!(matches!(*filter, Filter::Eq { .. }));
            }
            _ => panic!("Expected Not filter"),
        }
    }

    #[test]
    fn test_parse_projection_include() {
        let json = r#"{"filter": {}, "projection": {"name": 1, "age": 1}}"#;
        let query = QueryParser::parse(json).unwrap();

        let projection = query.projection.unwrap();
        assert!(projection.is_inclusion());
        assert!(projection.should_include("name"));
        assert!(projection.should_include("age"));
        assert!(!projection.should_include("email"));
    }

    #[test]
    fn test_parse_projection_exclude() {
        let json = r#"{"filter": {}, "projection": {"password": 0, "secret": 0}}"#;
        let query = QueryParser::parse(json).unwrap();

        let projection = query.projection.unwrap();
        assert!(projection.is_exclusion());
        assert!(projection.should_include("name"));
        assert!(!projection.should_include("password"));
        assert!(!projection.should_include("secret"));
    }

    #[test]
    fn test_parse_sort() {
        let json = r#"{"filter": {}, "sort": {"name": 1, "age": -1}}"#;
        let query = QueryParser::parse(json).unwrap();

        let sort = query.sort.unwrap();
        assert_eq!(sort.fields.len(), 2);
        
        // Check that both fields are present with correct order
        let name_field = sort.fields.iter().find(|(f, _)| f == "name");
        let age_field = sort.fields.iter().find(|(f, _)| f == "age");
        
        assert!(name_field.is_some());
        assert!(age_field.is_some());
        assert_eq!(name_field.unwrap().1, SortOrder::Ascending);
        assert_eq!(age_field.unwrap().1, SortOrder::Descending);
    }

    #[test]
    fn test_parse_skip_limit() {
        let json = r#"{"filter": {}, "skip": 10, "limit": 20}"#;
        let query = QueryParser::parse(json).unwrap();

        assert_eq!(query.skip, Some(10));
        assert_eq!(query.limit, Some(20));
    }

    #[test]
    fn test_parse_complex_query() {
        let json = r#"{
            "filter": {
                "$and": [
                    {"status": "active"},
                    {"age": {"$gte": 18, "$lte": 65}}
                ]
            },
            "projection": {"name": 1, "age": 1},
            "sort": {"name": 1},
            "skip": 0,
            "limit": 100
        }"#;

        let query = QueryParser::parse(json).unwrap();
        assert!(!query.filter.is_empty());
        assert!(query.projection.is_some());
        assert!(query.sort.is_some());
        assert_eq!(query.skip, Some(0));
        assert_eq!(query.limit, Some(100));
    }

    #[test]
    fn test_validate_skip_limit() {
        let mut query = Query::new();
        query.skip = Some(2_000_000);
        assert!(QueryParser::validate(&query).is_err());

        query.skip = Some(100);
        query.limit = Some(200_000);
        assert!(QueryParser::validate(&query).is_err());

        query.limit = Some(100);
        assert!(QueryParser::validate(&query).is_ok());
    }

    #[test]
    fn test_implicit_and() {
        let json = r#"{"filter": {"name": "John", "age": 30}}"#;
        let query = QueryParser::parse(json).unwrap();

        match query.filter {
            Filter::And(filters) => {
                assert_eq!(filters.len(), 2);
            }
            _ => panic!("Expected implicit And filter"),
        }
    }
}
