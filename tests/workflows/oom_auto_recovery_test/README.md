# OOM Auto-Recovery Test

This test verifies that `torc watch --recover` correctly detects OOM (Out of Memory) failures and
automatically increases memory allocations for retry.

## Workflow Description

- **10 work jobs** (`work_1` through `work_10`): Each requests 10GB memory and 10 CPUs
- Each job runs `allocate_memory.sh 30` which attempts to allocate 30GB of memory
- Jobs will fail with OOM since they only have 10GB allocated
- The watcher should detect OOM and increase memory by 1.5x on each retry

## Expected Recovery Sequence

With the default memory multiplier of 1.5x:

1. **Initial**: Jobs request 10GB, try to use 30GB -> OOM failure
2. **Retry 1**: Jobs get 15GB (10 x 1.5), still not enough -> OOM failure
3. **Retry 2**: Jobs get 22GB (15 x 1.5), still not enough -> OOM failure
4. **Retry 3**: Jobs get 33GB (22 x 1.5), enough for 30GB -> Success!

## Test Procedure

### 1. Update the account in workflow.yaml

Replace `PLACEHOLDER_ACCOUNT` with your actual Slurm account:

```bash
sed -i 's/PLACEHOLDER_ACCOUNT/your_account/g' workflow.yaml
```

### 2. Submit the workflow

```bash
torc slurm generate --account <your_account> workflow.yaml
torc submit workflow.yaml
```

Note the workflow ID from the output.

### 3. Run the watcher with auto-recover

```bash
torc watch <workflow_id> --recover --max-retries 5
```

### 4. Expected output

You should see output similar to:

```
Watching workflow <id> (poll interval: 60s, auto-recover enabled, max retries: 5)
...
Workflow completed with failures:
  - Failed: 10

Attempting automatic recovery (attempt 1/5)

Diagnosing failures...
Applying recovery heuristics...
  Job 1 (work_1): OOM detected, increasing memory 10g -> 15g
  Job 2 (work_2): OOM detected, increasing memory 10g -> 15g
  ...
  Applied fixes: 10 OOM, 0 timeout

Resetting failed jobs...
Regenerating Slurm schedulers and submitting...

Recovery initiated. Resuming monitoring...
...
```

This cycle repeats until jobs get enough memory (33GB) and succeed.

## Files

- `workflow.yaml` - The workflow specification
- `README.md` - This file

The `allocate_memory.sh` script is located at `slurm-tests/scripts/allocate_memory.sh`.

## Verification

After the test completes successfully:

```bash
# Check workflow status
torc workflows status <workflow_id>

# Verify all jobs completed
torc jobs list <workflow_id>

# Check the final resource requirements (should show ~33GB)
torc jobs list-resource-requirements <workflow_id>

# Check resource utilization report
torc workflows check-resources <workflow_id>
```

## Adjusting the Test

To make the test faster, you can:

1. Use a higher memory multiplier:
   ```bash
   torc watch <workflow_id> --recover --memory-multiplier 2.0
   ```
   With 2.0x: 10GB -> 20GB -> 40GB (success in 2 retries)

2. Start with higher initial memory in `workflow.yaml`: Change `memory: 10g` to `memory: 20g` to
   reduce the number of retries needed.
