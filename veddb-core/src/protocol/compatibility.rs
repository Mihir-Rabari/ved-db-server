//! v0.1.x Protocol Compatibility Layer
//!
//! This module provides backward compatibility for v0.1.x protocol commands,
//! translating them to v0.2.0 equivalents and routing them to the appropriate handlers.

use std::collections::BTreeMap;
use log::{warn, debug};
use crate::document::{Document, DocumentId, DocumentMetadata, Value};
use chrono::Utc;
use crate::protocol::{
    Command, Response, OpCode, Status, PROTOCOL_V1, PROTOCOL_V2,
    InsertDocRequest, QueryRequest, UpdateDocRequest, DeleteDocRequest,
    OperationResponse,
};

/// Special collection name for legacy key-value operations
pub const LEGACY_KV_COLLECTION: &str = "_legacy_kv";

/// Protocol compatibility handler
pub struct CompatibilityHandler {
    /// Whether to log warnings for deprecated protocol usage
    log_warnings: bool,
}

impl CompatibilityHandler {
    pub fn new(log_warnings: bool) -> Self {
        Self { log_warnings }
    }

    /// Translate a v0.1.x command to v0.2.0 equivalent
    pub fn translate_command(&self, mut cmd: Command) -> Result<Command, String> {
        // Check if this is a v0.1.x command
        if cmd.header.version != PROTOCOL_V1 {
            return Ok(cmd); // Already v0.2.0, no translation needed
        }

        if self.log_warnings {
            warn!(
                "Deprecated v0.1.x protocol command received: {:?}. Consider upgrading client to v0.2.0",
                cmd.header.opcode()
            );
        }

        debug!("Translating v0.1.x command: {:?}", cmd.header.opcode());

        // Update version to v0.2.0
        cmd.header.version = PROTOCOL_V2;

        // Translate based on opcode
        match cmd.header.opcode().map_err(|_| "Invalid opcode")? {
            OpCode::Ping => {
                // Ping remains the same
                Ok(cmd)
            }
            OpCode::Set => {
                // Convert SET key value to INSERT_DOC in _legacy_kv collection
                self.translate_set_to_insert_doc(cmd)
            }
            OpCode::Get => {
                // Convert GET key to QUERY in _legacy_kv collection
                self.translate_get_to_query(cmd)
            }
            OpCode::Del => {
                // Convert DEL key to DELETE_DOC in _legacy_kv collection
                self.translate_del_to_delete_doc(cmd)
            }
            OpCode::Cas => {
                // Convert CAS to UPDATE_DOC with version check
                self.translate_cas_to_update_doc(cmd)
            }
            OpCode::Subscribe | OpCode::Unsubscribe | OpCode::Publish => {
                // Pub/sub commands remain the same in v0.2.0
                Ok(cmd)
            }
            OpCode::Fetch => {
                // Convert FETCH to QUERY with range
                self.translate_fetch_to_query(cmd)
            }
            OpCode::Info => {
                // Info command remains the same
                Ok(cmd)
            }
            _ => {
                // This shouldn't happen for v0.1.x commands
                Err(format!("Unsupported v0.1.x command: {:?}", cmd.header.opcode()))
            }
        }
    }

    /// Translate SET key value to INSERT_DOC
    fn translate_set_to_insert_doc(&self, cmd: Command) -> Result<Command, String> {
        let key = String::from_utf8(cmd.key)
            .map_err(|_| "Invalid UTF-8 in key")?;
        
        // Create a document with the key-value pair
        let mut fields = BTreeMap::new();
        fields.insert("key".to_string(), Value::String(key.clone()));
        fields.insert("value".to_string(), Value::Binary(cmd.value));
        
        // Note: v0.2.0 doesn't use extra field for TTL

        let document = Document {
            id: DocumentId::new(), // Generate a new UUID-based ID
            fields,
            metadata: DocumentMetadata {
                version: 1,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                size_bytes: 0, // Will be calculated later
            },
        };

        let request = InsertDocRequest {
            collection: LEGACY_KV_COLLECTION.to_string(),
            document,
        };

        let payload = serde_json::to_vec(&request)
            .map_err(|e| format!("Serialization error: {}", e))?;

        let mut new_cmd = Command {
            header: cmd.header.with_key_val(0, payload.len() as u32),
            key: Vec::new(),
            value: payload,
        };
        new_cmd.header.op = OpCode::InsertDoc as u8;

        Ok(new_cmd)
    }

