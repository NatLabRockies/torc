# TLS/HTTPS Configuration

This guide explains how to configure Torc clients to connect to HTTPS servers, including custom CA
certificates and self-signed certificate support.

## Overview

When the Torc server runs behind a TLS-terminating reverse proxy (e.g., OpenShift ingress, nginx,
HAProxy) or with `--https` enabled directly, all clients must be configured to use HTTPS. Torc
provides two client-side TLS options:

- **`--tls-ca-cert`** - Path to a PEM-encoded CA certificate to trust
- **`--tls-insecure`** - Skip certificate verification (testing only)

These options apply to all Torc client components: the CLI, TUI, job runners, the MCP server, and
the web dashboard.

## When You Need This

| Scenario                                       | Configuration Needed                      |
| ---------------------------------------------- | ----------------------------------------- |
| Server behind corporate proxy with internal CA | `--tls-ca-cert /path/to/ca.pem`           |
| Self-signed certificates in development        | `--tls-insecure`                          |
| Server with publicly trusted certificate       | Just use `https://` URL (no extra config) |
| Server on plain HTTP (localhost, internal)     | No TLS config needed                      |

## Configuration Methods

TLS settings can be configured through CLI flags, environment variables, or the configuration file.
They follow the standard priority order: CLI flags > environment variables > config file > defaults.

### CLI Flags

```bash
# Connect using a custom CA certificate
torc --url https://torc.hpc.nrel.gov:8080/torc-service/v1 \
     --tls-ca-cert /etc/pki/tls/certs/corporate-ca.pem \
     workflows list

# Skip certificate verification (development/testing only)
torc --url https://localhost:8443/torc-service/v1 \
     --tls-insecure \
     workflows list

# Both flags work with any command
torc --tls-ca-cert /path/to/ca.pem workflows create workflow.yaml
torc --tls-ca-cert /path/to/ca.pem tui
```

### Environment Variables

```bash
# Set once, used by all torc commands in the session
export TORC_TLS_CA_CERT=/etc/pki/tls/certs/corporate-ca.pem
export TORC_API_URL=https://torc.hpc.nrel.gov:8080/torc-service/v1

# Now all commands use the CA certificate automatically
torc workflows list
torc workflows create workflow.yaml
torc tui
```

For testing with self-signed certificates:

```bash
export TORC_TLS_INSECURE=true
export TORC_API_URL=https://localhost:8443/torc-service/v1
torc workflows list
```

### Configuration File

Add TLS settings to your Torc configuration file (`~/.config/torc/config.toml` or `./torc.toml`):

```toml
[client]
api_url = "https://torc.hpc.nrel.gov:8080/torc-service/v1"

[client.tls]
# Path to PEM-encoded CA certificate
ca_cert = "/etc/pki/tls/certs/corporate-ca.pem"

# Skip certificate verification (default: false)
# insecure = true
```

Generate a default config file with TLS section included:

```bash
torc config init --user
```

## Deployment Patterns

### Pattern 1: OpenShift / Kubernetes with Ingress TLS

The most common production pattern. The server runs HTTP inside a container, and the platform
handles TLS termination at the ingress.

```
[Torc Client] ──HTTPS──> [OpenShift Ingress] ──HTTP──> [Torc Server Pod]
                 ^
            Custom CA cert
         (corporate PKI)
```

**Setup:**

```bash
# Your IT department provides a CA certificate
# Configure all clients to trust it
export TORC_TLS_CA_CERT=/etc/pki/tls/certs/corporate-ca.pem
export TORC_API_URL=https://torc.hpc.nrel.gov:8080/torc-service/v1

torc workflows list
```

### Pattern 2: Direct HTTPS with Custom Certificate

The server runs with `--https` using a certificate signed by an internal CA.

```
[Torc Client] ──HTTPS──> [Torc Server]
                 ^
            Custom CA cert
```

**Setup:**

```bash
# Server side
torc-server run --https --auth-file /etc/torc/htpasswd --require-auth

# Client side
torc --url https://torc.hpc.nrel.gov:8080/torc-service/v1 \
     --tls-ca-cert /path/to/internal-ca.pem \
     workflows list
```

### Pattern 3: Load Balancer with TLS Termination

```
[Torc Client] ──HTTPS──> [Load Balancer] ──HTTP──> [Torc Server]
                 ^
         Public or internal CA
```

If the load balancer uses a publicly trusted certificate, no `--tls-ca-cert` is needed. If it uses
an internal CA, configure the CA certificate as shown above.

### Pattern 4: Development with Self-Signed Certificates

