# VedDB Deployment Guide

**Safe Deployment Practices**

**Last updated:** 2025-12-28  
**Version:** v0.2.0  
**Tone:** Protective, conservative

---

## Purpose

This document exists to **prevent unsafe VedDB deployments**.

It defines:
- Safe deployment scenarios
- Unsafe deployment scenarios
- Required hardening steps
- Pre-production checklist

**If you're unsure, default to NO.**

---

## Deployment Decision Tree

```
┌─ Are you exposing to the internet?
│
├─ YES ──> STOP ❌
│          Read "P1 Security Requirements" first
│
└─ NO ──> ┌─ Is this trusted internal network?
           │
           ├─ YES ──> ┌─ Dataset < 10M documents?
           │          │
           │          ├─ YES ──> ✅ SAFE (proceed)
           │          │
           │          └─ NO ──> ⚠️ VALIDATE SCALE FIRST
           │
           └─ NO ──> ❌ STOP
                      Not safe without TLS validation
```

---

## Safe Deployment Scenarios

These are explicitly approved for VedDB v0.2.0.

### ✅ 1. Internal Development

**Environment:**
- Local machine
- Development VM
- Internal dev network

**Characteristics:**
- No external access
- Test/experimental data
- Single developer

**Requirements:**
- None (use defaults)

**Command:**
```bash
veddb-server \
  --data-dir ./dev_data \
  --port 50051
```

---

### ✅ 2. Trusted Corporate Network

**Environment:**
- Internal corporate network
- Behind firewall
- Authenticated VPN access

**Characteristics:**
- Controlled access
- Known users
- Non-public data

**Requirements:**
- [ ] Enable authentication
- [ ] Enable encryption (if sensitive data)
- [ ] Configure backups
- [ ] Monitor logs

**Command:**
```bash
veddb-server \
  --data-dir /var/lib/veddb/data \
  --enable-encryption \
  --master-key "${VEDDB_MASTER_KEY}" \
  --cache-size-mb 512 \
  --port 50051
```

---

### ✅ 3. Staging Environment

**Purpose:**
Validate VedDB before production.

**Requirements:**
- [ ] Mirror production config
- [ ] Load test with real-world data volume
- [ ] Test key rotation
- [ ] Simulate crashes
- [ ] Verify backups restore correctly
- [ ] Monitor for 72+ hours

**Command:**
```bash
veddb-server \
  --data-dir /var/lib/veddb/data \
  --enable-backups \
  --backup-dir /var/lib/veddb/backups \
  --enable-encryption \
  --master-key "${VEDDB_MASTER_KEY}" \
  --cache-size-mb 1024 \
  --port 50051
```

---

## Unsafe Deployment Scenarios

These are **explicitly prohibited** without additional work.

### ❌ 1. Public Internet Exposure

**Why Forbidden:**
- TLS certificate validation incomplete
- No rate limiting
- No DDoS protection
- No audit logging

**Required Before Allowed:**
1. Complete P1.1 (TLS hardening)
2. Implement rate limiting
3. Enable audit logging
4. WAF/reverse proxy in front
5. Security audit

**Timeline:**
Not safe until v0.3.0+

---

### ❌ 2. Compliance-Critical Data

**Examples:**
- Healthcare (HIPAA)
- Payment cards (PCI-DSS)
- Personal data (GDPR strict interpretation)

**Why Forbidden:**
- No audit trail
- Incomplete token revocation
- TLS validation gaps

**Required Before Allowed:**
1. Audit logging implementation (P1.3)
2. Token revocation (P1.2)
3. TLS hardening (P1.1)
4. Compliance audit
5. Penetration testing

---

### ❌ 3. Billion-Row Datasets

**Why Forbidden:**
- Not tested at scale
- Aggregation memory limits
- Unknown performance characteristics

**Required Before Allowed:**
1. Scale testing with target dataset size
2. Streaming aggregation (P2.1)
3. Cache optimization
4. WAL compaction
5. Performance profiling

---

### ❌ 4. Mission-Critical Production (High SLA)

**Examples:**
- E-commerce checkout
- Financial transactions
- Real-time monitoring

**Why Caution:**
- Replication not stress-tested
- No automatic failover
- Limited production validation

**Required Before Allowed:**
1. Staging validation (minimum 30 days)
2. Chaos testing
3. Runbook documentation
4. On-call procedures
5. Backup restoration drills

---

## Pre-Production Checklist

Before deploying to ANY production environment:

### Security

- [ ] TLS enabled (even if not fully validated)
- [ ] Authentication enabled
- [ ] Strong passwords set (no default admin/admin123)
- [ ] Encryption enabled for sensitive data
- [ ] Network access restricted (firewall rules)

### Reliability

- [ ] Backups configured
- [ ] Backup restoration tested
- [ ] Monitoring configured
- [ ] Alert thresholds set
- [ ] Crash recovery tested

### Operations

- [ ] Runbook created
- [ ] On-call rotation defined
- [ ] Escalation path documented
- [ ] Rollback procedure tested

### Data

- [ ] Data migration plan
- [ ] Data validation scripts
- [ ] Rollback data strategy

### Testing

- [ ] Load tested with realistic data volume
- [ ] Failure modes tested (disk full, network loss, crashes)
- [ ] Key rotation tested with real dataset
- [ ] 72-hour stability test passed

