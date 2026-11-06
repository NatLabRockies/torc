#!/bin/bash
#SBATCH --job-name=torc-bench
#SBATCH --nodes=100
#SBATCH --time=01:00:00
#SBATCH --exclusive
#SBATCH --output=torc-bench-%j.out
# Add --account / --partition on the sbatch command line.
#
# Torc throughput benchmark. Runs every (server, shape) combination in one allocation:
#   server: regular   -> your existing torc-server (TORC_API_URL / TORC_PASSWORD from env)
#           ephemeral -> torc-server -d :memory: started on this batch node
#   shape:  flat      -> N_JOBS independent no-op jobs
#           stages    -> STAGES stages of N_JOBS/STAGES jobs, separated by barrier jobs
#
# Jobs run on every node, including the batch node that hosts the ephemeral server (the jobs
# are no-ops, so the server is not starved). One `torc run` per worker node.
#
# Knobs (env vars, export before sbatch):
#   SERVERS="regular ephemeral"  SHAPES="flat stages"  N_JOBS=100000  STAGES=5
#   JOB_CMD=true  JOBS_PER_NODE=$SLURM_CPUS_ON_NODE  POLL=1  BACKOFF_MAX=2
#   SERVER_THREADS=16  UNBLOCK_INTERVAL=30  SCENARIO_MINUTES=12  WORKER_NODES=<all>
#   TORC_BIN / TORC_SERVER_BIN (default: from PATH)
#
# Extra sweeps, run on the ephemeral server with the flat shape (set to "" to skip):
#   SCALE_NODES="10 50"   SCALE_N_JOBS=20000      worker-node scaling (the main run is the top point)
#   SLEEPS="1 10"         SLEEP_N_JOBS=50000      job duration: JOB_CMD="sleep <s>"
#   FAST_UNBLOCK_INTERVAL=1                       stages again on a server restarted with this
#                                                 --completion-check-interval-secs
#
# Results: one CSV line per scenario in $RUN_DIR/results.csv.

set -uo pipefail

SERVERS=${SERVERS:-"regular ephemeral"}
SHAPES=${SHAPES:-"flat stages"}
N_JOBS=${N_JOBS:-50000}
STAGES=${STAGES:-5}
JOB_CMD=${JOB_CMD:-true}
JOBS_PER_NODE=${JOBS_PER_NODE:-${SLURM_CPUS_ON_NODE:-104}}
POLL=${POLL:-1}
BACKOFF_MAX=${BACKOFF_MAX:-2}
SERVER_THREADS=${SERVER_THREADS:-16}
UNBLOCK_INTERVAL=${UNBLOCK_INTERVAL:-30}  # match the regular server
SCENARIO_MINUTES=${SCENARIO_MINUTES:-6}
WORKER_NODES=${WORKER_NODES:-$SLURM_NNODES}
TORC_BIN=${TORC_BIN:-$(command -v torc)}
TORC_SERVER_BIN=${TORC_SERVER_BIN:-$(command -v torc-server)}
RUN_DIR=${RUN_DIR:-$PWD/torc-bench-$SLURM_JOB_ID}
HEAD=${HEAD:-$(hostname -s)}  # must resolve from worker nodes
SCALE_NODES=${SCALE_NODES-"10 50"}
SCALE_N_JOBS=${SCALE_N_JOBS:-20000}
SLEEPS=${SLEEPS-"1 10"}
SLEEP_N_JOBS=${SLEEP_N_JOBS:-50000}
FAST_UNBLOCK_INTERVAL=${FAST_UNBLOCK_INTERVAL-1}
mkdir -p "$RUN_DIR"
REGULAR_URL=${TORC_API_URL:-}
REGULAR_PASSWORD=${TORC_PASSWORD:-}

now() { date +%s.%N; }

write_spec() { # shape -> spec path
  local shape=$1 spec="$RUN_DIR/$1.yaml"
  cat >"$spec" <<EOF
name: bench_${shape}_${SLURM_JOB_ID}
execution_config:
  mode: direct
  stdio:
    mode: none
jobs:
EOF
  if [ "$shape" = flat ]; then
    printf '  - name: "j{i}"\n    command: "%s"\n    parameters: {i: "1:%d"}\n' \
      "$JOB_CMD" "$N_JOBS" >>"$spec"
  else
    local per=$((N_JOBS / STAGES)) s
    for s in $(seq 1 "$STAGES"); do
      printf '  - name: "s%d_{i}"\n    command: "%s"\n    parameters: {i: "1:%d"}\n' \
        "$s" "$JOB_CMD" "$per" >>"$spec"
      [ "$s" -gt 1 ] && printf '    depends_on: ["barrier%d"]\n' $((s - 1)) >>"$spec"
      if [ "$s" -lt "$STAGES" ]; then
        printf '  - name: barrier%d\n    command: "true"\n    depends_on_regexes: ["^s%d_"]\n' \
          "$s" "$s" >>"$spec"
      fi
    done
  fi
  echo "$spec"
}

