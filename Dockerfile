# Multi-stage Dockerfile for VedDB Server
# Optimized for production deployment with minimal image size

# ============================================================================
# Stage 1: Builder
# ============================================================================
FROM rust:1-bullseye AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /build

# Copy workspace manifests
COPY Cargo.toml Cargo.lock ./

# Copy only what we need for veddb-server
COPY veddb-core ./veddb-core
COPY veddb-server ./veddb-server

# Build release binary for veddb-server only
RUN cargo build --package veddb-server --release

# ============================================================================
# Stage 2: Runtime
# ============================================================================
FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
RUN useradd -m -u 1000 -s /bin/bash veddb

# Create data directories
RUN mkdir -p /var/lib/veddb/data \
    /var/lib/veddb/backups \
    /var/lib/veddb/keys \
    && chown -R veddb:veddb /var/lib/veddb

# Copy binary from builder
COPY --from=builder /build/target/release/veddb-server /usr/local/bin/veddb-server

# Set ownership
RUN chown veddb:veddb /usr/local/bin/veddb-server

# Switch to non-root user
USER veddb

# Set working directory
WORKDIR /var/lib/veddb

# Expose TCP port
EXPOSE 50051

# Health check - verify server is listening on port 50051
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD timeout 2 bash -c 'cat < /dev/null > /dev/tcp/localhost/50051' || exit 1

# Default command with sensible defaults
CMD ["veddb-server", \
     "--data-dir", "/var/lib/veddb/data", \
     "--host", "0.0.0.0", \
     "--port", "50051", \
     "--cache-size-mb", "256"]
