# VedDB Server

![status-active](https://img.shields.io/badge/status-active-brightgreen)
![rust-stable](https://img.shields.io/badge/rust-stable-orange)
![platforms](https://img.shields.io/badge/platforms-windows%20%7C%20linux%20%7C%20macOS-informational)
![license-MIT](https://img.shields.io/badge/license-MIT-blue)

VedDB Server is a **high-performance, in-memory database server** built in Rust. It provides **zero-copy shared memory access** for local processes and an experimental **QUIC/gRPC networking layer** for remote connections.

This repository contains the **server implementation only**. Client libraries are maintained in separate repositories.

---

## 🚀 Features (v0.0.1)

* **Core Database Engine**

  * In-memory key-value store with shared memory arena allocator.
  * Command/response protocol with `OpCode` and `Status`.
  * Supports basic CRUD: `GET`, `SET`, `DELETE`.
* **Concurrency & Performance**

  * SPSC ring buffers for low-latency IPC.
  * Worker pool with atomic operations for thread-safe sessions.
* **Networking**

  * Experimental QUIC/gRPC layer (wire format subject to change).
* **Session Management**

  * Unique session IDs for each connected client.
  * Structured error handling via `ClientError`.
* **Extensible Protocol**

  * Easily extendable command handling for future operations like `CAS`, pub/sub, and transactions.

---

## 📦 Installation

### Build from Source

```bash
git clone https://github.com/Mihir-Rabari/ved-db-server.git
cd ved-db-server
cargo build --release
```

### Run the Server

```bash
# Linux / macOS
./target/release/ved-db-server --create --name veddb_main --memory-mb 256 --workers 4 --port 50051 --debug

# Windows (PowerShell)
.\target\release\veddb-server.exe --create --name veddb_main --memory-mb 256 --workers 4 --port 50051 --debug
```

Default server settings:

* Port: `50051`
* Memory: `256MB`
* Workers: `4`

---

## 🛠 Usage

Currently, only the **server binary** is provided.
To interact with the server, use client libraries (separate repos).
Example commands handled by the server:

* `SET key value`
* `GET key`
* `DELETE key`
* `CAS` (planned for future releases)
* Pub/Sub commands stubbed (not yet functional)

---

## 📖 Documentation

* **Architecture Overview:** [ARCHITECTURE.md](ARCHITECTURE.md)
* **Protocol Spec:** [docs/protocol.md](docs/protocol.md) (WIP)
* **Changelog:** [CHANGELOG.md](CHANGELOG.md)

This release focuses on a **functional server prototype**; next releases will improve networking, persistence, and metrics.

---

## 📜 Changelog

**v0.0.1 – Initial server prototype**

* Core in-memory KV engine with shared memory arena allocator.
* Worker pool with session management.
* Experimental QUIC/gRPC networking.
* Command/response protocol: `GET`, `SET`, `DELETE`.
* Structured error handling via `ClientError`.
* Server binary ready for deployment on Windows, Linux, and macOS.

Full changelog: [CHANGELOG.md](./CHANGELOG.md)

---

## 🤝 Contributing

Contributions are welcome!
Open an issue or submit a pull request for bug fixes, feature requests, or discussions.

Please follow the [Code of Conduct](CODE_OF_CONDUCT.md).

---

## 📬 Contact

* Email: **[mihirrabari2604@gmail.com](mailto:mihirrabari2604@gmail.com)**
* Instagram: **@mihirrabariii**

---

## 📄 License

VedDB Server is licensed under the **MIT License** – see [LICENSE](LICENSE) for details.

---

Do you want me to do that next?
