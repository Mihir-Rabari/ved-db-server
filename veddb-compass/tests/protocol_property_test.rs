//! Property-based tests for VedDB binary protocol correctness
//! 
//! **Feature: compass-connection-fix, Property 7: Binary Protocol Correctness**
//! **Validates: Requirements 2.1, 2.3, 2.5**
//! 
//! This test verifies that the binary protocol implementation is correct:
//! - Generate random commands
//! - Verify byte layout: CmdHeader (24 bytes) + key + value
//! - Verify little-endian encoding
//! - Verify protocol version is PROTOCOL_V2

use proptest::prelude::*;
use veddb_client::{Command, OpCode, PROTOCOL_V2, bytes::Bytes};

/// Strategy for generating random OpCodes
fn opcode_strategy() -> impl Strategy<Value = OpCode> {
    prop_oneof![
        Just(OpCode::Ping),
        Just(OpCode::Set),
        Just(OpCode::Get),
        Just(OpCode::Delete),
        Just(OpCode::Cas),
        Just(OpCode::Auth),
        Just(OpCode::Query),
        Just(OpCode::InsertDoc),
        Just(OpCode::UpdateDoc),
        Just(OpCode::DeleteDoc),
        Just(OpCode::CreateCollection),
        Just(OpCode::CreateIndex),
    ]
}

/// Strategy for generating random byte sequences (keys and values)
fn bytes_strategy() -> impl Strategy<Value = Bytes> {
    prop::collection::vec(any::<u8>(), 0..256)
        .prop_map(|v| Bytes::from(v))
}

/// Strategy for generating random sequence numbers
fn seq_strategy() -> impl Strategy<Value = u32> {
    any::<u32>()
}

/// Strategy for generating random extra data (for CAS operations, etc.)
fn extra_strategy() -> impl Strategy<Value = u64> {
    any::<u64>()
}

