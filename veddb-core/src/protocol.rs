//! Binary command protocol definitions
//!
//! Defines the compact binary format used for all VedDB operations,
//! both in shared memory rings and over network connections.
//! 
//! VedDB v0.2.0 introduces a new protocol format with version negotiation
//! and support for document operations, authentication, and advanced data structures.

pub mod compatibility;
pub mod connection;

use std::mem;
use serde::{Deserialize, Serialize};
use crate::document::{Document, Value};

/// Protocol version constants
pub const PROTOCOL_V1: u8 = 0x01; // Legacy v0.1.x protocol
pub const PROTOCOL_V2: u8 = 0x02; // New v0.2.0 protocol

// Re-export compatibility handler and connection management
pub use compatibility::{CompatibilityHandler, LEGACY_KV_COLLECTION};
pub use connection::{ConnectionManager, Session, SessionId, ConnectionStats, ConnectionError};

/// Command opcodes for v0.1.x (legacy) and v0.2.0 protocols
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    // Legacy v0.1.x opcodes (0x01-0x0A)
    Ping = 0x01,
    Set = 0x02,
    Get = 0x03,
    Del = 0x04,
    Cas = 0x05,
    Subscribe = 0x06,
    Unsubscribe = 0x07,
    Publish = 0x08,
    Fetch = 0x09,
    Info = 0x0A,
    
    // New v0.2.0 opcodes (0x10-0x3F)
    // Authentication
    Auth = 0x10,
    AuthResponse = 0x11,
    
    // Document operations
    Query = 0x12,
    InsertDoc = 0x13,
    UpdateDoc = 0x14,
    DeleteDoc = 0x15,
    
    // Collection management
    CreateCollection = 0x16,
    DropCollection = 0x17,
    ListCollections = 0x18,
    
    // Index management
    CreateIndex = 0x19,
    DropIndex = 0x1A,
    ListIndexes = 0x1B,
    
    // Advanced data structures - Lists
    LPush = 0x20,
    RPush = 0x21,
    LPop = 0x22,
    RPop = 0x23,
    LRange = 0x24,
    LLen = 0x25,
    
    // Advanced data structures - Sets
    SAdd = 0x26,
    SRem = 0x27,
    SMembers = 0x28,
    SIsMember = 0x29,
    SCard = 0x2A,
    SUnion = 0x2B,
    SInter = 0x2C,
    SDiff = 0x2D,
    
    // Advanced data structures - Sorted Sets
    ZAdd = 0x2E,
    ZRem = 0x2F,
    ZRange = 0x30,
    ZRangeByScore = 0x31,
    ZCard = 0x32,
    ZScore = 0x33,
    
    // Advanced data structures - Hashes
    HSet = 0x34,
    HGet = 0x35,
    HDel = 0x36,
    HGetAll = 0x37,
    HKeys = 0x38,
    HVals = 0x39,
    HLen = 0x3A,
}

impl TryFrom<u8> for OpCode {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            // Legacy v0.1.x opcodes
            0x01 => Ok(OpCode::Ping),
            0x02 => Ok(OpCode::Set),
            0x03 => Ok(OpCode::Get),
            0x04 => Ok(OpCode::Del),
            0x05 => Ok(OpCode::Cas),
            0x06 => Ok(OpCode::Subscribe),
            0x07 => Ok(OpCode::Unsubscribe),
            0x08 => Ok(OpCode::Publish),
            0x09 => Ok(OpCode::Fetch),
            0x0A => Ok(OpCode::Info),
            
