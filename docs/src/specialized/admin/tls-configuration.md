# TLS/HTTPS Configuration

When the Torc server uses HTTPS — either directly or behind a reverse proxy with an internal CA —
clients need to know which CA certificate to trust. This page shows how to configure that.

## Quick Setup

Most HPC users only need to do this once.

### Step 1: Get the CA Certificate

Ask your system administrator for the PEM-encoded CA certificate file used by the Torc server. It
typically lives on a shared filesystem, e.g., `/shared/certs/corporate-ca.pem`.

### Step 2: Generate a Config File

```bash
torc config init --user
```

This creates `~/.config/torc/config.toml` with all available settings and defaults.

### Step 3: Edit the Config File

Open `~/.config/torc/config.toml` and set the server URL and CA certificate path:

```toml
[client]
api_url = "https://torc.hpc.nrel.gov:8080/torc-service/v1"

[client.tls]
ca_cert = "/shared/certs/corporate-ca.pem"
```

### Step 4: Verify

```bash
torc workflows list
```

If the connection succeeds, you're done. All Torc components — CLI, TUI, job runners, MCP server,
and dashboard — use these settings automatically.

## Slurm / HPC

When you submit a workflow with `torc slurm generate` + `torc submit`, TLS settings from your config
file (or CLI flags) are automatically propagated as CLI flags on the `torc-slurm-job-runner` command
that runs on compute nodes. No extra environment variable setup is needed.

The only requirement is that the CA certificate file must be on a **shared filesystem** accessible
from all compute nodes.

```bash
# Your config file handles TLS — just submit as normal
torc slurm generate --account myproject workflow.yaml && torc submit workflow.yaml
```

## Advanced Configuration

### CLI Flags

You can override config file settings on any command:

```bash
torc --url https://torc.hpc.nrel.gov:8080/torc-service/v1 \
     --tls-ca-cert /path/to/ca.pem \
     workflows list
```

### Environment Variables

```bash
export TORC_API_URL=https://torc.hpc.nrel.gov:8080/torc-service/v1
export TORC_TLS_CA_CERT=/path/to/ca.pem

torc workflows list
```

### Priority Order

Settings are resolved from highest to lowest priority:

1. CLI flags (`--tls-ca-cert`, `--tls-insecure`)
2. Environment variables (`TORC_TLS_CA_CERT`, `TORC_TLS_INSECURE`)
3. Project config (`./torc.toml`)
4. User config (`~/.config/torc/config.toml`)
5. System config (`/etc/torc/config.toml`)
6. Built-in defaults

### Insecure Mode (Development Only)

For local development with self-signed certificates, you can skip verification:

```bash
torc --url https://localhost:8443/torc-service/v1 \
     --tls-insecure \
     workflows list
```

> **Warning:** Never use `--tls-insecure` in production. It disables all certificate verification,
> making connections vulnerable to man-in-the-middle attacks.

### Programmatic Access (Rust)

```rust
use std::path::PathBuf;
use torc::client::apis::configuration::{Configuration, TlsConfig};

let tls = TlsConfig {
    ca_cert_path: Some(PathBuf::from("/path/to/ca.pem")),
    insecure: false,
};
let mut config = Configuration::with_tls(tls);
config.base_path =
    "https://torc.hpc.nrel.gov:8080/torc-service/v1".to_string();
```

## Troubleshooting

### "certificate verify failed" or "unable to get local issuer certificate"

The client cannot verify the server's certificate chain. Provide the CA certificate:

```bash
torc --tls-ca-cert /path/to/ca.pem workflows list
```

If you don't have the CA certificate, ask your system administrator. Common locations:

```bash
# RHEL / CentOS / Fedora
ls /etc/pki/tls/certs/

# Ubuntu / Debian
ls /etc/ssl/certs/
```

### Connection Refused with HTTPS URL

Verify the server is actually listening on HTTPS. If the server runs HTTP behind a reverse proxy,
check that the proxy is configured and the URL is correct.

### TLS Not Working on Compute Nodes

1. Confirm the CA certificate path is on a shared filesystem visible to compute nodes
2. Check that your config file or CLI flags are set before submitting the workflow

## Reference

| CLI Flag               | Environment Variable | Config Key            | Description                     |
| ---------------------- | -------------------- | --------------------- | ------------------------------- |
| `--tls-ca-cert <PATH>` | `TORC_TLS_CA_CERT`   | `client.tls.ca_cert`  | PEM-encoded CA certificate path |
| `--tls-insecure`       | `TORC_TLS_INSECURE`  | `client.tls.insecure` | Skip certificate verification   |
| `--url <URL>`          | `TORC_API_URL`       | `client.api_url`      | Torc server URL                 |

### Certificate Requirements

- **Format:** PEM-encoded (Base64 ASCII, begins with `-----BEGIN CERTIFICATE-----`)
- **Type:** CA certificate (not the server's leaf certificate)

## See Also

- [Authentication](./authentication.md) — Setting up user authentication
- [Security Reference](./security.md) — Security best practices and threat model
- [Server Deployment](./server-deployment.md) — Deploying the Torc server
- [Configuration Reference](../../core/reference/configuration.md) — All configuration options
