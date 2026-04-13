#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
OUTPUT_PATH="${1:-${SCRIPT_DIR}/openapi.codegen.yaml}"

cd "${REPO_ROOT}"

cargo run \
  --quiet \
  --no-default-features \
  --features server-bin \
  --bin torc-openapi \
  > "${OUTPUT_PATH}"
