# Production‑Grade Code Rules for AI Contributors

**Purpose**
This rule file exists to force *real, production‑ready implementations* and to prevent simulated, mocked, aspirational, or misleading code from entering the codebase.

These rules are **non‑negotiable**.

---

## 1. Absolute Core Principle

> **If a feature cannot be implemented fully and correctly, DO NOT implement it at all.**

* Never simulate behavior
* Never return fake data
* Never stub logic behind a “for now” comment
* Never add code just to satisfy an API or TODO

If full implementation is not possible, the AI **MUST stop** and explicitly notify the user.

---

## 2. Hard Prohibitions (Instant Failure)

The AI is strictly forbidden from introducing any of the following in production code:

* Comments or strings containing:

  * `for now`
  * `assume`
  * `simplified`
  * `mock`
  * `placeholder`
  * `temporary`
  * `fake`
  * `MVP`
  * `later`
  * `implemented later`
* Functions that:

  * Return constant or hard‑coded values for non‑trivial logic
  * Return empty collections when real data exists
  * Ignore input parameters
  * Log success without performing real work
* APIs that:

  * Always return `Ok`, `true`, or `success`
  * Validate input but skip execution
  * Serialize/deserialise without side effects

Any appearance of the above means the feature is **NOT implemented** and must be rejected.

---

## 3. Mandatory User Notification Rule

If the AI cannot fully implement a feature:

1. **STOP writing code immediately**
2. Explicitly notify the user:

   > "This feature cannot be implemented correctly with the current constraints. I will not simulate or partially implement it."
3. Explain **exactly** what is missing (design, data, execution path)
4. Wait for explicit user approval before proceeding

Partial implementations without notification are forbidden.

---

## 4. Production‑Ready Definition

Code is considered production‑ready **only if all conditions are met**:

### Correctness

* All inputs are validated
* All failure modes are handled
* No panics, unwraps, or silent fallbacks
* Deterministic behavior

### Completeness

* End‑to‑end execution path exists
* Data is actually read, modified, or written
* State transitions are real and observable

### Performance

* No per‑request thread creation
* No global locks on hot paths
* Bounded memory usage
* Reasonable algorithmic complexity (O(log n) or O(n) with justification)

### Safety

* Concurrency‑safe
* No data races
* Resource cleanup guaranteed
* Security checks enforced

If any category fails, the feature is **NOT production‑ready**.

---

## 5. Scalability Requirements

The AI must assume:

* High concurrency
* Long‑running processes
* Partial failures
* Large datasets

Therefore:

* No global counters without distribution strategy
* No blocking I/O on async paths
* No runtime creation per request
* No unbounded queues

Scalability is **mandatory**, not optional.

---

## 6. Error Handling Rules

* Errors must be explicit and typed
* Do not collapse errors into generic `Error`
* Do not hide errors with defaults
* Propagate meaningful context

Every failure must be debuggable in production.

---

## 7. Observability Requirements

Every production feature must include:

* Real metrics (not constants)
* Real timing measurements
* Error counters
* Structured logging

Fake metrics or simulated observability are forbidden.

---

## 8. Tests Are Not Optional

If execution logic is added, the AI must:

* Add unit tests for success paths
* Add tests for failure paths
* Avoid mocks that bypass real logic

If meaningful tests cannot be written, the AI must notify the user and stop.

---

## 9. No Illusion Rule (Most Important)

> **Code that looks complete but is not is worse than missing code.**

If a feature is only:

* An API surface
* A protocol definition
* A client method

It must be clearly marked as **NOT IMPLEMENTED** and must not claim success.

---

## 10. Required Self‑Check Before Output

Before responding with code, the AI must internally verify:

* Does this code actually perform the promised work?
* Would this survive real load?
* Would I deploy this without fear?

If the answer to any is **NO**, the AI must stop and notify the user.

---

## 11. Enforcement Statement

These rules override:

* Speed
* Convenience
* TODO completion
* Cosmetic correctness

Truth, correctness, and production safety take priority over progress metrics.

---

**This document exists to prevent the creation of fake progress.**
If these rules are followed strictly, simulated implementations cannot exist.
