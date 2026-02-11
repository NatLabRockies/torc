# TLS/HTTPS Manual Test

Tests client-side TLS features (`--tls-ca-cert` and `--tls-insecure`) using a Python HTTPS reverse
proxy in front of a plain HTTP torc-server.

## Prerequisites

- `openssl` CLI
- `python3`
- `torc` and `torc-server` built and on PATH

## Usage

```bash
# Terminal 1: start torc-server on HTTP
torc-server run

# Terminal 2: run the test
cd tests/workflows/tls_test
bash run_test.sh
```

## What It Tests

1. `--tls-insecure` connects to self-signed HTTPS server
2. `--tls-ca-cert` with correct CA cert connects
3. No TLS flags rejects self-signed cert (expected failure)
4. Wrong CA cert rejects server (expected failure)
5. `TORC_TLS_CA_CERT` environment variable works
6. `TORC_TLS_INSECURE` environment variable works

## Configuration

| Variable           | Default | Description                    |
| ------------------ | ------- | ------------------------------ |
| `TORC_SERVER_PORT` | 8080    | Port where torc-server listens |
| `TORC_PROXY_PORT`  | 8443    | Port for the HTTPS proxy       |

## Architecture

```
[torc client] --HTTPS:8443--> [Python reverse proxy] --HTTP:8080--> [torc-server]
                  ^
         Generated CA + server cert
       (with macOS-compatible extensions)
```

The script generates a proper CA certificate with `keyUsage=keyCertSign` (required by macOS
Security.framework) and a server certificate with `subjectAltName=DNS:localhost,IP:127.0.0.1`.
Certificates are created in a temp directory and cleaned up automatically.
