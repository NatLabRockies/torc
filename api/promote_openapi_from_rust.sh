#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TMP_SPEC="$(mktemp "${TMPDIR:-/tmp}/torc-openapi-promote.XXXXXX.yaml")"
trap 'rm -f "${TMP_SPEC}"' EXIT

"${SCRIPT_DIR}/emit_openapi_from_rust.sh" "${TMP_SPEC}"
cp "${TMP_SPEC}" "${SCRIPT_DIR}/openapi.codegen.yaml"
cp "${TMP_SPEC}" "${SCRIPT_DIR}/openapi.yaml"

cat <<'EOF'
Promoted Rust-emitted OpenAPI spec.

Updated files:
  - api/openapi.codegen.yaml
  - api/openapi.yaml
EOF