---

## Environment-Specific Guidance

### Docker Deployment

**Basic (Development):**
```bash
docker run -d \
  -p 50051:50051 \
  -v veddb-data:/var/lib/veddb/data \
  mihirrabariii/veddb-server:latest
```

**Production (Trusted Network):**
```bash
docker run -d \
  --name veddb-prod \
  -p 50051:50051 \
  -v veddb-data:/var/lib/veddb/data \
  -v veddb-backups:/var/lib/veddb/backups \
  -e VEDDB_MASTER_KEY="${VEDDB_MASTER_KEY}" \
  --restart unless-stopped \
  --memory 4g \
  --cpus 2 \
  mihirrabariii/veddb-server:latest \
    veddb-server \
    --data-dir /var/lib/veddb/data \
    --enable-encryption \
    --enable-backups \
    --backup-dir /var/lib/veddb/backups \
    --cache-size-mb 2048
```

### Kubernetes Deployment

**NOT RECOMMENDED YET**

Reasons:
- No health check endpoints
- No readiness probes
- No graceful shutdown
- Stateful set complexity

**Timeline:** v0.3.0+

---

## Sizing Recommendations

### Small Deployment

**Workload:**
- < 1M documents
- < 100 req/sec
- < 10 concurrent users

**Resources:**
- CPU: 2 cores
- RAM: 4 GB
- Disk: SSD, 50 GB

**Config:**
```
--cache-size-mb 512
```

### Medium Deployment

**Workload:**
- 1M - 10M documents
- 100 - 1k req/sec
- 10 - 100 concurrent users

**Resources:**
- CPU: 4 cores
- RAM: 16 GB
- Disk: SSD, 500 GB

**Config:**
```
--cache-size-mb 4096
```

### Large Deployment

**NOT VALIDATED**

Requires:
- Scale testing
- Performance profiling
- Custom optimization

**Timeline:** Post-P2.1 completion

---

## Monitoring Requirements

**Minimum Viable Monitoring:**

1. **Uptime**
   - Process running
   - Port accessible

2. **Resource Usage**
   - CPU < 80%
   - RAM < 80%
   - Disk < 80%

3. **Error Logs**
   - Encryption failures
   - Connection errors
   - Storage errors

4. **Backup Status**
   - Last backup timestamp < 24h
   - Backup success/failure

**Recommended:**
- Latency percentiles (when metrics wired)
- Request rate
- Cache hit rate
- Replication lag (if using replication)

---

## Security Hardening Checklist

Even for trusted networks:

### Access Control

- [ ] Firewall rules limiting access
- [ ] VPN required for remote access
- [ ] Default credentials changed
- [ ] Principle of least privilege

### Encryption

- [ ] Encryption enabled
- [ ] Master key stored securely (not in command line)
- [ ] Key rotation tested
- [ ] Backup encryption verified

### Network

- [ ] Bind to specific interface (not 0.0.0.0 if single-host)
- [ ] TLS enabled (even with self-signed for now)
- [ ] Rate limiting at proxy/firewall level

### Operations

- [ ] Logs monitored
- [ ] Alerts configured
- [ ] Incident response plan
- [ ] Regular security updates

---

## Troubleshooting Deployment Issues

### Server Won't Start

**Check:**
1. Encryption state (`./encryption/rotation_state.json`)
2. Port availability (`netstat -an | grep 50051`)
3. Data directory permissions
4. Logs for explicit errors

### Performance Issues

**Check:**
1. Cache size vs. dataset size
2. Disk I/O (use `iostat`)
3. Network latency
4. Query complexity

### Data Corruption

**Recovery:**
1. STOP the server immediately
2. Do NOT attempt repairs
3. Restore from last known-good backup
4. Review logs for root cause
5. Report issue with logs

---

## Rollback Procedures

### Before Key Rotation

**NOT SAFE** - Key rotation is one-way.

**Mitigation:**
- Full backup before rotation
- Test rotation in staging first

### Before Upgrade

1. Stop server gracefully
2. Full backup (data + metadata)
3. Test upgrade in staging
4. Document rollback steps
5. Keep old binary available

**Rollback:**
1. Stop new version
2. Restore data from backup
3. Start old version
4. Verify data integrity

---

## When to Call for Help

**Immediate (stop server):**
- Encryption errors
- Data corruption suspicion
- Security breach
- WAL corruption

**High Priority:**
- Replication lag growing
- Performance degradation
- Backup failures

**Normal Priority:**
- Feature questions
- Optimization suggestions
- Enhancement requests

**Contact:**
- GitHub Issues: https://github.com/Mihir-Rabari/ved-db-server/issues
- Email: mihirrabari2604@gmail.com

---

## Closing Guidance

VedDB v0.2.0 is **production-viable for trusted environments**.

It is **NOT** production-hardened for:
- Public internet
- Compliance-critical data
- Billion-row scale
- High-SLA mission-critical systems

**The right deployment:**
- Matches your workload to VedDB's tested capabilities
- Accepts documented limitations
- Plans for known gaps

**The wrong deployment:**
- Assumes completeness
- Skips staging
- Ignores security warnings

---

**Deploy conservatively. Validate extensively. Scale cautiously.**
