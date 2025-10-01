//! Binary command protocol definitions
//!
//! Defines the compact binary format used for all VedDB operations,
//! both in shared memory rings and over gRPC/QUIC connections.

use std::mem;

/// Command opcodes
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
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
}

impl TryFrom<u8> for OpCode {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
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
            _ => Err(()),
        }
    }
}

/// Command header (24 bytes, little-endian)
///
/// This is the wire format used in ring buffers and over the network.
/// All fields are little-endian for consistency across platforms.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct CmdHeader {
    pub op: u8,        // OpCode
    pub flags: u8,     // Command flags
    pub reserved: u16, // Reserved for future use
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

    pub fn with_extra(mut self, extra: u64) -> Self {
        self.extra = extra;
        self
    }

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
    pub extra: u64,       // Version, offset, or other metadata
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
            extra: 0,
        }
    }

    pub fn with_payload(mut self, len: u32) -> Self {
        self.payload_len = len;
        self
    }

    pub fn with_extra(mut self, extra: u64) -> Self {
        self.extra = extra;
        self
    }

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
        cmd.header.extra = expected_version;
        cmd
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
        let extra = cmd.header.extra;
        assert_eq!(extra, 123);
    }
}
