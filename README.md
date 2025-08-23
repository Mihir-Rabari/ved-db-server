# VedDB Server

![status-active](https://img.shields.io/badge/status-active-brightgreen)
![rust-stable](https://img.shields.io/badge/rust-stable-orange)
![platform-windows](https://img.shields.io/badge/platform-windows-lightgrey)
![license-MIT](https://img.shields.io/badge/license-MIT-blue)

VedDB Server is a **high-performance, in-memory database server** built in Rust.
It provides **zero-copy shared memory access** for local processes and an experimental **QUIC/gRPC networking layer** for remote connections.

⚠️ **Currently, the server binary is only available and tested on Windows.**.

This repository contains the **server implementation only**. Client libraries are maintained in separate repositories.

---

## 🚀 Features (v0.0.1)

* **Core Database Engine**

  * In-memory key-value store with shared memory arena allocator.
  * Command/response protocol with `OpCode` and `Status`.
  * Supports basic CRUD: `GET`, `SET`, `DELETE`.
* **Concurrency & Performance**

  * SPSC ring buffers for zero-copy local IPC.
  * Worker pool with atomic operations for thread-safe sessions.
* **Networking**

  * Experimental QUIC/gRPC layer (prototype, wire format subject to change).
* **Session Management**

  * Unique session IDs for each connected client.
  * Structured error handling via `ClientError`.
* **Extensible Protocol**

  * Easily extendable command handling for future operations like CAS and Pub/Sub.

---

## 📦 Installation & Running (Windows)

### 1. Download the Server Binary

* Download the Windows `.exe` for v0.0.1 from the **release assets**

### 2. Place in Folder

* Place in a folder of your choice, e.g., `C:\VedDB\`.

### 3. Add Folder to System `PATH` (Optional, Recommended)

Adding the folder to your environment variables lets you run `veddb-server-windows.exe` from **any location** in PowerShell or CMD.

1. Press `Win + R`, type `sysdm.cpl`, and press **Enter**.
2. Go to the **Advanced** tab → click **Environment Variables**.
3. Under **System Variables**, find and select `Path`, then click **Edit**.
4. Click **New** and add your folder path (e.g., `C:\VedDB\`).
5. Click **OK** to save.
6. Close and reopen PowerShell or CMD.

Now you can run:

```powershell
veddb-server-windows --create --name veddb_main --memory-mb 256 --workers 4 --port 50051 --debug
```

from **any directory**.

### 4. Open PowerShell or CMD

If you didn’t add it to `PATH`, navigate manually:

```powershell
cd C:\VedDB\
```

### 5. Run the Server via CLI

* Example command:

```powershell
veddb-server-windows.exe --create --name veddb_main --memory-mb 256 --workers 4 --port 50051 --debug
```

**Default server settings:**

* **Port:** 50051
* **Memory:** 64MB
* **Workers:** 4

The server will start in the current console. Logs and stats will print directly to the CLI.

To stop the server, press `Ctrl+C` in PowerShell or CMD.

---

## 💻 Building from Source

If you want to build from source:

```powershell
git clone https://github.com/Mihir-Rabari/ved-db-server.git
cd ved-db-server
cargo build --release
```

### Experimental: Other Platforms

Linux/macOS users can attempt to build from source:

```bash
git clone https://github.com/Mihir-Rabari/ved-db-server.git
cd ved-db-server
cargo build --release
./target/release/veddb-server-windows --create --name veddb_main --memory-mb 256 --workers 4 --port 50051 --debug
```

**Note:** Shared memory behavior and networking are experimental on non-Windows platforms.

---

## 🛠 Usage

* Server binary only (Windows officially).
* Client interaction through separate client repositories.
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

---
