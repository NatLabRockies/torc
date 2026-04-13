#!/bin/bash
set -euo pipefail

# Pin both version (for readability) and digest (for reproducibility).
# To update: docker pull openapitools/openapi-generator-cli:NEW_VERSION
#            docker inspect --format='{{index .RepoDigests 0}}' openapitools/openapi-generator-cli:NEW_VERSION
OPENAPI_CLI_VERSION="${OPENAPI_CLI_VERSION:-v7.16.0}"
OPENAPI_CLI_DIGEST="sha256:e56372add5e038753fb91aa1bbb470724ef58382fdfc35082bf1b3e079ce353c"
CONTAINER_EXEC="${CONTAINER_EXEC:-docker}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
SPEC_PATH="${SCRIPT_DIR}/openapi.yaml"
TEMPLATE_DIR="${SCRIPT_DIR}/openapi-generator-templates/rust"

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

if [[ ! -d "${TEMPLATE_DIR}" ]]; then
  echo "Rust client template directory not found: ${TEMPLATE_DIR}" >&2
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
  -v "${TEMPLATE_DIR}":/templates \
  "docker.io/openapitools/openapi-generator-cli:${OPENAPI_CLI_VERSION}@${OPENAPI_CLI_DIGEST}" \
  generate -g rust \
  --input-spec="/spec/${SPEC_FILE}" \
  -o /rust_client \
  -t /templates \
  --additional-properties=supportAsync=false

find "${REPO_ROOT}/src/client/apis" \
  -maxdepth 1 \
  -name '*_api.rs' \
  ! -name 'ro_crate_api.rs' \
  -delete
cp "${TMP_RUST_CLIENT}/src/apis/"*_api.rs "${REPO_ROOT}/src/client/apis/"

cargo fmt --manifest-path "${REPO_ROOT}/Cargo.toml" -- "${REPO_ROOT}/src/client/apis/"*_api.rs
