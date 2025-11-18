# VedDB v0.2.0 Workspace Structure

This workspace contains the core VedDB server components for v0.2.0.

## Workspace Members

### veddb-core
Core shared library containing:
- Data structures (Document, Value, Schema)
- Storage engines (Cache Layer, Persistent Layer, Hybrid Storage Engine)
- Cryptography (Encryption, Key Management)
- Authentication and Authorization
- WAL and Snapshot management
- Query engine and indexing
- Replication system
- Pub/Sub messaging

### veddb-server
The main VedDB server binary:
- Network protocol handler (v0.1.x and v0.2.0)
- TLS connection management
- Server configuration and startup
- Admin CLI commands
- Monitoring and metrics (Prometheus endpoint)

### veddb-compass
Desktop GUI application built with Tauri:
- React + TypeScript frontend
- Rust backend for VedDB operations
- Database management interface
- Query builder and document viewer
- Real-time metrics dashboard
- User and index management

## Building

### Build all components
```bash
cargo build --workspace
```

### Build specific component
```bash
cargo build -p veddb-core
cargo build -p veddb-server
cargo build -p veddb-compass
```

### Build for release
```bash
cargo build --release --workspace
```

## Running

### Run VedDB Server
```bash
cargo run -p veddb-server
```

### Run VedDB Compass (Development)
```bash
cd veddb-compass
npm install
npm run tauri dev
```

### Run VedDB Compass (Production Build)
```bash
cd veddb-compass
npm run tauri build
```

## Testing

### Run all tests
```bash
cargo test --workspace
```

### Run tests for specific component
```bash
cargo test -p veddb-core
cargo test -p veddb-server
```

## CI/CD

The workspace includes GitHub Actions workflows:
- **ci.yml**: Runs tests and linting on every push/PR
- **release.yml**: Builds release binaries for all platforms on version tags

## Related Repositories

VedDB v0.2.0 is a multi-repository project:

- **ved-db-server** (this repo): Core server and Compass GUI
- **ved-db-rust-client**: Rust client library
- **veddb-js-client**: JavaScript/TypeScript client library
- **veddy**: ODM library (Mongoose-like) - separate npm package
- **veddb-python-client**: Python client library
- **website**: Documentation and marketing site

## Version

Current version: **0.2.0**

All workspace members share the same version number defined in the root `Cargo.toml`.
