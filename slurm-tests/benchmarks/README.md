# Torc Throughput Benchmarks

`throughput.sbatch` measures how fast torc can dispatch short jobs across a large Slurm allocation.
It is sized for one 1-hour, 100-node, CPU-only allocation and runs every scenario sequentially
inside that allocation.

## Layout

- The batch node (first node of the allocation) hosts the ephemeral server and also runs jobs. The
  jobs only sleep, so they leave the server plenty of CPU and memory.
- Jobs run on all nodes in every scenario so results are comparable across servers.
- Each worker node runs one `torc run <workflow_id>` with `--max-parallel-jobs` equal to its CPU
  count.
- Jobs use `execution_config.mode: direct` (no per-job `srun`) and `stdio.mode: none`, so the
  benchmark measures torc overhead rather than Slurm step launch or shared-filesystem writes.

## Scenarios

| Scenario                    | Server                                           | Shape                                      | Default jobs | Worker nodes |
| --------------------------- | ------------------------------------------------ | ------------------------------------------ | ------------ | ------------ |
| `regular_flat`              | regular                                          | independent `sleep 5` jobs                 | 50,000       | all          |
| `regular_stages`            | regular                                          | 5 stages separated by barrier jobs         | 50,000       | all          |
| `ephemeral_flat`            | ephemeral                                        | independent `sleep 5` jobs                 | 50,000       | all          |
| `ephemeral_stages`          | ephemeral                                        | 5 stages separated by barrier jobs         | 50,000       | all          |
| `ephemeral_flat_nodes10`    | ephemeral                                        | independent `sleep 5` jobs (node scaling)  | 20,000       | 10           |
| `ephemeral_flat_nodes50`    | ephemeral                                        | independent `sleep 5` jobs (node scaling)  | 20,000       | 50           |
| `ephemeral_flat_sleep10`    | ephemeral                                        | independent `sleep 10` jobs (job duration) | 50,000       | all          |
| `ephemeral_flat_sleep30`    | ephemeral                                        | independent `sleep 30` jobs (job duration) | 50,000       | all          |
| `ephemeral_stages_unblock1` | ephemeral, restarted with a 1 s unblock interval | 5 stages separated by barrier jobs         | 50,000       | all          |

- **regular**: your existing torc-server, taken from `TORC_API_URL` (and `TORC_PASSWORD` if set).
- **ephemeral**: `torc-server run -d :memory:` started on the batch node, with no authentication.
- **stages**: each stage's jobs depend on a single barrier job that depends on the previous stage
  via `depends_on_regexes`, keeping dependency count linear in the number of jobs.

What each group answers:

- **Regular vs. ephemeral**: the cost of a shared, persistent server compared with an in-memory
  server next to the workers.
- **Flat vs. stages**: the cost of stage transitions (unblocking, then idle runners reclaiming
  work).
- **Node scaling**: whether throughput keeps rising with more nodes. If it flattens, the server is
  the bottleneck.
- **Job duration**: how short jobs can be before torc becomes the bottleneck. With about 100 × 104
  slots, ideal run time is about 25 s for the default `sleep 5`, 50 s for `sleep 10` and 150 s for
  `sleep 30`; the gap between ideal and measured is torc overhead.
- **Unblock interval** (`ephemeral_stages` vs. `ephemeral_stages_unblock1`): how much of the stage
  cost is the `--completion-check-interval-secs` timer, and whether releasing dependents every
  second (frequent short write locks) slows job claims and completions compared with the production
  interval (rare long ones).

## Running

Do a small trial run first to check hostnames, binaries, and workflow-creation time:

```bash
N_JOBS=2000 SCALE_N_JOBS=1000 SLEEP_N_JOBS=1000 \
  sbatch -N 3 -t 15 -A <account> -p debug slurm-tests/benchmarks/throughput.sbatch
```

Full run:

```bash
export TORC_API_URL=http://<regular-server>:8080/torc-service/v1
sbatch -A <account> slurm-tests/benchmarks/throughput.sbatch
```

`torc` and `torc-server` must be on `PATH` (or set `TORC_BIN` / `TORC_SERVER_BIN`). `jq` is
required.

## Settings

All are optional environment variables, exported before `sbatch`.

