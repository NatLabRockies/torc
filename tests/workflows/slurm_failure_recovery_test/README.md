# Slurm Failure Recovery Test

This test verifies that `torc` correctly handles automatic job retry with failure handlers when
submitting workflows to Slurm.

## How It Works

Failure handlers are part of the workflow specification and are automatically applied by
**torc-slurm-job-runner** during job execution in the Slurm allocation. When a job fails with an
exit code that matches a handler rule, the torc-slurm-job-runner:

1. Detects the failure
2. Finds the matching failure handler rule
3. Automatically retries the job (without manual intervention)
4. Re-runs with a fresh attempt ID

This is different from `torc watch --recover`, which manually diagnoses and recovers from
already-failed jobs.

## Workflow Description

The workflow consists of 3 stages:

### 1. Preprocess Stage

- **Single job**: `preprocess` - Sets up the workflow environment
- Creates the working directory
- Runs quickly

### 2. Work Stage (Parallel)

- **5 work jobs** (`work_1` through `work_5`) running in parallel
- Each job requests 2 CPUs and 1GB memory
- All jobs depend on `preprocess` to complete
- **Key failure point**: `work_3` is configured to:
  - Fail on first attempt with exit code 42 (transient error)
  - Succeed on retry (second attempt)
- Other jobs (`work_1`, `work_2`, `work_4`, `work_5`) succeed normally
- All work jobs use the `retry_on_exit_42` failure handler

### 3. Postprocess Stage

- **Single job**: `postprocess` - Final aggregation
- Depends on all 5 work jobs to complete
- Runs after all work jobs (including retried `work_3`)

## Failure Handler Configuration

The workflow defines a `retry_on_exit_42` failure handler:

```yaml
failure_handlers:
  - name: retry_on_exit_42
    rules:
      # Retry jobs failing with exit code 42 up to 2 times
      - exit_codes: [42]
        max_retries: 2
```

This handler:

- Matches jobs that exit with code 42
- Automatically retries up to 2 times (without manual intervention)
- Does not require a recovery script (simple retry)
- All work jobs reference this handler via `failure_handler: retry_on_exit_42`

## Expected Behavior

### Timeline

1. **Workflow submitted to Slurm** → `torc submit-slurm` creates jobs and Slurm allocation
2. **torc-slurm-job-runner starts** inside the Slurm allocation
3. **Preprocess runs** and completes successfully
4. **5 work jobs start in parallel**:
   - `work_1`, `work_2`, `work_4`, `work_5` → complete successfully on first run
   - `work_3` → fails with exit code 42 (first attempt)
5. **torc-slurm-job-runner detects failure**:
   - Reads the job result (exit code 42)
   - Finds the matching failure handler rule
   - Retries the job automatically (no manual intervention needed)
6. **work_3 runs again** (second attempt):
   - `TORC_ATTEMPT_ID=2` is set in the environment
   - Script detects retry and succeeds
   - Job completes with exit code 0
7. **postprocess runs** once all 5 work jobs complete
8. **Workflow completes successfully**

### Job Status Progression for work_3

```
Initial submission to Slurm:
  uninitialized → ready → pending → scheduled_on_compute_node

First execution:
  running → failed (exit code 42)

Failure handler triggers retry:
  [retry_job API called to mark for retry]

Second execution:
  pending → running → completed (exit code 0, with TORC_ATTEMPT_ID=2)
```

## Test Procedure

### 1. Run from the repository root

The workflow must be run from the **repository root directory** (`/path/to/torc/`) because the
workflow references the script with a relative path:

```bash
cd /path/to/torc
```

### 2. Update the account in workflow.yaml

Replace `PLACEHOLDER_ACCOUNT` with your actual Slurm account:

```bash
sed -i 's/PLACEHOLDER_ACCOUNT/your_account/g' tests/workflows/slurm_failure_recovery_test/workflow.yaml
```

### 3. Submit the workflow

```bash
torc submit tests/workflows/slurm_failure_recovery_test/workflow.yaml
```

Note the workflow ID from the output (e.g., `56`).

### 4. Monitor the workflow with automatic recovery

```bash
torc watch 56 --auto-schedule
```

```
...
Workflow running...
work_3 completed with status: failed (exit code 42)
work_3 will be retried (failure handler matched exit code 42)
...
work_3 completed with status: completed (exit code 0)
All work jobs now complete
Postprocess: running...
Postprocess: completed
Workflow complete
```

### 5. Verify results

```bash
# Check overall workflow status
torc workflows status 56

# List all jobs and their statuses
torc jobs list 56

# Check work_3 specifically (should show 2 attempts, final status: completed)
torc jobs get <work_3_job_id>

# View work_3 logs to confirm it failed then succeeded
torc jobs logs <work_3_job_id> --attempt 1    # First failure (exit 42)
torc jobs logs <work_3_job_id> --attempt 2    # Successful retry (exit 0)
```

## Key Test Validations

✅ **Job Failure Detected**: `work_3` fails with exit code 42 in Slurm ✅ **Failure Handler
Applies**: torc-slurm-job-runner finds matching rule (exit code 42) ✅ **Automatic Retry**: Job is
retried without manual `torc watch --recover` intervention ✅ **Retry Succeeds**: Second attempt
completes successfully (exit code 0) ✅ **Dependency Resolution**: `postprocess` waits for all work
jobs including retried `work_3` ✅ **Workflow Completion**: Entire workflow completes successfully

## Implementation Details

### How work_3 Fails and Recovers

The `fail_then_succeed.sh` script:

```bash
#!/bin/bash
ATTEMPT=${TORC_ATTEMPT_ID:-1}

if [ "$ATTEMPT" -eq "1" ]; then
    echo "Work job 3: Simulating transient failure (exit code 42)"
    exit 42              # Fail on first attempt
else
    echo "Work job 3: Retry attempt $ATTEMPT - succeeding now"
    exit 0               # Succeed on retry
fi
```

Key aspects:

- Uses `TORC_ATTEMPT_ID` environment variable (set by torc-slurm-job-runner on each attempt)
- Exit code 42 on first attempt triggers the failure handler
- On retry (attempt 2+), the job completes successfully
- This simulates real-world transient failures (network timeout, lock contention, etc.)

### How torc-slurm-job-runner Handles It

Inside the Slurm allocation, torc-slurm-job-runner:

1. Claims job work_3 and executes it
2. Job exits with code 42
3. torc-slurm-job-runner's JobRunner detects the failure
4. Fetches the failure handler via API
5. Parses the rules JSON
6. Finds matching rule: `exit_codes: [42], max_retries: 2`
7. Checks attempt count (1 < 2, so retry is allowed)
8. Calls `retry_job` API to reserve a retry slot
9. Marks job as pending for next execution
10. On next job claim, work_3 runs again with `TORC_ATTEMPT_ID=2`
11. Script succeeds this time
12. Workflow continues normally

## See Also

- [failure_handler_demo.yaml](../../examples/yaml/failure_handler_demo.yaml) - More complex example
- [simple_retry.yaml](../../examples/yaml/simple_retry.yaml) - Simpler retry example with `torc run`
