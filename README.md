# VedDB — High-Performance Shared Memory KV Store + Pub/Sub

![status-badge](https://img.shields.io/badge/status-active-brightgreen)
![license-badge](https://img.shields.io/badge/license-MIT-blue)
![rust-badge](https://img.shields.io/badge/rust-stable-orange)
![platforms-badge](https://img.shields.io/badge/platforms-windows%20%7C%20linux%20%7C%20macOS-informational)

VedDB is a single-node, high-throughput, zero-protocol shared-memory in-memory KV store with Pub/Sub capabilities and remote connectivity via gRPC/QUIC. It is designed for ultra-low latency local access and scalable throughput via sharding.

---

## Table of Contents

- [Features](#features)
- [Architecture](#architecture)
- [Quick Start](#quick-start)
- [Install and Build](#install-and-build)
- [Configuration](#configuration)
- [Running as a Windows Service](#running-as-a-windows-service)
- [Project Structure](#project-structure)
- [Development](#development)
- [Roadmap](#roadmap)
- [Changelog](#changelog)
- [Security](#security)
- [License](#license)

## Features

- **Zero-copy local access** via shared memory (mmap/shm)
- **Sub-10µs latency** for local GET/SET operations
- **Millions of operations/sec** with CPU core pinning and sharding
- **Topic-based Pub/Sub** with MPMC delivery
- **Remote connectivity** via gRPC streaming and QUIC
- **Language bindings** for Go, Python, Node.js via stable C ABI
- **Optional persistence** with WAL and snapshots

Additional goals:
- **Observability** with structured logs and pluggable metrics
- **Simple deployment**: Static binary, minimal runtime deps

## Architecture

```
┌──────────────────┐      ┌──────────────────────┐      ┌──────────────────┐
│ Local Service 1  │      │ VedDB Core (Rust)    │      │ Local Service N  │
│ (client lib)     │ <──> │ - Session Manager    │ <──> │ (client lib)     │
│ writes to rings  │      │ - Sharded KV         │      │ reads rings      │
└──────────────────┘      │ - Topic Manager      │      └──────────────────┘
                          │ - Workers (pinned)   │
                          │ - gRPC/QUIC bridge   │
                          └──────────────────────┘
                                     ↑
                                     │
                       Remote clients via gRPC/QUIC
```

## Quick Start

```bash
# Build (workspace)
cargo build --release

# Or build server only
cargo build --release -p veddb-server

# Run the server (creates instance if missing)
target/release/veddb-server --create --name veddb_main --memory-mb 256 --workers 4 --port 50051 --debug
```

Windows (PowerShell):
```powershell
./target/release/veddb-server.exe --create --name veddb_main --memory-mb 256 --workers 4 --port 50051 --debug
```

Linux/macOS:
```sh
./target/release/veddb-server --create --name veddb_main --memory-mb 256 --workers 4 --port 50051 --debug
```

Benchmarks (if enabled):
```sh
cargo bench
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for internal design details.

## Install and Build

Prereqs:
- Rust stable (1.75+ recommended)
- Windows, Linux, or macOS

Build from source:
```sh
git clone <your-repo-url>
cd veddb
cargo build --release -p veddb-server
```

Artifacts:
- Windows: `target\release\veddb-server.exe`
- Linux/macOS: `target/release/veddb-server`

## Configuration

Server flags (from `veddb-server/src/main.rs`):
- `--name <string>`: shared memory name (default: `veddb_main`)
- `--memory-mb <usize>`: memory size in MB (default: `64`)
- `--workers <usize>`: number of worker threads (default: `4`)
- `--port <u16>`: gRPC port (default: `50051`)
- `--session-timeout <u64>`: seconds (default: `300`)
- `--create`: create instance if missing
- `--debug`: enable debug logging

Examples:
```sh
# Production-ish example
./veddb-server --create --name prod_db --memory-mb 1024 --workers 8 --port 50051

# Redirect logs
./veddb-server --create --name prod_db --memory-mb 1024 > veddb.log 2>&1
```

## Running as a Windows Service

Using `sc` (built-in):
```powershell
sc create VedDbServer binPath= "\"C:\\path\\to\\veddb-server.exe\" --create --name prod_db --memory-mb 1024 --workers 8 --port 50051" start= auto
sc start VedDbServer
sc stop VedDbServer
sc delete VedDbServer
```

Using NSSM (recommended for easier management):
```powershell
nssm install VedDbServer "C:\\path\\to\\veddb-server.exe" --create --name prod_db --memory-mb 1024 --workers 8 --port 50051
nssm set VedDbServer Start SERVICE_AUTO_START
nssm start VedDbServer
```

## Performance Goals

- **Latency**: Sub-10µs p50 for local operations
- **Throughput**: Millions of small messages/sec
- **Scalability**: Linear scaling with CPU cores via sharding
- **Memory**: Predictable allocation with arena-based management

## Project Structure

- `veddb-core/` - Core shared memory primitives and data structures
- `veddb-server/` - Main server process with gRPC/QUIC endpoints

Additional docs:
- [ARCHITECTURE.md](ARCHITECTURE.md) — deep dive into components and data flow
- [CONTRIBUTING.md](CONTRIBUTING.md) — how to build, test, and submit PRs
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) — community guidelines

## Development

Common tasks:
- Build: `cargo build --release -p veddb-server`
- Test: `cargo test`
- Format: `cargo fmt --all`
- Lints: `cargo clippy --all-targets -- -D warnings`

Release checklist:
- Update [CHANGELOG.md](CHANGELOG.md)
- Tag version and publish release artifacts

## Roadmap

See [ROADMAP.md](ROADMAP.md) for planned features and improvements.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for release notes.

## License

MIT License - see LICENSE file for details.