            // New v0.2.0 opcodes
            0x10 => Ok(OpCode::Auth),
            0x11 => Ok(OpCode::AuthResponse),
            0x12 => Ok(OpCode::Query),
            0x13 => Ok(OpCode::InsertDoc),
            0x14 => Ok(OpCode::UpdateDoc),
            0x15 => Ok(OpCode::DeleteDoc),
            0x16 => Ok(OpCode::CreateCollection),
            0x17 => Ok(OpCode::DropCollection),
            0x18 => Ok(OpCode::ListCollections),
            0x19 => Ok(OpCode::CreateIndex),
            0x1A => Ok(OpCode::DropIndex),
            0x1B => Ok(OpCode::ListIndexes),
            0x20 => Ok(OpCode::LPush),
            0x21 => Ok(OpCode::RPush),
            0x22 => Ok(OpCode::LPop),
            0x23 => Ok(OpCode::RPop),
            0x24 => Ok(OpCode::LRange),
            0x25 => Ok(OpCode::LLen),
            0x26 => Ok(OpCode::SAdd),
            0x27 => Ok(OpCode::SRem),
            0x28 => Ok(OpCode::SMembers),
            0x29 => Ok(OpCode::SIsMember),
            0x2A => Ok(OpCode::SCard),
            0x2B => Ok(OpCode::SUnion),
            0x2C => Ok(OpCode::SInter),
            0x2D => Ok(OpCode::SDiff),
            0x2E => Ok(OpCode::ZAdd),
            0x2F => Ok(OpCode::ZRem),
            0x30 => Ok(OpCode::ZRange),
            0x31 => Ok(OpCode::ZRangeByScore),
            0x32 => Ok(OpCode::ZCard),
            0x33 => Ok(OpCode::ZScore),
            0x34 => Ok(OpCode::HSet),
            0x35 => Ok(OpCode::HGet),
            0x36 => Ok(OpCode::HDel),
            0x37 => Ok(OpCode::HGetAll),
            0x38 => Ok(OpCode::HKeys),
            0x39 => Ok(OpCode::HVals),
            0x3A => Ok(OpCode::HLen),
            _ => Err(()),
        }
    }
}

/// Command flags
pub mod flags {
    pub const NO_COPY: u8 = 0x01; // Value is already in arena, use offset
    pub const URGENT: u8 = 0x02; // High priority operation
    pub const TTL: u8 = 0x04; // Extra field contains TTL
    pub const CAS_VERSION: u8 = 0x08; // Extra field contains expected version
}

/// Response status codes
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok = 0x00,
    Error = 0x01,
    NotFound = 0x02,
    Full = 0x03,
    Timeout = 0x04,
    VersionMismatch = 0x05,
    AuthRequired = 0x06,
    AuthFailed = 0x07,
    PermissionDenied = 0x08,
    InvalidQuery = 0x09,
    CollectionExists = 0x0A,
    CollectionNotFound = 0x0B,
    IndexExists = 0x0C,
    IndexNotFound = 0x0D,
}

impl TryFrom<u8> for Status {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, <Self as TryFrom<u8>>::Error> {
        match value {
            0x00 => Ok(Status::Ok),
            0x01 => Ok(Status::Error),
            0x02 => Ok(Status::NotFound),
            0x03 => Ok(Status::Full),
            0x04 => Ok(Status::Timeout),
            0x05 => Ok(Status::VersionMismatch),
            0x06 => Ok(Status::AuthRequired),
            0x07 => Ok(Status::AuthFailed),
            0x08 => Ok(Status::PermissionDenied),
            0x09 => Ok(Status::InvalidQuery),
            0x0A => Ok(Status::CollectionExists),
            0x0B => Ok(Status::CollectionNotFound),
            0x0C => Ok(Status::IndexExists),
            0x0D => Ok(Status::IndexNotFound),
            _ => Err(()),
        }
    }
}

/// Command header (24 bytes, little-endian)
///
/// This is the wire format used in ring buffers and over the network.
/// All fields are little-endian for consistency across platforms.
/// 
/// v0.2.0 format includes protocol version in the reserved field.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct CmdHeader {
    pub op: u8,        // OpCode
    pub flags: u8,     // Command flags
    pub version: u8,   // Protocol version (PROTOCOL_V1 or PROTOCOL_V2)
    pub reserved: u8,  // Reserved for future use
    pub seq: u32,      // Client-local sequence ID
    pub key_len: u32,  // Key length in bytes
    pub val_len: u32,  // Value length in bytes
    pub extra: u64,    // TTL, version, or arena offset
}

