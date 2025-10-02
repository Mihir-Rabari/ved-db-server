# VedDB Server v0.1.21

**High-performance in-memory key-value database server with TCP protocol**

VedDB Server is a fast, lightweight, and easy-to-use in-memory database designed for low-latency data access. Built in Rust with a focus on simplicity and performance.

![Windows](https://img.shields.io/badge/platform-windows-blue)
![Rust](https://img.shields.io/badge/rust-1.75+-orange)
![License](https://img.shields.io/badge/license-MIT-green)

## ✨ Features

- **⚡ Fast KV Operations**: Sub-millisecond SET/GET/DELETE operations
- **🔌 TCP Protocol**: Simple binary protocol for network access
- **🔒 Thread-Safe**: Lock-free concurrent access using DashMap
- **📊 Session Management**: Multi-client support with automatic cleanup
- **🛠️ Worker Pool**: Multi-threaded request processing
- **📝 List Keys**: Enumerate all stored keys
- **🎯 Simple Protocol**: Easy to implement clients in any language

## 🚀 Quick Start

### Download & Installation (Windows)

VedDB Server is currently tested and supported on **Windows**. You can download the pre-built executable:

**Option 1: Download from Website**
- Visit our website and download the latest Windows `.exe`

**Option 2: GitHub Releases**
- Go to [Releases](https://github.com/yourusername/ved-db/releases)
- Download `veddb-server-v0.1.21-windows.exe`

### Running the Server

```
# Run with default settings (64MB memory, 4 workers, port 50051)
veddb-server.exe

# Or with custom configuration
veddb-server.exe --memory 128 --workers 8 --port 50051
```

### Configuration Options

| Option | Default | Description |
|--------|---------|-------------|
| `--memory` | 64 | Memory size in MB |
| `--workers` | 4 | Number of worker threads |
| `--port` | 50051 | TCP server port |
| `--path` | veddb_main | Database file path |

## 📡 Protocol

VedDB uses a simple binary protocol over TCP. All integers are **little-endian**.

### Command Format (24 bytes header + payload)
```
┌─────────┬───────┬──────────┬─────┬─────────┬─────────┬───────┬─────┬───────┐
│ opcode  │ flags │ reserved │ seq │ key_len │ val_len │ extra │ key │ value │
│ (1 byte)│(1 byte)│(2 bytes)│(4)  │  (4)    │  (4)    │  (8)  │ ... │  ...  │
└─────────┴───────┴──────────┴─────┴─────────┴─────────┴───────┴─────┴───────┘
```

### Response Format (20 bytes header + payload)
```
┌────────┬───────┬──────────┬─────┬─────────────┬───────┬─────────┐
│ status │ flags │ reserved │ seq │ payload_len │ extra │ payload │
│(1 byte)│(1 byte)│(2 bytes)│(4)  │     (4)     │  (8)  │   ...   │
└────────┴───────┴──────────┴─────┴─────────────┴───────┴─────────┘
```

### Supported Operations

| OpCode | Command | Description |
|--------|---------|-------------|
| `0x01` | PING | Health check - returns "pong" |
| `0x02` | SET | Store key-value pair |
| `0x03` | GET | Retrieve value by key |
| `0x04` | DELETE | Remove key |
| `0x09` | LIST | List all keys (newline-separated) |

### Status Codes

| Code | Status | Description |
|------|--------|-------------|
| `0x00` | OK | Operation successful |
| `0x01` | NotFound | Key not found |
| `0x04` | InternalError | Server error |

## 🏗️ Architecture

```
┌─────────────────────────────────────┐
│    TCP Server (0.0.0.0:50051)       │
├─────────────────────────────────────┤
│  Worker Pool (4 threads)            │
│  - Concurrent request handling      │
│  - Session management               │
├─────────────────────────────────────┤
│  SimpleKvStore (DashMap)            │
│  - Lock-free concurrent access      │
│  - Thread-safe operations           │
│  - O(1) average lookup              │
└─────────────────────────────────────┘
```

## 📊 Performance

Tested on Windows with Intel i7, 16GB RAM:

- **Latency**: < 1ms for most operations
- **Throughput**: 10,000+ ops/sec per connection
- **Concurrency**: Lock-free data structure for parallel access
- **Memory**: Efficient in-memory storage with DashMap

## 🔧 Usage Examples

### Using with veddb-cli (Rust Client)

```
# Ping server
veddb-cli.exe ping

# Set a key
veddb-cli.exe kv set name John

# Get a key
veddb-cli.exe kv get name

# List all keys
veddb-cli.exe kv list

# Delete a key
veddb-cli.exe kv del name
```

### Implementing Your Own Client

The protocol is simple enough to implement in any language. See the protocol section above for details.

Example pseudo-code:
```
1. Connect to 127.0.0.1:50051
2. Build command: [opcode][flags][reserved][seq][key_len][val_len][extra][key][value]
3. Send command bytes
4. Read response: [status][flags][reserved][seq][payload_len][extra][payload]
5. Parse response based on status code
```

## 🛠️ Development

### Building from Source

**Prerequisites:**
- Rust 1.75 or later ([Install Rust](https://rustup.rs/))
- Windows 10/11

```
git clone https://github.com/yourusername/ved-db.git
cd ved-db\ved-db-server
cargo build --release
```

Binary will be at: `target\release\veddb-server.exe`

### Running Tests

```
cargo test --workspace
```

### Logging

VedDB uses `tracing` for structured logging. Set `RUST_LOG` environment variable:
- `info` - Default level
- `debug` - Verbose logging
- `trace` - Very verbose logging

## 📦 Components

- **veddb-core**: Core data structures and protocol definitions
- **veddb-server**: TCP server implementation
- **simple_kv**: Lock-free KV store using DashMap

## 🗺️ Roadmap

### Current (v0.1.21)
- ✅ Basic KV operations (SET, GET, DELETE)
- ✅ LIST keys operation
- ✅ TCP protocol
- ✅ Multi-threaded worker pool
- ✅ Session management
- ✅ Windows support

### Planned (v0.2.x)
- ⏳ Persistence (WAL + snapshots)
- ⏳ Authentication
- ⏳ Pub/Sub messaging
- ⏳ TTL (time-to-live) for keys
- ⏳ Pattern matching for LIST

### Future (v1.0.x)
- ⏳ Replication
- ⏳ Clustering
- ⏳ Linux/macOS support
- ⏳ gRPC protocol option

## 📄 License

MIT License - see [LICENSE](LICENSE) for details.

## 🤝 Contributing

Contributions welcome! Please open an issue or PR on GitHub.

## 📧 Contact

- **Email**: mihirrabari2604@gmail.com
- **Instagram**: @mihirrabariii

---

**Built with ❤️ in Rust**
