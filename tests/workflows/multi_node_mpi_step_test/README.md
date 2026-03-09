# Multi-Node MPI Step Test

Tests that torc correctly passes `--nodes=num_nodes` to `srun` when `num_nodes` is greater than 1,
creating a true multi-node Slurm job step. This exercises the `num_nodes` field and verifies sacct
captures the correct node list.

## Workflow Description

- **Allocation**: 2 nodes via `sbatch --nodes=2`, single torc worker
- **Job**: `mpi_job` with `num_nodes=2`
- **srun invocation**: `srun --nodes=2 --ntasks=1 --overlap bash run_mpi_step.sh`
- The job prints `SLURM_STEP_NODELIST` and `SLURM_STEP_NUM_NODES` to confirm it spans 2 nodes

## Test Procedure

### 1. Set the Slurm account

```bash
sed -i 's/PLACEHOLDER_ACCOUNT/your_account/g' \
  tests/workflows/multi_node_mpi_step_test/workflow.yaml
```

### 2. Submit the workflow (from repository root)

```bash
torc submit tests/workflows/multi_node_mpi_step_test/workflow.yaml
```

Note the workflow ID (e.g., `42`).

### 3. Monitor

```bash
torc watch 42 --auto-schedule
```

### 4. Inspect job logs

```bash
# Show job ID for mpi_job
torc jobs list 42

# Read stdout log — should show SLURM_STEP_NUM_NODES=2
torc jobs logs <job_id>
```

Look for output like:

```
SLURM_STEP_NODELIST: node001,node002
SLURM_STEP_NUM_NODES: 2
Node count visible to this step: 2
```

If `SLURM_STEP_NUM_NODES` is `1`, srun did not receive `--nodes=2`. Confirm that `num_nodes: 2` is
set in the resource requirements and rebuild torc.

### 5. Verify sacct data (node_list)

```bash
torc slurm stats 42
```

Expected output should include a row for the `mpi_job` step with:

- `node_list` containing **two nodes** (e.g., `node001,node002`)
- `max_rss_bytes` populated (non-null) if cgroup memory accounting is enabled

If `node_list` shows only one node, the step did not span two nodes. Check the srun flags by
inspecting the torc log from inside the allocation (`slurm-<jobid>.out`).

### 6. Verify execution results

```bash
torc reports results 42
```

Check that:

- `return_code = 0` (job succeeded)
- `peak_memory_bytes` is set (sysinfo monitoring was active)

## Key Validations

| Check                             | Command                        | Expected                        |
| --------------------------------- | ------------------------------ | ------------------------------- |
| Job log shows 2-node step         | `torc jobs logs <id>`          | `SLURM_STEP_NUM_NODES=2`        |
| sacct node_list has 2 nodes       | `torc slurm stats <wf_id>`     | Two node names in `node_list`   |
| Return code is 0                  | `torc reports results <wf_id>` | `return_code=0`                 |
| Slurm step name visible in squeue | `squeue --me --steps`          | `wf<id>_j<id>_r1_a1` during run |

## Troubleshooting

**`SLURM_STEP_NUM_NODES` shows 1**: `num_nodes` is not being passed to srun. Check
`src/client/async_cli_command.rs` for the `--nodes=num_nodes` argument.

**`node_list` is null in sacct output**: sacct may not have the step in the accounting database yet
(best-effort collection). Wait a few minutes and retry `torc slurm stats`.

**Workflow fails immediately**: The `debug` partition may not support 2-node allocations. Try a
different partition by editing `workflow.yaml`.
