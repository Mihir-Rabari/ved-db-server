# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.0.1] - 2025-08-23
### Added
- Initial server implementation for **VedDB**.
- Core database engine with in-memory storage backend.
- Shared memory ring buffer (SPSC) for zero-copy communication.
- Basic command execution support (`GET`, `SET`, `DELETE`).
- Session management using unique session IDs.
- Error handling and structured responses with `OpCode` and `Status`.
- QUIC/gRPC stubs for planned networking layer integration.

### Notes
- This release is **server-only**.  
- Client implementations are maintained in a separate repository.  
- This version is a **prototype** release intended for testing shared memory performance.
