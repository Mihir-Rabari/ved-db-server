# VedDB Architecture

**System-Level Design Documentation**

**Last updated:** 2025-12-28  
**Version:** v0.2.0  
**Audience:** Core contributors, reviewers, maintainers

---

## Purpose

This document explains **how VedDB is supposed to work** at the architectural level.

It exists to prevent misimplementation and provide authoritative guidance when code behavior is unclear.

---

## Core Principles

VedDB's architecture enforces three non-negotiable properties:

### 1. Determinism

Given the same inputs and state:
- Operations produce identical results
- Failures happen the same way
- Recovery follows the same path

This enables reproducible testing and crash recovery.

### 2. Crash Safety

At any point, VedDB can be killed (SIGKILL, power loss, kernel panic) and:
- No data corruption occurs
- State machines resume correctly
- Logs remain consistent

This is enforced through:
- Write-ahead logging (WAL)
- Checkpoint-based state machines
- Atomic metadata updates

### 3. Fail-Closed Security

When security invariants cannot be guaranteed:
- Operations abort rather than proceed insecurely
- Server refuses to start in unsafe states
- Errors are explicit, never silent

Examples:
- Server won't start mid-rotation
- Missing encryption keys halt operations
- Invalid crypto states cause startup failure

---

## System Overview

```
┌─────────────────────────────────────────────┐
│           TCP Server (Port 50051)           │
│                                             │
│  ┌─────────────────────────────────────┐   │
│  │   Protocol Layer                    │   │
│  │   - Binary protocol parsing         │   │
│  │   - Command dispatch                │   │
│  │   - Session management              │   │
│  └─────────────────────────────────────┘   │
│              ↓                              │
│  ┌─────────────────────────────────────┐   │
│  │   Authentication & Authorization    │   │
│  │   - JWT validation                  │   │
│  │   - Role-based access control       │   │
│  └─────────────────────────────────────┘   │
│              ↓                              │
│  ┌─────────────────────────────────────┐   │
│  │   Query Layer                       │   │
│  │   ├─ Parser                         │   │
│  │   ├─ Planner (325 LOC)              │   │
│  │   ├─ Executor                       │   │
│  │   └─ Aggregation Pipeline (505 LOC) │   │
│  └─────────────────────────────────────┘   │
│              ↓                              │
│  ┌─────────────────────────────────────┐   │
│  │   Storage Engine                    │   │
│  │   ├─ Cache (DashMap)                │   │
│  │   ├─ RocksDB (persistent)           │   │
│  │   └─ WAL                            │   │
│  └─────────────────────────────────────┘   │
│              ↓                              │
│  ┌─────────────────────────────────────┐   │
│  │   Enterprise Features               │   │
│  │   ├─ Encryption (1,292 LOC)         │   │
│  │   │   └─ Key Rotation Scheduler     │   │
│  │   ├─ Replication (757 LOC)          │   │
│  │   │   └─ WAL Streaming              │   │
│  │   ├─ Pub/Sub (517 LOC)              │   │
│  │   └─ Monitoring (598 LOC)           │   │
│  └─────────────────────────────────────┘   │
└─────────────────────────────────────────────┘
```

---

## Data Flow

### Document Write Path

```
Client → Protocol → Auth → Executor → Storage
                                      ↓
                                    Cache Update
                                      ↓
                                    RocksDB Write
                                      ↓
                                    WAL Append
```

**Guarantees:**
- WAL write completes before response sent
- Cache and RocksDB stay synchronized
- Failures leave no partial state

### Document Read Path

```
Client → Protocol → Auth → Executor → Cache Check
                                      ↓ (miss)
                                    RocksDB Read
                                      ↓
                                    Cache Population
```

**Guarantees:**
- Reads are always from committed data
- Cache misses reload from persistent storage
- No stale data returned after writes

### Aggregation Pipeline Path

```
Client → Protocol → Auth → Pipeline Parser
                            ↓
                          Planner (index selection)
                            ↓
                          Executor (streaming)
                            ↓
                          Result Assembly
```

**Guarantees:**
- Memory-bounded execution
- Deterministic results
- Streaming where possible

---

## State Machines

VedDB uses explicit state machines for correctness-critical operations.

### Key Rotation State Machine

**States:**
- `Idle` - No rotation in progress
- `ReEncrypting` - Active re-encryption
- `Completed` - Rotation finished, metadata pending
- `Failed` - Rotation failed, requires intervention

**Transitions:**

```
Idle ──────────┐
               ↓
         ReEncrypting ────→ Failed
               ↓
           Completed
               ↓
             Idle
```

**Invariants:**
1. Metadata NEVER updated before `Completed` state
2. Server REFUSES to start in `ReEncrypting` or `Failed`
3. Checkpoint written before state changes
4. Old keys invalidated only after full success

**Implementation:** `veddb-core/src/encryption/key_rotation.rs` (1,292 LOC)

### Replication State Machine

**Current State:** Implemented but not fully validated

**States:**
- Master
- Slave
- Syncing

**Implementation:** `veddb-core/src/replication/` (757 LOC)

---

## Storage Model

### Document Storage

**Logical Model:**
```
Collection → Documents → Fields
```

**Physical Model:**
```
RocksDB Key: "collection:{name}:doc:{id}"
RocksDB Value: Serialized Document
```

