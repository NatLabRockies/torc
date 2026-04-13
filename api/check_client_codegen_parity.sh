#!/bin/bash
set -euo pipefail

# Verify that the checked-in Python and Julia API clients match what
# openapi-generator would produce from the current api/openapi.yaml.
# Exits non-zero and prints a diff when drift is detected.

OPENAPI_CLI_VERSION="${OPENAPI_CLI_VERSION:-v7.16.0}"
OPENAPI_CLI_DIGEST="sha256:e56372add5e038753fb91aa1bbb470724ef58382fdfc35082bf1b3e079ce353c"
CONTAINER_EXEC="${CONTAINER_EXEC:-docker}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
SPEC_PATH="${SCRIPT_DIR}/openapi.yaml"
API_VERSION="$(
  awk -F'"' '/pub const HTTP_API_VERSION:/ { print $2; exit }' "${REPO_ROOT}/src/api_version.rs"
)"

if [[ -z "${API_VERSION}" ]]; then
  echo "Failed to read HTTP API version from src/api_version.rs" >&2
  exit 1
fi

SPEC_DIR="$(cd "$(dirname "${SPEC_PATH}")" && pwd)"
SPEC_FILE="$(basename "${SPEC_PATH}")"

TMP_PYTHON="$(mktemp -d "${TMPDIR:-/tmp}/torc-py-check.XXXXXX")"
TMP_JULIA="$(mktemp -d "${TMPDIR:-/tmp}/torc-jl-check.XXXXXX")"

# shellcheck disable=SC2329,SC2317  # invoked indirectly via trap
cleanup_tmp() {
  # Docker may create root-owned files that the CI runner cannot delete directly.
  # Use a container to remove them, then clean up the (now-empty) temp dirs.
  for d in "${TMP_PYTHON}" "${TMP_JULIA}"; do
    "${CONTAINER_EXEC}" run --rm -v "${d}":/tmp_clean alpine rm -rf /tmp_clean/* 2>/dev/null || true
    rm -rf "${d}" 2>/dev/null || true
  done
}
trap cleanup_tmp EXIT

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

echo "Generating Python client from ${SPEC_FILE}…"
docker_run run \
  -v "${SCRIPT_DIR}":/data \
  -v "${SPEC_DIR}":/spec \
  -v "${TMP_PYTHON}":/python_client \
  "docker.io/openapitools/openapi-generator-cli:${OPENAPI_CLI_VERSION}@${OPENAPI_CLI_DIGEST}" \
  generate -g python --input-spec="/spec/${SPEC_FILE}" -o /python_client -c /data/config.json \
  --additional-properties=packageVersion="${API_VERSION}"

echo "Generating Julia client from ${SPEC_FILE}…"
docker_run run \
  -v "${SCRIPT_DIR}":/data \
  -v "${SPEC_DIR}":/spec \
  -v "${TMP_JULIA}":/julia_client \
  "docker.io/openapitools/openapi-generator-cli:${OPENAPI_CLI_VERSION}@${OPENAPI_CLI_DIGEST}" \
  generate -g julia-client --input-spec="/spec/${SPEC_FILE}" -o /julia_client \
  --additional-properties=packageVersion="${API_VERSION}"

RC=0

echo "Checking Python client parity…"
if ! diff -rq "${TMP_PYTHON}/torc/openapi_client" \
              "${REPO_ROOT}/python_client/src/torc/openapi_client" >/dev/null 2>&1; then
  echo "Python client is out of date with ${SPEC_FILE}" >&2
  diff -ru "${REPO_ROOT}/python_client/src/torc/openapi_client" \
           "${TMP_PYTHON}/torc/openapi_client" || true
  RC=1
else
  echo "Python client is up to date."
fi

echo "Checking Julia client parity…"
JULIA_DRIFT=0
if ! diff -rq "${TMP_JULIA}/src" \
              "${REPO_ROOT}/julia_client/Torc/src/api" >/dev/null 2>&1; then
  echo "Julia client API sources are out of date with ${SPEC_FILE}" >&2
  diff -ru "${REPO_ROOT}/julia_client/Torc/src/api" \
           "${TMP_JULIA}/src" || true
  JULIA_DRIFT=1
fi
if ! diff -rq "${TMP_JULIA}/docs" \
              "${REPO_ROOT}/julia_client/julia_client/docs" >/dev/null 2>&1; then
  echo "Julia client docs are out of date with ${SPEC_FILE}" >&2
  diff -ru "${REPO_ROOT}/julia_client/julia_client/docs" \
           "${TMP_JULIA}/docs" || true
  JULIA_DRIFT=1
fi
if ! diff -q "${TMP_JULIA}/README.md" \
             "${REPO_ROOT}/julia_client/julia_client/README.md" >/dev/null 2>&1; then
  echo "Julia client README is out of date with ${SPEC_FILE}" >&2
  diff -u "${REPO_ROOT}/julia_client/julia_client/README.md" \
          "${TMP_JULIA}/README.md" || true
  JULIA_DRIFT=1
fi

if [[ "${JULIA_DRIFT}" -eq 1 ]]; then
  RC=1
else
  echo "Julia client is up to date."
fi

if [[ "${RC}" -ne 0 ]]; then
  echo "" >&2
  echo "Run 'bash api/sync_openapi.sh clients' to regenerate." >&2
fi

exit "${RC}"
