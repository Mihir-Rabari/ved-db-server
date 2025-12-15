# VedDB Migration Tool v0.1.x → v0.2.0

The VedDB migration tool (`veddb-migrate`) helps you migrate data from VedDB v0.1.x (simple key-value store) to VedDB v0.2.0 (hybrid document database).

## Overview

VedDB v0.2.0 introduces a completely new architecture:

- **v0.1.x**: Simple in-memory key-value store with binary protocol
- **v0.2.0**: Hybrid document database with persistent storage, collections, schemas, and caching

This tool converts your v0.1.x key-value data into v0.2.0 document format and stores it in a special `_legacy_kv` collection.

## Installation

The migration tool is included with VedDB v0.2.0:

```bash
# Build from source
cd ved-db-server
cargo build --release --bin veddb-migrate

# The binary will be at: target/release/veddb-migrate
```

## Usage

### Basic Migration

```bash
veddb-migrate --input /path/to/v0.1.x/data --output /path/to/v0.2.0/data
```

### Command Line Options

```bash
veddb-migrate [OPTIONS] --input <PATH> --output <PATH>

Options:
  -i, --input <PATH>        Path to v0.1.x data directory or backup file
  -o, --output <PATH>       Path to v0.2.0 data directory
  -c, --collection <NAME>   Target collection name [default: _legacy_kv]
      --dry-run            Perform validation without writing data
      --verify             Verify migration integrity after completion
      --force              Overwrite existing v0.2.0 data
  -h, --help               Print help
  -V, --version            Print version
```

### Examples

#### Migrate from backup file
```bash
veddb-migrate -i backup.json -o /var/lib/veddb-v2 --verify
```

#### Dry run to check data
```bash
veddb-migrate -i /old/veddb/data -o /new/veddb/data --dry-run
```

#### Force overwrite existing data
```bash
veddb-migrate -i data.backup -o /veddb-v2 --force --verify
```

#### Custom collection name
```bash
veddb-migrate -i backup.json -o /veddb-v2 -c my_legacy_data
```

## Input Data Formats

The migration tool supports several v0.1.x data formats:

### 1. Backup Files (JSON)

Standard VedDB v0.1.x backup format:

```json
{
  "version": "0.1.21",
  "timestamp": "2025-01-15T10:30:00Z",
  "data": {
    "dXNlcjE=": "QWxpY2U=",
    "dXNlcjI=": "Qm9i"
  },
  "metadata": {
    "dXNlcjE=": {
      "created_at": "2025-01-15T10:00:00Z",
      "ttl": 3600,
      "version": 1
    }
  }
}
```

Keys and values are base64-encoded.

### 2. Raw JSON Key-Value

Simple JSON object with string keys and values:

```json
{
  "user1": "Alice",
  "user2": "Bob",
  "config": "{\"theme\":\"dark\"}"
}
```

### 3. Data Directory

Directory containing multiple backup files:

```
/old/veddb/data/
├── backup-2025-01-15.json
├── backup-2025-01-14.json
└── export.json
```

The tool will process all `.json` and `.backup` files.

## Output Format

The migration creates a v0.2.0 database with the following structure:

```
/veddb-v2/
├── collections/
│   └── _legacy_kv/
│       ├── schema.json
│       ├── documents.rocksdb/
│       └── indexes/
├── metadata/
├── wal/
└── snapshots/
```

### Document Schema

Each v0.1.x key-value pair becomes a document:

```json
{
  "_id": "01HN123...",
  "key": "user1",
  "value": "QWxpY2U=",
  "original_key": "dXNlcjE=",
  "migrated_at": "2025-01-15T12:00:00Z",
  "original_ttl": 3600,
  "original_version": 1
}
```

Fields:
- `key`: UTF-8 string representation of the key (searchable)
- `value`: Base64-encoded original value
- `original_key`: Base64-encoded original key (exact preservation)
- `migrated_at`: Migration timestamp
- `original_ttl`: Original TTL value (if any)
- `original_version`: Original version (if any)

