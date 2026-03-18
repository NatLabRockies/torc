#!/bin/bash
OPENAPI_CLI_VERSION=v7.16.0

set -x
set -e

if [ -z "${CONTAINER_EXEC}" ]; then
  CONTAINER_EXEC=docker
fi

if [ -z "${PYTHON_CLIENT}" ]; then
  PYTHON_CLIENT="$(pwd)/python_client"
fi
rm -rf "${PYTHON_CLIENT}"
mkdir "${PYTHON_CLIENT}"

if [ -z "${JULIA_CLIENT}" ]; then
  JULIA_CLIENT="$(pwd)/julia_client"
fi
rm -rf "${JULIA_CLIENT}"
mkdir "${JULIA_CLIENT}"

# MSYS_NO_PATHCONV=1 prevents Git Bash/MSYS2 on Windows from converting
# container-internal paths like /data/config.json to C:/Program Files/Git/...
MSYS_NO_PATHCONV=1 "${CONTAINER_EXEC}" run \
  -v "$(pwd)":/data \
  -v "${PYTHON_CLIENT}":/python_client \
  "docker.io/openapitools/openapi-generator-cli:${OPENAPI_CLI_VERSION}" \
  generate -g python --input-spec=/data/openapi.yaml -o /python_client -c /data/config.json

MSYS_NO_PATHCONV=1 "${CONTAINER_EXEC}" run \
  -v "$(pwd)":/data \
  -v "${JULIA_CLIENT}":/julia_client \
  "docker.io/openapitools/openapi-generator-cli:${OPENAPI_CLI_VERSION}" \
  generate -g julia-client --input-spec=/data/openapi.yaml -o /julia_client

rm -rf ../python_client/src/torc/openapi_client/*
rm -rf ../julia_client/Torc/src/api/*
rm -rf ../julia_client/julia_client/docs/*
rm -f ../julia_client/julia_client/README.md
cp -r "${PYTHON_CLIENT}/torc/openapi_client/"* ../python_client/src/torc/openapi_client/
cp -r "${JULIA_CLIENT}/src/"* ../julia_client/Torc/src/api/
cp -r "${JULIA_CLIENT}/docs/"* ../julia_client/julia_client/docs/
cp "${JULIA_CLIENT}/README.md" ../julia_client/julia_client/
rm -rf python_client julia_client
