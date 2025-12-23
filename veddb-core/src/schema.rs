//! Schema definition and validation for VedDB v0.2.0
//!
//! This module provides schema definition, field validation, and JSON Schema support

use crate::document::{Document, Value};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Schema definition for a collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    /// Schema version
    pub version: u32,
    /// Field definitions
    pub fields: BTreeMap<String, FieldDefinition>,
    /// Index definitions
    pub indexes: Vec<IndexDefinition>,
    /// Cache configuration
    pub cache_config: CollectionCacheConfig,
    /// Optional JSON Schema for validation
    pub validation: Option<String>,
}

impl Schema {
    /// Create a new empty schema
    pub fn new() -> Self {
        Self {
            version: 1,
            fields: BTreeMap::new(),
            indexes: Vec::new(),
            cache_config: CollectionCacheConfig::default(),
            validation: None,
        }
    }

    /// Add a field definition
    pub fn add_field(&mut self, name: String, field_def: FieldDefinition) {
        self.fields.insert(name, field_def);
    }

    /// Add an index definition
    pub fn add_index(&mut self, index_def: IndexDefinition) {
        self.indexes.push(index_def);
    }

    /// Validate a document against this schema
    pub fn validate(&self, doc: &Document) -> Result<(), ValidationError> {
        // Validate each field
        for (field_name, field_def) in &self.fields {
            let value = doc.get(field_name);

            // Check required fields
            if field_def.required && value.is_none() {
                return Err(ValidationError::RequiredFieldMissing(field_name.clone()));
            }

            // Validate field if present
            if let Some(value) = value {
                field_def.validate(field_name, value)?;
            }
        }

        // Check for unknown fields if strict mode
        for field_name in doc.fields.keys() {
            if !self.fields.contains_key(field_name) {
                // Allow _id field
                if field_name != "_id" {
                    // In non-strict mode, we allow unknown fields
                    // In strict mode, this would be an error
                }
            }
        }

        Ok(())
    }

    /// Validate and apply defaults to a document
    pub fn apply_defaults(&self, doc: &mut Document) -> Result<(), ValidationError> {
        for (field_name, field_def) in &self.fields {
            if !doc.contains_key(field_name) {
                if let Some(default_value) = &field_def.default {
                    doc.insert(field_name.clone(), default_value.clone());
                }
            }
        }
        Ok(())
    }
}

impl Default for Schema {
    fn default() -> Self {
        Self::new()
    }
}

/// Field definition with type, constraints, and validators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDefinition {
    /// Field type
    pub field_type: FieldType,
    /// Whether the field is required
    pub required: bool,
    /// Default value if not provided
    pub default: Option<Value>,
    /// Whether the field must be unique
    pub unique: bool,
    /// Whether the field should be indexed
    pub indexed: bool,
    /// Validators to apply
    pub validators: Vec<Validator>,
    
    // Encryption settings (Phase 5)
    /// Is this field encrypted at rest?
    pub encrypted: bool,
    /// Encryption key ID (if encrypted)
    pub encryption_key_id: Option<String>,
}

impl FieldDefinition {
    /// Create a new field definition
    pub fn new(field_type: FieldType) -> Self {
        Self {
            field_type,
            required: false,
            default: None,
            unique: false,
            indexed: false,
            validators: Vec::new(),
            encrypted: false,
            encryption_key_id: None,
        }
    }

    /// Set as required
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Set default value
    pub fn default_value(mut self, value: Value) -> Self {
        self.default = Some(value);
        self
    }

    /// Set as unique
    pub fn unique(mut self) -> Self {
        self.unique = true;
        self
    }

    /// Set as indexed
    pub fn indexed(mut self) -> Self {
        self.indexed = true;
        self
    }

    /// Add a validator
    pub fn add_validator(mut self, validator: Validator) -> Self {
        self.validators.push(validator);
        self
    }

    /// Validate a value against this field definition
    pub fn validate(&self, field_name: &str, value: &Value) -> Result<(), ValidationError> {
        // Check type compatibility
        if !self.field_type.is_compatible(value) {
            return Err(ValidationError::TypeMismatch {
                field: field_name.to_string(),
                expected: format!("{:?}", self.field_type),
                actual: format!("{:?}", value),
            });
        }

        // Apply validators
        for validator in &self.validators {
            validator.validate(field_name, value)?;
        }

        Ok(())
    }
}

