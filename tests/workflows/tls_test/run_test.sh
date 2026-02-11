#!/usr/bin/env bash
#
# Manual test for client-side TLS (--tls-ca-cert and --tls-insecure).
#
# Sets up:
#   1. A proper CA + server certificate (with macOS-compatible extensions)
#   2. A Python HTTPS reverse proxy on port 8443 → torc-server on port 8080
#
# Prerequisites:
#   - openssl CLI
#   - python3
#   - torc-server running on port 8080 (plain HTTP)
#
# Usage:
#   # Start torc-server in another terminal:
#   torc-server run
#
#   # Run this script:
#   cd tests/workflows/tls_test
#   bash run_test.sh

set -euo pipefail

CERT_DIR="$(mktemp -d)"
PROXY_PID=""
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SERVER_PORT="${TORC_SERVER_PORT:-8080}"
PROXY_PORT="${TORC_PROXY_PORT:-8443}"

cleanup() {
    if [ -n "$PROXY_PID" ]; then
        echo "Stopping reverse proxy (PID $PROXY_PID)..."
        kill "$PROXY_PID" 2>/dev/null || true
        wait "$PROXY_PID" 2>/dev/null || true
    fi
    echo "Cleaning up certificates in $CERT_DIR"
    rm -rf "$CERT_DIR"
}
trap cleanup EXIT

# -- Check prerequisites -----------------------------------------------------

check_command() {
    if ! command -v "$1" &>/dev/null; then
        echo "ERROR: $1 is required but not found on PATH"
        exit 1
    fi
}

check_command openssl
check_command python3
check_command torc

echo "Checking torc-server on port $SERVER_PORT..."
if ! curl -sf "http://localhost:$SERVER_PORT/torc-service/v1/workflows?limit=1" >/dev/null 2>&1; then
    echo "ERROR: torc-server not reachable on port $SERVER_PORT"
    echo "Start it first:  torc-server run"
    exit 1
fi
echo "  torc-server is running."

# -- Generate certificates ----------------------------------------------------

echo ""
echo "Generating certificates in $CERT_DIR..."

cat > "$CERT_DIR/ca.cnf" << 'EOF'
[req]
distinguished_name = req_dn
x509_extensions = v3_ca
prompt = no

[req_dn]
CN = Torc Test CA

[v3_ca]
basicConstraints = critical,CA:TRUE
keyUsage = critical,keyCertSign,cRLSign
subjectKeyIdentifier = hash
EOF

cat > "$CERT_DIR/ext.cnf" << 'EOF'
subjectAltName=DNS:localhost,IP:127.0.0.1
extendedKeyUsage=serverAuth
basicConstraints=CA:FALSE
EOF

# CA key + cert
openssl req -x509 -newkey rsa:2048 \
    -keyout "$CERT_DIR/ca-key.pem" \
    -out "$CERT_DIR/ca-cert.pem" \
    -days 1 -nodes -config "$CERT_DIR/ca.cnf" 2>/dev/null

# Server key + CSR + signed cert
openssl genrsa -out "$CERT_DIR/server-key.pem" 2048 2>/dev/null
openssl req -new -key "$CERT_DIR/server-key.pem" \
    -out "$CERT_DIR/server.csr" -subj "/CN=localhost" 2>/dev/null
openssl x509 -req -in "$CERT_DIR/server.csr" \
    -CA "$CERT_DIR/ca-cert.pem" -CAkey "$CERT_DIR/ca-key.pem" \
    -CAcreateserial -out "$CERT_DIR/server-cert.pem" \
    -days 1 -extfile "$CERT_DIR/ext.cnf" 2>/dev/null

echo "  CA cert:     $CERT_DIR/ca-cert.pem"
echo "  Server cert: $CERT_DIR/server-cert.pem"

# -- Start HTTPS reverse proxy -----------------------------------------------

# Check if the proxy port is already in use
if lsof -i ":$PROXY_PORT" >/dev/null 2>&1; then
    echo "ERROR: Port $PROXY_PORT is already in use. Kill the existing process first:"
    echo "  lsof -i :$PROXY_PORT"
    echo "Or set a different port:  TORC_PROXY_PORT=9443 bash run_test.sh"
    exit 1
fi

echo ""
echo "Starting HTTPS reverse proxy on port $PROXY_PORT -> localhost:$SERVER_PORT..."

python3 "$SCRIPT_DIR/https_proxy.py" \
    "$CERT_DIR/server-cert.pem" \
    "$CERT_DIR/server-key.pem" \
    "$PROXY_PORT" \
    "$SERVER_PORT" &
PROXY_PID=$!

# Wait for proxy to be ready
for i in $(seq 1 30); do
    if curl -sk "https://localhost:$PROXY_PORT/" >/dev/null 2>&1; then
        echo "  Proxy is ready."
        break
    fi
    if ! kill -0 "$PROXY_PID" 2>/dev/null; then
        echo "ERROR: Proxy process died. Check for port conflicts or cert errors."
        exit 1
    fi
    if [ "$i" -eq 30 ]; then
        echo "ERROR: Proxy did not start within 3 seconds"
        exit 1
    fi
    sleep 0.1
done

# -- Run tests ----------------------------------------------------------------

HTTPS_URL="https://localhost:$PROXY_PORT/torc-service/v1"
PASS=0
FAIL=0

run_test() {
    local name="$1"
    shift
    echo -n "  $name... "
    if output=$("$@" 2>&1); then
        echo "PASS"
        PASS=$((PASS + 1))
    else
        echo "FAIL"
        echo "    $output" | head -5
        FAIL=$((FAIL + 1))
    fi
}

run_test_expect_fail() {
    local name="$1"
    shift
    echo -n "  $name... "
    if output=$("$@" 2>&1); then
        echo "FAIL (expected failure but got success)"
        FAIL=$((FAIL + 1))
    else
        echo "PASS"
        PASS=$((PASS + 1))
    fi
}

echo ""
echo "Running TLS tests against $HTTPS_URL"
echo ""

# Test 1: --tls-insecure should work
run_test \
    "tls-insecure connects" \
    torc --url "$HTTPS_URL" --tls-insecure workflows list --limit 1

# Test 2: --tls-ca-cert with correct CA should work
run_test \
    "tls-ca-cert with correct CA connects" \
    torc --url "$HTTPS_URL" --tls-ca-cert "$CERT_DIR/ca-cert.pem" workflows list --limit 1

# Test 3: no TLS flags should fail (self-signed cert not trusted)
run_test_expect_fail \
    "no TLS flags rejects self-signed" \
    torc --url "$HTTPS_URL" workflows list --limit 1

# Test 4: wrong CA cert should fail
run_test_expect_fail \
    "wrong CA cert rejects server" \
    torc --url "$HTTPS_URL" --tls-ca-cert "$CERT_DIR/ca-key.pem" workflows list --limit 1

# Test 5: env vars work
run_test \
    "TORC_TLS_CA_CERT env var works" \
    env TORC_TLS_CA_CERT="$CERT_DIR/ca-cert.pem" \
    torc --url "$HTTPS_URL" workflows list --limit 1

# Test 6: TORC_TLS_INSECURE env var works
run_test \
    "TORC_TLS_INSECURE env var works" \
    env TORC_TLS_INSECURE=true \
    torc --url "$HTTPS_URL" workflows list --limit 1

# -- Summary ------------------------------------------------------------------

echo ""
echo "Results: $PASS passed, $FAIL failed (out of $((PASS + FAIL)))"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