proptest! {
    /// Property Test: Command serialization produces correct byte layout
    /// 
    /// For any command, the serialized bytes should have:
    /// - Header: exactly 24 bytes
    /// - Followed by key bytes
    /// - Followed by value bytes
    /// - Total length = 24 + key_len + value_len
    #[test]
    fn prop_command_byte_layout(
        seq in seq_strategy(),
        key in bytes_strategy(),
        value in bytes_strategy()
    ) {
        // Create a SET command (which has both key and value)
        let cmd = Command::set(seq, key.clone(), value.clone());
        
        // Serialize to bytes
        let bytes = cmd.to_bytes();
        
        // Verify total length
        let expected_len = 24 + key.len() + value.len();
        prop_assert_eq!(
            bytes.len(),
            expected_len,
            "Total byte length should be 24 (header) + {} (key) + {} (value) = {}",
            key.len(),
            value.len(),
            expected_len
        );
        
        // Verify header is 24 bytes
        prop_assert!(
            bytes.len() >= 24,
            "Serialized command must have at least 24 bytes for header"
        );
        
        // Verify key and value are present after header
        if key.len() > 0 {
            prop_assert_eq!(
                &bytes[24..24 + key.len()],
                &key[..],
                "Key bytes should match at offset 24"
            );
        }
        
        if value.len() > 0 {
            prop_assert_eq!(
                &bytes[24 + key.len()..24 + key.len() + value.len()],
                &value[..],
                "Value bytes should match after key"
            );
        }
    }

    /// Property Test: Command header uses little-endian encoding
    /// 
    /// For any command, all multi-byte fields in the header should be
    /// encoded in little-endian format
    #[test]
    fn prop_command_little_endian_encoding(
        seq in seq_strategy(),
        key in bytes_strategy(),
        value in bytes_strategy()
    ) {
        // Create command
        let cmd = Command::set(seq, key.clone(), value.clone());
        let bytes = cmd.to_bytes();
        
        // Verify sequence number is little-endian (bytes 4-7)
        let seq_bytes = [bytes[4], bytes[5], bytes[6], bytes[7]];
        let decoded_seq = u32::from_le_bytes(seq_bytes);
        prop_assert_eq!(
            decoded_seq,
            seq,
            "Sequence number should be encoded in little-endian"
        );
        
        // Verify key_len is little-endian (bytes 8-11)
        let key_len_bytes = [bytes[8], bytes[9], bytes[10], bytes[11]];
        let decoded_key_len = u32::from_le_bytes(key_len_bytes);
        prop_assert_eq!(
            decoded_key_len,
            key.len() as u32,
            "Key length should be encoded in little-endian"
        );
        
        // Verify value_len is little-endian (bytes 12-15)
        let value_len_bytes = [bytes[12], bytes[13], bytes[14], bytes[15]];
        let decoded_value_len = u32::from_le_bytes(value_len_bytes);
        prop_assert_eq!(
            decoded_value_len,
            value.len() as u32,
            "Value length should be encoded in little-endian"
        );
    }

    /// Property Test: Command header version is PROTOCOL_V2
    /// 
    /// For any command created, the version field should always be set to PROTOCOL_V2 (0x02)
    #[test]
    fn prop_command_protocol_version(
        seq in seq_strategy(),
        key in bytes_strategy(),
        value in bytes_strategy()
    ) {
        // Create command (defaults to PROTOCOL_V2)
        let cmd = Command::set(seq, key, value);
        let bytes = cmd.to_bytes();
        
        // Verify version field (byte 2) is PROTOCOL_V2
        prop_assert_eq!(
            bytes[2],
            PROTOCOL_V2,
            "Protocol version should be PROTOCOL_V2 (0x02)"
        );
    }

    /// Property Test: Command header opcode is correctly encoded
    /// 
    /// For any command, the opcode should be correctly encoded as the first byte
    #[test]
    fn prop_command_opcode_encoding(
        seq in seq_strategy(),
        key in bytes_strategy(),
        value in bytes_strategy()
    ) {
        // Create SET command
        let cmd = Command::set(seq, key, value);
        let bytes = cmd.to_bytes();
        
        // Verify opcode is the first byte (SET = 0x02)
        prop_assert_eq!(
            bytes[0],
            OpCode::Set as u8,
            "Opcode should be encoded as the first byte"
        );
    }

    /// Property Test: Command header structure is exactly 24 bytes
    /// 
    /// For any command, the header should always be exactly 24 bytes,
    /// regardless of key and value sizes
    #[test]
    fn prop_command_header_size(
        seq in seq_strategy(),
        key in bytes_strategy(),
        value in bytes_strategy()
    ) {
        // Create command
        let cmd = Command::set(seq, key.clone(), value.clone());
        let bytes = cmd.to_bytes();
        
        // The header is always 24 bytes, so the payload starts at byte 24
        // Verify that the total length is 24 + key_len + value_len
        let expected_total = 24 + key.len() + value.len();
        prop_assert_eq!(
            bytes.len(),
            expected_total,
            "Total length should be 24 (header) + key_len + value_len"
        );
        
        // Verify the header fields are within the first 24 bytes
        // Opcode: byte 0
        // Flags: byte 1
        // Version: byte 2
        // Reserved: byte 3
        // Seq: bytes 4-7 (4 bytes)
        // Key_len: bytes 8-11 (4 bytes)
        // Value_len: bytes 12-15 (4 bytes)
        // Extra: bytes 16-23 (8 bytes)
        // Total: 24 bytes
        
        // Verify we can read all header fields from the first 24 bytes
        prop_assert!(bytes.len() >= 24, "Must have at least 24 bytes for header");
    }

    /// Property Test: Key and value lengths are correctly encoded
    /// 
    /// For any command, the key_len and value_len fields in the header
    /// should match the actual lengths of the key and value
    #[test]
    fn prop_command_length_fields(
        seq in seq_strategy(),
        key in bytes_strategy(),
        value in bytes_strategy()
    ) {
        // Create command
        let cmd = Command::set(seq, key.clone(), value.clone());
        let bytes = cmd.to_bytes();
        
        // Extract key_len from header (bytes 8-11, little-endian)
        let key_len_bytes = [bytes[8], bytes[9], bytes[10], bytes[11]];
        let encoded_key_len = u32::from_le_bytes(key_len_bytes);
        
        // Extract value_len from header (bytes 12-15, little-endian)
        let value_len_bytes = [bytes[12], bytes[13], bytes[14], bytes[15]];
        let encoded_value_len = u32::from_le_bytes(value_len_bytes);
        
        // Verify lengths match
        prop_assert_eq!(
            encoded_key_len,
            key.len() as u32,
            "Encoded key_len should match actual key length"
        );
        
        prop_assert_eq!(
            encoded_value_len,
            value.len() as u32,
            "Encoded value_len should match actual value length"
        );
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    /// Unit test: Verify PING command has correct structure
    #[test]
    fn test_ping_command_structure() {
        let cmd = Command::ping(42);
        let bytes = cmd.to_bytes();
        
        // PING should have 24-byte header with no key or value
        assert_eq!(bytes.len(), 24, "PING command should be exactly 24 bytes");
        assert_eq!(bytes[0], OpCode::Ping as u8, "First byte should be PING opcode");
        assert_eq!(bytes[2], PROTOCOL_V2, "Version should be PROTOCOL_V2");
        
        // Verify sequence number
        let seq_bytes = [bytes[4], bytes[5], bytes[6], bytes[7]];
        let seq = u32::from_le_bytes(seq_bytes);
        assert_eq!(seq, 42, "Sequence number should be 42");
        
        // Verify key_len and value_len are 0
        let key_len_bytes = [bytes[8], bytes[9], bytes[10], bytes[11]];
        let key_len = u32::from_le_bytes(key_len_bytes);
        assert_eq!(key_len, 0, "PING should have key_len = 0");
        
        let value_len_bytes = [bytes[12], bytes[13], bytes[14], bytes[15]];
        let value_len = u32::from_le_bytes(value_len_bytes);
        assert_eq!(value_len, 0, "PING should have value_len = 0");
    }

    /// Unit test: Verify SET command has correct structure
    #[test]
    fn test_set_command_structure() {
        let cmd = Command::set(100, "mykey", "myvalue");
        let bytes = cmd.to_bytes();
        
        // SET should have 24-byte header + key + value
        let expected_len = 24 + 5 + 7; // "mykey" = 5 bytes, "myvalue" = 7 bytes
        assert_eq!(bytes.len(), expected_len, "SET command should be 24 + 5 + 7 = 36 bytes");
        assert_eq!(bytes[0], OpCode::Set as u8, "First byte should be SET opcode");
        assert_eq!(bytes[2], PROTOCOL_V2, "Version should be PROTOCOL_V2");
        
        // Verify key and value
        assert_eq!(&bytes[24..29], b"mykey", "Key should be at offset 24");
        assert_eq!(&bytes[29..36], b"myvalue", "Value should follow key");
    }

    /// Unit test: Verify GET command has correct structure
    #[test]
    fn test_get_command_structure() {
        let cmd = Command::get(200, "testkey");
        let bytes = cmd.to_bytes();
        
        // GET should have 24-byte header + key (no value)
        let expected_len = 24 + 7; // "testkey" = 7 bytes
        assert_eq!(bytes.len(), expected_len, "GET command should be 24 + 7 = 31 bytes");
        assert_eq!(bytes[0], OpCode::Get as u8, "First byte should be GET opcode");
        
        // Verify value_len is 0
        let value_len_bytes = [bytes[12], bytes[13], bytes[14], bytes[15]];
        let value_len = u32::from_le_bytes(value_len_bytes);
        assert_eq!(value_len, 0, "GET should have value_len = 0");
        
        // Verify key
        assert_eq!(&bytes[24..31], b"testkey", "Key should be at offset 24");
    }

    /// Unit test: Verify CAS command includes extra field
    #[test]
    fn test_cas_command_with_extra() {
        let expected_version = 12345u64;
        let cmd = Command::cas(300, "caskey", expected_version, "casvalue");
        let bytes = cmd.to_bytes();
        
        // Verify extra field contains the expected version
        let extra_bytes = [
            bytes[16], bytes[17], bytes[18], bytes[19],
            bytes[20], bytes[21], bytes[22], bytes[23]
        ];
        let extra = u64::from_le_bytes(extra_bytes);
        assert_eq!(extra, expected_version, "Extra field should contain expected version");
    }

    /// Unit test: Verify empty key and value work correctly
    #[test]
    fn test_command_with_empty_key_and_value() {
        let cmd = Command::set(1, Bytes::new(), Bytes::new());
        let bytes = cmd.to_bytes();
        
        // Should be exactly 24 bytes (header only)
        assert_eq!(bytes.len(), 24, "Command with empty key and value should be 24 bytes");
        
        // Verify key_len and value_len are 0
        let key_len_bytes = [bytes[8], bytes[9], bytes[10], bytes[11]];
        let key_len = u32::from_le_bytes(key_len_bytes);
        assert_eq!(key_len, 0, "key_len should be 0");
        
        let value_len_bytes = [bytes[12], bytes[13], bytes[14], bytes[15]];
        let value_len = u32::from_le_bytes(value_len_bytes);
        assert_eq!(value_len, 0, "value_len should be 0");
    }

    /// Unit test: Verify large key and value work correctly
    #[test]
    fn test_command_with_large_key_and_value() {
        let large_key = vec![0xAB; 1000];
        let large_value = vec![0xCD; 2000];
        
        let cmd = Command::set(999, Bytes::from(large_key.clone()), Bytes::from(large_value.clone()));
        let bytes = cmd.to_bytes();
        
        // Verify total length
        let expected_len = 24 + 1000 + 2000;
        assert_eq!(bytes.len(), expected_len, "Total length should be 24 + 1000 + 2000");
        
        // Verify key_len and value_len
        let key_len_bytes = [bytes[8], bytes[9], bytes[10], bytes[11]];
        let key_len = u32::from_le_bytes(key_len_bytes);
        assert_eq!(key_len, 1000, "key_len should be 1000");
        
        let value_len_bytes = [bytes[12], bytes[13], bytes[14], bytes[15]];
        let value_len = u32::from_le_bytes(value_len_bytes);
        assert_eq!(value_len, 2000, "value_len should be 2000");
        
        // Verify key and value content
        assert_eq!(&bytes[24..1024], &large_key[..], "Key content should match");
        assert_eq!(&bytes[1024..3024], &large_value[..], "Value content should match");
    }
}