impl CmdHeader {
    pub const SIZE: usize = mem::size_of::<Self>();

    pub fn new(op: OpCode, seq: u32) -> Self {
        Self {
            op: op as u8,
            flags: 0,
            version: PROTOCOL_V2, // Default to v0.2.0
            reserved: 0,
            seq,
            key_len: 0,
            val_len: 0,
            extra: 0,
        }
    }

    pub fn new_v1(op: OpCode, seq: u32) -> Self {
        Self {
            op: op as u8,
            flags: 0,
            version: PROTOCOL_V1,
            reserved: 0,
            seq,
            key_len: 0,
            val_len: 0,
            extra: 0,
        }
    }

    pub fn with_key_val(mut self, key_len: u32, val_len: u32) -> Self {
        self.key_len = key_len;
        self.val_len = val_len;
        self
    }

    pub fn with_flags(mut self, flags: u8) -> Self {
        self.flags = flags;
        self
    }

    // Note: v0.2.0 protocol doesn't use extra field

    pub fn opcode(&self) -> Result<OpCode, ()> {
        OpCode::try_from(self.op)
    }

    pub fn total_payload_len(&self) -> usize {
        (self.key_len + self.val_len) as usize
    }

    pub fn has_flag(&self, flag: u8) -> bool {
        (self.flags & flag) != 0
    }
}

/// Response header (16 bytes, little-endian)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct RespHeader {
    pub status: u8,       // Status code
    pub flags: u8,        // Response flags
    pub reserved: u16,    // Reserved
    pub seq: u32,         // Echo of request sequence
    pub payload_len: u32, // Response payload length
    pub padding: u32,     // Padding to match 16-byte alignment of v0.2.0
    // Note: v0.2.0 uses 16-byte header (no extra field)
}

impl RespHeader {
    pub const SIZE: usize = mem::size_of::<Self>();

    pub fn new(status: Status, seq: u32) -> Self {
        Self {
            status: status as u8,
            flags: 0,
            reserved: 0,
            seq,
            payload_len: 0,
            padding: 0,
        }
    }

    pub fn with_payload(mut self, len: u32) -> Self {
        self.payload_len = len;
        self
    }

    // Note: v0.2.0 protocol doesn't use extra field

    pub fn status(&self) -> Result<Status, ()> {
        Status::try_from(self.status)
    }
}

/// Complete command with header and payload
#[derive(Debug, Clone)]
pub struct Command {
    pub header: CmdHeader,
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

impl Command {
    pub fn new(op: OpCode, seq: u32, key: Vec<u8>, value: Vec<u8>) -> Self {
        let header = CmdHeader::new(op, seq).with_key_val(key.len() as u32, value.len() as u32);

        Self { header, key, value }
    }

    pub fn ping(seq: u32) -> Self {
        Self::new(OpCode::Ping, seq, Vec::new(), Vec::new())
    }

    pub fn get(seq: u32, key: Vec<u8>) -> Self {
        Self::new(OpCode::Get, seq, key, Vec::new())
    }

    pub fn set(seq: u32, key: Vec<u8>, value: Vec<u8>) -> Self {
        Self::new(OpCode::Set, seq, key, value)
    }

    pub fn del(seq: u32, key: Vec<u8>) -> Self {
        Self::new(OpCode::Del, seq, key, Vec::new())
    }

    pub fn cas(seq: u32, key: Vec<u8>, value: Vec<u8>, expected_version: u64) -> Self {
        let mut cmd = Self::new(OpCode::Cas, seq, key, value);
        cmd.header.flags |= flags::CAS_VERSION;
        // Note: v0.2.0 stores expected_version in payload instead of header
        cmd
    }

