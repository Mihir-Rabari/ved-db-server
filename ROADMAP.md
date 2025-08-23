# Roadmap

## Near‑term (0.x)
- Harden remote API:
  - Replace experimental TCP loop with `tonic` gRPC service surface
  - Backpressure, timeouts, and connection limits
- Packaging & UX:
  - Config file (TOML) support mirroring CLI flags
  - Windows Service template and Linux systemd unit
  - GitHub Releases for Windows/Linux/macOS (CI in place)
- Examples & SDKs:
  - Minimal Rust client examples (local and remote)
  - Start Go/Python thin clients (unstable wire format noted)
- Observability: structured logs, log rotation guidance, basic counters

## Mid‑term
- Persistence: WAL and periodic snapshots with recovery
- Replication: async followers, eventual consistency, leader election design
- Metrics: Prometheus exporter and useful dashboards
- Auth/Z: tokens and ACLs for remote access
- Memory: compaction and arena defragmentation strategies

## Stretch
- Multi‑tenant isolation and quotas
- Extended data types (lists, sets) over KV core
- Clustered deployments and sharding across nodes (research)
