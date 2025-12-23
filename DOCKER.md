# 🐳 Docker Deployment Guide

## Quick Start

### Pull and Run
```bash
docker pull mihirrabariii/veddb-server:latest

docker run -d \
  --name veddb \
  -p 50051:50051 \
  -v veddb-data:/var/lib/veddb/data \
  mihirrabariii/veddb-server:latest
```

### Check Status
```bash
docker ps
docker logs veddb
```

## Docker Compose

Create `docker-compose.yml`:

```yaml
version: '3.8'

services:
  veddb:
    image: mihirrabariii/veddb-server:latest
    container_name: veddb
    ports:
      - "50051:50051"
    volumes:
      - veddb-data:/var/lib/veddb/data
      - veddb-backups:/var/lib/veddb/backups
    environment:
      - RUST_LOG=info
    restart: unless-stopped
    healthcheck:
      test: ["CMD-SHELL", "timeout 2 bash -c 'cat < /dev/null > /dev/tcp/localhost/50051'"]
      interval: 30s
      timeout: 5s
      retries: 3

volumes:
  veddb-data:
  veddb-backups:
```

Run:
```bash
docker-compose up -d
```

## Configuration

### Environment Variables

Create `.env` file:
```bash
RUST_LOG=info              # Logging level (trace|debug|info|warn|error)
VEDDB_CACHE_SIZE=512       # Cache size in MB
VEDDB_MASTER_KEY=secret     # Master encryption key
```

### Command Line Arguments

```bash
docker run -d \
  -p 50051:50051 \
  -v veddb-data:/var/lib/veddb/data \
  mihirrabariii/veddb-server:latest \
  veddb-server \
    --data-dir /var/lib/veddb/data \
    --cache-size-mb 512 \
    --enable-backups \
    --backup-dir /var/lib/veddb/backups
```

## Production Deployment

### With All Features
```bash
docker run -d \
  --name veddb \
  -p 50051:50051 \
  -v veddb-data:/var/lib/veddb/data \
  -v veddb-backups:/var/lib/veddb/backups \
  -v veddb-keys:/var/lib/veddb/keys \
  -e RUST_LOG=info \
  --restart unless-stopped \
  mihirrabariii/veddb-server:latest \
  veddb-server \
    --data-dir /var/lib/veddb/data \
    --enable-backups \
    --backup-dir /var/lib/veddb/backups \
    --enable-encryption \
    --master-key your-secure-key \
    --cache-size-mb 1024
```

### Resource Limits
```bash
docker run -d \
  --name veddb \
  --memory="2g" \
  --cpus="2.0" \
  -p 50051:50051 \
  -v veddb-data:/var/lib/veddb/data \
  mihirrabariii/veddb-server:latest
```

## Maintenance

### View Logs
```bash
docker logs veddb
docker logs -f veddb  # Follow mode
```

### Backup Data
```bash
# Create backup
docker exec veddb tar -czf /tmp/backup.tar.gz /var/lib/veddb/data

# Copy to host
docker cp veddb:/tmp/backup.tar.gz ./veddb-backup.tar.gz
```

### Update Image
```bash
docker pull mihirrabariii/veddb-server:latest
docker stop veddb
docker rm veddb
# Run with same config
```

## Troubleshooting

### Check Container Health
```bash
docker ps --format "table {{.Names}}\t{{.Status}}"
```

### Access Container Shell
```bash
docker exec -it veddb /bin/bash
```

### View Resource Usage
```bash
docker stats veddb
```

## Tags

- `latest` - Latest stable release
- `0.2.1` - Version 0.2.1 (current)
- `0.2.0` - Version 0.2.0

## Links

- **Docker Hub**: https://hub.docker.com/r/mihirrabariii/veddb-server
- **GitHub**: https://github.com/Mihir-Rabari/ved-db-server
- **Issues**: https://github.com/Mihir-Rabari/ved-db-server/issues
