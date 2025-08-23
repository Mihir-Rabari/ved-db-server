# Contributing to VedDB Server

Thank you for your interest in contributing! This document explains how to build, test, file issues, and submit PRs.

## Code of Conduct

Please read and follow our [Code of Conduct](CODE_OF_CONDUCT.md).

## Development Environment

- Rust 1.75+ (stable)
- Windows, Linux, or macOS
- Recommended: `rustup`, `cargo`, `clippy`, `rustfmt`

## Building

- Workspace build: `cargo build --release`
- Server only: `cargo build --release -p veddb-server`

## Testing

- Unit tests: `cargo test`
- Benchmarks (if enabled): `cargo bench`

## Lint & Format

- Format: `cargo fmt --all`
- Lints: `cargo clippy --all-targets -- -D warnings`

## Pull Requests

1. Fork the repo and create a branch from `main`.
2. Keep PRs small and focused. Add tests where possible.
3. Ensure CI passes and no lints remain.
4. PR title and description should explain the change and rationale.

## Commit Message Guidelines

- Use imperative mood: "Add X", "Fix Y".
- Reference issues: `Fixes #123` or `Refs #456`.

## Security

If you discover a security issue, please follow [SECURITY.md](SECURITY.md) and do not open a public issue.

## Questions

Open a discussion or issue with the `question` label.