    /// Translate GET key to QUERY
    fn translate_get_to_query(&self, cmd: Command) -> Result<Command, String> {
        let key = String::from_utf8(cmd.key)
            .map_err(|_| "Invalid UTF-8 in key")?;

        // Create a query to find the document by key
        let mut filter = BTreeMap::new();
        filter.insert("key".to_string(), Value::String(key));

        let request = QueryRequest {
            collection: LEGACY_KV_COLLECTION.to_string(),
            filter: Some(Value::Object(filter)),
            projection: None,
            sort: None,
            skip: None,
            limit: Some(1),
        };

        let payload = serde_json::to_vec(&request)
            .map_err(|e| format!("Serialization error: {}", e))?;

        let mut new_cmd = Command {
            header: cmd.header.with_key_val(0, payload.len() as u32),
            key: Vec::new(),
            value: payload,
        };
        new_cmd.header.op = OpCode::Query as u8;

        Ok(new_cmd)
    }

    /// Translate DEL key to DELETE_DOC
    fn translate_del_to_delete_doc(&self, cmd: Command) -> Result<Command, String> {
        let key = String::from_utf8(cmd.key)
            .map_err(|_| "Invalid UTF-8 in key")?;

        // Create a delete request to remove the document by key
        let mut filter = BTreeMap::new();
        filter.insert("key".to_string(), Value::String(key));

        let request = DeleteDocRequest {
            collection: LEGACY_KV_COLLECTION.to_string(),
            filter: Value::Object(filter),
        };

        let payload = serde_json::to_vec(&request)
            .map_err(|e| format!("Serialization error: {}", e))?;

        let mut new_cmd = Command {
            header: cmd.header.with_key_val(0, payload.len() as u32),
            key: Vec::new(),
            value: payload,
        };
        new_cmd.header.op = OpCode::DeleteDoc as u8;

        Ok(new_cmd)
    }

    /// Translate CAS to UPDATE_DOC with version check
    fn translate_cas_to_update_doc(&self, cmd: Command) -> Result<Command, String> {
        let key = String::from_utf8(cmd.key)
            .map_err(|_| "Invalid UTF-8 in key")?;
        
        // Parse expected_version from payload (first 8 bytes as u64 little-endian)
        let expected_version = if cmd.value.len() >= 8 {
            u64::from_le_bytes(cmd.value[0..8].try_into().unwrap_or([0u8; 8]))
        } else {
            0 // Default to 0 if payload doesn't contain version
        };

        // Create filter to match key and version
        let mut filter = BTreeMap::new();
        filter.insert("key".to_string(), Value::String(key));
        filter.insert("version".to_string(), Value::Int64(expected_version as i64));

        // Create update to set new value and increment version
        let mut update_fields = BTreeMap::new();
        let mut set_fields = BTreeMap::new();
        set_fields.insert("value".to_string(), Value::Binary(cmd.value));
        
        let mut inc_fields = BTreeMap::new();
        inc_fields.insert("version".to_string(), Value::Int64(1));
        
        update_fields.insert("$set".to_string(), Value::Object(set_fields));
        update_fields.insert("$inc".to_string(), Value::Object(inc_fields));

        let request = UpdateDocRequest {
            collection: LEGACY_KV_COLLECTION.to_string(),
            filter: Value::Object(filter),
            update: Value::Object(update_fields),
            upsert: false,
        };

        let payload = serde_json::to_vec(&request)
            .map_err(|e| format!("Serialization error: {}", e))?;

        let mut new_cmd = Command {
            header: cmd.header.with_key_val(0, payload.len() as u32),
            key: Vec::new(),
            value: payload,
        };
        new_cmd.header.op = OpCode::UpdateDoc as u8;

        Ok(new_cmd)
    }

    /// Translate FETCH to QUERY with range
    fn translate_fetch_to_query(&self, cmd: Command) -> Result<Command, String> {
        // FETCH is typically used to get multiple keys or a range
        // For simplicity, we'll treat it as a query on the entire _legacy_kv collection
        let request = QueryRequest {
            collection: LEGACY_KV_COLLECTION.to_string(),
            filter: None, // No filter = get all
            projection: None,
            sort: None,
            skip: None,
            limit: Some(1000), // Reasonable default limit
        };

        let payload = serde_json::to_vec(&request)
            .map_err(|e| format!("Serialization error: {}", e))?;

        let mut new_cmd = Command {
            header: cmd.header.with_key_val(0, payload.len() as u32),
            key: Vec::new(),
            value: payload,
        };
        new_cmd.header.op = OpCode::Query as u8;

        Ok(new_cmd)
    }

    /// Translate a v0.2.0 response back to v0.1.x format if needed
    pub fn translate_response(&self, response: Response, original_opcode: OpCode) -> Result<Response, String> {
        match original_opcode {
            OpCode::Get => {
                // For GET commands, extract the value from the document
                self.translate_query_response_to_get(response)
            }
            OpCode::Set | OpCode::Del | OpCode::Cas => {
                // For write operations, just return success/failure
                Ok(response)
            }
            OpCode::Fetch => {
                // For FETCH, return the documents as a list
                self.translate_query_response_to_fetch(response)
            }
            _ => {
                // Other commands don't need translation
                Ok(response)
            }
        }
    }

