# Changelog

All notable changes to VedDB Server will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [0.1.21] - 2025-10-02

### Added
- ✨ **LIST command** (opcode 0x09) - List all stored keys
- 🔧 **SimpleKvStore** - Lock-free KV store using DashMap for better concurrency
- 📝 **Detailed logging** - Enhanced server-side logging for debugging
- 🔄 **Protocol fixes** - Proper little-endian encoding throughout
- ✅ **Status code alignment** - Fixed client/server status code mismatch

### Changed
- 🚀 **Improved performance** - Replaced mutex-based KV store with DashMap
- 📊 **Better error handling** - Clearer error messages and status codes
- 🔧 **Simplified protocol** - Clean binary protocol implementation

### Fixed
- 🐛 **Packed struct issues** - Resolved undefined behavior from packed struct field access
- 🔄 **Endianness bugs** - Fixed big-endian/little-endian mismatches
- 📡 **Response header size** - Corrected from 16 to 20 bytes
- ⚡ **Deadlock issues** - Eliminated KV store deadlocks with lock-free implementation
- 🔌 **Connection handling** - Proper TCP stream management

### Technical Details
- Protocol now uses consistent little-endian encoding
- Response header: 20 bytes (status:1, flags:1, reserved:2, seq:4, payload_len:4, extra:8)
- Command header: 24 bytes (opcode:1, flags:1, reserved:2, seq:4, key_len:4, val_len:4, extra:8)
- Status codes: 0x00=OK, 0x01=NotFound, 0x04=InternalError

### Platform Support
- ✅ **Windows** - Fully tested and supported
- ⏳ **Linux/macOS** - Planned for future releases

---

## [0.1.0] - Initial Release

### Added
- Basic KV operations (SET, GET, DELETE)
- TCP server with binary protocol
- Multi-threaded worker pool
- Session management
- PING command for health checks

---

## Future Releases

### Planned for v0.2.x
- Persistence (Write-Ahead Log + Snapshots)
- Authentication and authorization
- Pub/Sub messaging
- TTL (time-to-live) for keys
- Pattern matching for LIST command

### Planned for v1.0.x
- Replication support
- Clustering
- Cross-platform support (Linux, macOS)
- gRPC protocol option
- Production-ready stability
