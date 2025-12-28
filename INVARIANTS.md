# VedDB Invariants

**Formal Correctness Rules**

**Last updated:** 2025-12-28  
**Version:** v0.2.0  
**Audience:** Core contributors, reviewers, security auditors

---

## Purpose

Invariants are **properties that must always be true** regardless of code path, concurrency, or failure mode.

If an invariant is violated:
- The system is in an **undefined state**
- Data corruption or security compromise is possible
- The violation is a **critical bug**

This document makes **implicit correctness rules explicit**.

---

## How to Use This Document

**When writing code:**
1. Identify which invariants apply
2. Prove your code preserves them
3. Add assertions to enforce them

**When reviewing code:**
1. Check if invariants are maintained
2. If unsure, reject the PR
3. Require tests that verify invariants

**When debugging:**
1. Check which invariant was violated
2. Find the violating code path
3. Fix the root cause, not the symptom

---

## Global Invariants

These apply to the entire system at all times.

### G1: Atomic Metadata

**Statement:**
Metadata updates MUST be atomic. Either all fields update or none.

**Rationale:**
Partial metadata leaves the system in an inconsistent state.

**Enforcement:**
- RocksDB transactions for metadata writes
- No multi-step metadata updates

**Violation Example:**
```rust
// WRONG - Two separate writes
db.put("key_id", new_key_id);
db.put("rotation_state", "complete"); // If this fails, inconsistent
```

**Correct:**
```rust
// RIGHT - Single transaction
let mut batch = WriteBatch::default();
batch.put("key_id", new_key_id);
batch.put("rotation_state", "complete");
db.write(batch)?;
```

### G2: WAL Before Response

