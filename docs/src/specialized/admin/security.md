# Security Reference

This document describes Torc's security features, threat model, and best practices for secure
deployments.

## Authentication & Authorization

### HTTP Basic Authentication

Torc uses HTTP Basic authentication with bcrypt password hashing.

**Security Properties:**

- ✅ Industry-standard authentication method
- ✅ Bcrypt hashing with configurable work factor (cost 4-31)
- ✅ No plaintext password storage
- ✅ Salt automatically generated per password
- ⚠️ Credentials sent base64-encoded (requires HTTPS)

**Architecture:**

```
Client Request
    ↓
[Authorization: Basic base64(username:password)]
    ↓
Server Middleware
    ↓
Extract credentials → Verify against htpasswd file (bcrypt)
    ↓
Success: Add username to request context
Failure: Return None authorization (logged)
    ↓
API Handler (receives authorization context)
```

### Authentication Modes

| Mode         | Configuration                | Behavior                                          |
| ------------ | ---------------------------- | ------------------------------------------------- |
| **Disabled** | No `--auth-file`             | All requests allowed, no authentication           |
| **Optional** | `--auth-file` only           | Valid credentials logged, invalid/missing allowed |
| **Required** | `--auth-file --require-auth` | Invalid/missing credentials rejected              |

**Recommendation:** Use **Required** mode in production.

## Transport Security

### HTTPS/TLS

**When to use HTTPS:**

- ✅ **Always** when authentication is enabled
- ✅ When transmitting sensitive workflow data
- ✅ Over untrusted networks (internet, shared networks)
- ✅ Compliance requirements (PCI-DSS, HIPAA, etc.)

**Configuration:**

```bash
# Server
torc-server run --https --auth-file /etc/torc/htpasswd

# Client with custom CA certificate (e.g., corporate PKI)
torc --url https://torc.hpc.nrel.gov:8080/torc-service/v1 \
     --tls-ca-cert /etc/pki/tls/certs/corporate-ca.pem \
     workflows list

# Client with publicly trusted certificate (no extra config needed)
torc --url https://torc.hpc.nrel.gov:8080/torc-service/v1 workflows list
```

**TLS Version:** Torc uses the system's OpenSSL/native-tls library. Ensure:

- TLS 1.2 minimum (TLS 1.3 preferred)
- Strong cipher suites enabled
- Valid certificates from trusted CA

For detailed client-side TLS setup, see [TLS/HTTPS Configuration](./tls-configuration.md).

### Network Security

**Deployment Patterns:**

**Pattern 1: Internal Network Only**

```
[Torc Clients] ←→ [Torc Server]
    (Trusted internal network)
```

- May use HTTP if network is truly isolated
- Still recommend HTTPS for defense in depth

**Pattern 2: Load Balancer with TLS Termination**

```
[Torc Clients] ←HTTPS→ [Load Balancer] ←HTTP→ [Torc Server]
    (Internet)              (Internal trusted network)
```

- TLS terminates at load balancer
- Internal traffic may use HTTP
- Ensure load balancer validates certificates

**Pattern 3: End-to-End TLS**

```
[Torc Clients] ←HTTPS→ [Torc Server]
    (Internet or untrusted network)
```

- Most secure pattern
- TLS all the way to Torc server
- Required for compliance scenarios

### Internal Network Quick Setup

For internal, trusted networks where the primary goal is preventing accidental access rather than
defending against malicious attacks, use this optimized configuration:

```bash
# 1. Create htpasswd with lower cost factor (faster auth)
torc-htpasswd add --cost 8 /etc/torc/htpasswd alice
torc-htpasswd add --cost 8 /etc/torc/htpasswd bob

# 2. Run server with auth and access control
torc-server run \
  --auth-file /etc/torc/htpasswd \
  --require-auth \
  --enforce-access-control \
  --credential-cache-ttl-secs 60
```

This provides:

- User isolation (users only see their own workflows)
- Fast CLI response (~10ms first auth, <1ms cached)
- Protection against accidental modifications
- Simple username/password authentication

For higher security requirements (internet-facing, compliance, sensitive data), use the default cost
factor (12) and HTTPS.

