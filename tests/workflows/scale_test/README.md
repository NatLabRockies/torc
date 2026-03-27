# Database Scale Test (100K Jobs)

Stress test for database scaling with **100,000 jobs**. All jobs are no-ops (`echo`), so the
bottleneck is purely database throughput: job creation, dependency resolution, job claiming, and
completion processing.

## Workflow Structure

- **4 rounds** with **25,000 jobs each** = **100,000 work jobs**
- 3 inter-stage barriers + 1 final barrier = **4 barrier jobs**
- **Total: 100,004 jobs**
- **~100,003 dependencies** (barrier pattern keeps this linear)

```text
Round 1 (25,000 jobs)  ──▶  barrier_r1  ──▶  Round 2 (25,000 jobs)  ──▶  barrier_r2
    ──▶  Round 3 (25,000 jobs)  ──▶  barrier_r3  ──▶  Round 4 (25,000 jobs)  ──▶  barrier_r4
```

Without the barrier pattern, the dependency count would be 25,000 x 25,000 = 625,000,000 per stage
transition. The barrier pattern reduces this to ~50,000 per transition (25,000 in + 25,000 out).

## What This Tests

1. **Workflow creation time** - Can the server create 100K jobs and resolve dependencies quickly?
2. **Job initialization** - Can the server build the dependency graph efficiently?
3. **Job claiming throughput** - Can multiple runners claim jobs fast enough?
4. **Completion processing** - Can the background unblocking thread keep up with 25K near-instant
   completions?
5. **Database lock contention** - Do SQLite write locks cause bottlenecks under heavy concurrent
   access?

## Running the Test

### Prerequisites

Start the server:

```bash
torc-server run
```

### Option A: Quick Start (Single Process)

```bash
torc run tests/workflows/scale_test/workflow.yaml --num-parallel-processes 16
```

### Option B: Explicit Workflow Management with Multiple Runners

For maximum throughput, start multiple independent job runner processes. This creates more database
contention (the point of the test) and better simulates a real multi-node deployment.

#### Step 1: Create the Workflow

```bash
torc workflows create tests/workflows/scale_test/workflow.yaml
```

Note the workflow ID from the output.

#### Step 2: Start 16 Job Runners

Each runner independently polls the server for ready jobs, claims them, executes them, and reports
results. Start 16 runners to simulate a compute node with 16 cores dedicated to job execution:

```bash
for i in $(seq 1 16); do
  torc run <workflow_id> &
done
```

Or use a more controlled approach with logging:

```bash
WORKFLOW_ID=<workflow_id>
mkdir -p output/runner_logs

for i in $(seq 1 16); do
  torc run "$WORKFLOW_ID" > "output/runner_logs/runner_${i}.log" 2>&1 &
  echo "Started runner $i (PID: $!)"
done

echo "All 16 runners started. Monitor with: torc tui"
```

#### Step 3: Monitor Progress

```bash
# Interactive TUI
torc tui

# Or check status periodically
watch -n 5 torc workflows status <workflow_id>
```

### Multi-Node Setup

To test with multiple compute nodes (e.g., 4 nodes x 16 runners = 64 concurrent runners), start 16
runners on each node. All runners connect to the same torc-server:

```bash
# On each compute node:
export TORC_API_URL="http://<server-host>:8080/torc-service/v1"
WORKFLOW_ID=<workflow_id>

for i in $(seq 1 16); do
  torc run "$WORKFLOW_ID" &
done
```

## Expected Behavior

### Success

- All 100,004 jobs complete successfully
- Transitions between rounds happen smoothly via barrier jobs
- No ERROR-level "database is locked" messages (DEBUG level retries are normal)

### Metrics to Watch

| Metric                 | What to look for                                   |
| ---------------------- | -------------------------------------------------- |
| Workflow creation time | How long to create 100K jobs + dependencies        |
| Round transition time  | Time from last job in round N to first job in N+1  |
| Total wall time        | End-to-end completion time                         |
| Server CPU usage       | Should not be pegged at 100% on SQLite lock waits  |
| Database size          | SQLite file size after workflow creation           |
| Runner idle time       | Time runners spend waiting for jobs between rounds |

### Potential Issues

- **Slow workflow creation**: If creating 100K jobs takes more than a few minutes, the batch
  insertion path may need optimization.
- **Stuck barriers**: If a barrier never completes, the background unblocking thread may have fallen
  behind. Check server logs for errors.
- **Runner starvation**: If runners can't claim jobs fast enough, the `claim_next_jobs` endpoint may
  be a bottleneck.
- **Database lock timeouts**: If you see ERROR-level "database is locked" messages, the SQLite busy
  timeout or retry strategy may need tuning.
