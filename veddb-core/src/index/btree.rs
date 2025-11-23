//! B-tree index implementation
//!
//! Provides efficient indexing using B-tree data structure

use crate::document::{DocumentId, Value};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

/// B-tree index for efficient document lookups
pub struct BTreeIndex {
    /// Index name
    name: String,
    /// Fields being indexed
    fields: Vec<String>,
    /// Whether the index enforces uniqueness
    unique: bool,
    /// Whether the index is sparse (only indexes documents with the field)
    sparse: bool,
    /// The actual B-tree storage
    tree: Arc<RwLock<BTreeMap<IndexKey, Vec<DocumentId>>>>,
    /// Index statistics
    stats: Arc<RwLock<IndexStats>>,
}

impl BTreeIndex {
    /// Create a new B-tree index
    pub fn new(name: String, fields: Vec<String>, unique: bool, sparse: bool) -> Self {
        Self {
            name,
            fields,
            unique,
            sparse,
            tree: Arc::new(RwLock::new(BTreeMap::new())),
            stats: Arc::new(RwLock::new(IndexStats::default())),
        }
    }

    /// Get index name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get indexed fields
    pub fn fields(&self) -> &[String] {
        &self.fields
    }

    /// Check if index is unique
    pub fn is_unique(&self) -> bool {
        self.unique
    }

    /// Check if index is sparse
    pub fn is_sparse(&self) -> bool {
        self.sparse
    }

    /// Insert a document into the index
    pub fn insert(&self, doc_id: DocumentId, entry: IndexEntry) -> Result<(), IndexError> {
        let key = IndexKey::from_entry(&entry, &self.fields)?;
        
        // Skip if sparse and key has null values
        if self.sparse && key.has_null() {
            return Ok(());
        }

        let mut tree = self.tree.write().unwrap();
        let mut stats = self.stats.write().unwrap();

        if self.unique {
            // Check for uniqueness violation
            if let Some(existing_docs) = tree.get(&key) {
                if !existing_docs.is_empty() && !existing_docs.contains(&doc_id) {
                    return Err(IndexError::UniqueConstraintViolation {
                        index: self.name.clone(),
                        key: key.to_string(),
                    });
                }
            }
            
            // For unique indexes, store single document
            tree.insert(key, vec![doc_id]);
        } else {
            // For non-unique indexes, append to document list
            tree.entry(key).or_insert_with(Vec::new).push(doc_id);
        }

        stats.total_entries += 1;
        stats.total_size_bytes += entry.size_estimate();

        Ok(())
    }