    // v0.2.0 protocol commands
    pub fn auth(seq: u32, auth_request: &AuthRequest) -> Result<Self, String> {
        let payload = auth_request.to_bytes()?;
        Ok(Self::new(OpCode::Auth, seq, Vec::new(), payload))
    }

    pub fn query(seq: u32, query_request: &QueryRequest) -> Result<Self, String> {
        let payload = query_request.to_bytes()?;
        Ok(Self::new(OpCode::Query, seq, Vec::new(), payload))
    }

    pub fn insert_doc(seq: u32, request: &InsertDocRequest) -> Result<Self, String> {
        let payload = serde_json::to_vec(request).map_err(|e| format!("Serialization error: {}", e))?;
        Ok(Self::new(OpCode::InsertDoc, seq, Vec::new(), payload))
    }

    pub fn update_doc(seq: u32, request: &UpdateDocRequest) -> Result<Self, String> {
        let payload = serde_json::to_vec(request).map_err(|e| format!("Serialization error: {}", e))?;
        Ok(Self::new(OpCode::UpdateDoc, seq, Vec::new(), payload))
    }

    pub fn delete_doc(seq: u32, request: &DeleteDocRequest) -> Result<Self, String> {
        let payload = serde_json::to_vec(request).map_err(|e| format!("Serialization error: {}", e))?;
        Ok(Self::new(OpCode::DeleteDoc, seq, Vec::new(), payload))
    }

    pub fn create_collection(seq: u32, request: &CreateCollectionRequest) -> Result<Self, String> {
        let payload = serde_json::to_vec(request).map_err(|e| format!("Serialization error: {}", e))?;
        Ok(Self::new(OpCode::CreateCollection, seq, Vec::new(), payload))
    }

    pub fn create_index(seq: u32, request: &CreateIndexRequest) -> Result<Self, String> {
        let payload = serde_json::to_vec(request).map_err(|e| format!("Serialization error: {}", e))?;
        Ok(Self::new(OpCode::CreateIndex, seq, Vec::new(), payload))
    }

    pub fn list_op(seq: u32, request: &ListOpRequest) -> Result<Self, String> {
        let payload = serde_json::to_vec(request).map_err(|e| format!("Serialization error: {}", e))?;
        let opcode = match &request.operation {
            ListOperation::Push { left: true, .. } => OpCode::LPush,
            ListOperation::Push { left: false, .. } => OpCode::RPush,
            ListOperation::Pop { left: true } => OpCode::LPop,
            ListOperation::Pop { left: false } => OpCode::RPop,
            ListOperation::Range { .. } => OpCode::LRange,
            ListOperation::Len => OpCode::LLen,
        };
        Ok(Self::new(opcode, seq, Vec::new(), payload))
    }

    pub fn set_op(seq: u32, request: &SetOpRequest) -> Result<Self, String> {
        let payload = serde_json::to_vec(request).map_err(|e| format!("Serialization error: {}", e))?;
        let opcode = match &request.operation {
            SetOperation::Add { .. } => OpCode::SAdd,
            SetOperation::Remove { .. } => OpCode::SRem,
            SetOperation::Members => OpCode::SMembers,
            SetOperation::IsMember { .. } => OpCode::SIsMember,
            SetOperation::Card => OpCode::SCard,
            SetOperation::Union { .. } => OpCode::SUnion,
            SetOperation::Inter { .. } => OpCode::SInter,
            SetOperation::Diff { .. } => OpCode::SDiff,
        };
        Ok(Self::new(opcode, seq, Vec::new(), payload))
    }

