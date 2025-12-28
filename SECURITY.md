# Security Policy

**VedDB Security Practices and Vulnerability Reporting**

**Last updated:** 2025-12-28  
**Version:** v0.2.0

---

## Threat Model

VedDB v0.2.0 is designed for **trusted network deployments**.

### What VedDB Protects Against

✅ **Encryption at Rest:**
- Data encrypted with AES-256-GCM
- Key rotation with full re-encryption
- Crash-safe rotation state machine

✅ **Authentication:**
- JWT-based authentication
- Role-based access control (RBAC)
- Password hashing

✅ **Data Integrity:**
- WAL-based durability
- Atomic metadata updates
- Crash recovery

### What VedDB Does NOT Protect Against (Yet)

❌ **Network Attacks (without additional hardening):**
- Man-in-the-middle (TLS validation incomplete)
- DDoS attacks (no rate limiting)
- Connection flooding

❌ **Compliance Gaps:**
- No audit trail (audit logging missing)
- Incomplete JWT revocation
- No session management

❌ **Operational Security:**
- No runtime secret rotation
- No automatic threat detection
- No intrusion detection

---

## Current Security Status

### Production-Ready

- **Encryption at rest** (1,292 LOC)
- **Key rotation** with crash recovery
- **Authentication** (JWT)
- **Authorization** (RBAC)

### Requires Hardening (P1)

- **TLS validation** - Certificate chains not verified
- **Rate limiting** - No protection against floods
- **Audit logging** - No compliance trail
- **Token revocation** - Incomplete implementation

### Not Implemented

- **Network-level security**
- **DDoS protection**
- **Intrusion detection**
- **Automated threat response**

---

## Deployment Security Guidance

### ✅ Safe Deployments

**VedDB v0.2.0 is safe for:**
- Internal networks (firewalled)
- Development environments
- Trusted corporate networks
- Staging validation

**Requirements:**
- Network access control (firewall)
- Strong authentication enabled
- Encryption enabled for sensitive data

### ❌ Unsafe Deployments

**DO NOT deploy VedDB v0.2.0 to:**
- Public internet (without TLS validation)
- Compliance-critical systems (without audit logging)
- Untrusted networks
- High-value targets without WAF

**Required Mitigations:**
1. Complete P1 security items
2. Add reverse proxy/WAF
3. Implement rate limiting
4. Security audit
5. Penetration testing

---

## Security Checklist

Before deploying to any production environment:

### Authentication & Authorization

- [ ] Default credentials changed
- [ ] Strong passwords enforced (not admin/admin123)
- [ ] JWT secret is cryptographically random
- [ ] Role assignments reviewed
- [ ] Principle of least privilege applied

### Encryption

- [ ] Encryption enabled
- [ ] Master key stored securely (not in command line)
- [ ] Master key backed up securely
- [ ] Key rotation tested
- [ ] Backup encryption verified

### Network Security

- [ ] Firewall rules configured
- [ ] Access limited to known IPs/networks
- [ ] TLS enabled (even with self-signed certs)
- [ ] VPN required for remote access
- [ ] Monitoring configured

### Operational Security

- [ ] Logs reviewed regularly
- [ ] Alerts configured
- [ ] Incident response plan documented
- [ ] Backup restoration tested
- [ ] Security updates applied

---

## Known Security Limitations

See [LIMITATIONS.md](LIMITATIONS.md) for complete list.

**Critical:**
- TLS certificate validation incomplete
- JWT revocation incomplete
- No audit logging
- No rate limiting

**Medium:**
- No session management
- Cache invalidation global (performance issue)

**Low:**
- No automated threat detection

---

## Reporting a Vulnerability

### DO NOT

- Open a public GitHub issue
- Post in public forums
- Disclose before patch availability

### DO

**Email:** mihirrabari2604@gmail.com

**Subject:** `[SECURITY] VedDB Vulnerability Report`

**Include:**
1. Affected version/commit
2. Detailed description
3. Reproduction steps or proof-of-concept
4. Impact assessment (confidentiality, integrity, availability)
5. Suggested mitigation (if known)
6. Your contact information