## Migration Process

The tool follows these steps:

1. **Read v0.1.x Data**: Parse input files and load key-value pairs
2. **Validate Input**: Check for oversized keys/values, duplicates, etc.
3. **Pre-migration Checks**: Verify output directory, permissions, disk space
4. **Initialize v0.2.0 Storage**: Create directory structure and collection
5. **Migrate Data**: Convert each key-value pair to a document
6. **Post-migration Validation**: Verify data integrity and completeness

## Validation and Verification

### Input Validation

The tool validates input data for:
- Maximum key size: 1MB
- Maximum value size: 16MB (v0.2.0 document limit)
- Maximum total keys: 10 million
- Duplicate keys (warns but continues)
- Empty keys (warns but continues)

### Migration Verification

With `--verify` flag, the tool:
- Compares document count with original key count
- Verifies a sample of migrated documents
- Checks data integrity by decoding and comparing values
- Validates collection structure and schema

## Compatibility Mode

After migration, VedDB v0.2.0 can run in compatibility mode to accept v0.1.x protocol commands. These commands will operate on the `_legacy_kv` collection:

```bash
# v0.1.x client commands work on migrated data
veddb-cli get user1  # Reads from _legacy_kv collection
veddb-cli set user3 Charlie  # Writes to _legacy_kv collection
```

## Performance

Migration performance depends on:
- **Data size**: Larger datasets take longer
- **Storage type**: SSDs are much faster than HDDs
- **Key/value sizes**: Many small pairs are faster than few large pairs

Typical performance:
- **Small datasets** (< 1GB): 1-5 minutes
- **Medium datasets** (1-10GB): 5-30 minutes  
- **Large datasets** (10-100GB): 30 minutes - 2 hours

## Troubleshooting

### Common Issues

#### "Output directory already exists"
```bash
# Use --force to overwrite
veddb-migrate -i data.json -o /veddb-v2 --force
```

#### "No v0.1.x data files found"
```bash
# Check input path and file formats
ls -la /path/to/input/
file /path/to/backup.json
```

#### "Value exceeds 16MB limit"
```bash
# Use --dry-run to identify oversized values
veddb-migrate -i data.json -o /tmp/test --dry-run
```

#### "Permission denied"
```bash
# Check write permissions
sudo chown -R $USER:$USER /veddb-v2/
chmod -R 755 /veddb-v2/
```

### Logging

Enable detailed logging:

```bash
RUST_LOG=veddb_migrate=debug veddb-migrate -i data.json -o /veddb-v2
```

Log levels:
- `error`: Critical errors only
- `warn`: Warnings and errors
- `info`: General progress (default)
- `debug`: Detailed operation info
- `trace`: Very verbose debugging

## Migration Checklist

Before migration:

- [ ] **Backup your v0.1.x data** (if not already backed up)
- [ ] **Stop the v0.1.x server** (if migrating from live data)
- [ ] **Check available disk space** (estimate 2x input size)
- [ ] **Verify input data format** (run with `--dry-run`)
- [ ] **Test migration on a copy** (recommended for large datasets)

After migration:

- [ ] **Verify migration results** (use `--verify` flag)
- [ ] **Test v0.2.0 server startup** with migrated data
- [ ] **Test compatibility mode** with v0.1.x clients
- [ ] **Update client applications** to use v0.2.0 features
- [ ] **Monitor performance** and adjust configuration as needed

## Support

For migration issues:

1. **Check logs** with `RUST_LOG=debug`
2. **Run dry-run** to identify problems
3. **Verify input format** matches supported formats
4. **Check GitHub issues** for known problems
5. **Create issue** with logs and data samples (anonymized)

## See Also

- [VedDB v0.2.0 Documentation](../README.md)
- [Migration Notes](../V0.2.0_MIGRATION_NOTES.md)
- [Compatibility Mode Guide](../docs/compatibility.md)
- [Performance Tuning](../docs/performance.md)