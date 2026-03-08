# Multi-Node Parallel Jobs Test

Tests that torc correctly starts workers across multiple nodes and that single-node jobs are
dispatched in parallel across both workers. Also verifies that sstat CPU metrics and sacct
accounting data are correctly captured for each srun step.

## Workflow Description

- **Allocation**: 2 nodes via `sbatch --nodes=2`
- **Workers**: 2 torc workers — one on each node
- **Jobs**: 6 independent single-node work jobs (`work_1` through `work_6`)
- **Expected concurrency**: 2 jobs running simultaneously (one per worker), completing in ~3 waves
- Each job prints its hostname and timestamps so parallelism can be confirmed in the logs

## Test Procedure

### 1. Set the Slurm account

```bash
sed -i 's/PLACEHOLDER_ACCOUNT/your_account/g' \
  tests/workflows/multi_node_parallel_jobs_test/workflow.yaml
```

### 2. Submit the workflow (from repository root)

```bash
torc submit tests/workflows/multi_node_parallel_jobs_test/workflow.yaml
```

Note the workflow ID (e.g., `42`).

### 3. Monitor

```bash
torc watch 42 --auto-schedule
```

Enable time-series monitoring if not already set in the workflow spec to capture CPU readings.

### 4. Verify parallelism from job logs

After the workflow completes:

```bash
torc jobs list 42
# Get job IDs for work_1..work_6, then for each:
torc jobs logs <job_id>
```

Each job log should show a **different hostname** for jobs that ran concurrently. Two jobs that
started at the same clock time confirm parallel dispatch.

For example:

```
=== Job 1 starting at 10:02:05 on node001 ===
=== Job 2 starting at 10:02:05 on node002 ===
=== Job 3 starting at 10:02:11 on node001 ===
...
```

If all jobs show the **same hostname** or strictly sequential start times, parallelism is broken.

### 5. Verify sacct per-step data

```bash
torc slurm stats 42
```

Expected output should have **6 rows** (one per job step) with:

- Each row's `node_list` showing a single node name (`node001` or `node002`)
- Jobs that ran on the same node in sequence should share the same node name
- `max_rss_bytes` populated if cgroup memory accounting is enabled on the cluster
- `ave_cpu_seconds` non-zero (the work script generates CPU load)

If all 6 rows are missing or show null values, sacct collection is not working. Check:

- `SLURM_JOB_ID` is set inside the allocation (run `echo $SLURM_JOB_ID` in a test job)
- The `sacct` binary is available on compute nodes (`which sacct`)
- Slurm accounting is enabled (`sacct -j <SLURM_JOB_ID>` returns data)

### 6. Verify sstat CPU metrics

If time-series resource monitoring was enabled, check that `peak_cpu_percent` is non-zero:

```bash
torc reports results 42
```

A `peak_cpu_percent` value near or above 100% (for 2-CPU jobs) confirms that sstat was polled
successfully during job execution. A value of exactly 0.0% means sstat returned no data — verify
that the `sstat` binary is available on compute nodes.

### 7. Verify step naming in squeue (during run)

While the workflow is running, check step visibility:

```bash
squeue --me --steps
```

You should see entries named like `wf42_j<id>_r1_a1` — one per running job. This confirms torc's
srun step naming is working and administrators can track individual jobs.

## Key Validations

| Check                        | Command                        | Expected                              |
| ---------------------------- | ------------------------------ | ------------------------------------- |
| Jobs ran on 2 distinct nodes | `torc jobs logs <id>` (all 6)  | Mix of `node001` and `node002`        |
| Concurrent start times       | Job logs                       | 2+ jobs start within 1s of each other |
| sacct has 6 rows             | `torc slurm stats <wf_id>`     | 6 step records                        |
| sacct node_list set          | `torc slurm stats <wf_id>`     | Non-null `node_list` per row          |
| peak_cpu_percent > 0         | `torc reports results <wf_id>` | Non-zero CPU for each job             |
| All jobs completed           | `torc jobs list <wf_id>`       | 6 jobs with status `completed`        |

## Troubleshooting

**All jobs show the same hostname**: Check that the Slurm allocation has multiple nodes and that
torc is configured to spawn workers on each node.

**`peak_cpu_percent` is 0.0% for all jobs**: sstat polling is not working. Possible causes:

- `sstat` binary not available on compute nodes
- Time-series monitoring not enabled in the workflow spec (add `resource_monitor.enabled: true`)

**sacct rows are empty/null**: sacct data collection may have failed. Check:

- `sacct -j $SLURM_JOB_ID` returns output from compute nodes
- The step names match the pattern `wf<id>_j<id>_r<id>_a<id>`

**Workflow total time >= 6 x job_time**: Jobs are running sequentially on a single worker. Confirm
the Slurm allocation requested 2 nodes.