    /// Translate query response back to GET response format
    fn translate_query_response_to_get(&self, response: Response) -> Result<Response, String> {
        if response.header.status().map_err(|_| "Invalid status")? != Status::Ok {
            return Ok(response); // Error responses don't need translation
        }

        // Parse the operation response
        let op_response: OperationResponse = serde_json::from_slice(&response.payload)
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        if !op_response.success {
            return Ok(Response::error(response.header.seq));
        }

        // Extract the value from the document
        if let Some(Value::Array(docs)) = op_response.data {
            if let Some(Value::Object(doc_fields)) = docs.first() {
                if let Some(Value::Binary(value)) = doc_fields.get("value") {
                    return Ok(Response::ok(response.header.seq, value.clone()));
                }
            }
        }

        // Document not found or invalid format
        Ok(Response::not_found(response.header.seq))
    }

    /// Translate query response back to FETCH response format
    fn translate_query_response_to_fetch(&self, response: Response) -> Result<Response, String> {
        if response.header.status().map_err(|_| "Invalid status")? != Status::Ok {
            return Ok(response); // Error responses don't need translation
        }

        // Parse the operation response
        let op_response: OperationResponse = serde_json::from_slice(&response.payload)
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        if !op_response.success {
            return Ok(Response::error(response.header.seq));
        }

        // Convert documents to key-value pairs
        let mut kv_pairs = Vec::new();
        if let Some(Value::Array(docs)) = op_response.data {
            for doc in docs {
                if let Value::Object(fields) = doc {
                    if let (Some(Value::String(key)), Some(Value::Binary(value))) = 
                        (fields.get("key"), fields.get("value")) {
                        kv_pairs.push((key.clone(), value.clone()));
                    }
                }
            }
        }

        // Serialize as simple key-value list
        let payload = serde_json::to_vec(&kv_pairs)
            .map_err(|e| format!("Serialization error: {}", e))?;

        Ok(Response::ok(response.header.seq, payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_set_command() {
        let handler = CompatibilityHandler::new(false);
        
        let mut cmd = Command::new(OpCode::Set, 1, b"test_key".to_vec(), b"test_value".to_vec());
        cmd.header.version = PROTOCOL_V1;
        
        let translated = handler.translate_command(cmd).unwrap();
        
        assert_eq!(translated.header.version, PROTOCOL_V2);
        assert_eq!(translated.header.opcode().unwrap(), OpCode::InsertDoc);
        
        // Verify the payload contains the correct document
        let request: InsertDocRequest = serde_json::from_slice(&translated.value).unwrap();
        assert_eq!(request.collection, LEGACY_KV_COLLECTION);
        assert_eq!(request.document.fields.get("key"), Some(&Value::String("test_key".to_string())));
    }

    #[test]
    fn test_translate_get_command() {
        let handler = CompatibilityHandler::new(false);
        
        let mut cmd = Command::new(OpCode::Get, 2, b"test_key".to_vec(), Vec::new());
        cmd.header.version = PROTOCOL_V1;
        
        let translated = handler.translate_command(cmd).unwrap();
        
        assert_eq!(translated.header.version, PROTOCOL_V2);
        assert_eq!(translated.header.opcode().unwrap(), OpCode::Query);
        
        // Verify the payload contains the correct query
        let request: QueryRequest = serde_json::from_slice(&translated.value).unwrap();
        assert_eq!(request.collection, LEGACY_KV_COLLECTION);
        assert_eq!(request.limit, Some(1));
    }

    #[test]
    fn test_translate_cas_command() {
        let handler = CompatibilityHandler::new(false);
        
        let mut cmd = Command::cas(3, b"test_key".to_vec(), b"new_value".to_vec(), 42);
        cmd.header.version = PROTOCOL_V1;
        
        let translated = handler.translate_command(cmd).unwrap();
        
        assert_eq!(translated.header.version, PROTOCOL_V2);
        assert_eq!(translated.header.opcode().unwrap(), OpCode::UpdateDoc);
        
        // Verify the payload contains the correct update request
        let request: UpdateDocRequest = serde_json::from_slice(&translated.value).unwrap();
        assert_eq!(request.collection, LEGACY_KV_COLLECTION);
        assert!(!request.upsert);
    }

    #[test]
    fn test_no_translation_for_v2_commands() {
        let handler = CompatibilityHandler::new(false);
        
        let cmd = Command::new(OpCode::Set, 1, b"key".to_vec(), b"value".to_vec());
        // Default version is PROTOCOL_V2
        
        let translated = handler.translate_command(cmd.clone()).unwrap();
        
        // Should be unchanged
        assert_eq!(translated.header.version, PROTOCOL_V2);
        // Copy fields to avoid packed struct alignment issues
        let translated_seq = translated.header.seq;
        let cmd_seq = cmd.header.seq;
        assert_eq!(translated_seq, cmd_seq);
        assert_eq!(translated.key, cmd.key);
        assert_eq!(translated.value, cmd.value);
    }
}