start_ephemeral() {
  local port=8765
  "$TORC_SERVER_BIN" run --host 0.0.0.0 -p "$port" -d :memory: -t "$SERVER_THREADS" \
    --completion-check-interval-secs "$UNBLOCK_INTERVAL" \
    >"$RUN_DIR/ephemeral-server-unblock$UNBLOCK_INTERVAL.log" 2>&1 &
  SERVER_PID=$!
  export TORC_API_URL="http://${HEAD}:${port}/torc-service/v1"
  unset TORC_PASSWORD
  for _ in $(seq 60); do "$TORC_BIN" ping >/dev/null 2>&1 && return 0; sleep 1; done
  echo "ephemeral server failed to start"; stop_server; return 1
}

stop_server() {
  [ -n "$SERVER_PID" ] && kill "$SERVER_PID" && wait "$SERVER_PID" 2>/dev/null
  SERVER_PID=""
}

run_scenario() { # server shape [tag_suffix]
  local server=$1 shape=$2 spec wf t0 t1 t2 t3 counts
  local tag="${server}_${shape}${3:+_$3}"
  echo "=== $tag: $N_JOBS jobs, $WORKER_NODES worker nodes x $JOBS_PER_NODE slots ==="
  spec=$(write_spec "$shape")

  t0=$(now)
  wf=$("$TORC_BIN" -f json create "$spec" | jq -r '.id // .workflow_id')
  t1=$(now)
  "$TORC_BIN" workflows init "$wf" >/dev/null || return 1
  t2=$(now)

  srun --nodes="$WORKER_NODES" --ntasks-per-node=1 \
    --time="$SCENARIO_MINUTES" --kill-on-bad-exit=0 \
    "$TORC_BIN" run "$wf" --max-parallel-jobs "$JOBS_PER_NODE" -p "$POLL" \
    --claim-backoff-max-secs "$BACKOFF_MAX" --persistent \
    -o "$RUN_DIR/$tag/output" >"$RUN_DIR/$tag.runners.log" 2>&1
  t3=$(now)

  counts=$("$TORC_BIN" -f json status "$wf" 2>/dev/null | tee "$RUN_DIR/$tag.status.json" |
    jq -r '[.jobs_by_status.completed, .jobs_by_status.failed, .walltime_seconds] | @csv')
  awk -v tag="$tag" -v wf="$wf" -v n="$N_JOBS" -v c="$counts" -v w="$WORKER_NODES" \
    -v t0="$t0" -v t1="$t1" -v t2="$t2" -v t3="$t3" 'BEGIN {
      run = t3 - t2
      split(c, a, ",")
      printf "%s,%s,%d,%d,%s,%s,%.1f,%.1f,%.1f,%.0f,%.1f,%.0f\n", tag, wf, n, w, a[1], a[2],
        t1 - t0, t2 - t1, run, a[1] / run, a[3], (a[3] > 0 ? a[1] / a[3] : 0)
    }' | tee -a "$RUN_DIR/results.csv"
}

echo "scenario,workflow_id,n_jobs,worker_nodes,completed,failed,create_s,init_s,run_s,jobs_per_s,server_walltime_s,server_jobs_per_s" \
  >"$RUN_DIR/results.csv"
for server in $SERVERS; do
  SERVER_PID=""
  if [ "$server" = ephemeral ]; then
    start_ephemeral || continue
  else
    [ -n "$REGULAR_URL" ] || { echo "skip regular: TORC_API_URL not set"; continue; }
    export TORC_API_URL=$REGULAR_URL
    [ -n "$REGULAR_PASSWORD" ] && export TORC_PASSWORD=$REGULAR_PASSWORD
  fi
  for shape in $SHAPES; do run_scenario "$server" "$shape"; done
  if [ "$server" = ephemeral ]; then
    for n in $SCALE_NODES; do
      N_JOBS=$SCALE_N_JOBS WORKER_NODES=$n run_scenario "$server" flat "nodes$n"
    done
    for sec in $SLEEPS; do
      N_JOBS=$SLEEP_N_JOBS JOB_CMD="sleep $sec" run_scenario "$server" flat "sleep$sec"
    done
    if [ -n "$FAST_UNBLOCK_INTERVAL" ]; then
      stop_server
      UNBLOCK_INTERVAL=$FAST_UNBLOCK_INTERVAL start_ephemeral &&
        run_scenario "$server" stages "unblock$FAST_UNBLOCK_INTERVAL"
    fi
  fi
  stop_server
done

column -s, -t "$RUN_DIR/results.csv"