**Statement:**
NO response may be sent to a client before the corresponding WAL entry is durably  written (fsync'd).

**Rationale:**
Crash before WAL flush loses acknowledged writes.

**Enforcement:**
- Explicit `fsync()` before response construction
- Integration tests with crash simulation

**Violation Risk:**
- Data loss
- Broken replication
- Violated client guarantees

### G3: Fail-Closed

**Statement:**
When correctness cannot be guaranteed, the operation MUST abort with an explicit error.

This applies to:
- Encryption failures
- State machine violations
- Corrupted data

**Examples:**
- Cannot decrypt → abort, don't return partial data
- Rotation state invalid → refuse to start server
- Unknown opcode → close connection, don't ignore

**Never:**
- Silently skip operations
- Return default/empty values on errors
- Log and continue when invariants break

---

## Encryption Invariants

### E1: Key Lifecycle

**Statement:**
A data encryption key (DEK) MUST NOT be used after rotation completes.

**Rationale:**
Using old keys defeats the purpose of rotation.

**Enforcement:**
- Key invalidation as final rotation step
- Decrypt attempts with old key = hard error
- No "try old key then new key" fallback

**Implementation:**
`veddb-core/src/encryption/mod.rs`

### E2: Metadata Finality

**Statement:**
Encryption metadata (key ID, algorithm) MUST NOT be updated until ALL documents are re-encrypted.

**Rationale:**
Metadata change before re-encryption completes = data loss.

**Enforcement:**
- Rotation state machine (4 states)
- Metadata update only in `Completed → Idle` transition
- State file written before metadata update

**Implementation:**
`veddb-core/src/encryption/key_rotation.rs`

### E3: Startup Enforcement

**Statement:**
Server MUST refuse to start if encryption state is anything other than `Idle` or `Completed`.

**Allowed States:**
- `Idle` - No rotation in progress
- `Completed` - Rotation finished, metadata update pending (safe to finalize)

**Forbidden States:**
- `ReEncrypting` - Incomplete rotation
- `Failed` - Previous rotation failed

**Rationale:**
Starting mid-rotation risks data inconsistency.

**Enforcement:**
```rust
fn enforce_rotation_state_on_startup(state: RotationState) -> Result<()> {
    match state {
        RotationState::Idle | RotationState::Completed => Ok(()),
        RotationState::ReEncrypting | RotationState::Failed => {
            Err(anyhow!("Cannot start server with rotation state: {:?}", state))
        }
    }
}
```

### E4: Checkpoint Integrity

**Statement:**
During rotation, checkpoints MUST be written BEFORE processing the next batch.

**Rationale:**
Crash mid-batch without checkpoint = redo work, possible double-encryption.

**Enforcement:**
- Checkpoint every N documents
- Crash recovery resumes from last checkpoint
- Deterministic document ordering

**Violation:**
Processing documents without checkpointing risks:
- Duplicate re-encryption
- Skipped documents
- Non-deterministic recovery

---

## Replication Invariants

### R1: Sequence Monotonicity

**Statement:**
WAL sequence numbers MUST be strictly monotonically increasing.

**Rationale:**
Out-of-order sequences break replication and recovery.

**Enforcement:**
- Atomic counter for sequence assignment
- Replica rejects non-consecutive sequences

### R2: Snapshot Consistency

**Statement:**
A replication snapshot MUST reflect a single point-in-time state.

**Rationale:**
Mixing snapshot data from different times = corrupted replica.

**Enforcement:**
- Quiesce writes during snapshot capture
- Atomic snapshot operation

**Current Status:**
Implemented but not stress-tested under load.

### R3: WAL Completeness

**Statement:**
Slaves MUST receive ALL WAL entries starting from snapshot sequence number.

**Rationale:**
Missing WAL entries = data divergence.

**Enforcement:**
- Master tracks slave positions
- Gap detection and retry logic

**Current Gap:**
Backpressure handling incomplete (P2 item).

---

## State Machine Invariants

### S1: Valid Transitions Only

**Statement:**
State machines MUST only transition via explicitly defined edges.

**Example (Key Rotation):**
```
Valid:   Idle → ReEncrypting
Valid:   ReEncrypting → Completed
Invalid: Idle → Completed (skips re-encryption)
Invalid: Completed → ReEncrypting (reverses progress)
```

**Enforcement:**
- Enum-based state representation
- Match statements for transitions
- Unreachable states = panic in debug builds

### S2: Crash Recovery Determinism

**Statement:**
Crashing and resuming a state machine MUST produce the same result as completing without crash.

**Rationale:**
Non-deterministic recovery = undefined behavior.

**Enforcement:**
- Checkpoint-based resume
- Deterministic ordering (e.g., sort document IDs)
- Idempotent operations

**Implementation:**
Key rotation resumes from checkpoint with same document order.

---

## Storage Invariants

### ST1: Cache-Persistence Consistency

**Statement:**
At all times, cache and RocksDB MUST contain identical data for the same key.

**Rationale:**
Cache/DB divergence = stale reads or lost writes.

**Enforcement:**
- Write to RocksDB first, then cache
- Eviction doesn't delete from RocksDB
- Cache miss reloads from RocksDB

**Violation Example:**
```rust
// WRONG - Cache updated without RocksDB
cache.insert(key, value);
```

**Correct:**
```rust
// RIGHT - RocksDB first, then cache
db.put(key, value)?;
cache.insert(key, value);
```

### ST2: Index-Document Sync

**Statement:**
Index entries MUST correspond to existing documents. No dangling index pointers.

**Rationale:**
Garbage index entries slow queries and waste space.

**Enforcement:**
- Delete document → delete index entries atomically
- Index rebuild scans actual documents

**Current Gap:**
Index consistency not fully validated (known issue, low priority).

---

## Concurrency Invariants

### C1: No Lost Updates (FUTURE)

**Statement:**
Concurrent updates to the same document MUST NOT result in lost writes.

**Current Status:**
**VIOLATED** - CAS exists but not enforced.

**Priority:**
P1 - Must fix before production use.

**Enforcement (Planned):**
- Mandatory version checks on update
- Reject stale versions
- Return conflict errors to client

---

## Query Invariants

### Q1: Filter Determinism

**Statement:**
Given identical documents and filter, query results MUST be identical across runs.

**Rationale:**
Non-deterministic queries = unpredictable behavior.

**Enforcement:**
- Stable sort orders (tie-breaking by document ID)
- No random sampling
- No time-based filters without explicit timestamps

### Q2: Aggregation Memory Bounds

**Statement:**
Aggregation pipelines MUST enforce memory limits and reject operations that exceed them.

**Rationale:**
Unbounded memory = OOM kills.

**Enforcement:**
- MAX_SORT_DOCS = 1M
- MAX_GROUP_SIZE = 100k
- MAX_RESULT_DOCS = 100k

**Error on violation:**
Return explicit error to client, don't crash server.

---

## Error Handling Invariants

### ER1: Explicit Errors

**Statement:**
All error paths MUST return explicit errors, never silently succeed.

**Forbidden:**
```rust
fn operation() -> bool {
    // WRONG - Silent failure
    if error_condition { return false; }
    true
}
```

**Required:**
```rust
fn operation() -> Result<(), Error> {
    // RIGHT - Explicit error
    if error_condition {
        return Err(Error::SpecificReason);
    }
    Ok(())
}
```

### ER2: No Panics in Library Code

**Statement:**
Library code (veddb-core) MUST NEVER panic on user input.

**Rationale:**
Panics crash the entire server.

**Enforcement:**
- Use `Result<T, E>` for all fallible operations
- `unwrap()` only for:
  - Internal bugs (use `expect("why this can't fail")`)
  - Infallible operations
- Client input validation returns errors

**Exception:**
Test code and CLI may panic.

---

## How Invariants Evolve

**Adding an invariant:**
1. Document it here
2. Add runtime assertions
3. Add tests that verify it
4. Update reviews to check for violations

**Removing an invariant:**
Requires:
1. Security/correctness review
2. Justification in git commit
3. Update this document

**Violating an invariant:**
Is a **critical bug**, not a "nice-to-have fix".

---

## Verification Strategy

**Static:**
- Assertions in code (`debug_assert!`, `assert!`)
- Type system enforcement where possible

**Dynamic:**
- Integration tests
- Crash simulation tests
- Chaos testing (planned)

**Review:**
- PR checklist includes invariant verification
- Reviewers explicitly check invariants

---

## Closing Statement

Invariants are **non-negotiable**.

They exist because:
- Distributed systems have subtle bugs
- Crashes expose race conditions
- Security requires correctness

If you're unsure whether your code violates an invariant, **ask before merging**.

If an invariant seems wrong, **challenge it before ignoring it**.

If you discover a missing invariant, **document it here**.

---

**Invariants are the difference between a database and a data hazard.**
