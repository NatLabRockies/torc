# Slurm Exit Codes

Torc sets per-step walltimes via `srun --time`, which produces deterministic exit codes that you can
inspect with `torc results list` and `torc slurm sacct`.

## Exit Code Reference

| Scenario                     | Exit Code | Slurm State     | Torc Status | Description                             |
| ---------------------------- | --------- | --------------- | ----------- | --------------------------------------- |
| **Out of memory**            | 137       | `OUT_OF_MEMORY` | failed      | Exceeded `--mem` cgroup limit (SIGKILL) |
| **Timeout, SIGTERM handled** | 0         | `COMPLETED`     | completed   | Caught SIGTERM, saved state, exited     |
| **Timeout, SIGKILL**         | 152       | `TIMEOUT`       | terminated  | Did not exit before `--time` limit      |

### Out of Memory (exit code 137)

The job exceeded its `--mem` cgroup limit. Slurm's OOM killer sent SIGKILL (signal 9).
`137 = 128 + 9`.

```console
$ torc results list $WORKFLOW_ID
╭────┬──────────┬─────────┬─────────────╮
│ ID │ Job Name │ Status  │ Return Code │
├────┼──────────┼─────────┼─────────────┤
│ 1  │ train    │ failed  │ 137         │
╰────┴──────────┴─────────┴─────────────╯

$ torc slurm sacct $WORKFLOW_ID
╭──────────────────────┬───────────────┬──────────╮
│ Step Name            │ State         │ MaxRSS   │
├──────────────────────┼───────────────┼──────────┤
│ wf1_j1_r1_a1         │ OUT_OF_MEMORY │ 4096000K │
╰──────────────────────┴───────────────┴──────────╯
```

**Fix:** Increase `memory` in resource requirements, or use
`torc workflows check-resources --correct` to auto-adjust based on peak usage.

### Timeout with Graceful Shutdown (exit code 0)

The job received SIGTERM via `srun --signal`, saved a checkpoint, and called `sys.exit(0)`. From
Slurm's perspective, the job completed normally.

```console
$ torc results list $WORKFLOW_ID
╭────┬──────────┬───────────┬─────────────╮
│ ID │ Job Name │ Status    │ Return Code │
├────┼──────────┼───────────┼─────────────┤
│ 1  │ simulate │ completed │ 0           │
╰────┴──────────┴───────────┴─────────────╯
```

This is the expected outcome when using `srun_termination_signal`. The job handled the signal
correctly but did not finish all its work. Reinitialize and re-submit to continue from the
checkpoint:

```bash
torc workflows reinit $WORKFLOW_ID
torc submit $WORKFLOW_ID
```

See the [Graceful Job Termination](../fault-tolerance/checkpointing.md) tutorial for a complete
example with a Python signal handler.

### Timeout without Handler (exit code 152)

The job did not exit before the step's `--time` limit. Slurm sent SIGTERM, waited `KillWait` seconds
(typically 30s, configured in `slurm.conf`), then sent SIGKILL. `152 = 128 + 24` (SIGXCPU).

```console
$ torc results list $WORKFLOW_ID
╭────┬──────────┬────────────┬─────────────╮
│ ID │ Job Name │ Status     │ Return Code │
├────┼──────────┼────────────┼─────────────┤
│ 1  │ train    │ terminated │ 152         │
╰────┴──────────┴────────────┴─────────────╯

$ torc slurm sacct $WORKFLOW_ID
╭──────────────────────┬─────────┬──────────╮
│ Step Name            │ State   │ MaxRSS   │
├──────────────────────┼─────────┼──────────┤
│ wf1_j1_r1_a1         │ TIMEOUT │ 2048000K │
╰──────────────────────┴─────────┴──────────╯
```

**Fix:**

- Add a SIGTERM handler using the
  [shutdown-flag pattern](../fault-tolerance/checkpointing.md#step-1-write-the-python-job)
- Set `srun_termination_signal` to give more lead time (e.g., `"TERM@300"` for 5 minutes)
- Increase the allocation walltime

## Why Torc Sets `--time`

Without `srun --time`, steps inherit the allocation's walltime. When the allocation expires, Slurm
cancels all steps with `State=CANCELLED`, which is ambiguous — it could mean the user canceled the
job, the admin preempted it, or time ran out.

By setting `--time` to the remaining allocation time (rounded down to whole minutes), Torc ensures
the step times out **before** the allocation expires. This produces the unambiguous `State=TIMEOUT`
with exit code 152, which Torc can distinguish from user-initiated cancellation.
