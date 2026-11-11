# Throughput Benchmark Results (November 2026)

This page records what we learned by running the benchmarks in this directory on a 100-node,
CPU-only allocation. It is meant for power users who want to know how far torc scales, what limits
it, and which settings matter. See [README.md](README.md) for how the benchmarks work.

## Setup

- 100 nodes, 112 job slots per node (Slurm's CPU count; the runner sees 224 hardware threads), so
  **11,200 concurrent jobs**.
- torc-server 0.41.0, torc client 1.0.1.
- Jobs are `sleep 5` unless noted, with `execution_config.mode: direct` and `stdio.mode: none`, so
  the numbers measure torc overhead, not Slurm step launch or filesystem writes.
- One `torc run` per node, `-p 1 --claim-backoff-max-secs 2`.
- **regular**: a shared, persistent torc-server whose completion-check interval is 30 s.
- **ephemeral**: `torc-server run -d :memory: -t 16` started on the first node of the allocation.
- **flat**: independent jobs. **stages**: 5 equal stages separated by barrier jobs, so 4 stage
  transitions.

"Ideal" below is the time the jobs would take with zero torc overhead: number of waves of 11,200
jobs times the job duration. "Efficiency" is ideal time divided by measured time. All times are the
server-reported workflow walltime (first job start to last job end).

## Runs

| Run                            | Script                       | What changed                   |
| ------------------------------ | ---------------------------- | ------------------------------ |
| 1                              | `throughput.sbatch`          | baseline, 50,000 jobs          |
| 2                              | `throughput.sbatch`          | baseline, 100,000 jobs         |
| 3                              | `throughput.sbatch`          | ephemeral only, 1 s unblock    |
| 4 (pending at time of writing) | `throughput_followup.sbatch` | job duration, backoff, threads |

Run 3 used `SERVERS=ephemeral UNBLOCK_INTERVAL=1 FAST_UNBLOCK_INTERVAL=`: the regular server removed
and the completion-check interval lowered from 30 s to 1 s. `throughput_followup.sbatch` runs
several sweeps with those settings back to back in one allocation; see its header.

Every run completed every job with no failures and no server 5xx errors.

## Results

### Flat workflows

| Jobs    | Server                 | Time    | Jobs/s | Ideal | Efficiency |
| ------- | ---------------------- | ------- | ------ | ----- | ---------- |
| 50,000  | regular                | 69.5 s  | 720    | 25 s  | 36%        |
| 50,000  | ephemeral              | 52.8 s  | 947    | 25 s  | 47%        |
| 100,000 | regular                | 168.6 s | 593    | 45 s  | 27%        |
| 100,000 | ephemeral              | 127.8 s | 782    | 45 s  | 35%        |
| 100,000 | ephemeral, 1 s unblock | 131.9 s | 758    | 45 s  | 34%        |

### Staged workflows

| Jobs    | Server                  | Time    | Jobs/s | Ideal |
| ------- | ----------------------- | ------- | ------ | ----- |
| 50,000  | regular (30 s unblock)  | 260.0 s | 192    | 25 s  |
| 50,000  | ephemeral, 30 s unblock | 247.1 s | 202    | 25 s  |
| 50,000  | ephemeral, 1 s unblock  | 62.3 s  | 803    | 25 s  |
| 100,000 | regular (30 s unblock)  | 397.4 s | 252    | 50 s  |
| 100,000 | ephemeral, 30 s unblock | 306.2 s | 327    | 50 s  |
| 100,000 | ephemeral, 1 s unblock  | 155.5 s | 643    | 50 s  |
| 100,000 | ephemeral, 1 s unblock  | 150.4 s | 665    | 50 s  |

### Node count and job duration (ephemeral, flat)

| Nodes | Jobs   | Job        | Time   | Jobs/s | Ideal | Efficiency |
| ----- | ------ | ---------- | ------ | ------ | ----- | ---------- |
| 10    | 20,000 | `sleep 5`  | 94.5 s | 212    | 90 s  | 95%        |
| 50    | 20,000 | `sleep 5`  | 27.3 s | 733    | 20 s  | 73%        |
| 100   | 50,000 | `sleep 5`  | 52.8 s | 947    | 25 s  | 47%        |
| 100   | 50,000 | `sleep 10` | 76.5 s | 652    | 50 s  | 65%        |

These sweeps ran three times with identical settings and agreed within 1%, so differences of a few
percent elsewhere may be noise. Runs 2 and 3 used the same flat 100,000-job settings apart from the
unblock interval.

## What we learned

### The server tops out around 750–950 completed jobs per second

Throughput is near ideal at 10 nodes, starts to fall away by 50, and plateaus at 100. Longer jobs
use the nodes better because the server has fewer completions to handle per second. The rough rule
from these numbers: **the server is the bottleneck when the cluster finishes more jobs per second
than the server can handle**, that is when

```
nodes × jobs per node ÷ job duration (s)  >  about 800
```

At 11,200 slots, that means jobs shorter than about 15 s. Typical workflows with jobs of minutes to
hours are nowhere near this limit: 11,200 one-hour jobs finish about 3 per second.

### The bottleneck is SQLite's single write lock

The ephemeral server logs a `slow statement` warning when a `BEGIN IMMEDIATE` (the start of a write
transaction) waits more than 1 s for the lock. In the 100,000-job runs it logged 240–290 of these,
median wait about 1.6 s and maximum 5–6 s, while handling 6,800–7,900 requests per second at peak.
The 50,000-job runs logged 70–80, with a maximum of 2.8 s. Every runner claims, starts and completes
jobs through write transactions on one database, so they queue on that lock.

### The in-memory ephemeral server is 25–30% faster than a shared persistent server

The difference covers storage (memory vs. disk), network distance, authentication and any other
users on the shared server; these runs do not separate them. For very large workflows of short jobs,
running an ephemeral server inside the allocation is worth considering.

### The completion-check interval is a cost per dependency level

When a job finishes, the server does not release its dependents immediately. A background task does
that on a timer (`--completion-check-interval-secs`), handling every completion since the last tick
in one batch. Each stage transition in the staged workflow waits for up to two ticks: the last stage
job's completion releases the barrier, and the barrier's completion releases the next stage.

- At 30 s, the 4 transitions added roughly 180–200 s of idle nodes; staged runs took 2.4–4.7× as
  long as flat runs.
- At 1 s, the staged 100,000-job run took 150–156 s, within about 15% of the flat run.
- Lowering the interval to 1 s did not measurably slow the flat workflow (127.8 s vs. 131.9 s, a 3%
  difference from one run each).

Why 30 s is still a reasonable default: torc's typical workload is jobs that run for an hour or
more. Then a 30 s delay is under 1% of a dependency level, and completions are rare enough that the
interval has no effect on server load either way. The interval matters only for workflows with
**dependencies and short jobs**. Our data showed no meaningful cost to a 1 s interval, so the server
now releases dependents on demand instead (see "Possible improvements").

### Other observations

- Workflow creation and initialization are fast: 2–5 s for 100,000 jobs.
- Runners on some scenarios logged harmless `422 RO-Crate entity already exists` warnings when
  several runners registered the same software entry at once.
- `torc admin api-stats` requires an admin login on a server with authentication, so we have no
  request statistics for the regular server.

## Guidance for power users

- **Jobs of a minute or longer** will not hit the server limit at this scale. Below about 15 s per
  job at 10,000+ concurrent slots, expect the server to cap throughput; combine small tasks into
  larger jobs if you can.
- **Short jobs with dependencies**: servers released before the on-demand change wait up to two
  `--completion-check-interval-secs` ticks per dependency level; lower it on a server you control.
  Newer servers release dependents within about 1 s and ignore the flag.
- **Very large short-job workflows**: an ephemeral `torc-server -d :memory:` on the first node of
  the allocation was the fastest option measured. Its data is lost when it stops unless you snapshot
  it (`SIGUSR1`).

## Possible improvements (not started)

- **Measure who holds the write lock.** The current warnings show waiting time, not which request
  types hold the lock and for how long. Timing in the transaction helper would show where time goes.
- **Batch job starts (done after these runs).** The runner used to report each job start in its own
  request (`start_job`), one write transaction per job. Claims now mark jobs running in the claim
  transaction, and the runner fetches each resource requirement once per claim instead of once per
  job. These runs predate the change.
- **Spread out runner requests.** All runners poll on similar schedules; random jitter would smooth
  bursts. The backoff sweeps in run 4 test part of this.
- **Release dependents on demand (done after these runs).** A completion now wakes the unblock step
  directly, and passes are at least 1 s apart so completions that arrive close together are batched.
  This removes the per-level delay without a tuning knob, and `--completion-check-interval-secs` is
  deprecated and ignored. These runs predate the change; the 1 s rows above approximate it.
- **PostgreSQL.** It would allow concurrent writers (`FOR UPDATE SKIP LOCKED` is the standard
  job-queue claim pattern), which targets the measured bottleneck. It would also mean porting
  SQLite-specific SQL and migrations, slower individual transactions over the network, and losing
  the zero-setup in-memory server that suits HPC allocations. SQLite can sustain far more small
  write transactions per second than we used here, which suggests that reducing write transactions
  per job should come first.
