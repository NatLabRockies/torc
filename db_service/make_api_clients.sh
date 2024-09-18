OPENAPI_CLI_VERSION=latest

set -x
set -e

if [ -z ${CONTAINER_EXEC} ]; then
    CONTAINER_EXEC=docker
fi

if [ -z ${PYTHON_CLIENT} ]; then
    PYTHON_CLIENT=$(pwd)/python_client
fi
rm -rf ${PYTHON_CLIENT}
mkdir ${PYTHON_CLIENT}

if [ -z ${JULIA_CLIENT} ]; then
    JULIA_CLIENT=$(pwd)/julia_client
fi
rm -rf ${JULIA_CLIENT}
mkdir ${JULIA_CLIENT}

${CONTAINER_EXEC} run \
    -v $(pwd):/data \
    -v ${PYTHON_CLIENT}:/python_client \
    docker.io/openapitools/openapi-generator-cli:${OPENAPI_CLI_VERSION} \
    generate -g python --input-spec=/data/openapi.yaml -o /python_client -c /data/config.json

${CONTAINER_EXEC} run \
    -v $(pwd):/data \
    -v ${JULIA_CLIENT}:/julia_client \
    docker.io/openapitools/openapi-generator-cli:latest \
    generate -g julia-client --input-spec=/data/openapi.yaml -o /julia_client

rm -rf ../torc_package/torc/openapi_client
rm -rf ../julia/Torc/src/api
rm -rf ../julia/julia_client/docs
rm ../julia/julia_client/README.md
mv ${PYTHON_CLIENT}/torc/openapi_client ../torc_package/torc/openapi_client
mv ${JULIA_CLIENT}/src ../julia/Torc/src/api
mv ${JULIA_CLIENT}/docs ../julia/julia_client/
mv ${JULIA_CLIENT}/README.md ../julia/julia_client/
