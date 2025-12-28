# VedDB Limitations

**Known Gaps and Trade-Offs**

**Last updated:** 2025-12-28  
**Version:** v0.2.0  
**Purpose:** Prevent surprise, guide decisions, set expectations

---

## Philosophy

VedDB does not hide limitations.

If something doesn't work, is unsafe, or hasn't been tested — it's documented here.

This document is **not an excuse list**. It's a **risk register**.

---

## Scalability Limitations

### Dataset Size

**Current Reality:**
- In-memory cache architecture
- RocksDB backed but not optimized for large datasets
- Aggregation loads all results into memory

**Practical Limits:**
- **Documents per collection:** Works well up to ~10M
- **Aggregation result sets:** 100k documents max (hard limit)
- **Sort operations:** 1M documents max (hard limit)
- **Group aggregations:** 100k groups max (hard limit)

**What breaks beyond these limits:**
- OOM kills
- Timeouts
- Unpredictable performance

**Not tested:**
- Billion-row datasets
- Terabyte-scale storage
- Multi-datacenter replication

### Throughput

**Current Reality:**
- Single-threaded aggregation
- Synchronous key rotation
- No request throttling

**Practical Limits:**
- **Writes/sec:** ~10k (untested at scale)
- **Reads/sec:** ~50k cached (untested at scale)
- **Key rotation:** Blocks ALL writes during re-encryption

**What breaks under high load:**
- Connection queue exhaustion
- WAL write amplification
- Cache thrashing

### Concurrency

**Current Reality:**
- Lock-free reads (DashMap)
- Write serialization via RocksDB
- NO update concurrency control

**Risk:**
- Lost updates possible under concurrent writes
- CAS (Compare-And-Swap) exists but not enforced
- No isolation guarantees

**Status:** Must be fixed in P1

---

## Security Limitations

### TLS / Transport Security

**Current Reality:**
- TLS code exists
- Certificate validation INCOMPLETE
- Self-signed certs implicitly accepted

**What this means:**
- Man-in-the-middle attacks possible
- Not safe for untrusted networks
- Not compliance-ready

**Deployment Rule:**
**DO NOT** expose to the public internet without:
1. Proper TLS validation
2. Rate limiting
3. Audit logging

### Authentication

**Current Reality:**
- JWT auth works
- Token revocation INCOMPLETE
- No global session invalidation

**Gaps:**
- Stolen tokens remain valid until expiry
- No forced logout
- No token blacklist persistence

**Risk:**
- Compromised tokens can't be revoked immediately

### Authorization

**Current Reality:**
- Role-based checks exist
- Enforcement is inconsistent across endpoints

**Gap:**
- Some operations bypass role checks
- No fine-grained permissions

### Audit Trail

**Current Reality:**
- NO durable audit log
- Security events not traceable
- Key rotation not auditable

**Compliance Impact:**
- Cannot prove who did what
- Not SOC2 ready
- Not HIPAA ready
- GDPR compliance unclear

---

## Performance Limitations

### Aggregation

**Current Reality:**
- Synchronous, in-memory execution
- No streaming
- No spill-to-disk

**What this means:**
- Large aggregations OOM
- Blocking other queries during execution
- No progress reporting

**Workaround:**
- Pre-filter with `$match`
- Limit result sets
- Run during low-traffic windows

### Key Rotation

**Current Reality:**
- Synchronous, blocking operation
- Re-encrypts ALL documents serially
- No pause/resume (except crash recovery)

**What this means:**
- Writes blocked for duration of rotation
- No progress visibility
- Cannot cancel mid-rotation

**Operational Impact:**
- Plan maintenance windows
- Expect downtime for large datasets

### Indexing

**Current Reality:**
- B-tree indexes only
- No compound indexes
- No covering indexes

**What this means:**
- Multi-field queries don't use indexes
- All queries fetch full documents

### Replication

**Current Reality:**
- Basic WAL streaming
- No backpressure handling
- Network partition handling untested

**Gaps:**
- Slave lag can grow unbounded
- No automatic recovery from partitions
- Snapshot transfer not chunked

**Not production-hardened for:**
- Unstable networks
- High-latency links
- Large replica lag

---

## Operational Limitations

### Deployment

**Current Reality:**
- Single-node architecture
- No clustering
- No automatic failover

