# Contributing to VedDB Server

Thanks for checking out VedDB! This guide shows you how to build, test, run the server locally, and ship changes confidently. We keep things simple and fast so you can stay in the flow.

## Code of Conduct

Please read and follow our [Code of Conduct](CODE_OF_CONDUCT.md).

## Development Environment

- Rust 1.75+ (stable)
- Windows, Linux, or macOS
- Recommended: `rustup`, `cargo`, `clippy`, `rustfmt`

## Quick dev loop

```sh
cargo build -p veddb-server
./target/debug/veddb-server --create --name dev_db --memory-mb 128 --workers 4 --port 50051 --debug
```

In another shell, exercise the server with a small client or integration tests (see examples in `README.md` under “Using VedDB”).

## Building

- Workspace build: `cargo build --release`
- Server only: `cargo build --release -p veddb-server`

## Testing

- Unit tests: `cargo test`
- Benchmarks (if enabled): `cargo bench`
- Windows CI runs on GitHub Actions; please ensure tests pass on your platform too if possible.

## Lint & Format

- Format: `cargo fmt --all`
- Lints: `cargo clippy --all-targets -- -D warnings`

## Running the server locally

- Create or open an instance:
  - Create: `veddb-server --create --name dev_db --memory-mb 128`
  - Open existing: `veddb-server --name dev_db`
- Flags are documented in `README.md > Configuration`.

## Release process

- We publish binaries on tags via GitHub Actions.
- Steps:
  1. Update `CHANGELOG.md` under `[Unreleased]` and bump versions as needed.
  2. Create a tag: `git tag vX.Y.Z` and `git push origin vX.Y.Z`.
  3. CI builds/upload artifacts to the GitHub Release page.

## Issue triage

- Labels:
  - `bug`, `enhancement`, `good first issue`, `help wanted`, `docs`, `perf`.
- Please include OS, Rust version, repro steps, and logs when applicable.

## Pull Requests

1. Fork and branch from `main`.
2. Keep PRs tight; include tests when it makes sense.
3. Run fmt/clippy/tests locally. CI must be green.
4. In the PR description: what changed, why, how tested, and any follow‑ups.

## Style & tone

- Clear names, small modules, prefer explicit over clever.
- Document externally‑visible types and tricky code paths.

## Commit Message Guidelines

- Use imperative mood: "Add X", "Fix Y".
- Reference issues: `Fixes #123` or `Refs #456`.

## Security

If you discover a security issue, please follow [SECURITY.md](SECURITY.md) and do not open a public issue.

## Questions

Open a discussion or issue with the `question` label. We’re happy to help.