/// Field type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FieldType {
    /// String type with optional max length
    String { max_length: Option<usize> },
    /// 32-bit integer
    Int32,
    /// 64-bit integer
    Int64,
    /// 64-bit floating point
    Float64,
    /// Boolean
    Boolean,
    /// Date/time
    Date,
    /// Binary data
    Binary,
    /// ObjectId
    ObjectId,
    /// Array with item type
    Array { item_type: Box<FieldType> },
    /// Nested object with field definitions
    Object {
        fields: BTreeMap<String, FieldDefinition>,
    },
    /// Reference to another collection
    Reference { collection: String },
    /// Any type (no validation)
    Any,
}

impl FieldType {
    /// Check if a value is compatible with this field type
    pub fn is_compatible(&self, value: &Value) -> bool {
        match (self, value) {
            (FieldType::String { .. }, Value::String(_)) => true,
            (FieldType::Int32, Value::Int32(_)) => true,
            (FieldType::Int64, Value::Int64(_)) | (FieldType::Int64, Value::Int32(_)) => true,
            (FieldType::Float64, Value::Float64(_))
            | (FieldType::Float64, Value::Int32(_))
            | (FieldType::Float64, Value::Int64(_)) => true,
            (FieldType::Boolean, Value::Bool(_)) => true,
            (FieldType::Date, Value::DateTime(_)) => true,
            (FieldType::Binary, Value::Binary(_)) => true,
            (FieldType::ObjectId, Value::ObjectId(_)) => true,
            (FieldType::Array { item_type }, Value::Array(arr)) => {
                arr.iter().all(|v| item_type.is_compatible(v))
            }
            (FieldType::Object { fields }, Value::Object(obj)) => {
                // Check if all required fields are present and valid
                for (field_name, field_def) in fields {
                    if field_def.required && !obj.contains_key(field_name) {
                        return false;
                    }
                    if let Some(value) = obj.get(field_name) {
                        if !field_def.field_type.is_compatible(value) {
                            return false;
                        }
                    }
                }
                true
            }
            (FieldType::Reference { .. }, Value::ObjectId(_)) => true,
            (FieldType::Any, _) => true,
            _ => false,
        }
    }
}

/// Validator for field values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Validator {
    /// Minimum numeric value
    Min(f64),
    /// Maximum numeric value
    Max(f64),
    /// Minimum string/array length
    MinLength(usize),
    /// Maximum string/array length
    MaxLength(usize),
    /// Regular expression pattern
    Regex(String),
    /// Enum of allowed values
    Enum(Vec<Value>),
    /// Custom validator (JavaScript function name)
    Custom(String),
}

impl Validator {
    /// Validate a value
    pub fn validate(&self, field_name: &str, value: &Value) -> Result<(), ValidationError> {
        match self {
            Validator::Min(min) => {
                if let Some(num) = value.as_f64() {
                    if num < *min {
                        return Err(ValidationError::MinValueViolation {
                            field: field_name.to_string(),
                            min: *min,
                            actual: num,
                        });
                    }
                }
            }
            Validator::Max(max) => {
                if let Some(num) = value.as_f64() {
                    if num > *max {
                        return Err(ValidationError::MaxValueViolation {
                            field: field_name.to_string(),
                            max: *max,
                            actual: num,
                        });
                    }
                }
            }
            Validator::MinLength(min_len) => {
                let len = match value {
                    Value::String(s) => s.len(),
                    Value::Array(arr) => arr.len(),
                    Value::Binary(b) => b.len(),
                    _ => return Ok(()),
                };
                if len < *min_len {
                    return Err(ValidationError::MinLengthViolation {
                        field: field_name.to_string(),
                        min: *min_len,
                        actual: len,
                    });
                }
            }
            Validator::MaxLength(max_len) => {
                let len = match value {
                    Value::String(s) => s.len(),
                    Value::Array(arr) => arr.len(),
                    Value::Binary(b) => b.len(),
                    _ => return Ok(()),
                };
                if len > *max_len {
                    return Err(ValidationError::MaxLengthViolation {
                        field: field_name.to_string(),
                        max: *max_len,
                        actual: len,
                    });
                }
            }
            Validator::Regex(pattern) => {
                if let Value::String(s) = value {
                    let re = Regex::new(pattern).map_err(|e| {
                        ValidationError::InvalidRegex {
                            pattern: pattern.clone(),
                            error: e.to_string(),
                        }
                    })?;
                    if !re.is_match(s) {
                        return Err(ValidationError::RegexMismatch {
                            field: field_name.to_string(),
                            pattern: pattern.clone(),
                            value: s.clone(),
                        });
                    }
                }
            }
            Validator::Enum(allowed_values) => {
                if !allowed_values.contains(value) {
                    return Err(ValidationError::EnumViolation {
                        field: field_name.to_string(),
                        allowed: allowed_values.clone(),
                        actual: value.clone(),
                    });
                }
            }
            Validator::Custom(_func_name) => {
                // Custom validators would be implemented by the application
                // For now, we just pass validation
            }
        }
        Ok(())
    }
}

