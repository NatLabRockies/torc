# Timeout Auto-Recovery Test

This test verifies that `torc watch --recover` correctly detects timeout failures and automatically
increases runtime allocations for retry.

## Workflow Description

- **job_fast**: Runs for 1 minute, completes successfully
- **job_slow**: Runs for 10 minutes, but only has 5 minute runtime specified
- **Slurm walltime**: 8 minutes

## Expected Behavior

1. Both jobs start
2. `job_fast` completes in ~1 minute
3. `job_slow` gets killed by Slurm at ~8 minutes (walltime exceeded)
4. Watcher detects timeout and increases runtime:
   - 5 min -> 7.5 min (1.5x) - still not enough
   - 7.5 min -> 11.25 min (1.5x) - enough for 10 min job
5. Scheduler regenerates with appropriate walltime
6. `job_slow` retries and completes successfully

## Test Procedure

### 1. Update the account in workflow.yaml

Replace `PLACEHOLDER_ACCOUNT` with your actual Slurm account:

```bash
sed -i 's/PLACEHOLDER_ACCOUNT/your_account/g' workflow.yaml
```

### 2. Submit the workflow

```bash
torc submit workflow.yaml
```

Note the workflow ID from the output.

### 3. Run the watcher with auto-recover

```bash
torc watch <workflow_id> --recover --max-retries 3
```

### 4. Expected output

You should see output similar to:

```
Watching workflow <id> (poll interval: 60s, auto-recover enabled, max retries: 3)
...
Workflow completed with failures:
  - Failed: 1
  - Completed: 1

Attempting automatic recovery (attempt 1/3)

Diagnosing failures...
Applying recovery heuristics...
  Job 2 (job_slow): Timeout detected, increasing runtime PT5M -> PT7M30S
  Applied fixes: 0 OOM, 1 timeout

Resetting failed jobs...
Regenerating Slurm schedulers and submitting...

Recovery initiated. Resuming monitoring...
...
```

The cycle may repeat once more until runtime reaches ~11 minutes.

## Timing Details

| Component                | Duration      |
| ------------------------ | ------------- |
| job_fast actual runtime  | 1 minute      |
| job_slow actual runtime  | 10 minutes    |
| Initial resource runtime | 5 minutes     |
| Initial Slurm walltime   | 8 minutes     |
| First recovery runtime   | 7.5 minutes   |
| Second recovery runtime  | 11.25 minutes |

## Files

- `workflow.yaml` - The workflow specification
- `README.md` - This file

## Verification

After the test completes successfully:

```bash
# Check workflow status
torc workflows status <workflow_id>

# Verify all jobs completed
torc jobs list <workflow_id>

# Check the final resource requirements (should show ~11 min runtime)
torc jobs list-resource-requirements <workflow_id>

# Check resource utilization report
torc reports check-resource-utilization <workflow_id>
```

## Adjusting the Test

To make the test faster:

1. Use a higher runtime multiplier:
   ```bash
   torc watch <workflow_id> --recover --runtime-multiplier 2.5
   ```
   With 2.5x: 5min -> 12.5min (success in 1 retry)

2. Reduce the sleep time in job_slow (edit workflow.yaml): Change `sleep 60` to `sleep 30` for a
   5-minute job instead of 10.
