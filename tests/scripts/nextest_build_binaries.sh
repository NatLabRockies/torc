#!/usr/bin/env bash
# nextest setup script: build the binaries that integration tests spawn, once per run.
# Tests skip their own `cargo build` when TORC_TEST_BINARIES_BUILT is set.
set -euo pipefail
cargo build --workspace --bins --features server-bin,slurm-runner
echo "TORC_TEST_BINARIES_BUILT=1" >> "$NEXTEST_ENV"
