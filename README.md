# VedDB — High-Performance Shared Memory KV Store + Pub/Sub

![status-badge](https://img.shields.io/badge/status-active-brightgreen)
![license-badge](https://img.shields.io/badge/license-MIT-blue)
![rust-badge](https://img.shields.io/badge/rust-stable-orange)
![platforms-badge](https://img.shields.io/badge/platforms-windows%20%7C%20linux%20%7C%20macOS-informational)

VedDB is a single-node, high-throughput, zero-protocol shared-memory in-memory KV store with Pub/Sub capabilities and remote connectivity. It is designed for ultra‑low latency local access and scalable throughput via sharding.

## What is VedDB?

VedDB is a blazing‑fast, shared‑memory key‑value database built in Rust. It keeps your hottest data in a single machine’s memory and lets local processes talk to it with minimal overhead. Think: microservices on the same box exchanging data in microseconds with a tiny CPU footprint.

- __Local fast path__: processes on the same host interact via shared memory rings and arenas.
- __Network path__: an experimental TCP server today; gRPC/QUIC hardening on the roadmap.
- __Primitives__: sharded KV with CAS, session management, and a topic‑based Pub/Sub core.

---

## Table of Contents

- [Features](#features)
- [What is VedDB?](#what-is-veddb)
- [Architecture](#architecture)
- [Quick Start](#quick-start)
- [Install and Build](#install-and-build)
- [Configuration](#configuration)
- [Using VedDB](#using-veddb)
- [Running as a Windows Service](#running-as-a-windows-service)
- [Releases](#releases)
- [Project Structure](#project-structure)
- [Development](#development)
- [Roadmap](#roadmap)
- [Changelog](#changelog)
- [Security](#security)
- [Contact](#contact)
- [License](#license)

## Features

- **Zero-copy local access** via shared memory (mmap/shm)
- **Sub-10µs latency** for local GET/SET operations
- **Millions of operations/sec** with CPU core pinning and sharding
- **Topic-based Pub/Sub** with MPMC delivery
- **Remote connectivity** via gRPC streaming and QUIC
- **Language bindings** for Go, Python, Node.js via stable C ABI ( planned )
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

## Using VedDB

There are two ways to use VedDB today:

1) __Local (embedded) in Rust processes__ — link `veddb-core` and operate in‑process using the same memory region.

2) __Remote (experimental TCP)__ — connect to the server’s TCP port and exchange `Command` frames as raw bytes. A full gRPC service is planned.

### 1) Local (embedded) usage in Rust

Add to your Cargo project and use the `veddb-core` API to create or open an instance and execute commands:

```rust
use veddb_core::{VedDb, VedDbConfig, Command, Status};

fn main() {
    let config = VedDbConfig { memory_size: 128 * 1024 * 1024, ..Default::default() };
    // Create (or open via VedDb::open("my_db"))
    let db = VedDb::create("my_db", config).expect("create veddb");

    // SET key=value
    let set = Command::set(1, b"greeting".to_vec(), b"hello".to_vec());
    let r1 = db.process_command(set);
    assert_eq!(r1.header.status().unwrap(), Status::Ok);

    // GET key
    let get = Command::get(2, b"greeting".to_vec());
    let r2 = db.process_command(get);
    assert_eq!(r2.header.status().unwrap(), Status::Ok);
    assert_eq!(r2.payload, b"hello");
}
```

This is ideal for colocated services that want the absolute lowest latency and are comfortable using Rust.

### 2) Remote (experimental TCP) usage

`veddb-server` currently exposes a simple TCP listener on `--port` that accepts serialized `Command` messages and returns `Response` messages. Until the gRPC/QUIC surface is finalized, this is primarily for experimentation and internal testing.

- Protocol structs: see `veddb-core/src/protocol.rs` for `Command`/`Response` formats.
- Example flow: send `Command::set`, then `Command::get`, read back `Response` bytes.
- Compatibility note: wire format is not yet stable; expect breaking changes before v1.0.

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

## Releases

Prebuilt binaries are published for each tag starting with `v*` via GitHub Actions.

- Windows: `veddb-server-Windows.zip`
- Linux: `veddb-server-Linux.tar.gz`
- macOS: `veddb-server-macOS.tar.gz`

How to create a release:

```sh
git tag v0.1.0
git push origin v0.1.0
```

Then download artifacts from the GitHub Release page.

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
 - [SECURITY.md](SECURITY.md) — reporting vulnerabilities
 - [CHANGELOG.md](CHANGELOG.md) — notable changes and versions

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

## Security

See [SECURITY.md](SECURITY.md) for how to report vulnerabilities.

## Contact

- Email: __mihirrabari2604@gmail.com__
- Instagram: __@mihirrabariii__

## License

MIT License - see LICENSE file for details.