## Credential Management

### Password Requirements

**Enforced Validation:**

The `torc-htpasswd` tool automatically validates passwords using
[zxcvbn](https://github.com/shssoichern/zxcvbn-rs), a realistic password strength estimator inspired
by password crackers. Unlike rule-based checkers (e.g., "must include a symbol"), zxcvbn recognizes
common patterns such as dictionary words, keyboard sequences, repeated characters, l33t
substitutions, and dates, then estimates the number of guesses an attacker would need.

Passwords must meet two requirements:

1. **Minimum length of 8 characters** (per
   [NIST SP 800-63B](https://pages.nist.gov/800-63-3/sp800-63b.html))
2. **zxcvbn strength score of at least 3 out of 4** ("safely unguessable: moderate effort to crack")

When a password is rejected, the tool displays the score, a warning (e.g., "This is a top-10 common
password"), and a suggestion for improvement.

**Additional Recommendations:**

- Minimum 12 characters for high-security environments
- Unique per user and environment

**Bcrypt Cost Factor:**

| Cost | Approx Time | Use Case                                  |
| ---- | ----------- | ----------------------------------------- |
| 4    | ~1ms        | Testing only                              |
| 8    | ~10ms       | Internal trusted networks                 |
| 10   | ~50ms       | Low-security internal                     |
| 12   | ~250ms      | **Default**, general use, internet-facing |
| 14   | ~1s         | High security, sensitive data             |
| 15+  | > 2s        | Maximum security, infrequent logins       |

**Cost Selection Criteria:**

- Higher cost = more CPU, slower login
- Balance security vs. user experience
- Consider attack surface (internet-facing vs. internal)
- For internal networks with trusted users, cost 8 provides reasonable security with fast response

**Creating passwords with a specific cost:**

```bash
# Default cost (12) - good for most deployments
torc-htpasswd add /etc/torc/htpasswd alice

# Lower cost (8) - faster, suitable for internal networks
torc-htpasswd add --cost 8 /etc/torc/htpasswd alice

# Higher cost (14) - slower, for high-security environments
torc-htpasswd add --cost 14 /etc/torc/htpasswd alice
```

### Credential Caching

To improve CLI responsiveness, the server caches successful authentications for a configurable
duration (default: 60 seconds). This avoids repeated bcrypt verification for the same credentials.

**Configuration:**

```bash
# CLI option
torc-server run --credential-cache-ttl-secs 60

# Environment variable
export TORC_CREDENTIAL_CACHE_TTL_SECS=60

# Config file (torc.toml)
[server]
credential_cache_ttl_secs = 60
```

**Performance impact:**

| Scenario               | Response Time |
| ---------------------- | ------------- |
| First auth (cost 12)   | ~250ms        |
| First auth (cost 8)    | ~10ms         |
| Cached auth (any cost) | < 1ms         |

**Security notes:**

- Passwords are never stored in plaintext in the cache
- Cache keys are SHA-256 hashes of credentials
- Cache entries auto-expire after TTL
- Failed authentications are never cached
- Set `--credential-cache-ttl-secs 0` to disable caching

### Htpasswd File Security

**File Permissions:**

```bash
# Restrict to server process owner only
chmod 600 /etc/torc/htpasswd
chown torc-server:torc-server /etc/torc/htpasswd
```

**Storage Best Practices:**

- ❌ Never commit to version control
- ❌ Never share between environments
- ✅ Store in secure configuration management (Ansible Vault, HashiCorp Vault)
- ✅ Backup with encryption
- ✅ Rotate regularly (quarterly recommended)

**File Format Security:**

```
# Comments allowed
username:$2b$12$hash...
```

- Only bcrypt hashes accepted (`$2a$`, `$2b$`, or `$2y$`)
- No plaintext passwords
- No MD5, SHA-1, or weak hashes

### Client Credential Storage

**Best Practices:**

| Method                    | Security   | Use Case                       |
| ------------------------- | ---------- | ------------------------------ |
| **Environment variables** | ⭐⭐⭐     | Scripts, automation, CI/CD     |
| **Password prompt**       | ⭐⭐⭐⭐⭐ | Interactive sessions           |
| **Config files**          | ⭐         | Not recommended                |
| **Command-line args**     | ⚠️          | Visible in process list, avoid |

**Examples:**

```bash
# Good: Environment variables
read -s TORC_PASSWORD && export TORC_PASSWORD
torc workflows list

# Good: Password prompt
torc alice workflows list
Password: ****

# Acceptable: CI/CD with secrets
TORC_PASSWORD=${{ secrets.TORC_PASSWORD }} torc workflows create

# Bad: Command-line argument (visible in `ps`)
torc --password mypassword workflows list
```

## Threat Model

### Threats Mitigated

| Threat                          | Mitigation                        | Effectiveness  |
| ------------------------------- | --------------------------------- | -------------- |
| **Unauthorized API access**     | Required authentication           | ✅ High        |
| **Credential stuffing**         | Bcrypt work factor, rate limiting | ✅ Medium-High |
| **Password cracking**           | Bcrypt (cost ≥12)                 | ✅ High        |
| **Man-in-the-middle**           | HTTPS/TLS                         | ✅ High        |
| **Credential theft (database)** | No plaintext storage, bcrypt      | ✅ High        |

### Threats Not Mitigated

| Threat                   | Impact | Recommendation                         |
| ------------------------ | ------ | -------------------------------------- |
| **DDoS attacks**         | High   | Use rate limiting, firewalls, CDN      |
| **SQL injection**        | Medium | Use parameterized queries (Torc does)  |
| **Insider threats**      | High   | Audit logging, least privilege         |
| **Compromised client**   | High   | Network segmentation, monitoring       |
| **Side-channel attacks** | Low    | Constant-time operations (bcrypt does) |

### Attack Scenarios

**Scenario 1: Compromised htpasswd file**

**Impact:** Attacker has password hashes

**Risk:** Medium - Bcrypt makes cracking difficult

**Mitigation:**

1. Immediately revoke all user accounts
2. Generate new htpasswd file with fresh passwords
3. Investigate how file was compromised
4. Increase bcrypt cost if needed

**Scenario 2: Leaked credentials in logs**

**Impact:** Credentials in plaintext in logs

**Risk:** High

**Prevention:**

- Never log passwords
- Sanitize logs before sharing
- Restrict log access

**Response:**

1. Rotate affected credentials immediately
2. Audit all log access
3. Review code for password logging

**Scenario 3: Network eavesdropping (HTTP)**

**Impact:** Credentials intercepted in transit

**Risk:** Critical over untrusted networks

**Prevention:**

- **Always use HTTPS** when authentication is enabled
- Especially critical for internet-facing deployments

**Response:**

1. Enable HTTPS immediately
2. Rotate all credentials (assume compromised)
3. Review access logs for suspicious activity

## Audit & Monitoring

### Authentication Events

**Server logs authentication events:**

```
# Successful authentication
DEBUG torc::server::auth: User 'alice' authenticated successfully

# Failed authentication (wrong password)
WARN torc::server::auth: Authentication failed for user 'alice'

# Missing credentials when required
WARN torc::server::auth: Authentication required but no credentials provided

# No authentication configured
DEBUG torc::server::auth: No authentication configured, allowing request
```

### Recommended Monitoring

**Metrics to track:**

1. Failed authentication attempts (per user, total)
2. Successful authentications (per user)
3. Requests without credentials (when auth enabled)
4. Unusual access patterns (time, volume, endpoints)

**Alerting thresholds:**

- 5+ failed attempts from same user in 5 minutes
- 100+ failed attempts total in 1 hour
- Authentication from unexpected IP ranges
- Access during unusual hours (if applicable)

**Log aggregation:**

```bash
# Collect auth events
grep "torc::server::auth" /var/log/torc-server.log

# Count failed attempts per user
grep "Authentication failed" /var/log/torc-server.log | \
  awk '{print $(NF)}' | sort | uniq -c

# Monitor in real-time
tail -f /var/log/torc-server.log | grep "WARN.*auth"
```

## Compliance Considerations

### GDPR / Privacy

**User data in htpasswd:**

- Usernames may be personal data (email addresses)
- Password hashes are not personal data (irreversible)

**Recommendations:**

- Allow users to request account deletion
- Don't use email addresses as usernames (use aliases)
- Document data retention policies

### PCI-DSS / SOC2

**Requirements that apply:**

1. **Transport encryption:** Use HTTPS
2. **Access control:** Enable required authentication
3. **Password complexity:** Enforce strong passwords
4. **Audit logging:** Enable and monitor auth logs
5. **Regular reviews:** Audit user accounts quarterly

**Configuration:**

```bash
# PCI-DSS compliant setup
torc-server run \
  --https \
  --auth-file /etc/torc/htpasswd \
  --require-auth \
  --log-level info
```

## Security Checklist

### Server Deployment

- [ ] HTTPS enabled in production
- [ ] Strong TLS configuration (TLS 1.2+, strong ciphers)
- [ ] Valid certificate from trusted CA
- [ ] Client `--tls-ca-cert` configured if using internal CA
- [ ] Required authentication enabled (`--require-auth`)
- [ ] Htpasswd file permissions: `chmod 600`
- [ ] Htpasswd file owned by server process user
- [ ] Bcrypt cost ≥ 12 (≥14 for high-security)
- [ ] Strong passwords enforced
- [ ] Audit logging enabled
- [ ] Log rotation configured
- [ ] Firewall rules limit access
- [ ] Server runs as non-root user
- [ ] Regular security updates applied

### Client Usage

- [ ] HTTPS URLs used when auth enabled
- [ ] `--tls-ca-cert` or `TORC_TLS_CA_CERT` set if using internal CA
- [ ] `--tls-insecure` is **not** used in production
- [ ] Credentials stored in environment variables (not command-line)
- [ ] Passwords not logged
- [ ] Passwords not committed to version control
- [ ] Password prompting used for interactive sessions
- [ ] CI/CD secrets used for automation
- [ ] Regular password rotation

### Operational

- [ ] User accounts reviewed quarterly
- [ ] Inactive accounts disabled/removed
- [ ] Failed login attempts monitored
- [ ] Access logs reviewed regularly
- [ ] Incident response plan documented
- [ ] Backup htpasswd files encrypted
- [ ] Disaster recovery tested

## Access Control

### Admin Group

Torc uses a special **admin** group to control who can manage access groups. Key properties:

- **Config-driven**: Admin users are configured via `--admin-user` flag or `admin_users` config
- **Auto-created**: The admin group is created/synced automatically on server startup
- **System group**: Cannot be deleted or have membership modified via the API
- **Required for management**: Only admin members can create, delete, or modify access groups

**Configuration:**

```bash
# Via CLI
torc-server run --admin-user alice --admin-user bob --enforce-access-control

# Via config file
[server]
admin_users = ["alice", "bob"]
enforce_access_control = true
```

### Access Group Permissions

| Operation                  | Required Permission                   |
| -------------------------- | ------------------------------------- |
| Create group               | System admin                          |
| Delete group               | System admin (non-system groups only) |
| Add user to group          | System admin or group admin           |
| Remove user from group     | System admin or group admin           |
| Add workflow to group      | Workflow owner or group admin         |
| Remove workflow from group | Workflow owner or group admin         |
| List groups                | Any authenticated user                |
| Get group details          | Any authenticated user                |

## Future Enhancements

Planned security features:

1. **Token-based authentication:** JWT/OAuth2 support
2. **API keys:** Long-lived tokens for automation
3. **LDAP/Active Directory integration:** Enterprise SSO
4. **Rate limiting:** Prevent brute force attacks
5. **2FA/MFA support:** Multi-factor authentication
6. **Session management:** Token expiration, refresh
7. **Audit trail:** Detailed access logging

## Resources

- [OWASP Authentication Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html)
- [bcrypt Wikipedia](https://en.wikipedia.org/wiki/Bcrypt)
- [HTTP Basic Authentication RFC 7617](https://tools.ietf.org/html/rfc7617)
- [NIST Password Guidelines](https://pages.nist.gov/800-63-3/sp800-63b.html)
