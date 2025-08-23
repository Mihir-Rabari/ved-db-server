# VedDB Server

![status-active](https://img.shields.io/badge/status-active-brightgreen)
![rust-stable](https://img.shields.io/badge/rust-stable-orange)
![platform-windows](https://img.shields.io/badge/platform-windows-lightgrey)
![license-MIT](https://img.shields.io/badge/license-MIT-blue)

VedDB Server is a **high-performance, in-memory database server** built in Rust.
It provides **zero-copy shared memory access** for local processes and an experimental **QUIC/gRPC networking layer** for remote connections.

⚠️ **Currently, the server binary is only available and tested on Windows.** Other platforms are planned for future releases.

This repository contains the **server implementation only**. Client libraries are maintained in separate repositories.

---

## 🚀 Features (v0.0.1)

* **Core Database Engine**

  * In-memory key-value store with shared memory arena allocator.
  * Command/response protocol with `OpCode` and `Status`.
  * Supports basic CRUD: `GET`, `SET`, `DELETE`.
* **Concurrency & Performance**

  * SPSC ring buffers for low-latency inter-process communication.
  * Worker pool with atomic operations for thread-safe sessions.
* **Networking**

  * Experimental QUIC/gRPC layer (prototype, wire format subject to change).
* **Session Management**

  * Unique session IDs for each connected client.
  * Structured error handling via `ClientError`.
* **Extensible Protocol**

  * Easily extendable command handling for future operations like CAS and Pub/Sub.

---

## 📦 Installation & Running the Server (Windows Only)

### Build from Source (Windows)

```powershell
git clone https://github.com/Mihir-Rabari/ved-db-server.git
cd ved-db-server
cargo build --release
```

### Run the Server

```powershell
.\target\release\ved-db-server.exe --create --name veddb_main --memory-mb 256 --workers 4 --port 50051 --debug
```

**Default server settings:**

* Port: `50051`
* Memory: `256MB`
* Workers: `4`

---

## 💻 Building for Other Platforms (Experimental)

You can **attempt to build the server on Linux or macOS**, but these builds are **not officially tested** in v0.0.1:

### Linux / macOS

```bash
# Clone repo
git clone https://github.com/Mihir-Rabari/ved-db-server.git
cd ved-db-server

# Build release binary
cargo build --release

# Run the server
./target/release/ved-db-server --create --name veddb_main --memory-mb 256 --workers 4 --port 50051 --debug
```

**Notes:**

* Shared memory and IPC behavior may differ between platforms.
* QUIC/gRPC networking is experimental and may require additional dependencies.
* Please report any platform-specific issues via GitHub Issues.

---

## 🛠 Usage

* Server binary only (Windows officially).
* Client interaction through separate client repositories (Rust, Python, Go planned).
* Supported commands in v0.0.1:

  * `GET key`
  * `SET key value`
  * `DELETE key`
* CAS and Pub/Sub operations are planned for future releases.

---

## 📖 Documentation

* **Architecture Overview:** [ARCHITECTURE.md](ARCHITECTURE.md)
* **Changelog:** [CHANGELOG.md](./CHANGELOG.md)

---

## 📜 Changelog

See [CHANGELOG.md](./CHANGELOG.md) for details.
Latest release: **v0.0.1 – Windows-only server prototype**.

---

## 🤝 Contributing

Open an issue or PR for bug reports, feature requests, or discussions.

Please follow the [Code of Conduct](CODE_OF_CONDUCT.md).

---

## 📬 Contact

* Email: **[mihirrabari2604@gmail.com](mailto:mihirrabari2604@gmail.com)**
* Instagram: **@mihirrabariii**

---

## 📄 License

MIT License – see [LICENSE](LICENSE) for details.