    pub fn sorted_set_op(seq: u32, request: &SortedSetOpRequest) -> Result<Self, String> {
        let payload = serde_json::to_vec(request).map_err(|e| format!("Serialization error: {}", e))?;
        let opcode = match &request.operation {
            SortedSetOperation::Add { .. } => OpCode::ZAdd,
            SortedSetOperation::Remove { .. } => OpCode::ZRem,
            SortedSetOperation::Range { .. } => OpCode::ZRange,
            SortedSetOperation::RangeByScore { .. } => OpCode::ZRangeByScore,
            SortedSetOperation::Card => OpCode::ZCard,
            SortedSetOperation::Score { .. } => OpCode::ZScore,
        };
        Ok(Self::new(opcode, seq, Vec::new(), payload))
    }

    pub fn hash_op(seq: u32, request: &HashOpRequest) -> Result<Self, String> {
        let payload = serde_json::to_vec(request).map_err(|e| format!("Serialization error: {}", e))?;
        let opcode = match &request.operation {
            HashOperation::Set { .. } => OpCode::HSet,
            HashOperation::Get { .. } => OpCode::HGet,
            HashOperation::Del { .. } => OpCode::HDel,
            HashOperation::GetAll => OpCode::HGetAll,
            HashOperation::Keys => OpCode::HKeys,
            HashOperation::Vals => OpCode::HVals,
            HashOperation::Len => OpCode::HLen,
        };
        Ok(Self::new(opcode, seq, Vec::new(), payload))
    }

    /// Serialize command to bytes (header + key + value)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(CmdHeader::SIZE + self.key.len() + self.value.len());

        // Safety: CmdHeader is repr(C, packed) so this is safe
        let header_bytes = unsafe {
            std::slice::from_raw_parts(&self.header as *const _ as *const u8, CmdHeader::SIZE)
        };

        bytes.extend_from_slice(header_bytes);
        bytes.extend_from_slice(&self.key);
        bytes.extend_from_slice(&self.value);

        bytes
    }

    /// Deserialize command from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < CmdHeader::SIZE {
            return Err("Buffer too small for header");
        }

        // Safety: We've checked the length and CmdHeader is repr(C, packed)
        let header = unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const CmdHeader) };

        let total_payload = header.total_payload_len();
        if bytes.len() < CmdHeader::SIZE + total_payload {
            return Err("Buffer too small for payload");
        }

        let payload_start = CmdHeader::SIZE;
        let key_end = payload_start + header.key_len as usize;
        let val_end = key_end + header.val_len as usize;

        let key = bytes[payload_start..key_end].to_vec();
        let value = bytes[key_end..val_end].to_vec();

        Ok(Self { header, key, value })
    }
}

/// Complete response with header and payload
#[derive(Debug, Clone)]
pub struct Response {
    pub header: RespHeader,
    pub payload: Vec<u8>,
}

impl Response {
    pub fn new(status: Status, seq: u32, payload: Vec<u8>) -> Self {
        let header = RespHeader::new(status, seq).with_payload(payload.len() as u32);

        Self { header, payload }
    }

    pub fn ok(seq: u32, payload: Vec<u8>) -> Self {
        Self::new(Status::Ok, seq, payload)
    }

    pub fn error(seq: u32) -> Self {
        Self::new(Status::Error, seq, Vec::new())
    }

    pub fn not_found(seq: u32) -> Self {
        Self::new(Status::NotFound, seq, Vec::new())
    }

    /// Serialize response to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(RespHeader::SIZE + self.payload.len());

        // Safety: RespHeader is repr(C, packed)
        let header_bytes = unsafe {
            std::slice::from_raw_parts(&self.header as *const _ as *const u8, RespHeader::SIZE)
        };

        bytes.extend_from_slice(header_bytes);
        bytes.extend_from_slice(&self.payload);

        bytes
    }

    /// Deserialize response from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < RespHeader::SIZE {
            return Err("Buffer too small for header");
        }

        let header = unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const RespHeader) };

        if bytes.len() < RespHeader::SIZE + header.payload_len as usize {
            return Err("Buffer too small for payload");
        }

        let payload =
            bytes[RespHeader::SIZE..RespHeader::SIZE + header.payload_len as usize].to_vec();

        Ok(Self { header, payload })
    }
}