**What you CANNOT do:**
- Run multi-master
- Automatic leader election
- Cross-region replication

### Monitoring

**Current Reality:**
- Metrics exist (598 LOC)
- Some endpoints return real data
- Some return placeholders

**Gap:**
- Not all operations wired to metrics
- No Prometheus integration yet
- No alerting

### Backup

**Current Reality:**
- Backup system exists
- Point-in-time recovery implemented
- Incremental backups work

**Gaps:**
- No online backup (requires quiesce)
- No continuous backup
- No automated retention

### Upgrades

**Current Reality:**
- NO online upgrade support
- NO schema migration tooling
- NO version compatibility guarantees

**What this means:**
- Downtime required for upgrades
- Manual migration planning
- Rollback requires restore from backup

---

## Testing Gaps

### What IS Tested

- Core state machine correctness
- Encryption invariants
- Basic CRUD operations
- Protocol compliance

### What is NOT Tested

- **Scale:**
  - Billion-row datasets
  - High concurrency
  - Network saturation

- **Failure Modes:**
  - Disk full
  - Network partitions
  - Byzantine failures
  - OOM conditions

- **Long-Running:**
  - Multi-day uptime
  - Log rotation
  - Memory leaks

- **Replication:**
  - Failover scenarios
  - Split-brain
  - Data reconciliation

**Reality:**
Most failure paths are **untested in production-like conditions**.

---

## API / Protocol Limitations

### Missing Operations

- ❌ Transactions (BEGIN/COMMIT/ROLLBACK)
- ❌ Bulk operations (batch writes)
- ❌ Upsert with merge
- ❌ Pagination cursors
- ❌ Query explain plans

### Protocol Gaps

- ❌ Compression
- ❌ Connection pooling hints
- ❌ Query cancellation
- ❌ Progress streaming

---

## Data Model Limitations

### Document Structure

**Current Reality:**
- JSON-based documents
- No schema enforcement (optional validation)
- No computed fields

### Field Types

**Supported:**
- Strings, numbers, booleans
- Arrays, nested objects
- Null

**NOT Supported:**
- Binary data (BLOBs)
- Dates (stored as strings)
- Geospatial types
- Time-series optimizations

### Constraints

**Current Reality:**
- Unique indexes work
- Foreign key constraints DO NOT EXIST
- Referential integrity NOT ENFORCED

---

## Known Bugs / TODOs

This is not a bug tracker. This is for **architectural-level issues**.

### Update Concurrency

**Status:** Lost updates possible  
**Impact:** HIGH  
**Fix:** P1

### Cache Invalidation

**Status:** Global invalidation on single update  
**Impact:** MEDIUM (performance)  
**Fix:** P2

### Query Planner Index Validation

**Status:** Assumes indexes exist without checking  
**Impact:** LOW (graceful degradation)  
**Fix:** P2

---

## What We Explicitly Will NOT Fix (Yet)

### P4 (Deferred)

- Multi-master clustering
- Geospatial queries
- Full-text search enhancements
- Graph traversals
- Time-series optimizations

**Reason:** Not required for core database correctness.

---

## Deployment Decision Matrix

| Use Case | Safe? | Notes |
|----------|-------|-------|
| Internal dev/test | ✅ YES | Ideal |
| Trusted corporate network | ✅ YES | With monitoring |
| Staging validation | ✅ YES | Recommended |
| Small production app (<10M docs) | ⚠️ MAYBE | After P1 completion |
| Public-facing API | ❌ NO | TLS validation required |
| Compliance-critical | ❌ NO | Audit logging required |
| Billion-row dataset | ❌ NO | Not tested |
| Multi-region | ❌ NO | Not supported |

---

## How to Use This Document

**Before deploying:**
1. Read this document completely
2. Map your use case to limitations
3. Identify unacceptable risks
4. Plan mitigations or defer deployment

**Before contributing:**
1. Check if your feature addresses a limitation here
2. Update this doc when closing gaps
3. Add new limitations if discovered

**Before reporting bugs:**
1. Verify it's not a documented limitation
2. If it is, suggest a fix instead

---

## Closing Statement

VedDB is **honest software**.

It does not claim to be complete.
It does not hide failure modes.
It does not oversell capabilities.

If you discover a limitation not listed here, **open an issue**. Silence is not the same as completeness.

> Truth is cheaper than outages.
