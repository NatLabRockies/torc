#!/bin/bash
#SBATCH --job-name=torc-bench-followup
#SBATCH --nodes=100
#SBATCH --time=01:00:00
#SBATCH --exclusive
#SBATCH --output=torc-bench-followup-%j.out
# Add --account / --partition on the sbatch command line.
#
# Follow-up sweeps on the ephemeral server (1 s unblock interval), each a separate call of
# throughput.sbatch (fresh server per sweep) in its own subdirectory of $OUT:
#   sleeps     flat 100k sleep 5 baseline, then 50k jobs of sleep 30 and sleep 60
#   backoff5   flat 100k, --claim-backoff-max-secs 5
#   backoff10  flat 100k, --claim-backoff-max-secs 10
#   threads4   flat 100k, server -t 4
#   slots224   flat 50k sleep 10, 224 jobs per node (compare to sleep10 at 112: ~651 jobs/s)
#
# Slurm runs a spooled copy of this file, so it finds the benchmark script via BENCH_SCRIPT
# (default: throughput.sbatch in the directory you submit from).

set -uo pipefail

BENCH_SCRIPT=${BENCH_SCRIPT:-$SLURM_SUBMIT_DIR/throughput.sbatch}
OUT=${OUT:-$PWD/torc-bench-followup-$SLURM_JOB_ID}
[ -f "$BENCH_SCRIPT" ] || { echo "BENCH_SCRIPT not found: $BENCH_SCRIPT"; exit 1; }
mkdir -p "$OUT"

# Shared by every sweep: ephemeral server with a 1 s unblock interval, flat shape only, no
# node-scaling, sleep, or fast-unblock sweeps unless set below.
export SERVERS=ephemeral UNBLOCK_INTERVAL=1 FAST_UNBLOCK_INTERVAL= SHAPES=flat N_JOBS=100000 SCALE_NODES= SLEEPS=

sweep() { # name, then VAR=value overrides
  local name=$1; shift
  echo "##### $name: $*"
  env "$@" RUN_DIR="$OUT/$name" bash "$BENCH_SCRIPT"
}

sweep sleeps SLEEPS="30 60" SLEEP_N_JOBS=50000
sweep backoff5 BACKOFF_MAX=5
sweep backoff10 BACKOFF_MAX=10
sweep threads4 SERVER_THREADS=4
sweep slots224 N_JOBS=50000 JOB_CMD="sleep 10" JOBS_PER_NODE=224

# One combined table: sweep name prepended to each scenario row.
{
  echo "sweep,$(head -1 "$OUT/sleeps/results.csv")"
  for f in "$OUT"/*/results.csv; do
    tail -n +2 "$f" | sed "s|^|$(basename "$(dirname "$f")"),|"
  done
} >"$OUT/results.csv"
column -s, -t "$OUT/results.csv"
