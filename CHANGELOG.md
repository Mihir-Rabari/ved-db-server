# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## \[0.0.1] - 2025-08-23

### Added

* Initial **VedDB Server** implementation for **Windows**.
* Core **in-memory key-value store** with shared memory arena allocator.
* SPSC ring buffers for zero-copy local IPC.
* **Command/response protocol** supporting:

  * `GET`
  * `SET`
  * `DELETE`
* Worker pool with **session management** and thread-safe operations.
* Experimental **QUIC/gRPC networking layer** (prototype; wire format may change).
* Structured **error handling** with `ClientError` and `Status`.
* Server binary released for **Windows only**; Linux/macOS builds are experimental.

### Notes

* This release is **server-only**; client implementations exist in separate repositories.
* Pub/Sub and CAS operations are stubbed or planned for future releases.
* Designed for testing shared memory performance and command handling.
* Future releases will include official multi-platform support, persistence, and metrics.

---
