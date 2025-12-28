# Contributing to VedDB Server

Thanks for checking out VedDB! This guide shows you how to build, test, run the server locally, and ship changes confidently.

> **📊 Before contributing, read [STATUS.md](STATUS.md)** to understand what's real vs. what needs work.

## Code of Conduct

Please read and follow our [Code of Conduct](CODE_OF_CONDUCT.md).

## Development Environment

**Requirements:**
- Rust 1.75+ (stable)
- Windows, Linux, or macOS
- Recommended: `rustup`, `cargo`, `clippy`, `rustfmt`

## Quick Dev Loop

```sh
cargo build -p veddb-server
./target/debug/veddb-server \
  --data-dir ./dev_data \
  --port 50051 \
  --cache-size-mb 128
```

In another shell, test with a client or integration tests.

## Building

**Workspace build:**
```sh
cargo build --release
```

**Server only:**
```sh
cargo build --release -p veddb-server
```

**Library only:**
```sh
cargo build --lib
```

## Testing

**Unit tests:**
```sh
cargo test
```

**Specific tests:**
```sh
cargo test --lib security_tests
cargo test --package veddb-core
```

**With output:**
```sh
cargo test -- --nocapture
```

## Lint & Format

**Format:**
```sh
cargo fmt --all
```

**Lints:**
```sh
cargo clippy --all-targets -- -D warnings
```

## Running the Server Locally

**Basic:**
```sh
veddb-server --data-dir ./veddb_data --port 50051
```

**With encryption:**
```sh
veddb-server \
  --data-dir ./veddb_data \
  --enable-encryption \
  --master-key your-secret-key \
  --port 50051
```

**With all features:**
```sh
veddb-server \
  --data-dir ./veddb_data \
  --enable-backups \
  --backup-dir ./backups \
  --enable-encryption \
  --master-key your-secret-key \
  --cache-size-mb 512 \
  --port 50051
```

## Feature Verification

When implementing a feature:

1. **Verify it's REAL** - Not just mocked or simulated
2. **Wire protocol handlers** - Ensure OpCode handlers exist
3. **Integration test** - Test end-to-end
4. **Update STATUS.md** - Reflect verified status
5. **Add LOC count** - For new features

**Current Reality Score:** 95% (see [STATUS.md](STATUS.md))

## Pull Requests

1. **Fork and branch** from `main`
2. **Read STATUS.md** to understand current state
3. **Keep PRs focused** - One feature/fix per PR
4. **Include tests** when applicable
5. **Run fmt/clippy/tests** locally - CI must be green
6. **Fill PR template** completely

**PR Description Should Include:**
- What changed
- Why (problem statement)
- How tested
- Performance impact (if any)
- Breaking changes (if any)

## Code Quality Standards

- **Clear names** - Prefer explicit over clever
- **Small modules** - Keep files focused
- **Document public APIs** - Especially tricky code paths
- **Error handling** - Use `Result<T, E>` properly
- **No unwrap()** in library code - Handle errors gracefully

## Commit Message Guidelines

**Format:**
```
type: Brief description

Longer explanation if needed.

Refs #123
```

**Types:**
- `feat:` New feature
- `fix:` Bug fix
- `docs:` Documentation changes
- `refactor:` Code refactoring
- `test:` Test updates
- `perf:` Performance improvements

**Examples:**
```
feat: Add aggregation pipeline $group operator

Implements $group stage with $sum, $count accumulators.
Includes memory bounds (100k groups max).

Refs #45
```

```
fix: Resolve key rotation crash on network failure

Added checkpoint persistence before network operations.
Ensures rotation can resume after crash.

Fixes #67
```

## Release Process

1. **Update CHANGELOG.md** under `[Unreleased]`
2. **Bump versions** in `Cargo.toml` files
3. **Verify build:** `cargo build --release`
4. **Run tests:** `cargo test`
5. **Create tag:** `git tag vX.Y.Z`
6. **Push tag:** `git push origin vX.Y.Z`
7. **CI builds** and uploads artifacts

## Issue Triage

**Labels:**
- `bug` - Something's broken
- `enhancement` - New feature request
- `good first issue` - Beginner-friendly
- `help wanted` - Community help needed
- `docs` - Documentation
- `perf` - Performance
- `verified` - Code-verified as real

**When reporting bugs, include:**
- OS and Rust version
- Reproduction steps
- Expected vs actual behavior
- Logs (if applicable)
- Configuration used

## Areas Needing Help

**High Priority:**
- TLS certificate validation
- Scale testing (network partitions, large datasets)
- Comprehensive error handling
- Rate limiting

**Medium Priority:**
- Compound indexes
- Cost-based query optimizer
- Streaming aggregation
- Transaction support

**See [STATUS.md](STATUS.md)** for detailed status of all features.

## Style & Tone

- **Honest** - Document limitations clearly
- **Simple** - Prefer straightforward over clever
- **Tested** - Verify features are real, not mocked
- **Safe** - Handle errors, use Result types

## Security

If you discover a security issue:

1. **Do NOT open a public issue**
2. **Follow [SECURITY.md](SECURITY.md)**
3. **Email:** mihirrabari2604@gmail.com
4. **Include:** Version, reproduction steps, impact

## Questions

**For non-sensitive questions:**
- Open a discussion with `question` label
- Check [STATUS.md](STATUS.md) first

**Direct contact:**
- Email: mihirrabari2604@gmail.com
- Instagram: @mihirrabariii

---

**Thank you for contributing to VedDB!** 🚀

Remember: **Truth is cheaper than outages.** Always verify your implementations are real, not simulated.
