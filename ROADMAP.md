# VedDB Post-P0 Roadmap

**Reality-Based Execution Plan**

**Last updated:** 2025-12-28
**Current Version:** v0.2.0
**Status:** P0 COMPLETE
**Audience:** Contributors, reviewers, operators, security auditors

---

## Purpose of This Document

This document defines the **post-P0 roadmap** for VedDB based on a **60+ issue deep technical audit** of the codebase.

This is **not marketing**.
This is **not aspirational**.
This is an execution-ordered plan grounded in **current code reality**.

If something is listed here, it means:

* It was identified during audit
* It is incomplete, unsafe, misleading, or unoptimized
* It must be addressed before broader production use

---

## Current Baseline (v0.2.0)

**What is true today:**

* P0 features are **functionally complete**
* Build is **passing**
* Core invariants (crypto, state machines) are enforced
* Several systems are **architecturally present but operationally incomplete**

**Verdict:**
VedDB has transitioned from *prototype* → *production-viable foundation*, but **must not be treated as fully hardened yet**.

---

## Roadmap Philosophy

We prioritize work by **risk**, not excitement.

Order of importance:

1. **Security correctness**
2. **Data integrity**
3. **Crash safety**
4. **Observability**
5. **Performance**
6. **Scalability**
7. **Convenience / UX**

Anything that violates this order is explicitly deferred.

---

# P1 — Security, Correctness, Trust

**Status:** NOT COMPLETE
**Blocking:** Exposure to untrusted networks
**Goal:** Make VedDB safe for real users

---

## P1.1 Transport Security Hardening

**Current Weaknesses (from audit):**

* TLS does not validate CA chains
* Self-signed certificates are implicitly accepted
* Plain TCP paths still exist
* No cipher enforcement

**Planned Work:**

* Enforce TLS-only connections
* Validate certificate chains by default
* Reject invalid / expired certificates
* Explicit cipher suite selection
* Configurable insecure override (dev only)

**Risk if ignored:**
MITM attacks, credential theft, compliance failure

---

## P1.2 Authentication & Authorization Hardening

**Current Weaknesses:**

* JWT revocation incomplete
* No global token invalidation
* Token expiry not enforced on all paths
* Role checks inconsistent

**Planned Work:**

* JWT revocation list (memory + persistent)
* Forced token invalidation on key rotation
* Strict expiry enforcement
* Centralized auth middleware

---

## P1.3 Audit Logging (Compliance Critical)

**Current Weaknesses:**

* No durable audit log
* Security events are not traceable
* Key rotation lacks auditable trail

**Planned Work:**

* Append-only audit log
* Hash-chained log entries
* Log auth, writes, deletes, rotations, replication
* Retention + rotation policy

**Required for:** SOC2, ISO 27001, HIPAA, GDPR

---

## P1.4 Update / CAS Semantics

**Current Weaknesses:**

* Updates overwrite documents
* CAS checks exist but are not enforced
* Lost updates possible under concurrency

**Planned Work:**

* Mandatory CAS for updates
* Reject stale versions
* Atomic update semantics
* Concurrency stress tests

---

## P1.5 Real Monitoring & Metrics

**Current Weaknesses:**

* Metrics endpoints exist but return fake/static values
* No visibility into failures or latency
* No crypto rotation metrics

**Planned Work:**

* Replace fake metrics with real counters
* Latency histograms
* WAL lag metrics
* Key rotation duration + document counts
* Error rate tracking

---

## P1.6 Failure-Mode Testing

**Current Weaknesses:**

* Most tests bypass real logic
* Failure paths untested
* Crash scenarios not validated

**Planned Work:**

* SIGKILL mid-write tests
* Disk-full simulations
* WAL corruption tests
* Encryption crash-resume tests
* Replication interruption tests

---

# P2 — Performance & Scalability

**Status:** PARTIAL
**Goal:** Correct behavior under load

---

## P2.1 Aggregation Engine Optimization

**Current State:**

* Aggregation logic implemented
* Not memory-bounded
* Not index-assisted

**Planned Work:**

* Streaming aggregation
* Spill-to-disk for `$group`
* Memory caps
* Index-assisted `$match`

---

## P2.2 Replication Robustness

**Current Weaknesses:**

* Replication not stress-tested
* Network partition handling basic
* Backpressure handling incomplete

**Planned Work:**

* WAL backpressure handling
* Snapshot chunking
* Replica lag detection
* Partition recovery tests

---

## P2.3 Query Planner Evolution

**Current Weaknesses:**

* Planner assumes indexes exist
* Heuristic only
* No cost model

**Planned Work:**

* Index existence validation
* Basic statistics collection
* Cost-based planning (minimal)
* Explain plan support

---

## P2.4 Cache Invalidation Precision

**Current Weaknesses:**

* Cache cleared globally on single update
* Causes unnecessary cache misses

**Planned Work:**

* Key-level invalidation
* Dependency tracking
* Cache versioning

---

# P3 — Operational Maturity

**Status:** NOT STARTED
**Goal:** Safe long-running operation

---

## P3.1 Online Key Rotation

**Current State:**

* Rotation is synchronous and blocking
* Correct but operationally heavy

**Planned Work:**

* Background throttled rotation
* Dual-key read paths
* Progress reporting
* Pause / cancel support

---

## P3.2 Operational APIs

**Planned Work:**

* Admin health endpoints
* Replication status API
* Rotation status API
* JSON health reports

---

## P3.3 Testing Infrastructure

**Planned Work:**

* Deterministic test harness
* Chaos testing
* Fault injection
* WAL replay verification

---

# P4 — Non-Blocking Enhancements

Explicitly **not required for safe deployment**.

* Compound indexes
* Covering indexes
* Geospatial queries
* Multi-region replication
* UI tooling

---

## Summary Table

| Phase | Purpose          | Status        |
| ----- | ---------------- | ------------- |
| P0    | Core correctness | ✅ COMPLETE    |
| P1    | Security & trust | ❌ REQUIRED    |
| P2    | Performance      | ⏳ PARTIAL     |
| P3    | Ops maturity     | ❌ NOT STARTED |
| P4    | Enhancements     | ⏭️ DEFERRED   |

---

## Deployment Guidance

**DO NOT:**

* Expose to the public internet
* Claim full production readiness
* Skip staging validation

**CAN:**

* Deploy to trusted/internal networks
* Use for development & experimentation
* Run compliance evaluations (crypto ready)

**RECOMMENDED NEXT STEPS:**

1. Complete P1 security items
2. Run crash-invariant tests
3. Add real metrics
4. Deploy to staging

---

## Closing Statement

VedDB v0.2.0 is **honest software**.

It enforces invariants, survives crashes, and avoids silent corruption — even where features are incomplete.

This roadmap exists to ensure future work continues from **truth**, not illusion.

> Truth is cheaper than outages.
