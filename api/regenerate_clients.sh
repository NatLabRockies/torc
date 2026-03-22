#!/bin/bash
set -euo pipefail

OPENAPI_CLI_VERSION="${OPENAPI_CLI_VERSION:-v7.16.0}"
CONTAINER_EXEC="${CONTAINER_EXEC:-docker}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
SPEC_PATH="${SCRIPT_DIR}/openapi.yaml"

TMP_PYTHON_CLIENT="${TMP_PYTHON_CLIENT:-${SCRIPT_DIR}/python_client}"
TMP_JULIA_CLIENT="${TMP_JULIA_CLIENT:-${SCRIPT_DIR}/julia_client}"

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

SPEC_PATH="$(cd "$(dirname "${SPEC_PATH}")" && pwd)/$(basename "${SPEC_PATH}")"
SPEC_DIR="$(dirname "${SPEC_PATH}")"
SPEC_FILE="$(basename "${SPEC_PATH}")"

rm -rf "${TMP_PYTHON_CLIENT}" "${TMP_JULIA_CLIENT}"
mkdir -p "${TMP_PYTHON_CLIENT}" "${TMP_JULIA_CLIENT}"

"${CONTAINER_EXEC}" run \
  -v "${SCRIPT_DIR}":/data \
  -v "${SPEC_DIR}":/spec \
  -v "${TMP_PYTHON_CLIENT}":/python_client \
  "docker.io/openapitools/openapi-generator-cli:${OPENAPI_CLI_VERSION}" \
  generate -g python --input-spec="/spec/${SPEC_FILE}" -o /python_client -c /data/config.json

"${CONTAINER_EXEC}" run \
  -v "${SCRIPT_DIR}":/data \
  -v "${SPEC_DIR}":/spec \
  -v "${TMP_JULIA_CLIENT}":/julia_client \
  "docker.io/openapitools/openapi-generator-cli:${OPENAPI_CLI_VERSION}" \
  generate -g julia-client --input-spec="/spec/${SPEC_FILE}" -o /julia_client

rm -rf "${REPO_ROOT}/python_client/src/torc/openapi_client"/*
rm -rf "${REPO_ROOT}/julia_client/Torc/src/api"/*
rm -rf "${REPO_ROOT}/julia_client/julia_client/docs"/*
rm -f "${REPO_ROOT}/julia_client/julia_client/README.md"

cp -r "${TMP_PYTHON_CLIENT}/torc/openapi_client/"* "${REPO_ROOT}/python_client/src/torc/openapi_client/"
cp -r "${TMP_JULIA_CLIENT}/src/"* "${REPO_ROOT}/julia_client/Torc/src/api/"
cp -r "${TMP_JULIA_CLIENT}/docs/"* "${REPO_ROOT}/julia_client/julia_client/docs/"
cp "${TMP_JULIA_CLIENT}/README.md" "${REPO_ROOT}/julia_client/julia_client/"

rm -rf "${TMP_PYTHON_CLIENT}" "${TMP_JULIA_CLIENT}"
