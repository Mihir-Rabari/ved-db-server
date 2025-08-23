# Architecture Overview

## Components

- `veddb-core/`
  - Arena allocator (`arena.rs`)
  - KV store sharding (`kv/`): shards, hash table with open addressing and CAS
  - Pub/Sub (`pubsub/`) and ring buffers (`ring/`)
- `veddb-server/`
  - CLI, config, logging (`src/main.rs`)
  - Worker pool (`src/worker.rs`)
  - gRPC server (`src/server.rs`)

## Data Flow (High Level)

1. Client issues commands (local or remote gRPC).
2. Server parses command, routes to worker based on key hash.
3. Worker accesses shard (`KvShard`) and performs operation on `HashTable`.
4. Responses include status and optional payload (value, version, stats).

## Sharding and Hashing

- Shards selected via mask: `hash & (num_shards - 1)`.
- HashTable uses linear probing with reserved hash values for empty/tombstone.
- CAS uses per-entry versioning stored alongside key/value metadata.

## Memory Management

- Arena allocator provides offset-based allocations using size classes.
- Entries store `data_offset` to (key || value) payload in arena.
- Deletions mark tombstone and free arena blocks.

## Concurrency

- `KvShard` uses `parking_lot::RwLock` for quick read/write access.
- Server spawns N workers (configurable) handling requests concurrently.

## Telemetry

- `tracing` + `tracing-subscriber` with EnvFilter.
- Periodic stats logging: ops count, keys, sessions, topics, memory.

