//! Document and Value types for VedDB v0.2.0
//!
//! This module provides the core data structures for document storage:
//! - Document: A JSON-like document with nested fields
//! - Value: An enum supporting all JSON types plus ObjectId, DateTime, Binary
//! - Field path navigation for nested document access

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use uuid::Uuid;

/// Maximum document size in bytes (16 MB)
pub const MAX_DOCUMENT_SIZE: usize = 16 * 1024 * 1024;

/// Maximum nesting depth for documents (16 levels)
pub const MAX_NESTING_DEPTH: usize = 16;

/// Unique identifier for documents
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DocumentId(Uuid);

impl DocumentId {
    /// Create a new random document ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a document ID from a UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the inner UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Convert to bytes
    pub fn to_bytes(&self) -> [u8; 16] {
        *self.0.as_bytes()
    }

    /// Create from bytes
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }
}

impl Default for DocumentId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for DocumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// ObjectId type for MongoDB compatibility
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ObjectId([u8; 12]);

impl ObjectId {
    /// Create a new ObjectId
    pub fn new() -> Self {
        let mut bytes = [0u8; 12];
        // Timestamp (4 bytes)
        let timestamp = Utc::now().timestamp() as u32;
        bytes[0..4].copy_from_slice(&timestamp.to_be_bytes());
        
        // Random value (5 bytes)
        let uuid = Uuid::new_v4();
        let random = uuid.as_bytes();
        bytes[4..9].copy_from_slice(&random[0..5]);
        
        // Counter (3 bytes)
        let counter = rand::random::<u32>() & 0x00FFFFFF;
        bytes[9..12].copy_from_slice(&counter.to_be_bytes()[1..4]);
        
        Self(bytes)
    }

    /// Create from bytes
    pub fn from_bytes(bytes: [u8; 12]) -> Self {
        Self(bytes)
    }

    /// Get bytes
    pub fn as_bytes(&self) -> &[u8; 12] {
        &self.0
    }

    /// Get timestamp
    pub fn timestamp(&self) -> i64 {
        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&self.0[0..4]);
        u32::from_be_bytes(bytes) as i64
    }
}

impl Default for ObjectId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

/// Value type supporting all JSON types plus ObjectId, DateTime, Binary
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum Value {
    /// Null value
    Null,
    /// Boolean value
    Bool(bool),
    /// 32-bit integer
    Int32(i32),
    /// 64-bit integer
    Int64(i64),
    /// 64-bit floating point
    Float64(f64),
    /// String value
    String(String),
    /// Binary data
    Binary(Vec<u8>),
    /// Array of values
    Array(Vec<Value>),
    /// Object with string keys and value values
    Object(BTreeMap<String, Value>),
    /// ObjectId for MongoDB compatibility
    ObjectId(ObjectId),
    /// DateTime with UTC timezone
    DateTime(DateTime<Utc>),
}

impl Value {
    /// Check if value is null
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Check if value is a boolean
    pub fn is_bool(&self) -> bool {
        matches!(self, Value::Bool(_))
    }

    /// Check if value is a number (int or float)
    pub fn is_number(&self) -> bool {
        matches!(self, Value::Int32(_) | Value::Int64(_) | Value::Float64(_))
    }

    /// Check if value is a string
    pub fn is_string(&self) -> bool {
        matches!(self, Value::String(_))
    }

    /// Check if value is an array
    pub fn is_array(&self) -> bool {
        matches!(self, Value::Array(_))
    }

    /// Check if value is an object
    pub fn is_object(&self) -> bool {
        matches!(self, Value::Object(_))
    }

