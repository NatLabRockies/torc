#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  cat <<'EOF'
Usage:
  bash api/sync_openapi.sh emit
  bash api/sync_openapi.sh check
  bash api/sync_openapi.sh promote
  bash api/sync_openapi.sh clients [--use-rust-spec|--spec PATH]
  bash api/sync_openapi.sh all [--use-rust-spec|--promote]

Commands:
  emit      Emit api/openapi.codegen.yaml from Rust only.
  check     Emit a fresh Rust spec and verify checked-in specs are in sync.
  promote   Replace api/openapi.yaml with the Rust-emitted spec.
  clients   Regenerate Rust, Python, and Julia clients from a selected spec.
  all       Emit, verify, optionally promote, and regenerate all clients.

Options:
  --use-rust-spec  Generate clients from api/openapi.codegen.yaml.
  --promote        Promote the Rust spec before generating clients.
  --spec PATH      Generate clients from an explicit spec path.
EOF
}

if [[ $# -lt 1 ]]; then
  usage >&2
  exit 1
fi

COMMAND="$1"
shift

USE_RUST_SPEC=0
PROMOTE=0
SPEC_PATH=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --use-rust-spec)
      USE_RUST_SPEC=1
      shift
      ;;
    --promote)
      PROMOTE=1
      shift
      ;;
    --spec)
      if [[ $# -lt 2 ]]; then
        echo "--spec requires a path" >&2
        exit 1
      fi
      SPEC_PATH="$2"
      shift 2
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

case "${COMMAND}" in
  emit)
    "${SCRIPT_DIR}/emit_openapi_from_rust.sh"
    ;;
  check)
    "${SCRIPT_DIR}/check_openapi_codegen_parity.sh"
    ;;
  promote)
    "${SCRIPT_DIR}/promote_openapi_from_rust.sh"
    ;;
  clients)
    if [[ -n "${SPEC_PATH}" ]]; then
      "${SCRIPT_DIR}/regenerate_clients.sh" --spec "${SPEC_PATH}"
    elif [[ "${USE_RUST_SPEC}" -eq 1 ]]; then
      "${SCRIPT_DIR}/regenerate_clients.sh" --spec "${SCRIPT_DIR}/openapi.codegen.yaml"
    else
      "${SCRIPT_DIR}/regenerate_clients.sh" --spec "${SCRIPT_DIR}/openapi.yaml"
    fi
    ;;
  all)
    "${SCRIPT_DIR}/emit_openapi_from_rust.sh"
    "${SCRIPT_DIR}/check_openapi_codegen_parity.sh"

    if [[ "${PROMOTE}" -eq 1 ]]; then
      "${SCRIPT_DIR}/promote_openapi_from_rust.sh"
      "${SCRIPT_DIR}/regenerate_clients.sh" --spec "${SCRIPT_DIR}/openapi.yaml"
    elif [[ -n "${SPEC_PATH}" ]]; then
      "${SCRIPT_DIR}/regenerate_clients.sh" --spec "${SPEC_PATH}"
    elif [[ "${USE_RUST_SPEC}" -eq 1 ]]; then
      "${SCRIPT_DIR}/regenerate_clients.sh" --spec "${SCRIPT_DIR}/openapi.codegen.yaml"
    else
      "${SCRIPT_DIR}/regenerate_clients.sh" --spec "${SCRIPT_DIR}/openapi.yaml"
    fi
    ;;
  *)
    echo "Unknown command: ${COMMAND}" >&2
    usage >&2
    exit 1
    ;;
esac