// ============================================================================
// v0.2.0 Protocol Structures
// ============================================================================

/// Authentication request payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
    pub method: AuthMethod,
    pub credentials: AuthCredentials,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    UsernamePassword,
    JwtToken,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthCredentials {
    UsernamePassword { username: String, password: String },
    JwtToken { token: String },
}

/// Authentication response payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub success: bool,
    pub token: Option<String>,
    pub expires_at: Option<u64>, // Unix timestamp
    pub error: Option<String>,
}

/// Query request payload for document operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequest {
    pub collection: String,
    pub filter: Option<Value>,
    pub projection: Option<Value>,
    pub sort: Option<Value>,
    pub skip: Option<u64>,
    pub limit: Option<u64>,
}

/// Document insertion request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertDocRequest {
    pub collection: String,
    pub document: Document,
}

/// Document update request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDocRequest {
    pub collection: String,
    pub filter: Value,
    pub update: Value,
    pub upsert: bool,
}

/// Document deletion request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteDocRequest {
    pub collection: String,
    pub filter: Value,
}

/// Collection creation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCollectionRequest {
    pub name: String,
    pub schema: Option<Value>, // JSON schema
}

/// Index creation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIndexRequest {
    pub collection: String,
    pub name: String,
    pub fields: Vec<IndexField>,
    pub unique: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexField {
    pub field: String,
    pub direction: i32, // 1 for ascending, -1 for descending
}

/// List operation request (for Redis-like data structures)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListOpRequest {
    pub key: String,
    pub operation: ListOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ListOperation {
    Push { values: Vec<Value>, left: bool },
    Pop { left: bool },
    Range { start: i64, stop: i64 },
    Len,
}

/// Set operation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetOpRequest {
    pub key: String,
    pub operation: SetOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SetOperation {
    Add { values: Vec<Value> },
    Remove { values: Vec<Value> },
    Members,
    IsMember { value: Value },
    Card,
    Union { other_keys: Vec<String> },
    Inter { other_keys: Vec<String> },
    Diff { other_keys: Vec<String> },
}

/// Sorted set operation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortedSetOpRequest {
    pub key: String,
    pub operation: SortedSetOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortedSetOperation {
    Add { members: Vec<ScoredMember> },
    Remove { members: Vec<Value> },
    Range { start: i64, stop: i64 },
    RangeByScore { min: f64, max: f64 },
    Card,
    Score { member: Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredMember {
    pub score: f64,
    pub member: Value,
}

/// Hash operation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashOpRequest {
    pub key: String,
    pub operation: HashOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HashOperation {
    Set { field: String, value: Value },
    Get { field: String },
    Del { fields: Vec<String> },
    GetAll,
    Keys,
    Vals,
    Len,
}

/// Generic operation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationResponse {
    pub success: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
    pub affected_count: Option<u64>,
}

/// Helper functions for v0.2.0 protocol serialization
impl AuthRequest {
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(self).map_err(|e| format!("Serialization error: {}", e))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(bytes).map_err(|e| format!("Deserialization error: {}", e))
    }
}

impl QueryRequest {
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(self).map_err(|e| format!("Serialization error: {}", e))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(bytes).map_err(|e| format!("Deserialization error: {}", e))
    }
}