### Response Timeline

- **Acknowledgment:** Within 3 business days
- **Initial assessment:** Within 7 days
- **Fix timeline:** Depends on severity
  - Critical: 7-14 days
  - High: 14-30 days
  - Medium: 30-60 days
  - Low: Next release

### Coordinated Disclosure

We follow responsible disclosure:

1. **Report received** → Private investigation
2. **Fix developed** → Private testing
3. **Patch released** → Public disclosure with credit (if desired)
4. **CVE assigned** (if applicable)

**We will:**
- Keep you informed of progress
- Credit you in release notes (unless you prefer anonymity)
- Work with you on disclosure timing

---

## Security Fixes

### v0.2.0

**Encryption State Machine Hardening:**
- Added startup enforcement (won't start mid-rotation)
- Implemented checkpoint-based crash recovery
- Enforced metadata update ordering

**Authentication:**
- JWT validation implemented
- Role-based access control

### Planned (v0.3.0)

**P1 Security Items:**
- TLS certificate validation
- JWT revocation list
- Audit logging
- Rate limiting

---

## Security Best Practices

### Master Key Management

**DO:**
- Use environment variables (`VEDDB_MASTER_KEY`)
- Store in secure secret management (Vault, AWS Secrets Manager)
- Rotate periodically
- Back up separately from data

**DO NOT:**
- Hard-code in source
- Pass via command line (visible in `ps`)
- Store in plain text files
- Share across environments

### Password Policy

**Recommendations:**
- Minimum 12 characters
- Mix of upper/lower/numbers/symbols
- No dictionary words
- No reuse across systems

### Network Security

**Recommendations:**
- Firewall everything by default
- Whitelist specific IPs/networks
- Use VPN for remote access
- Monitor connection attempts
- Implement fail2ban or equivalent

### Monitoring

**Security Events to Monitor:**
- Failed authentication attempts
- Permission denied errors
- Encryption failures
- Unusual connection patterns
- Large data exports

---

## Compliance Considerations

VedDB v0.2.0 has **partial compliance support**.

### What Works

**Encryption:**
- AES-256-GCM (FIPS 140-2 approved algorithm)
- Key rotation with audit trail (in logs)

**Access Control:**
- Role-based access control
- Authentication required

### What's Missing

**SOC 2:**
- ❌ Audit logging
- ❌ Session management
- ❌ Comprehensive access logs

**HIPAA:**
- ❌ Audit trail
- ❌ Transmission security (TLS validation)
- ❌ Integrity controls (CAS enforcement)

**PCI-DSS:**
- ❌ Network segmentation documentation
- ❌ Logging and monitoring
- ❌ Penetration testing

**GDPR:**
- ✅ Encryption at rest
- ❌ Right to be forgotten (manual)
- ❌ Data portability (manual)
- ❌ Breach notification tooling

**Recommendation:** Complete P1 before claiming compliance.

---

## Security Roadmap

See [ROADMAP.md](ROADMAP.md) for detailed plan.

**P1 (Required for internet exposure):**
- TLS certificate validation
- JWT revocation
- Audit logging
- Rate limiting

**P2 (Operational hardening):**
- Intrusion detection
- Automated threat response
- Session management

---

## Security Contacts

**Primary:** mihirrabari2604@gmail.com  
**GitHub:** [Mihir-Rabari](https://github.com/Mihir-Rabari)

**For non-security issues:**
- GitHub Issues: https://github.com/Mihir-Rabari/ved-db-server/issues

---

## Closing Statement

VedDB takes security seriously, but is **honest about current limitations**.

We do not claim:
- Feature completeness
- Compliance readiness without hardening
- Internet-safety without additional work

We do guarantee:
- Honest disclosure of gaps
- Rapid response to reports
- Transparent communication

**If you discover a security issue not listed in [LIMITATIONS.md](LIMITATIONS.md), please report it.**

---

**Security is a journey, not a destination. VedDB is on that journey.**
