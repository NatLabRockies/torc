# Authentication

Torc supports HTTP Basic authentication to secure access to your workflow orchestration server. This
guide explains how to set up and use authentication.

## Overview

Torc's authentication system provides:

- **Multi-user support** via htpasswd files
- **Bcrypt password hashing** for secure credential storage
- **Backward compatibility** - authentication is optional by default
- **Flexible deployment** - can require authentication or allow mixed access
- **CLI and environment variable** support for credentials

## Server-Side Setup

### 1. Create User Accounts

Use the `torc-htpasswd` utility to manage user accounts:

```bash
# Add a user (will prompt for password)
torc-htpasswd add --file /path/to/htpasswd username

# Add a user with password on command line
torc-htpasswd add --file /path/to/htpasswd --password mypassword username

# Add a user with custom bcrypt cost (higher = more secure but slower)
torc-htpasswd add --file /path/to/htpasswd --cost 14 username

# Generate a password hash for remote registration (see below)
torc-htpasswd hash username

# List all users
torc-htpasswd list --file /path/to/htpasswd

# Verify a password
torc-htpasswd verify --file /path/to/htpasswd username

# Remove a user
torc-htpasswd remove --file /path/to/htpasswd username
```

The htpasswd file format is simple:

```
# Torc htpasswd file
# Format: username:bcrypt_hash
alice:$2b$12$abcdefghijklmnopqrstuvwxyz1234567890ABCDEFGHIJKLMNOP
bob:$2b$12$zyxwvutsrqponmlkjihgfedcba0987654321ZYXWVUTSRQPONMLK
```

### 2. Start Server with Authentication

```bash
# Optional authentication (backward compatible mode)
torc-server run --auth-file /path/to/htpasswd

# Required authentication (all requests must authenticate)
torc-server run --auth-file /path/to/htpasswd --require-auth

# With access control enforcement and admin users
torc-server run --auth-file /path/to/htpasswd --require-auth \
  --enforce-access-control --admin-user alice --admin-user bob

# Can also use environment variable
export TORC_AUTH_FILE=/path/to/htpasswd
export TORC_ADMIN_USERS=alice,bob
torc-server run
```

**Authentication Modes:**

- **No `--auth-file`**: Authentication disabled, all requests allowed (default)
- **`--auth-file` only**: Authentication optional - authenticated requests are logged,
  unauthenticated requests allowed
- **`--auth-file --require-auth`**: Authentication required - unauthenticated requests are rejected

**Access Control:**

- **`--enforce-access-control`**: Users can only access workflows they own or have group access to
- **`--admin-user`**: Adds users to the admin group (can specify multiple times)

### 3. Server Logs

The server logs authentication events:

```
INFO  torc_server: Loading htpasswd file from: /path/to/htpasswd
INFO  torc_server: Loaded 3 users from htpasswd file
INFO  torc_server: Authentication is REQUIRED for all requests
...
DEBUG torc::server::auth: User 'alice' authenticated successfully
WARN  torc::server::auth: Authentication failed for user 'bob'
WARN  torc::server::auth: Authentication required but no credentials provided
```

## Client-Side Usage

### Using Command-Line Flags

```bash
# Provide credentials via flags
torc --username alice --password mypassword workflows list

# Username via flag, password will be prompted
torc --username alice workflows list
Password: ****

# All commands support authentication
torc --username alice --password mypassword workflows create workflow.yaml
```

### Using Environment Variables

```bash
# Set credentials in environment
export TORC_PASSWORD=mypassword

# Run commands without flags
torc workflows list
torc jobs list my-workflow-id
```

### Mixed Approach

```bash
# Username from env, password prompted
torc workflows list
Password: ****
```

## Security Best Practices

### 1. Use HTTPS in Production

Basic authentication sends base64-encoded credentials (easily decoded). **Always use HTTPS** when
authentication is enabled:

```bash
# Start server with HTTPS
torc-server run --https --auth-file /path/to/htpasswd --require-auth

# Client connects via HTTPS (with custom CA certificate if needed)
torc --url https://torc.hpc.nrel.gov:8080/torc-service/v1 \
     --tls-ca-cert /path/to/ca.pem \
     --username alice workflows list
```

For detailed TLS/HTTPS setup including custom CA certificates, self-signed certificates, and Slurm
integration, see [TLS/HTTPS Configuration](./tls-configuration.md).