    /// Get as boolean
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Get as i64
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int32(i) => Some(*i as i64),
            Value::Int64(i) => Some(*i),
            _ => None,
        }
    }

    /// Get as f64
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int32(i) => Some(*i as f64),
            Value::Int64(i) => Some(*i as f64),
            Value::Float64(f) => Some(*f),
            _ => None,
        }
    }

    /// Get as string reference
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Get as array reference
    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// Get as object reference
    pub fn as_object(&self) -> Option<&BTreeMap<String, Value>> {
        match self {
            Value::Object(obj) => Some(obj),
            _ => None,
        }
    }

    /// Get as mutable object reference
    pub fn as_object_mut(&mut self) -> Option<&mut BTreeMap<String, Value>> {
        match self {
            Value::Object(obj) => Some(obj),
            _ => None,
        }
    }

    /// Calculate the size of this value in bytes (approximate)
    pub fn size_bytes(&self) -> usize {
        match self {
            Value::Null => 1,
            Value::Bool(_) => 1,
            Value::Int32(_) => 4,
            Value::Int64(_) => 8,
            Value::Float64(_) => 8,
            Value::String(s) => s.len(),
            Value::Binary(b) => b.len(),
            Value::Array(arr) => arr.iter().map(|v| v.size_bytes()).sum::<usize>() + 8,
            Value::Object(obj) => {
                obj.iter()
                    .map(|(k, v)| k.len() + v.size_bytes())
                    .sum::<usize>()
                    + 8
            }
            Value::ObjectId(_) => 12,
            Value::DateTime(_) => 8,
        }
    }

    /// Get the nesting depth of this value
    pub fn nesting_depth(&self) -> usize {
        match self {
            Value::Array(arr) => {
                1 + arr.iter().map(|v| v.nesting_depth()).max().unwrap_or(0)
            }
            Value::Object(obj) => {
                1 + obj.values().map(|v| v.nesting_depth()).max().unwrap_or(0)
            }
            _ => 0,
        }
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<i32> for Value {
    fn from(i: i32) -> Self {
        Value::Int32(i)
    }
}

impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Value::Int64(i)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::Float64(f)
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}

impl From<Vec<u8>> for Value {
    fn from(b: Vec<u8>) -> Self {
        Value::Binary(b)
    }
}

impl From<Vec<Value>> for Value {
    fn from(arr: Vec<Value>) -> Self {
        Value::Array(arr)
    }
}

impl From<BTreeMap<String, Value>> for Value {
    fn from(obj: BTreeMap<String, Value>) -> Self {
        Value::Object(obj)
    }
}

impl From<ObjectId> for Value {
    fn from(oid: ObjectId) -> Self {
        Value::ObjectId(oid)
    }
}

impl From<DateTime<Utc>> for Value {
    fn from(dt: DateTime<Utc>) -> Self {
        Value::DateTime(dt)
    }
}

/// Document metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetadata {
    /// Document version for optimistic locking
    pub version: u64,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Document size in bytes
    pub size_bytes: usize,
}

impl Default for DocumentMetadata {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            version: 1,
            created_at: now,
            updated_at: now,
            size_bytes: 0,
        }
    }
}

/// Document structure with nested fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Unique document identifier
    #[serde(rename = "_id")]
    pub id: DocumentId,
    
    /// Document fields stored in a BTreeMap for ordered iteration
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
    
    /// Document metadata (not serialized in normal operations)
    #[serde(skip)]
    pub metadata: DocumentMetadata,
}

impl Document {
    /// Create a new document with a random ID
    pub fn new() -> Self {
        Self {
            id: DocumentId::new(),
            fields: BTreeMap::new(),
            metadata: DocumentMetadata::default(),
        }
    }

    /// Create a document with a specific ID
    pub fn with_id(id: DocumentId) -> Self {
        Self {
            id,
            fields: BTreeMap::new(),
            metadata: DocumentMetadata::default(),
        }
    }

    /// Create a document from fields
    pub fn from_fields(fields: BTreeMap<String, Value>) -> Self {
        let mut doc = Self::new();
        doc.fields = fields;
        doc.update_metadata();
        doc
    }

    /// Insert a field
    pub fn insert(&mut self, key: String, value: Value) -> Option<Value> {
        let result = self.fields.insert(key, value);
        self.update_metadata();
        result
    }