| Variable                | Default                 | Meaning                                                                                   |
| ----------------------- | ----------------------- | ----------------------------------------------------------------------------------------- |
| `SERVERS`               | `regular ephemeral`     | Servers to test                                                                           |
| `SHAPES`                | `flat stages`           | Shapes for the main scenarios                                                             |
| `N_JOBS`                | `50000`                 | Jobs per main scenario                                                                    |
| `STAGES`                | `5`                     | Number of stages in the `stages` shape                                                    |
| `JOB_CMD`               | `sleep 5`               | Command each job runs in the main scenarios                                               |
| `SCALE_NODES`           | `10 50`                 | Worker-node counts for the scaling sweep (`""` skips it)                                  |
| `SCALE_N_JOBS`          | `20000`                 | Jobs per scaling scenario                                                                 |
| `SLEEPS`                | `10 30`                 | Sleep seconds for the duration sweep (`""` skips it)                                      |
| `SLEEP_N_JOBS`          | `50000`                 | Jobs per duration scenario                                                                |
| `JOBS_PER_NODE`         | `$SLURM_CPUS_ON_NODE`   | `--max-parallel-jobs` per worker node                                                     |
| `WORKER_NODES`          | `$SLURM_NNODES`         | Worker nodes for the main scenarios                                                       |
| `POLL`                  | `1`                     | Runner poll interval (seconds)                                                            |
| `BACKOFF_MAX`           | `2`                     | Runner idle backoff cap (seconds)                                                         |
| `SERVER_THREADS`        | `16`                    | Ephemeral server worker threads                                                           |
| `UNBLOCK_INTERVAL`      | `30`                    | Ephemeral server `--completion-check-interval-secs`; set it to the regular server's value |
| `FAST_UNBLOCK_INTERVAL` | `1`                     | Interval for the `stages` comparison run (`""` skips it)                                  |
| `SCENARIO_MINUTES`      | `6`                     | Per-scenario `srun --time` cap                                                            |
| `HEAD`                  | `hostname -s`           | Batch-node hostname as seen from worker nodes                                             |
| `RUN_DIR`               | `./torc-bench-<job_id>` | Output directory                                                                          |

With the defaults there are 9 scenarios, so the worst case is 54 minutes plus workflow-creation
time. Most scenarios finish well before the cap.

## Results

`$RUN_DIR/results.csv` has one row per scenario, and the table is printed at the end of the Slurm
output file.

| Column              | Meaning                                                         |
| ------------------- | --------------------------------------------------------------- |
| `create_s`          | Time for `torc create` (spec expansion and bulk insert)         |
| `init_s`            | Time for `torc workflows init`                                  |
| `run_s`             | Wall time of the `srun` step, including runner startup and exit |
| `jobs_per_s`        | `completed / run_s`                                             |
| `server_walltime_s` | Server-reported workflow walltime (first job start to last end) |
| `server_jobs_per_s` | `completed / server_walltime_s`; the cleaner throughput number  |
| `completed`         | Completed jobs, including barrier jobs in the `stages` shape    |
| `failed`            | Failed jobs; anything non-zero means the scenario is not valid  |

Per-scenario files in `$RUN_DIR`: the generated spec, `<scenario>.runners.log` (all runner output),
`<scenario>.status.json`, and `ephemeral-server-unblock<interval>.log`.

## Caveats

- Run the regular server with your production `--completion-check-interval-secs` and set
  `UNBLOCK_INTERVAL` to the same value, so regular vs. ephemeral differs only in the server and its
  storage. Dependents are released only on this timer (a completion does not wake it early). Each of
  the 4 stage boundaries in the `stages` shape waits for up to two ticks (stage → barrier → next
  stage), so at 30 s expect a few minutes of idle time in the `stages` runs. That is real cost, not
  noise.
- The regular and ephemeral servers differ in more than storage: the regular server likely requires
  authentication and may run with different thread counts.
- If a scenario hits `SCENARIO_MINUTES`, its `completed` count will be below the job count and its
  throughput numbers reflect a partial run.
- In-memory data is lost when the ephemeral server stops. Send it `SIGUSR1` during a run to snapshot
  the database to disk if you need to inspect it later.