### 2. Secure Credential Storage

**Do:**

- Store htpasswd files with restrictive permissions: `chmod 600 /path/to/htpasswd`
- Use environment variables for passwords in scripts
- Use password prompting for interactive sessions
- Rotate passwords periodically

**Don't:**

- Commit htpasswd files to version control
- Share htpasswd files between environments
- Pass passwords as command-line arguments in production (visible in process list)
- Use weak passwords or low bcrypt costs

### 3. Bcrypt Cost Factor

The cost factor determines password hashing strength:

- **Cost 4-8**: Fast but weaker (testing only)
- **Cost 10-12**: Balanced (default: 12)
- **Cost 13-15**: Strong (production systems)
- **Cost 16+**: Very strong (high-security environments)

```bash
# Use higher cost for production
torc-htpasswd add --file prod_htpasswd --cost 14 alice
```

### 4. Audit Logging

Monitor authentication events in server logs:

```bash
# Run server with debug logging for auth events
torc-server run --log-level debug --auth-file /path/to/htpasswd

# Or use RUST_LOG for granular control
RUST_LOG=torc::server::auth=debug torc-server run --auth-file /path/to/htpasswd
```

## Common Workflows

### Development Environment

```bash
# 1. Create test user
torc-htpasswd add --file dev_htpasswd --password devpass developer

# 2. Start server (auth optional)
torc-server run --auth-file dev_htpasswd --database dev.db

# 3. Use client without auth (still works)
torc workflows list

# 4. Or with auth
torc --username developer --password devpass workflows list
```

### Production Deployment

```bash
# 1. Create production users with strong passwords and high cost
torc-htpasswd add --file /etc/torc/htpasswd --cost 14 alice
torc-htpasswd add --file /etc/torc/htpasswd --cost 14 bob

# 2. Secure the file
chmod 600 /etc/torc/htpasswd
chown torc-server:torc-server /etc/torc/htpasswd

# 3. Start server with required auth, access control, and HTTPS
torc-server run \
  --https \
  --auth-file /etc/torc/htpasswd \
  --require-auth \
  --enforce-access-control \
  --admin-user alice \
  --database /var/lib/torc/production.db

# 4. Clients must authenticate
torc --url https://torc.hpc.nrel.gov:8080/torc-service/v1 --prompt-password workflows list
Password: ****
```

### CI/CD Pipeline

```bash
# Store credentials as CI secrets
# TORC_PASSWORD=<secure-password>

# Use in pipeline
export TORC_PASSWORD="${TORC_PASSWORD}"
export TORC_API_URL=https://torc.hpc.nrel.gov:8080/torc-service/v1

# Run workflow
torc workflows create pipeline.yaml
torc workflows start "${WORKFLOW_ID}"
```

### Remote User Registration (HPC Environments)

When users cannot directly access the server (e.g., HPC users connecting to a server they don't have
login access to), use the `hash` command to generate credentials:

**User (on HPC):**

```bash
# Generate password hash (username defaults to $USER)
torc-htpasswd hash
Password for 'alice':
Hashing password (cost=12)...
alice:$2b$12$xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
Send the line above to your server administrator.

# Or specify a different username
torc-htpasswd hash myusername
```

The hash output (`alice:$2b$12$...`) can be safely sent to the server administrator via email,
Slack, or any other channel - the bcrypt hash cannot be used to recover the original password.

**Administrator (on server):**

```bash
# Append the user's hash line to the htpasswd file
echo "alice:\$2b\$12\$xxxxx..." >> /etc/torc/htpasswd

# Or manually edit the file and paste the line
vim /etc/torc/htpasswd
```

**Notes:**

- The password is entered on the user's machine and never transmitted in plaintext
- The bcrypt hash is safe to transmit - it can only verify passwords, not recover them
- Users can customize the cost factor with `--cost` if needed
- For scripting, use `--password` flag (though less secure)

### Migrating from No Auth to Required Auth

```bash
# 1. Start: No authentication
torc-server run --database prod.db

# 2. Add authentication file (optional mode)
torc-server run --auth-file /etc/torc/htpasswd --database prod.db

# 3. Monitor logs, ensure clients are authenticating
# Look for "User 'X' authenticated successfully" messages

# 4. Once all clients authenticate, enable required auth
torc-server run --auth-file /etc/torc/htpasswd --require-auth --database prod.db
```