**Indexing:**
- B-tree indexes stored separately
- Index keys: "index:{name}:{field_value} → doc_id"

### Write-Ahead Log (WAL)

**Purpose:** Crash recovery and replication

**Format:**
```
[Sequence] [Operation] [Collection] [Document ID] [Payload]
```

**Guarantees:**
- Append-only
- fsync before response
- Monotonic sequence numbers

**NOT Implemented:**
- WAL compaction
- Log rotation
- Automatic cleanup

### Metadata Storage

**Stored in RocksDB:**
- Collection schemas
- Index definitions
- Encryption keys
- Rotation state
- Replication config

**Critical Path:**
Metadata updates are **always atomic** via RocksDB transactions.

---

## Encryption Architecture

### Layering

```
Application Data
      ↓
 Document Serialization
      ↓
 AES-256-GCM Encryption
      ↓
 RocksDB Storage
```

**Key Management:**
- Master key (env var or CLI)
- Derived data encryption keys (DEK)
- Key rotation via scheduler

**Rotation Process:**
1. Generate new DEK
2. Re-encrypt ALL documents (synchronous)
3. Write checkpoints during re-encryption
4. Update metadata ONLY after completion
5. Invalidate old key

**Crash Recovery:**
- State file: `{encryption_dir}/rotation_state.json`
- Resume from last checkpoint
- Deterministic document ordering

---

## Replication Architecture

### Topology

**Current:** Master-Slave (single master)

**NOT Implemented:**
- Multi-master
- Consensus (Raft)
- Automatic failover

### Sync Process

**Initial Sync:**
1. Snapshot capture
2. Transfer to slave
3. Apply snapshot
4. Switch to WAL streaming

**Incremental Sync:**
1. Master appends to WAL
2. WAL broadcast to slaves
3. Slaves apply operations

**Implementation:** `veddb-core/src/replication/sync.rs` (757 LOC)

**NOT Production-Hardened:**
- Network partition handling
- Backpressure under load
- Slave lag recovery

---

## Query Execution

### Planner

**Responsibilities:**
- Index selection
- Cost estimation (heuristic)
- Execution strategy

**Strategies:**
- `IndexScan` - Use available index
- `CollectionScan` - Full scan

**Implementation:** `veddb-core/src/query/planner.rs` (325 LOC)

**Limitations:**
- No statistics
- Heuristic-based only
- Assumes indexes exist (doesn't validate)

### Executor

**Responsibilities:**
- Filter evaluation
- Document iteration
- Result assembly

**NOT Implemented:**
- Parallel execution
- Query cancellation
- Timeouts

### Aggregation Pipeline

**Operators Implemented:**
- `$match` - Filtering
- `$project` - Field selection
- `$sort` - Ordering (in-memory)
- `$group` - Aggregation
- `$limit` / `$skip` - Pagination

**Memory Bounds:**
- Sort: 1M documents max
- Group: 100k groups max
- Results: 100k documents max

**NOT Implemented:**
- Streaming aggregation
- Spill-to-disk
- Index-assisted aggregation

---

## Concurrency Model

**Storage Layer:**
- DashMap for lock-free cache
- RocksDB handles write serialization

**Document Updates:**
- NO concurrency control currently
- CAS (Compare-And-Swap) parsing exists but not enforced
- Lost updates are possible

**Critical Gap:**
This must be fixed in P1.

---

## What Is Explicitly NOT Implemented

This section is authoritative. If it's listed here, it does not work.

### Storage

- ❌ WAL compaction
- ❌ Log rotation
- ❌ Automatic cleanup
- ❌ Multi-version concurrency control (MVCC)

### Query

- ❌ Compound indexes
- ❌ Covering indexes
- ❌ Statistics collection
- ❌ Cost-based optimization
- ❌ Query timeouts

### Replication

- ❌ Multi-master
- ❌ Automatic failover
- ❌ Consensus protocols
- ❌ Cross-region replication

### Security

- ❌ TLS certificate validation
- ❌ Rate limiting
- ❌ Audit logging
- ❌ Token revocation (complete)

### Operations

- ❌ Online backup
- ❌ Hot upgrades
- ❌ Schema migrations
- ❌ Incremental key rotation (all-at-once currently)

---

## Testing Philosophy

VedDB tests are **deterministic, not exhaustive**.

**What is tested:**
- Core invariants (state machines)
- Crash recovery paths
- Encryption correctness

**What is NOT tested:**
- Scale (billion-row datasets)
- Network partitions
- Byzantine failures

See `LIMITATIONS.md` for full testing gaps.

---

## Performance Characteristics

**What is fast:**
- Single-document reads (cached)
- Indexed queries (small result sets)

**What is slow:**
- Full collection scans
- Aggregations (in-memory sorting)
- Key rotation (synchronous, blocking)

**What is unknown:**
- Replication latency under load
- Cache invalidation cost at scale

---

## Closing Statement

This architecture is **intentionally conservative**.

When unsure:
- Fail closed
- Log explicitly
- Enforce invariants

If this document conflicts with code, **the code is wrong**.

If this document conflicts with STATUS.md, **STATUS.md wins** (it reflects current reality).

If this document is incomplete, **update it before shipping the feature**.

---

**Architecture reflects intention. STATUS.md reflects reality. Both matter.**
