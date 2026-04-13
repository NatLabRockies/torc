#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
TMP_SPEC="$(mktemp -t torc-openapi-check.XXXXXX)"
trap 'rm -f "${TMP_SPEC}"' EXIT

cd "${REPO_ROOT}"

"${SCRIPT_DIR}/emit_openapi_from_rust.sh" "${TMP_SPEC}"

if ! cmp -s "${TMP_SPEC}" "${SCRIPT_DIR}/openapi.codegen.yaml"; then
  echo "openapi.codegen.yaml is out of date with the Rust-emitted spec" >&2
  diff -u "${SCRIPT_DIR}/openapi.codegen.yaml" "${TMP_SPEC}" || true
  exit 1
fi

if ! cmp -s "${TMP_SPEC}" "${SCRIPT_DIR}/openapi.yaml"; then
  echo "openapi.yaml is out of date with the Rust-emitted spec" >&2
  diff -u "${SCRIPT_DIR}/openapi.yaml" "${TMP_SPEC}" || true
  exit 1
fi

cargo run \
  --quiet \
  --no-default-features \
  --features server-bin \
  --bin torc-openapi \
  -- compare "${SCRIPT_DIR}/openapi.yaml"