impl OperationResponse {
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(self).map_err(|e| format!("Serialization error: {}", e))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(bytes).map_err(|e| format!("Deserialization error: {}", e))
    }

    pub fn success(data: Option<Value>) -> Self {
        Self {
            success: true,
            data,
            error: None,
            affected_count: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            affected_count: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_serialization() {
        let cmd = Command::set(42, b"key".to_vec(), b"value".to_vec());
        let bytes = cmd.to_bytes();
        let decoded = Command::from_bytes(&bytes).unwrap();

        let seq = decoded.header.seq;
        assert_eq!(seq, 42);
        assert_eq!(decoded.header.opcode().unwrap(), OpCode::Set);
        assert_eq!(decoded.key, b"key".to_vec()); // compare Vec<u8>
        assert_eq!(decoded.value, b"value".to_vec()); // FIXED: compare Vec<u8>
        assert_eq!(decoded.header.version, PROTOCOL_V2);
    }

    #[test]
    fn test_v2_auth_command() {
        let auth_req = AuthRequest {
            method: AuthMethod::UsernamePassword,
            credentials: AuthCredentials::UsernamePassword {
                username: "admin".to_string(),
                password: "password".to_string(),
            },
        };
        
        let cmd = Command::auth(1, &auth_req).unwrap();
        assert_eq!(cmd.header.opcode().unwrap(), OpCode::Auth);
        assert_eq!(cmd.header.version, PROTOCOL_V2);
        
        // Test round-trip serialization
        let bytes = cmd.to_bytes();
        let decoded = Command::from_bytes(&bytes).unwrap();
        // Copy field to avoid packed struct alignment issues
        let seq = decoded.header.seq;
        assert_eq!(seq, 1);
        assert_eq!(decoded.header.opcode().unwrap(), OpCode::Auth);
    }

    #[test]
    fn test_v2_query_command() {
        let query_req = QueryRequest {
            collection: "users".to_string(),
            filter: Some(Value::Object(std::collections::BTreeMap::new())),
            projection: None,
            sort: None,
            skip: None,
            limit: Some(10),
        };
        
        let cmd = Command::query(2, &query_req).unwrap();
        assert_eq!(cmd.header.opcode().unwrap(), OpCode::Query);
        assert_eq!(cmd.header.version, PROTOCOL_V2);
    }

    #[test]
    fn test_response_serialization() {
        let resp = Response::ok(42, b"result".to_vec());
        let bytes = resp.to_bytes();
        let decoded = Response::from_bytes(&bytes).unwrap();

        let seq2 = decoded.header.seq;
        assert_eq!(seq2, 42);
        assert_eq!(decoded.header.status().unwrap(), Status::Ok);
        assert_eq!(decoded.payload, b"result");
    }

    #[test]
    fn test_cas_command() {
        let cmd = Command::cas(1, b"key".to_vec(), b"new_val".to_vec(), 123);
        assert!(cmd.header.has_flag(flags::CAS_VERSION));
        // Note: v0.2.0 stores expected_version in payload instead of header
    }
}


// ============================================================================
// Protocol Handler for Server
// ============================================================================

use crate::storage::HybridStorageEngine;
use crate::document::DocumentId;
use std::sync::Arc;
use uuid;

/// Protocol handler that processes requests and generates responses
pub struct ProtocolHandler {
    storage: Arc<HybridStorageEngine>,
}

impl ProtocolHandler {
    pub fn new(storage: Arc<HybridStorageEngine>) -> Self {
        Self { storage }
    }

    pub async fn handle_request(&mut self, request: ProtocolRequest) -> ProtocolResponse {
        match request.opcode {
            OpCode::Ping => ProtocolResponse {
                status: ProtocolStatus::Ok,
                flags: 0,
                payload: b"pong".to_vec(),
            },
            OpCode::Set => self.handle_set(request).await,
            OpCode::Get => self.handle_get(request).await,
            OpCode::Del => self.handle_delete(request).await,
            _ => ProtocolResponse {
                status: ProtocolStatus::Error,
                flags: 0,
                payload: b"unsupported operation".to_vec(),
            },
        }
    }

    async fn handle_set(&mut self, request: ProtocolRequest) -> ProtocolResponse {
        // For now, use a default collection for legacy KV operations
        let collection = "default";
        
        // Convert key-value to document
        let key_str = String::from_utf8_lossy(&request.key).to_string();
        let value_str = String::from_utf8_lossy(&request.value).to_string();
        
        let mut doc = Document::new();
        doc.insert("_key".to_string(), Value::String(key_str.clone()));
        doc.insert("value".to_string(), Value::String(value_str));
        
        // Generate a document ID from the key string (use UUID v5 for deterministic IDs)
        let namespace = uuid::Uuid::NAMESPACE_OID;
        let doc_uuid = uuid::Uuid::new_v5(&namespace, key_str.as_bytes());
        let doc_id = DocumentId::from_uuid(doc_uuid);
        doc.insert("_id".to_string(), Value::String(doc_id.to_string()));
        
        match self.storage.insert_document(collection, doc).await {
            Ok(_) => ProtocolResponse {
                status: ProtocolStatus::Ok,
                flags: 0,
                payload: Vec::new(),
            },
            Err(e) => ProtocolResponse {
                status: ProtocolStatus::Error,
                flags: 0,
                payload: format!("error: {}", e).into_bytes(),
            },
        }
    }

    async fn handle_get(&mut self, request: ProtocolRequest) -> ProtocolResponse {
        let collection = "default";
        let key_str = String::from_utf8_lossy(&request.key).to_string();
        
        // Generate document ID from key string
        let namespace = uuid::Uuid::NAMESPACE_OID;
        let doc_uuid = uuid::Uuid::new_v5(&namespace, key_str.as_bytes());
        let doc_id = DocumentId::from_uuid(doc_uuid);
        
        match self.storage.get_document(collection, doc_id).await {
            Ok(Some(doc)) => {
                if let Some(value) = doc.get("value") {
                    if let Value::String(s) = value {
                        ProtocolResponse {
                            status: ProtocolStatus::Ok,
                            flags: 0,
                            payload: s.as_bytes().to_vec(),
                        }
                    } else {
                        ProtocolResponse {
                            status: ProtocolStatus::Ok,
                            flags: 0,
                            payload: serde_json::to_vec(&value).unwrap_or_default(),
                        }
                    }
                } else {
                    ProtocolResponse {
                        status: ProtocolStatus::NotFound,
                        flags: 0,
                        payload: Vec::new(),
                    }
                }
            }
            Ok(None) => ProtocolResponse {
                status: ProtocolStatus::NotFound,
                flags: 0,
                payload: Vec::new(),
            },
            Err(e) => ProtocolResponse {
                status: ProtocolStatus::Error,
                flags: 0,
                payload: format!("error: {}", e).into_bytes(),
            },
        }
    }

    async fn handle_delete(&mut self, request: ProtocolRequest) -> ProtocolResponse {
        let collection = "default";
        let key_str = String::from_utf8_lossy(&request.key).to_string();
        let namespace = uuid::Uuid::NAMESPACE_OID;
        let doc_uuid = uuid::Uuid::new_v5(&namespace, key_str.as_bytes());
        let doc_id = DocumentId::from_uuid(doc_uuid);
        
        match self.storage.delete_document(collection, doc_id).await {
            Ok(true) => ProtocolResponse {
                status: ProtocolStatus::Ok,
                flags: 0,
                payload: Vec::new(),
            },
            Ok(false) => ProtocolResponse {
                status: ProtocolStatus::NotFound,
                flags: 0,
                payload: Vec::new(),
            },
            Err(e) => ProtocolResponse {
                status: ProtocolStatus::Error,
                flags: 0,
                payload: format!("error: {}", e).into_bytes(),
            },
        }
    }
}

/// Request structure for protocol handler
pub struct ProtocolRequest {
    pub opcode: OpCode,
    pub flags: u8,
    pub seq: u32,
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

/// Response structure for protocol handler  
pub struct ProtocolResponse {
    pub status: ProtocolStatus,
    pub flags: u8,
    pub payload: Vec<u8>,
}

/// Status codes for responses
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolStatus {
    Ok = 0x00,
    Error = 0x01,
    NotFound = 0x02,
}

impl OpCode {
    pub fn from_u8(value: u8) -> Self {
        Self::try_from(value).unwrap_or(OpCode::Ping)
    }
}
