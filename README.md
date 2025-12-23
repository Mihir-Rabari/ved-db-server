# VedDB Server v0.2.0

**High-performance document database with encryption, replication, and advanced features**

VedDB is a production-ready, in-memory document database built in Rust with enterprise features including encryption at rest, master-slave replication, point-in-time recovery, and comprehensive backup management.

![Docker](https://img.shields.io/badge/docker-ready-blue)
![Rust](https://img.shields.io/badge/rust-1.75+-orange)
![License](https://img.shields.io/badge/license-MIT-green)

## ✨ Features

### Core Capabilities
- **📄 Document Store**: JSON-based document storage with schema validation
- **🔍 Advanced Querying**: Complex queries with filtering, sorting, and aggregation
- **📊 Indexing**: Multiple index types (B-Tree, Hash, Full-Text)
- **💾 Hybrid Storage**: In-memory caching with RocksDB persistence

### Enterprise Features
- **🔐 Encryption at Rest**: AES-256-GCM encryption with key rotation
- **🔄 Master-Slave Replication**: Real-time replication with automatic failover
- **💾 Smart Backups**: Point-in-time recovery, incremental backups, compression
- **🛡️ Authentication**: JWT-based auth with role-based access control (RBAC)
- **📊 Monitoring**: Built-in Prometheus metrics

### Performance
- **⚡ Fast Operations**: Sub-millisecond queries with caching
- **🔒 Thread-Safe**: Lock-free concurrent access
- **📈 Scalable**: Handle thousands of operations per second

## 🐳 Quick Start with Docker

### Pull and Run
```bash
docker pull mihirrabariii/veddb-server:latest

docker run -d \
  -p 50051:50051 \
  -v veddb-data:/var/lib/veddb/data \
  mihirrabariii/veddb-server:latest
```

### Docker Compose
```yaml
version: '3.8'

services:
  veddb:
    image: mihirrabariii/veddb-server:latest
    ports:
      - "50051:50051"
    volumes:
      - veddb-data:/var/lib/veddb/data
      - veddb-backups:/var/lib/veddb/backups
    environment:
      - RUST_LOG=info
      - VEDDB_CACHE_SIZE=512
    restart: unless-stopped

volumes:
  veddb-data:
  veddb-backups:
```

### With All Features
```bash
docker run -d \
  -p 50051:50051 \
  -v veddb-data:/var/lib/veddb/data \
  -v veddb-backups:/var/lib/veddb/backups \
  mihirrabariii/veddb-server:latest \
  veddb-server \
    --data-dir /var/lib/veddb/data \
    --enable-backups \
    --backup-dir /var/lib/veddb/backups \
    --enable-encryption \
    --master-key your-secret-key \
    --cache-size-mb 512
```

## 🛠️ Building from Source

### Prerequisites
- Rust 1.75 or later ([Install Rust](https://rustup.rs/))
- Docker (optional, for containerization)

### Build
```bash
git clone https://github.com/Mihir-Rabari/ved-db-server.git
cd ved-db-server
cargo build --release --package veddb-server
```

Binary will be at: `target/release/veddb-server`

### Run
```bash
./target/release/veddb-server \
  --data-dir ./veddb_data \
  --port 50051 \
  --cache-size-mb 256
```

## 📡 Protocol

VedDB uses a binary TCP protocol on port 50051. See [PROTOCOL.md](docs/PROTOCOL.md) for details.

### Supported Operations

**Document Operations:**
- Insert, Update, Delete, Query documents
- Collection management (create, drop, list)
- Index management (create, drop, list)

**Advanced Features:**
- Backup Management (create, restore, list, delete)
- Key Management (import, export, rotate, metadata)
- Replication (add slave, remove slave, list, force sync)
- Authentication (login, logout, user info)

## 🔒 Security

### Encryption
```bash
# Enable encryption with master key
veddb-server --enable-encryption --master-key "your-secure-key"
```

### Authentication
```bash
# Default admin credentials
Username: admin
Password: admin123

# ⚠️ Change immediately in production!
```

### TLS/SSL
Coming in v0.3.0

## 📊 Monitoring

VedDB exposes Prometheus metrics at `/metrics` endpoint:

- Connection statistics
- Operation counts
- Cache hit/miss rates
- Replication lag
- Backup statistics

## 🗺️ Architecture

```
┌─────────────────────────────────────┐
│    TCP Server (0.0.0.0:50051)       │
├─────────────────────────────────────┤
│  Connection Manager                 │
│  - Session management               │
│  - Authentication                   │
├─────────────────────────────────────┤
│  Storage Layer                      │
│  ├─ In-Memory Cache (DashMap)       │
│  ├─ RocksDB (Persistent)            │
│  └─ Write-Ahead Log (WAL)           │
├─────────────────────────────────────┤
│  Advanced Features                  │
│  ├─ Encryption Engine (AES-256)     │
│  ├─ Backup Manager                  │
│  └─ Replication Manager             │
└─────────────────────────────────────┘
```

## 📦 Components

- **veddb-core**: Core data structures, protocol, and storage engine
- **veddb-server**: TCP server implementation and CLI
- **veddb-compass**: Desktop GUI management tool (Coming soon)
- **veddb-admin**: Web-based admin interface (Planned)

## 📚 Documentation

- **Docker Hub**: [mihirrabariii/veddb-server](https://hub.docker.com/r/mihirrabariii/veddb-server)
- **GitHub**: [Mihir-Rabari/ved-db-server](https://github.com/Mihir-Rabari/ved-db-server)
- **API Docs**: Coming soon

## 🔧 Configuration Options

| Option | Default | Description |
|--------|---------|-------------|
| `--data-dir` | `./veddb_data` | Data directory path |
| `--port` | `50051` | TCP server port |
| `--host` | `0.0.0.0` | Listen address |
| `--cache-size-mb` | `256` | Cache size in MB |
| `--enable-backups` | `false` | Enable backup system |
| `--backup-dir` | `./backups` | Backup directory |
| `--enable-encryption` | `false` | Enable encryption |
| `--master-key` | - | Master encryption key |

## 🚀 Roadmap

### ✅ Completed (v0.2.0)
- Document storage and querying
- Indexing (B-Tree, Hash, Full-Text)
- Encryption at rest (AES-256-GCM)
- Master-slave replication
- Point-in-time backup & recovery
- JWT authentication
- Prometheus metrics
- Docker deployment

### 🔜 Planned (v0.3.0)
- TLS/SSL support
- Clustering (multi-master)
- Transaction support
- GraphQL API
- REST API gateway

### 🎯 Future (v1.0.0)
- Distributed consensus (Raft)
- Cross-region replication
- Time-series data support
- Geospatial indexing

## 📄 License

MIT License - see [LICENSE](LICENSE) for details.

## 🤝 Contributing

Contributions welcome! Please:
1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Submit a pull request

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## 📧 Contact

- **Email**: mihirrabari2604@gmail.com
- **Instagram**: @mihirrabariii
- **GitHub**: [Mihir-Rabari](https://github.com/Mihir-Rabari)

---

**Built with ❤️ in Rust** | [Docker Hub](https://hub.docker.com/r/mihirrabariii/veddb-server) | [Report Issue](https://github.com/Mihir-Rabari/ved-db-server/issues)