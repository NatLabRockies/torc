# Usage:
#   # Download and extract release binaries
#   VERSION=0.17.0
#   mkdir -p artifact
#   curl -fsSL "https://github.com/NatLabRockies/torc/releases/download/v${VERSION}/torc-x86_64-unknown-linux-musl.tar.gz" \
#     | tar xz -C artifact/
#
#   # Build
#   docker build --build-arg VERSION=$VERSION -t ghcr.io/natlabrockies/torc:$VERSION .
#
#   # Run with required env vars
#   docker run -d -p 8080:8080 \
#     -e TORC_AUTH_FILE=/data/htpasswd \
#     -e TORC_ADMIN_USERS=admin \
#     -e TORC_THREADS=4 \
#     -v ./htpasswd:/data/htpasswd:ro \
#     -v torc-data:/data \
#     ghcr.io/natlabrockies/torc:$VERSION
#
#   # Run torc CLI
#   docker run --rm ghcr.io/natlabrockies/torc:$VERSION torc --version

FROM alpine:3.23

ARG VERSION
RUN test -n "$VERSION" || (echo "ERROR: VERSION build arg is required" >&2 && exit 1)

LABEL org.opencontainers.image.title="torc" \
      org.opencontainers.image.description="Distributed workflow orchestration system" \
      org.opencontainers.image.url="https://github.com/NatLabRockies/torc" \
      org.opencontainers.image.source="https://github.com/NatLabRockies/torc" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.licenses="BSD-3-Clause"

RUN apk add --no-cache ca-certificates sqlite

# Copy pre-extracted release binaries from the build context
COPY artifact/torc artifact/torc-server artifact/torc-slurm-job-runner \
     artifact/torc-dash artifact/torc-htpasswd artifact/torc-mcp-server \
     /usr/local/bin/

# Create data directory with OpenShift-compatible permissions.
# OpenShift runs containers as arbitrary non-root UIDs but always in group 0 (root).
RUN mkdir -p /data && chgrp -R 0 /data && chmod -R g=u /data

VOLUME /data

COPY docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

EXPOSE 8080

USER 1001:0

ENTRYPOINT ["docker-entrypoint.sh"]
CMD ["torc-server", "run"]