/// Index definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDefinition {
    /// Index name
    pub name: String,
    /// Index type
    pub index_type: IndexType,
    /// Whether the index is unique
    pub unique: bool,
    /// Whether the index is sparse (only indexes documents with the field)
    pub sparse: bool,
}

impl IndexDefinition {
    /// Create a new single-field index
    pub fn single(field: String) -> Self {
        Self {
            name: format!("idx_{}", field),
            index_type: IndexType::Single { field },
            unique: false,
            sparse: false,
        }
    }

    /// Create a new compound index
    pub fn compound(fields: Vec<String>) -> Self {
        let name = format!("idx_{}", fields.join("_"));
        Self {
            name,
            index_type: IndexType::Compound { fields },
            unique: false,
            sparse: false,
        }
    }

    /// Set as unique
    pub fn unique(mut self) -> Self {
        self.unique = true;
        self
    }

    /// Set as sparse
    pub fn sparse(mut self) -> Self {
        self.sparse = true;
        self
    }
}

/// Index type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexType {
    /// Single field index
    Single { field: String },
    /// Compound index on multiple fields
    Compound { fields: Vec<String> },
    /// Text index for full-text search
    Text { field: String },
    /// Geospatial index (future)
    Geospatial { field: String },
}

/// Cache configuration for a collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionCacheConfig {
    /// Cache strategy
    pub strategy: CacheStrategy,
    /// Time-to-live in seconds
    pub ttl: Option<u64>,
    /// Specific fields to cache (None means all fields)
    pub fields: Option<Vec<String>>,
    /// Cache warming strategy
    pub warming: CacheWarmingStrategy,
}

impl Default for CollectionCacheConfig {
    fn default() -> Self {
        Self {
            strategy: CacheStrategy::None,
            ttl: None,
            fields: None,
            warming: CacheWarmingStrategy::None,
        }
    }
}

/// Cache strategy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CacheStrategy {
    /// No caching
    None,
    /// Write to both cache and persistent storage
    WriteThrough,
    /// Write to cache immediately, persist asynchronously
    WriteBehind { delay_ms: u64 },
    /// Read from cache, populate on miss
    ReadThrough,
}

/// Cache warming strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheWarmingStrategy {
    /// No warming
    None,
    /// Preload on startup
    PreloadOnStartup { limit: usize },
    /// Lazy load on access
    LazyLoad,
    /// Scheduled refresh
    ScheduledRefresh { interval_seconds: u64 },
}