```bash
# For local development only
torc --url https://localhost:8443/torc-service/v1 \
     --tls-insecure \
     workflows list
```

> **Warning:** Never use `--tls-insecure` in production. It disables all certificate verification,
> making connections vulnerable to man-in-the-middle attacks.

## HPC / Slurm Integration

When running workflows on Slurm clusters, TLS settings are automatically propagated to compute nodes
through environment variables. Torc's Slurm submission scripts export `TORC_TLS_CA_CERT` and
`TORC_TLS_INSECURE` so that job runners on compute nodes inherit the same TLS configuration.

**Setup:**

```bash
# Set TLS environment variables before submitting
export TORC_TLS_CA_CERT=/shared/certs/corporate-ca.pem
export TORC_API_URL=https://torc.hpc.nrel.gov:8080/torc-service/v1

# Submit workflow - compute nodes will inherit TLS settings
torc submit-slurm --account myproject workflow.yaml
```

**Requirements:**

- The CA certificate file must be accessible from all compute nodes (e.g., on a shared filesystem)
- Environment variables are exported in the generated Slurm submission scripts automatically

## Component-Specific Configuration

### MCP Server

```bash
torc-mcp-server \
  --url https://torc.hpc.nrel.gov:8080/torc-service/v1 \
  --tls-ca-cert /path/to/ca.pem
```

### Web Dashboard

```bash
torc-dash \
  --api-url https://torc.hpc.nrel.gov:8080/torc-service/v1 \
  --tls-ca-cert /path/to/ca.pem
```

### Terminal UI (TUI)

The TUI inherits TLS settings from the CLI flags:

```bash
torc --tls-ca-cert /path/to/ca.pem tui
```

### Programmatic Access (Rust)

```rust
use std::path::PathBuf;
use torc::client::apis::configuration::{Configuration, TlsConfig};

let tls = TlsConfig {
    ca_cert_path: Some(PathBuf::from("/path/to/ca.pem")),
    insecure: false,
};
let mut config = Configuration::with_tls(tls);
config.base_path = "https://torc.hpc.nrel.gov:8080/torc-service/v1".to_string();
```

## Troubleshooting

### Certificate Errors

**Error:** `certificate verify failed` or `unable to get local issuer certificate`

The client cannot verify the server's certificate chain. This typically means the server uses a
certificate signed by a CA that the client does not trust.

**Solution:** Provide the CA certificate:

```bash
torc --tls-ca-cert /path/to/ca.pem workflows list
```

### Finding the CA Certificate

Ask your IT department for the CA certificate file, or extract it from the system trust store:

```bash
# On RHEL/CentOS/Fedora
ls /etc/pki/tls/certs/

# On Ubuntu/Debian
ls /etc/ssl/certs/

# On macOS, export from Keychain Access or use:
security find-certificate -a -p /System/Library/Keychains/SystemRootCertificates.keychain \
  > /tmp/system-ca.pem
```

### Self-Signed Certificate in Development

If you are testing with a self-signed certificate and cannot provide the CA:

```bash
# Quick workaround for development only
torc --tls-insecure workflows list
```

### Connection Refused with HTTPS URL

Ensure the server is actually listening on HTTPS. If the server runs plain HTTP behind a reverse
proxy, verify the proxy is configured and the URL is correct.

### TLS Settings Not Applied in Slurm Jobs

Verify that:

1. The environment variables were set **before** submitting the workflow
2. The CA certificate file path is accessible from compute nodes
3. Check the generated submission script for the exported variables:

```bash
# The submission script should contain lines like:
export TORC_TLS_CA_CERT="/shared/certs/corporate-ca.pem"
```

## Reference

### CLI Flags

| Flag                   | Environment Variable | Config File           | Description                     |
| ---------------------- | -------------------- | --------------------- | ------------------------------- |
| `--tls-ca-cert <PATH>` | `TORC_TLS_CA_CERT`   | `client.tls.ca_cert`  | PEM-encoded CA certificate path |
| `--tls-insecure`       | `TORC_TLS_INSECURE`  | `client.tls.insecure` | Skip certificate verification   |

### Certificate Requirements

- **Format:** PEM-encoded (Base64 ASCII, begins with `-----BEGIN CERTIFICATE-----`)
- **Type:** CA certificate (not the server's leaf certificate)
- **TLS versions:** Uses system OpenSSL/native-tls (TLS 1.2 minimum recommended)

## See Also

- [Authentication](./authentication.md) - Setting up user authentication
- [Security Reference](./security.md) - Security best practices and threat model
- [Server Deployment](./server-deployment.md) - Deploying the Torc server
- [Configuration Reference](../../core/reference/configuration.md) - All configuration options