    /// Get a field by key
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.fields.get(key)
    }

    /// Get a mutable field by key
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.fields.get_mut(key)
    }

    /// Remove a field
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let result = self.fields.remove(key);
        self.update_metadata();
        result
    }

    /// Check if a field exists
    pub fn contains_key(&self, key: &str) -> bool {
        self.fields.contains_key(key)
    }

    /// Get field by path (e.g., "user.address.city")
    pub fn get_by_path(&self, path: &str) -> Option<&Value> {
        let parts: Vec<&str> = path.split('.').collect();
        self.get_by_path_parts(&parts)
    }

    /// Get field by path parts
    fn get_by_path_parts(&self, parts: &[&str]) -> Option<&Value> {
        if parts.is_empty() {
            return None;
        }

        let mut current = self.fields.get(parts[0])?;

        for &part in &parts[1..] {
            match current {
                Value::Object(obj) => {
                    current = obj.get(part)?;
                }
                Value::Array(arr) => {
                    // Support array indexing
                    if let Ok(index) = part.parse::<usize>() {
                        current = arr.get(index)?;
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        Some(current)
    }

    /// Set field by path (e.g., "user.address.city")
    pub fn set_by_path(&mut self, path: &str, value: Value) -> Result<(), DocumentError> {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.is_empty() {
            return Err(DocumentError::InvalidPath(path.to_string()));
        }

        self.set_by_path_parts(&parts, value)?;
        self.update_metadata();
        Ok(())
    }

    /// Set field by path parts
    fn set_by_path_parts(&mut self, parts: &[&str], value: Value) -> Result<(), DocumentError> {
        if parts.is_empty() {
            return Err(DocumentError::InvalidPath("empty path".to_string()));
        }

        if parts.len() == 1 {
            self.fields.insert(parts[0].to_string(), value);
            return Ok(());
        }

        // Navigate to parent
        let parent_path = &parts[..parts.len() - 1];
        let field_name = parts[parts.len() - 1];

        // Get or create parent object
        let mut current = self.fields
            .entry(parent_path[0].to_string())
            .or_insert_with(|| Value::Object(BTreeMap::new()));

        for &part in &parent_path[1..] {
            match current {
                Value::Object(obj) => {
                    current = obj
                        .entry(part.to_string())
                        .or_insert_with(|| Value::Object(BTreeMap::new()));
                }
                _ => {
                    return Err(DocumentError::InvalidPath(
                        "path traverses non-object".to_string(),
                    ));
                }
            }
        }

        // Set the final value
        match current {
            Value::Object(obj) => {
                obj.insert(field_name.to_string(), value);
                Ok(())
            }
            _ => Err(DocumentError::InvalidPath(
                "parent is not an object".to_string(),
            )),
        }
    }

    /// Calculate and update document size
    fn update_metadata(&mut self) {
        let size = self.fields.iter()
            .map(|(k, v)| k.len() + v.size_bytes())
            .sum::<usize>();
        
        self.metadata.size_bytes = size;
        self.metadata.updated_at = Utc::now();
        self.metadata.version += 1;
    }

    /// Get document size in bytes
    pub fn size_bytes(&self) -> usize {
        self.metadata.size_bytes
    }

    /// Validate document constraints
    pub fn validate(&self) -> Result<(), DocumentError> {
        // Check document size
        if self.size_bytes() > MAX_DOCUMENT_SIZE {
            return Err(DocumentError::DocumentTooLarge {
                size: self.size_bytes(),
                max: MAX_DOCUMENT_SIZE,
            });
        }

        // Check nesting depth
        let max_depth = self.fields.values()
            .map(|v| v.nesting_depth())
            .max()
            .unwrap_or(0);

        if max_depth > MAX_NESTING_DEPTH {
            return Err(DocumentError::NestingTooDeep {
                depth: max_depth,
                max: MAX_NESTING_DEPTH,
            });
        }

        Ok(())
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String, DocumentError> {
        serde_json::to_string(self).map_err(|e| DocumentError::SerializationError(e.to_string()))
    }

    /// Convert to pretty JSON string
    pub fn to_json_pretty(&self) -> Result<String, DocumentError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| DocumentError::SerializationError(e.to_string()))
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, DocumentError> {
        serde_json::from_str(json).map_err(|e| DocumentError::DeserializationError(e.to_string()))
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

/// Document-related errors
#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    #[error("Document too large: {size} bytes (max: {max})")]
    DocumentTooLarge { size: usize, max: usize },

    #[error("Nesting too deep: {depth} levels (max: {max})")]
    NestingTooDeep { depth: usize, max: usize },

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("Field not found: {0}")]
    FieldNotFound(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_id_creation() {
        let id1 = DocumentId::new();
        let id2 = DocumentId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_document_id_bytes() {
        let id = DocumentId::new();
        let bytes = id.to_bytes();
        let id2 = DocumentId::from_bytes(bytes);
        assert_eq!(id, id2);
    }

    #[test]
    fn test_object_id_creation() {
        let oid1 = ObjectId::new();
        let oid2 = ObjectId::new();
        assert_ne!(oid1, oid2);
    }

    #[test]
    fn test_value_types() {
        assert!(Value::Null.is_null());
        assert!(Value::Bool(true).is_bool());
        assert!(Value::Int32(42).is_number());
        assert!(Value::String("test".to_string()).is_string());
        assert!(Value::Array(vec![]).is_array());
        assert!(Value::Object(BTreeMap::new()).is_object());
    }

    #[test]
    fn test_value_conversions() {
        let v: Value = true.into();
        assert_eq!(v.as_bool(), Some(true));

        let v: Value = 42i32.into();
        assert_eq!(v.as_i64(), Some(42));

        let v: Value = "test".into();
        assert_eq!(v.as_str(), Some("test"));
    }

    #[test]
    fn test_document_basic_operations() {
        let mut doc = Document::new();
        
        doc.insert("name".to_string(), "John".into());
        doc.insert("age".to_string(), 30i32.into());
        
        assert_eq!(doc.get("name").unwrap().as_str(), Some("John"));
        assert_eq!(doc.get("age").unwrap().as_i64(), Some(30));
        
        assert!(doc.contains_key("name"));
        assert!(!doc.contains_key("email"));
        
        doc.remove("age");
        assert!(!doc.contains_key("age"));
    }

    #[test]
    fn test_document_path_navigation() {
        let mut doc = Document::new();
        
        // Create nested structure
        let mut address = BTreeMap::new();
        address.insert("city".to_string(), "New York".into());
        address.insert("zip".to_string(), "10001".into());
        
        let mut user = BTreeMap::new();
        user.insert("name".to_string(), "John".into());
        user.insert("address".to_string(), Value::Object(address));
        
        doc.insert("user".to_string(), Value::Object(user));
        
        // Test path navigation
        assert_eq!(
            doc.get_by_path("user.name").unwrap().as_str(),
            Some("John")
        );
        assert_eq!(
            doc.get_by_path("user.address.city").unwrap().as_str(),
            Some("New York")
        );
        assert_eq!(
            doc.get_by_path("user.address.zip").unwrap().as_str(),
            Some("10001")
        );
        
        // Test non-existent path
        assert!(doc.get_by_path("user.email").is_none());
    }

    #[test]
    fn test_document_set_by_path() {
        let mut doc = Document::new();
        
        // Set nested value
        doc.set_by_path("user.name", "John".into()).unwrap();
        doc.set_by_path("user.address.city", "New York".into()).unwrap();
        
        assert_eq!(
            doc.get_by_path("user.name").unwrap().as_str(),
            Some("John")
        );
        assert_eq!(
            doc.get_by_path("user.address.city").unwrap().as_str(),
            Some("New York")
        );
    }

    #[test]
    fn test_document_size_calculation() {
        let mut doc = Document::new();
        doc.insert("name".to_string(), "John".into());
        doc.insert("age".to_string(), 30i32.into());
        
        assert!(doc.size_bytes() > 0);
    }

    #[test]
    fn test_document_validation() {
        let doc = Document::new();
        assert!(doc.validate().is_ok());
    }

    #[test]
    fn test_document_json_serialization() {
        let mut doc = Document::new();
        doc.insert("name".to_string(), "John".into());
        doc.insert("age".to_string(), 30i32.into());
        
        let json = doc.to_json().unwrap();
        assert!(json.contains("name"));
        assert!(json.contains("John"));
        
        let doc2 = Document::from_json(&json).unwrap();
        assert_eq!(doc2.get("name").unwrap().as_str(), Some("John"));
    }

    #[test]
    fn test_value_nesting_depth() {
        let v1 = Value::Int32(42);
        assert_eq!(v1.nesting_depth(), 0);
        
        let v2 = Value::Array(vec![Value::Int32(1), Value::Int32(2)]);
        assert_eq!(v2.nesting_depth(), 1);
        
        let v3 = Value::Array(vec![
            Value::Array(vec![Value::Int32(1)])
        ]);
        assert_eq!(v3.nesting_depth(), 2);
    }
}