/// Validation errors
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Required field missing: {0}")]
    RequiredFieldMissing(String),

    #[error("Type mismatch for field '{field}': expected {expected}, got {actual}")]
    TypeMismatch {
        field: String,
        expected: String,
        actual: String,
    },

    #[error("Min value violation for field '{field}': min={min}, actual={actual}")]
    MinValueViolation {
        field: String,
        min: f64,
        actual: f64,
    },

    #[error("Max value violation for field '{field}': max={max}, actual={actual}")]
    MaxValueViolation {
        field: String,
        max: f64,
        actual: f64,
    },

    #[error("Min length violation for field '{field}': min={min}, actual={actual}")]
    MinLengthViolation {
        field: String,
        min: usize,
        actual: usize,
    },

    #[error("Max length violation for field '{field}': max={max}, actual={actual}")]
    MaxLengthViolation {
        field: String,
        max: usize,
        actual: usize,
    },

    #[error("Regex mismatch for field '{field}': pattern='{pattern}', value='{value}'")]
    RegexMismatch {
        field: String,
        pattern: String,
        value: String,
    },

    #[error("Invalid regex pattern '{pattern}': {error}")]
    InvalidRegex { pattern: String, error: String },

    #[error("Enum violation for field '{field}': value not in allowed set")]
    EnumViolation {
        field: String,
        allowed: Vec<Value>,
        actual: Value,
    },

    #[error("Unique constraint violation for field '{0}'")]
    UniqueConstraintViolation(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::DocumentId;

    #[test]
    fn test_schema_creation() {
        let schema = Schema::new();
        assert_eq!(schema.version, 1);
        assert!(schema.fields.is_empty());
        assert!(schema.indexes.is_empty());
    }

    #[test]
    fn test_field_definition() {
        let field = FieldDefinition::new(FieldType::String { max_length: Some(100) })
            .required()
            .unique()
            .indexed();

        assert!(field.required);
        assert!(field.unique);
        assert!(field.indexed);
    }

    #[test]
    fn test_field_type_compatibility() {
        let string_type = FieldType::String { max_length: None };
        assert!(string_type.is_compatible(&Value::String("test".to_string())));
        assert!(!string_type.is_compatible(&Value::Int32(42)));

        let int_type = FieldType::Int32;
        assert!(int_type.is_compatible(&Value::Int32(42)));
        assert!(!int_type.is_compatible(&Value::String("test".to_string())));

        let array_type = FieldType::Array {
            item_type: Box::new(FieldType::Int32),
        };
        assert!(array_type.is_compatible(&Value::Array(vec![
            Value::Int32(1),
            Value::Int32(2)
        ])));
        assert!(!array_type.is_compatible(&Value::Array(vec![
            Value::String("test".to_string())
        ])));
    }

    #[test]
    fn test_validator_min() {
        let validator = Validator::Min(10.0);
        assert!(validator.validate("age", &Value::Int32(15)).is_ok());
        assert!(validator.validate("age", &Value::Int32(5)).is_err());
    }

    #[test]
    fn test_validator_max() {
        let validator = Validator::Max(100.0);
        assert!(validator.validate("age", &Value::Int32(50)).is_ok());
        assert!(validator.validate("age", &Value::Int32(150)).is_err());
    }

    #[test]
    fn test_validator_min_length() {
        let validator = Validator::MinLength(3);
        assert!(validator
            .validate("name", &Value::String("John".to_string()))
            .is_ok());
        assert!(validator
            .validate("name", &Value::String("Jo".to_string()))
            .is_err());
    }

    #[test]
    fn test_validator_max_length() {
        let validator = Validator::MaxLength(10);
        assert!(validator
            .validate("name", &Value::String("John".to_string()))
            .is_ok());
        assert!(validator
            .validate("name", &Value::String("VeryLongName".to_string()))
            .is_err());
    }

    #[test]
    fn test_validator_regex() {
        let validator = Validator::Regex(r"^\d{3}-\d{4}$".to_string());
        assert!(validator
            .validate("phone", &Value::String("123-4567".to_string()))
            .is_ok());
        assert!(validator
            .validate("phone", &Value::String("invalid".to_string()))
            .is_err());
    }

    #[test]
    fn test_validator_enum() {
        let validator = Validator::Enum(vec![
            Value::String("red".to_string()),
            Value::String("green".to_string()),
            Value::String("blue".to_string()),
        ]);
        assert!(validator
            .validate("color", &Value::String("red".to_string()))
            .is_ok());
        assert!(validator
            .validate("color", &Value::String("yellow".to_string()))
            .is_err());
    }

    #[test]
    fn test_schema_validation_required_field() {
        let mut schema = Schema::new();
        schema.add_field(
            "name".to_string(),
            FieldDefinition::new(FieldType::String { max_length: None }).required(),
        );

        let mut doc = Document::with_id(DocumentId::new());
        doc.insert("name".to_string(), Value::String("John".to_string()));
        assert!(schema.validate(&doc).is_ok());

        let doc_missing = Document::with_id(DocumentId::new());
        assert!(schema.validate(&doc_missing).is_err());
    }

    #[test]
    fn test_schema_validation_type_mismatch() {
        let mut schema = Schema::new();
        schema.add_field(
            "age".to_string(),
            FieldDefinition::new(FieldType::Int32),
        );

        let mut doc = Document::with_id(DocumentId::new());
        doc.insert("age".to_string(), Value::String("thirty".to_string()));
        assert!(schema.validate(&doc).is_err());
    }

    #[test]
    fn test_schema_apply_defaults() {
        let mut schema = Schema::new();
        schema.add_field(
            "status".to_string(),
            FieldDefinition::new(FieldType::String { max_length: None })
                .default_value(Value::String("active".to_string())),
        );

        let mut doc = Document::with_id(DocumentId::new());
        schema.apply_defaults(&mut doc).unwrap();
        assert_eq!(
            doc.get("status").unwrap().as_str(),
            Some("active")
        );
    }

    #[test]
    fn test_index_definition() {
        let index = IndexDefinition::single("email".to_string()).unique();
        assert!(index.unique);
        assert_eq!(index.name, "idx_email");

        let compound = IndexDefinition::compound(vec!["first_name".to_string(), "last_name".to_string()]);
        assert_eq!(compound.name, "idx_first_name_last_name");
    }

    #[test]
    fn test_cache_config() {
        let config = CollectionCacheConfig {
            strategy: CacheStrategy::WriteThrough,
            ttl: Some(3600),
            fields: Some(vec!["name".to_string(), "email".to_string()]),
            warming: CacheWarmingStrategy::PreloadOnStartup { limit: 1000 },
        };

        assert_eq!(config.strategy, CacheStrategy::WriteThrough);
        assert_eq!(config.ttl, Some(3600));
    }
}
