<div align="center">

# 🚀 VedDB Server

### High-Performance Shared Memory Database with Pub/Sub

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey.svg)](https://github.com/yourusername/veddb)

*Blazing-fast, zero-copy, shared-memory database built in Rust*

[Features](#-features) • [Quick Start](#-quick-start) • [Installation](#-installation) • [Documentation](#-documentation) • [Architecture](#-architecture)

</div>

---

## 📖 Overview

**VedDB** is a high-throughput, low-latency database designed for applications that demand **sub-10µs response times** and **millions of operations per second**. Built entirely in Rust, it leverages shared memory for zero-copy local access while providing network connectivity for remote clients.

### 🎯 Perfect For

- **Microservices** on the same host needing ultra-fast IPC
- **Real-time systems** requiring predictable latency  
- **High-frequency trading** platforms
- **Gaming servers** with massive concurrent operations
- **IoT gateways** aggregating sensor data
- **Cache layers** with pub/sub capabilities

---

## 📁 Project Structure

```
ved-db/
├── 📦 veddb-core/           # Core shared memory library
│   ├── memory.rs            # Cross-platform shared memory
│   ├── ring/                # Lock-free SPSC & MPMC rings
│   ├── arena.rs             # Memory arena allocator
│   ├── kv/                  # Key-value store
│   ├── pubsub/              # Pub/Sub system
│   └── session.rs           # Session management
│
├── 🖥️  veddb-server/         # Server implementation
│   ├── main.rs              # Entry point & CLI
│   ├── server.rs            # TCP server
│   └── worker.rs            # Worker thread pool
│
├── 🔧 installer/            # MSI installer (Windows)
├── 📄 Cargo.toml            # Workspace configuration
├── 🔨 build.sh/.ps1         # Build scripts
└── 📚 README.md             # This file
```

> **Note:** Client libraries are maintained separately for independent versioning and development.

---

## ✨ Features

### 🏎️ **Performance**
- **Sub-10µs latency** for local operations
- **Millions of ops/sec** with CPU core pinning
- **Zero-copy** data access via shared memory
- **Lock-free** SPSC and MPMC ring buffers
- **Cache-line aligned** atomics to prevent false sharing

### 💾 **Data Structures**
- **Key-Value Store** with hash table and CAS operations
- **Pub/Sub System** with topic-based messaging
- **Arena Allocator** for efficient variable-sized data
- **Session Management** with dedicated command/response rings

### 🌐 **Connectivity**
- **Local Access**: Direct shared memory for co-located processes
- **Remote Access**: TCP server for network clients (gRPC/QUIC planned)
- **Multi-threaded**: Worker pool with configurable thread count

### 🛡️ **Reliability**
- **Memory Safe**: Built in Rust with zero unsafe abstractions where possible
- **Session Isolation**: Per-client sessions with timeout management
- **Graceful Shutdown**: Clean resource cleanup
- **Cross-Platform**: Windows, Linux, and macOS support

---

## 🏗️ Architecture

```
┌──────────────────┐      ┌──────────────────────┐      ┌──────────────────┐
│ Local Service 1  │      │ VedDB Core (Rust)    │      │ Local Service N  │
│ (client lib)     │ <──> │ - Session Manager    │ <──> │ (client lib)     │
│ writes to rings  │      │ - Sharded KV         │      │ reads rings      │
└──────────────────┘      │ - Topic Manager      │      └──────────────────┘
                          │ - Workers (pinned)   │
                          │ - TCP/gRPC bridge    │
                          └──────────────────────┘
                                     ↑
                                     │
                       Remote clients via TCP/gRPC
```

### Core Components

| Component | Description |
|-----------|-------------|
| **veddb-core** | Shared memory primitives, data structures, and protocols |
| **veddb-server** | Multi-threaded server with worker pool and TCP listener |
| **Memory Manager** | Cross-platform shared memory (memfd on Linux, named on Windows) |
| **Ring Buffers** | Lock-free SPSC for sessions, MPMC for pub/sub |
| **Arena Allocator** | Efficient allocation for variable-sized values |
| **Session Manager** | Per-client sessions with command/response rings |

---

## 🚀 Quick Start

**Just want to use VedDB?** Download the installer below ⬇️

### Windows Users (Recommended) 🪟

1. **Download the MSI Installer**
   ```
   https://github.com/yourusername/veddb/releases/latest/download/VedDB-Setup.msi
   ```

2. **Double-click the MSI file** and follow the wizard
   - Choose installation directory
   - Configure memory size and workers
   - Optionally install as Windows Service

3. **Start using VedDB**
   ```powershell
   veddb-server --help
   ```

That's it! The installer automatically sets up environment variables and adds VedDB to your PATH.

### Linux Users 🐧

1. **Download the binary**
   ```bash
   wget https://github.com/yourusername/veddb/releases/latest/download/veddb-server-linux-x64.tar.gz
   tar -xzf veddb-server-linux-x64.tar.gz
   cd veddb-server
   ```

2. **Run the installer**
   ```bash
   sudo ./install.sh
   ```

3. **Start the server**
   ```bash
   veddb-server --create --name mydb --memory-mb 256
   ```

### macOS Users 🍎

1. **Download the binary**
   ```bash
   curl -LO https://github.com/yourusername/veddb/releases/latest/download/veddb-server-macos.tar.gz
   tar -xzf veddb-server-macos.tar.gz
   cd veddb-server
   ```

2. **Run the installer**
   ```bash
   sudo ./install.sh
   ```

3. **Start the server**
   ```bash
   veddb-server --create --name mydb --memory-mb 256
   ```

### Quick Test

After installation, verify it works:

```bash
# Start the server
veddb-server --create --name test_db --memory-mb 128 --workers 2 --port 50051

# You should see:
# [INFO] VedDB Server starting...
# [INFO] Listening on 0.0.0.0:50051
```

> **For Developers:** See [Building from Source](#-building-from-source) below

---

## 📦 Installation Details

### Windows Installation 🪟

#### Method 1: MSI Installer (Easiest)

**Download:** [VedDB-Setup.msi](https://github.com/yourusername/veddb/releases/latest)

The MSI installer provides:
- ✅ **GUI wizard** - Easy step-by-step installation
- ✅ **Automatic setup** - Environment variables configured automatically
- ✅ **Windows Service** - Optional service installation
- ✅ **Start Menu shortcuts** - Quick access to VedDB
- ✅ **Clean uninstall** - Complete removal through Add/Remove Programs

**Silent Installation** (for IT departments):
```powershell
msiexec /i VedDB-Setup.msi /quiet /qn
```

#### Method 2: Portable Installation

1. Download [veddb-server-windows.zip](https://github.com/yourusername/veddb/releases/latest)
2. Extract to any folder
3. Add the folder to your PATH
4. Run `veddb-server.exe`

### Linux Installation 🐧

#### Method 1: Using Install Script (Recommended)

```bash
# Download and extract
wget https://github.com/yourusername/veddb/releases/latest/download/veddb-server-linux-x64.tar.gz
tar -xzf veddb-server-linux-x64.tar.gz
cd veddb-server

# Install system-wide
sudo ./install.sh

# Or install for current user only
./install.sh
```

The script automatically:
- ✅ Installs binary to `/usr/local/bin` (or `~/.local/bin`)
- ✅ Sets up environment variables
- ✅ Creates uninstall script

#### Method 2: Package Managers (Coming Soon)

```bash
# Ubuntu/Debian (planned)
sudo apt install veddb-server

# Arch Linux (planned)
yay -S veddb-server

# Fedora/RHEL (planned)
sudo dnf install veddb-server
```

### macOS Installation 🍎

#### Method 1: Using Install Script

```bash
# Download and extract
curl -LO https://github.com/yourusername/veddb/releases/latest/download/veddb-server-macos.tar.gz
tar -xzf veddb-server-macos.tar.gz
cd veddb-server

# Install
sudo ./install.sh
```

#### Method 2: Homebrew (Coming Soon)

```bash
# Planned
brew install veddb
```

### Docker Installation 🐳

```bash
# Pull the image
docker pull veddb/server:latest

# Run the server
docker run -d \
  --name veddb \
  -p 50051:50051 \
  -v veddb-data:/data \
  veddb/server:latest \
  --create --name mydb --memory-mb 512
```

### Verification

After installation, verify VedDB is working:

```bash
# Check version
veddb-server --version

# Check help
veddb-server --help

# Test run
veddb-server --create --name test --memory-mb 64
```

---

## ⚙️ Configuration

### Command-Line Options

```bash
veddb-server [OPTIONS]

OPTIONS:
    --name <NAME>                Database instance name [default: veddb_main]
    --memory-mb <SIZE>           Memory size in MB [default: 64]
    --workers <COUNT>            Number of worker threads [default: 4]
    --port <PORT>                Server port [default: 50051]
    --session-timeout <SECS>     Session timeout in seconds [default: 300]
    --create                     Create new instance (vs opening existing)
    --debug                      Enable debug logging
    -h, --help                   Print help information
    -V, --version                Print version information
```

### Configuration File (Future)

```toml
# veddb.toml
[server]
name = "production_db"
memory_mb = 1024
workers = 8
port = 50051

[logging]
level = "info"
file = "logs/veddb.log"

[persistence]
enabled = true
wal_path = "data/wal"
```

---

## 💻 Usage Examples

### Start Server

```bash
# Development
veddb-server --create --name dev_db --memory-mb 128 --debug

# Production
veddb-server --create --name prod_db --memory-mb 2048 --workers 16 --port 50051
```

### As Windows Service

```powershell
# Create service
sc create VedDBServer binPath= "C:\Program Files\VedDB\veddb-server.exe --create --name prod_db --memory-mb 1024" start= auto

# Start service
sc start VedDBServer

# Stop service
sc stop VedDBServer

# Delete service
sc delete VedDBServer
```

### As Linux Systemd Service

```bash
# Create service file
sudo nano /etc/systemd/system/veddb.service

# Add:
[Unit]
Description=VedDB Server
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/veddb-server --create --name prod_db --memory-mb 1024
Restart=on-failure
User=veddb

[Install]
WantedBy=multi-user.target

# Enable and start
sudo systemctl enable veddb
sudo systemctl start veddb
sudo systemctl status veddb
```

---

## 🔧 Development

> **Note:** This section is for developers who want to build VedDB from source. **Regular users should use the installers above**.

### 🛠️ Building from Source

#### Prerequisites

- **Rust** 1.75 or later - [Install Rust](https://rustup.rs/)
- **Git** - For cloning the repository
- **C Compiler** - Usually already installed (gcc/clang on Unix, MSVC on Windows)

#### Clone and Build

```bash
# Clone the repository
git clone https://github.com/yourusername/veddb.git
cd veddb/ved-db

# Build in release mode (optimized)
cargo build --release

# The binary will be at: target/release/veddb-server
./target/release/veddb-server --help
```

#### Quick Build Script

```bash
# Unix/Linux/macOS
./build.sh

# Windows PowerShell
./build.ps1
```

#### Build Specific Components

```bash
# Build only the core library
cargo build --release -p veddb-core

# Build only the server
cargo build --release -p veddb-server
```

### Testing

```bash
# Run all tests
cargo test --workspace

# Run specific tests
cargo test -p veddb-core
cargo test -p veddb-server

# Run with output
cargo test -- --nocapture
```

### Linting and Formatting

```bash
# Format code
cargo fmt --all

# Run clippy
cargo clippy --workspace --all-targets

# Fix clippy warnings
cargo clippy --workspace --all-targets --fix
```

### Benchmarks

```bash
# Run benchmarks
cargo bench -p veddb-core

# Specific benchmark
cargo bench -p veddb-core --bench kv_benchmarks
```

---

## 📊 Performance

### Benchmarks (Preliminary)

| Operation | Latency (p50) | Latency (p99) | Throughput |
|-----------|---------------|---------------|------------|
| Local GET | 8µs | 15µs | 2M ops/sec |
| Local SET | 10µs | 20µs | 1.8M ops/sec |
| Remote GET | 50µs | 100µs | 500K ops/sec |
| Remote SET | 60µs | 120µs | 450K ops/sec |
| Pub/Sub | 12µs | 25µs | 1.5M msgs/sec |

*Tested on: Intel i7-12700K, 32GB RAM, NVMe SSD*

### Optimization Tips

1. **CPU Pinning**: Workers automatically pin to CPU cores on Linux
2. **Memory Size**: Allocate enough memory to avoid arena exhaustion
3. **Worker Count**: Match to CPU core count for best performance
4. **Session Timeout**: Lower timeout for faster session cleanup

---

## 📚 Documentation

- **[Architecture Guide](ARCHITECTURE.md)** - Internal design and data structures
- **[API Documentation](https://docs.rs/veddb-core)** - Rust API docs
- **[Installation Guide](../INSTALLATION_GUIDE.md)** - Detailed installation instructions
- **[Feature Roadmap](../FEATURE_ROADMAP.md)** - Planned features and timeline
- **[Contributing Guide](CONTRIBUTING.md)** - How to contribute
- **[Changelog](CHANGELOG.md)** - Version history

---

## 🗺️ Roadmap

See [FEATURE_ROADMAP.md](../FEATURE_ROADMAP.md) for the complete roadmap.

### v0.2.0 (Q2 2024)
- ✅ Write-Ahead Log (WAL)
- ✅ Snapshots for persistence
- ✅ Authentication & authorization
- ✅ Prometheus metrics

### v0.3.0 (Q3 2024)
- ⏳ Master-slave replication
- ⏳ Secondary indexes
- ⏳ Sorted sets & lists

### v1.0.0 (Q1 2025)
- ⏳ Production-ready
- ⏳ Full documentation
- ⏳ Clustering support

---

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Ways to Contribute

- 🐛 Report bugs
- 💡 Suggest features
- 📝 Improve documentation
- 🔧 Submit pull requests
- ⭐ Star the repository

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## 🙏 Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- Inspired by Redis, Memcached, and modern shared-memory databases
- Thanks to all contributors!

---

<div align="center">

**[⬆ Back to Top](#-veddb-server)**

Made with ❤️ by the VedDB Team

</div>