    /// Remove a document from the index
    pub fn remove(&self, doc_id: DocumentId, entry: IndexEntry) -> Result<bool, IndexError> {
        let key = IndexKey::from_entry(&entry, &self.fields)?;
        
        // Skip if sparse and key has null values
        if self.sparse && key.has_null() {
            return Ok(false);
        }

        let mut tree = self.tree.write().unwrap();
        let mut stats = self.stats.write().unwrap();

        if let Some(doc_list) = tree.get_mut(&key) {
            if let Some(pos) = doc_list.iter().position(|&id| id == doc_id) {
                doc_list.remove(pos);
                stats.total_entries -= 1;
                stats.total_size_bytes -= entry.size_estimate();

                // Remove empty entries
                if doc_list.is_empty() {
                    tree.remove(&key);
                }

                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Find documents by exact key match
    pub fn find_exact(&self, key: &IndexKey) -> Result<Vec<DocumentId>, IndexError> {
        let tree = self.tree.read().unwrap();
        Ok(tree.get(key).cloned().unwrap_or_default())
    }

    /// Find documents by key range
    pub fn find_range(
        &self,
        start: Option<&IndexKey>,
        end: Option<&IndexKey>,
        include_start: bool,
        include_end: bool,
    ) -> Result<Vec<DocumentId>, IndexError> {
        let tree = self.tree.read().unwrap();
        let mut results = Vec::new();

        // Collect all matching entries
        for (key, doc_ids) in tree.iter() {
            let mut include = true;

            // Check start bound
            if let Some(s) = start {
                if include_start {
                    if key < s {
                        include = false;
                    }
                } else {
                    if key <= s {
                        include = false;
                    }
                }
            }

            // Check end bound
            if let Some(e) = end {
                if include_end {
                    if key > e {
                        include = false;
                    }
                } else {
                    if key >= e {
                        include = false;
                    }
                }
            }

            if include {
                results.extend_from_slice(doc_ids);
            }
        }

        Ok(results)
    }

    /// Get index statistics
    pub fn statistics(&self) -> IndexStats {
        self.stats.read().unwrap().clone()
    }

    /// Get the number of unique keys in the index
    pub fn key_count(&self) -> usize {
        self.tree.read().unwrap().len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.tree.read().unwrap().is_empty()
    }

    /// Clear all entries from the index
    pub fn clear(&self) {
        let mut tree = self.tree.write().unwrap();
        let mut stats = self.stats.write().unwrap();
        
        tree.clear();
        *stats = IndexStats::default();
    }
}

/// Index key for B-tree storage
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct IndexKey {
    /// Values for each indexed field
    values: Vec<IndexValue>,
}

impl IndexKey {
    /// Create index key from entry and field list
    pub fn from_entry(entry: &IndexEntry, fields: &[String]) -> Result<Self, IndexError> {
        let mut values = Vec::new();
        
        for field in fields {
            let value = entry.get_field(field)
                .ok_or_else(|| IndexError::MissingField(field.clone()))?;
            values.push(IndexValue::from_value(value)?);
        }

        Ok(Self { values })
    }

    /// Create index key from values
    pub fn from_values(values: Vec<Value>) -> Result<Self, IndexError> {
        let index_values: Result<Vec<_>, _> = values
            .into_iter()
            .map(|v| IndexValue::from_value(&v))
            .collect();
        
        Ok(Self {
            values: index_values?,
        })
    }

    /// Check if key contains null values
    pub fn has_null(&self) -> bool {
        self.values.iter().any(|v| matches!(v, IndexValue::Null))
    }

    /// Get values
    pub fn values(&self) -> &[IndexValue] {
        &self.values
    }

    /// Convert to string representation
    pub fn to_string(&self) -> String {
        let value_strs: Vec<String> = self.values
            .iter()
            .map(|v| format!("{:?}", v))
            .collect();
        format!("[{}]", value_strs.join(", "))
    }
}

/// Index value that can be stored in B-tree
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum IndexValue {
    /// Null value (lowest sort order)
    Null,
    /// Boolean value
    Bool(bool),
    /// Integer value (stored as i64 for consistency)
    Int(i64),
    /// Float value (stored as ordered bytes for comparison)
    Float(OrderedFloat),
    /// String value
    String(String),
    /// Binary value
    Binary(Vec<u8>),
    /// ObjectId value
    ObjectId([u8; 12]),
    /// DateTime value (stored as timestamp)
    DateTime(i64),
}

impl IndexValue {
    /// Convert from document Value
    pub fn from_value(value: &Value) -> Result<Self, IndexError> {
        match value {
            Value::Null => Ok(IndexValue::Null),
            Value::Bool(b) => Ok(IndexValue::Bool(*b)),
            Value::Int32(i) => Ok(IndexValue::Int(*i as i64)),
            Value::Int64(i) => Ok(IndexValue::Int(*i)),
            Value::Float64(f) => Ok(IndexValue::Float(OrderedFloat(*f))),
            Value::String(s) => Ok(IndexValue::String(s.clone())),
            Value::Binary(b) => Ok(IndexValue::Binary(b.clone())),
            Value::ObjectId(oid) => Ok(IndexValue::ObjectId(*oid.as_bytes())),
            Value::DateTime(dt) => Ok(IndexValue::DateTime(dt.timestamp())),
            Value::Array(_) | Value::Object(_) => {
                Err(IndexError::UnsupportedValueType(format!("{:?}", value)))
            }
        }
    }
}

/// Ordered float wrapper for B-tree storage
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OrderedFloat(f64);

impl Eq for OrderedFloat {}

impl std::hash::Hash for OrderedFloat {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.partial_cmp(&other.0).unwrap_or(std::cmp::Ordering::Equal)
    }
}

/// Index entry containing field values for a document
#[derive(Debug, Clone)]
pub struct IndexEntry {
    /// Field values
    fields: BTreeMap<String, Value>,
}

impl IndexEntry {
    /// Create new index entry
    pub fn new() -> Self {
        Self {
            fields: BTreeMap::new(),
        }
    }

    /// Add field value
    pub fn add_field(&mut self, name: String, value: Value) {
        self.fields.insert(name, value);
    }

    /// Get field value
    pub fn get_field(&self, name: &str) -> Option<&Value> {
        self.fields.get(name)
    }

    /// Estimate size in bytes
    pub fn size_estimate(&self) -> usize {
        self.fields
            .iter()
            .map(|(k, v)| k.len() + v.size_bytes())
            .sum()
    }
}

impl Default for IndexEntry {
    fn default() -> Self {
        Self::new()
    }
}

/// Index statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexStats {
    /// Total number of entries
    pub total_entries: u64,
    /// Estimated total size in bytes
    pub total_size_bytes: usize,
    /// Number of lookups performed
    pub lookup_count: u64,
    /// Number of inserts performed
    pub insert_count: u64,
    /// Number of deletes performed
    pub delete_count: u64,
}

/// Index errors
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("Unique constraint violation in index '{index}' for key '{key}'")]
    UniqueConstraintViolation { index: String, key: String },

    #[error("Missing field: {0}")]
    MissingField(String),

    #[error("Unsupported value type for indexing: {0}")]
    UnsupportedValueType(String),

    #[error("Index operation failed: {0}")]
    OperationFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::ObjectId;
    use chrono::Utc;

    #[test]
    fn test_btree_index_creation() {
        let index = BTreeIndex::new(
            "test_index".to_string(),
            vec!["name".to_string()],
            false,
            false,
        );

        assert_eq!(index.name(), "test_index");
        assert_eq!(index.fields(), &["name"]);
        assert!(!index.is_unique());
        assert!(!index.is_sparse());
        assert!(index.is_empty());
    }

    #[test]
    fn test_index_entry_creation() {
        let mut entry = IndexEntry::new();
        entry.add_field("name".to_string(), Value::String("John".to_string()));
        entry.add_field("age".to_string(), Value::Int32(30));

        assert_eq!(entry.get_field("name"), Some(&Value::String("John".to_string())));
        assert_eq!(entry.get_field("age"), Some(&Value::Int32(30)));
        assert!(entry.size_estimate() > 0);
    }

    #[test]
    fn test_index_key_creation() {
        let mut entry = IndexEntry::new();
        entry.add_field("name".to_string(), Value::String("John".to_string()));
        entry.add_field("age".to_string(), Value::Int32(30));

        let key = IndexKey::from_entry(&entry, &["name".to_string()]).unwrap();
        assert_eq!(key.values().len(), 1);
        assert!(!key.has_null());
    }

    #[test]
    fn test_index_value_conversion() {
        assert_eq!(
            IndexValue::from_value(&Value::Null).unwrap(),
            IndexValue::Null
        );
        assert_eq!(
            IndexValue::from_value(&Value::Bool(true)).unwrap(),
            IndexValue::Bool(true)
        );
        assert_eq!(
            IndexValue::from_value(&Value::Int32(42)).unwrap(),
            IndexValue::Int(42)
        );
        assert_eq!(
            IndexValue::from_value(&Value::String("test".to_string())).unwrap(),
            IndexValue::String("test".to_string())
        );
    }

    #[test]
    fn test_index_value_ordering() {
        let null = IndexValue::Null;
        let bool_false = IndexValue::Bool(false);
        let bool_true = IndexValue::Bool(true);
        let int_small = IndexValue::Int(10);
        let int_large = IndexValue::Int(20);
        let string_a = IndexValue::String("a".to_string());
        let string_b = IndexValue::String("b".to_string());

        assert!(null < bool_false);
        assert!(bool_false < bool_true);
        assert!(bool_true < int_small);
        assert!(int_small < int_large);
        assert!(int_large < string_a);
        assert!(string_a < string_b);
    }

    #[test]
    fn test_btree_insert_and_find() {
        let index = BTreeIndex::new(
            "test_index".to_string(),
            vec!["name".to_string()],
            false,
            false,
        );

        let doc_id = DocumentId::new();
        let mut entry = IndexEntry::new();
        entry.add_field("name".to_string(), Value::String("John".to_string()));

        // Insert
        index.insert(doc_id, entry.clone()).unwrap();
        assert_eq!(index.key_count(), 1);

        // Find
        let key = IndexKey::from_entry(&entry, &["name".to_string()]).unwrap();
        let results = index.find_exact(&key).unwrap();
        assert_eq!(results, vec![doc_id]);
    }

    #[test]
    fn test_btree_unique_constraint() {
        let index = BTreeIndex::new(
            "unique_index".to_string(),
            vec!["email".to_string()],
            true,
            false,
        );

        let doc_id1 = DocumentId::new();
        let doc_id2 = DocumentId::new();
        
        let mut entry = IndexEntry::new();
        entry.add_field("email".to_string(), Value::String("test@example.com".to_string()));

        // First insert should succeed
        index.insert(doc_id1, entry.clone()).unwrap();

        // Second insert with same key should fail
        let result = index.insert(doc_id2, entry);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), IndexError::UniqueConstraintViolation { .. }));
    }

    #[test]
    fn test_btree_sparse_index() {
        let index = BTreeIndex::new(
            "sparse_index".to_string(),
            vec!["optional_field".to_string()],
            false,
            true,
        );

        let doc_id = DocumentId::new();
        let mut entry = IndexEntry::new();
        entry.add_field("optional_field".to_string(), Value::Null);

        // Sparse index should skip null values
        index.insert(doc_id, entry).unwrap();
        assert!(index.is_empty());
    }

    #[test]
    fn test_btree_remove() {
        let index = BTreeIndex::new(
            "test_index".to_string(),
            vec!["name".to_string()],
            false,
            false,
        );

        let doc_id = DocumentId::new();
        let mut entry = IndexEntry::new();
        entry.add_field("name".to_string(), Value::String("John".to_string()));

        // Insert and remove
        index.insert(doc_id, entry.clone()).unwrap();
        assert_eq!(index.key_count(), 1);

        let removed = index.remove(doc_id, entry).unwrap();
        assert!(removed);
        assert!(index.is_empty());
    }

    #[test]
    fn test_btree_range_query() {
        let index = BTreeIndex::new(
            "age_index".to_string(),
            vec!["age".to_string()],
            false,
            false,
        );

        // Insert multiple documents
        for age in [20, 25, 30, 35, 40] {
            let doc_id = DocumentId::new();
            let mut entry = IndexEntry::new();
            entry.add_field("age".to_string(), Value::Int32(age));
            index.insert(doc_id, entry).unwrap();
        }

        // Range query: age >= 25 and age <= 35
        let start_key = IndexKey::from_values(vec![Value::Int32(25)]).unwrap();
        let end_key = IndexKey::from_values(vec![Value::Int32(35)]).unwrap();
        
        let results = index.find_range(Some(&start_key), Some(&end_key), true, true).unwrap();
        assert_eq!(results.len(), 3); // ages 25, 30, 35
    }

    #[test]
    fn test_compound_index() {
        let index = BTreeIndex::new(
            "compound_index".to_string(),
            vec!["category".to_string(), "priority".to_string()],
            false,
            false,
        );

        let doc_id = DocumentId::new();
        let mut entry = IndexEntry::new();
        entry.add_field("category".to_string(), Value::String("bug".to_string()));
        entry.add_field("priority".to_string(), Value::Int32(1));

        index.insert(doc_id, entry.clone()).unwrap();

        let key = IndexKey::from_entry(&entry, &["category".to_string(), "priority".to_string()]).unwrap();
        let results = index.find_exact(&key).unwrap();
        assert_eq!(results, vec![doc_id]);
    }
}