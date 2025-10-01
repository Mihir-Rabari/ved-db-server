# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.1] - 2025-10-01

### Added

* **MSI Installer** for Windows with GUI wizard
  * Configurable installation path
  * Optional Windows Service installation
  * Automatic environment variable setup
  * Start menu shortcuts
  * Silent installation support
* **Installation Scripts** for automated setup
  * PowerShell installer for Windows
  * Bash installer for Linux/macOS
  * Automatic PATH configuration
* **Comprehensive Documentation**
  * Updated README with user-friendly installation
  * DOWNLOAD.md with platform-specific download links
  * BUILD_GUIDE.md for MSI installer creation
  * Reorganized docs to prioritize end users over developers

### Changed

* **Simplified Installation Process**
  * Installation now requires no build tools for end users
  * Pre-built binaries as primary distribution method
  * Building from source moved to developer section
* **README Structure**
  * Quick Start now shows download-and-install workflow
  * Installation section reorganized by platform
  * Added Docker installation instructions
  * Development section clearly marked for contributors

### Fixed

* Memory alignment issues in MPMC ring buffer
* Proper cleanup of aligned memory allocations
* Session registry alignment for AtomicU64 fields

### Documentation

* Created installer/BUILD_GUIDE.md with complete MSI build instructions
* Created ved-db/DOWNLOAD.md with platform-specific downloads
* Updated README.md with badges, emojis, and better structure
* Added performance benchmarks table
* Included roadmap timeline

---

## [0.0.1] - 2025-08-23

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