## Troubleshooting

### "Authentication required but no credentials provided"

**Cause:** Server has `--require-auth` but client didn't send credentials.

**Solution:**

```bash
# Add username and password
torc --username alice --password mypass workflows list
```

### "Authentication failed for user 'alice'"

**Cause:** Wrong password or user doesn't exist in htpasswd file.

**Solutions:**

```bash
# 1. Verify user exists
torc-htpasswd list --file /path/to/htpasswd

# 2. Verify password
torc-htpasswd verify --file /path/to/htpasswd alice

# 3. Reset password
torc-htpasswd add --file /path/to/htpasswd alice
```

### "No credentials provided, allowing anonymous access"

**Cause:** Server has `--auth-file` but not `--require-auth`, and client didn't authenticate.

**Solution:** This is normal in optional auth mode. To require auth:

```bash
torc-server run --auth-file /path/to/htpasswd --require-auth
```

### Password Prompting in Non-Interactive Sessions

**Problem:** Scripts or CI/CD fail waiting for password prompt.

**Solutions:**

```bash
# Use environment variable
export TORC_PASSWORD=mypassword
torc --username alice workflows list

# Or pass as flag (less secure - visible in process list)
torc --username alice --password mypassword workflows list
```

## Advanced Topics

### Multiple Environments

Maintain separate htpasswd files per environment:

```bash
# Development
torc-htpasswd add --file ~/.torc/dev_htpasswd --password devpass developer

# Staging
torc-htpasswd add --file /etc/torc/staging_htpasswd --cost 12 alice

# Production
torc-htpasswd add --file /etc/torc/prod_htpasswd --cost 14 alice
```

### Programmatic Access

When using Torc's Rust, Python, or Julia clients programmatically:

**Rust:**

```rust
use torc::client::apis::configuration::Configuration;

let mut config = Configuration::new();
config.base_path = "http://localhost:8080/torc-service/v1".to_string();
config.basic_auth = Some(("alice".to_string(), Some("password".to_string())));
```

**Python:**

```python
from torc import Configuration, ApiClient

config = Configuration(
    host="http://localhost:8080/torc-service/v1",
    username="alice",
    password="password"
)
```

**Julia:**

```julia
using Torc
using Base64
import OpenAPI

client = OpenAPI.Clients.Client(
    "http://localhost:8080/torc-service/v1";
    headers = Dict("Authorization" => "Basic " * base64encode("alice:password"))
)
api = Torc.APIClient.DefaultApi(client)
```

### Hot-Reloading Credentials

You can add, remove, or update user credentials without restarting the server. This is especially
useful in Docker, Kubernetes, and HPC environments where server restarts are disruptive.

**Workflow: Add a user and reload**

```bash
# 1. Add user to the htpasswd file
torc-htpasswd add --file /etc/torc/htpasswd alice_new

# 2. Reload credentials on the running server
torc admin reload-auth

# 3. The new user can now authenticate immediately
```

**Convenience flag: auto-reload after modification**

The `torc-htpasswd` tool supports a `--reload-auth` flag that automatically calls the server's
reload endpoint after modifying the htpasswd file:

```bash
# Add a user and reload in one step
torc-htpasswd add --file /etc/torc/htpasswd --reload-auth alice_new

# Remove a user and reload in one step
torc-htpasswd remove --file /etc/torc/htpasswd --reload-auth old_user

# Specify server URL and credentials for reload
torc-htpasswd add --file /etc/torc/htpasswd --reload-auth \
  --url https://torc.example.com:8080/torc-service/v1 \
  --server-password "$ADMIN_PASSWORD" alice_new
```

**Important notes:**

- Only admin users can call `torc admin reload-auth` (requires `--admin-user` on server)
- The credential cache is cleared on reload, so all users re-verify on their next request
- If the server was started without `--auth-file`, the reload endpoint returns an error
- The reload is atomic: either all credentials are updated or none are

**Docker/Kubernetes:**

```bash
# After updating the htpasswd ConfigMap/Secret:
kubectl exec torc-server -- torc admin reload-auth
```

No file-watching or container restart is needed.

### Load Balancer Considerations

When running multiple Torc servers behind a load balancer:

- Share the same htpasswd file across all servers (via NFS, S3, etc.)
- Or use a configuration management tool to sync htpasswd files
- After updating the htpasswd file, call `torc admin reload-auth` on each server instance
