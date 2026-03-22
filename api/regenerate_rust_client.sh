#!/bin/bash
set -euo pipefail

OPENAPI_CLI_VERSION="${OPENAPI_CLI_VERSION:-v7.16.0}"
CONTAINER_EXEC="${CONTAINER_EXEC:-docker}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
SPEC_PATH="${SCRIPT_DIR}/openapi.yaml"
PATCH_PATH="${SCRIPT_DIR}/rust_client.patch"

while [[ $# -gt 0 ]]; do
  case "$1" in
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
      exit 1
      ;;
  esac
done

if [[ ! -f "${SPEC_PATH}" ]]; then
  echo "OpenAPI spec not found: ${SPEC_PATH}" >&2
  exit 1
fi

if [[ ! -f "${PATCH_PATH}" ]]; then
  echo "Rust client patch overlay not found: ${PATCH_PATH}" >&2
  exit 1
fi

SPEC_PATH="$(cd "$(dirname "${SPEC_PATH}")" && pwd)/$(basename "${SPEC_PATH}")"
SPEC_DIR="$(dirname "${SPEC_PATH}")"
SPEC_FILE="$(basename "${SPEC_PATH}")"

docker_run() {
  case "${OSTYPE:-}" in
    msys*|cygwin*)
      MSYS_NO_PATHCONV=1 "${CONTAINER_EXEC}" "$@"
      ;;
    *)
      "${CONTAINER_EXEC}" "$@"
      ;;
  esac
}

TMP_RUST_CLIENT="$(mktemp -d "${TMPDIR:-/tmp}/torc-rust-client.XXXXXX")"
TMP_STAGE="$(mktemp -d "${TMPDIR:-/tmp}/torc-rust-client-stage.XXXXXX")"
trap 'rm -rf "${TMP_RUST_CLIENT}" "${TMP_STAGE}"' EXIT

docker_run run \
  -v "${SPEC_DIR}":/spec \
  -v "${TMP_RUST_CLIENT}":/rust_client \
  "docker.io/openapitools/openapi-generator-cli:${OPENAPI_CLI_VERSION}" \
  generate -g rust \
  --input-spec="/spec/${SPEC_FILE}" \
  -o /rust_client \
  --additional-properties=supportAsync=false

mkdir -p "${TMP_STAGE}/apis"
cp "${TMP_RUST_CLIENT}/src/apis/"*_api.rs "${TMP_STAGE}/apis/"

(cd "${TMP_STAGE}" && git apply "${PATCH_PATH}")

rm -f "${REPO_ROOT}/src/client/apis/"*_api.rs
cp "${TMP_STAGE}/apis/"*_api.rs "${REPO_ROOT}/src/client/apis/"
