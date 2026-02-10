# Test Workflows

This directory contains workflow specifications for testing Torc features. These are not intended
for end users - they are for development and testing purposes.

## Workflows

### slurm_oom_test.yaml

Tests Slurm debugging features by intentionally triggering an Out-of-Memory (OOM) condition.

**Purpose:** Verify that the following tools correctly detect and report OOM failures:

- `torc slurm parse-logs` - Should find OOM-related errors in Slurm logs
- `torc slurm sacct` - Should show `OUT_OF_MEMORY` state in the summary table
- torc-dash Debugging tab - Should display the errors in the web UI
- torc TUI - Should allow viewing Slurm logs with 'l' key

**Usage:**

```bash
# Set your Slurm account (or the workflow will use 'default')
export SLURM_ACCOUNT=myaccount
export SLURM_PARTITION=standard
export SLURM_QOS=normal

# Create and submit the workflow
torc workflows create tests/workflows/slurm_oom_test.yaml
torc slurm schedule-nodes <workflow_id>

# Wait for job to fail (check with: squeue --me)

# Test the debugging tools
torc slurm parse-logs <workflow_id>
torc slurm sacct <workflow_id>

# Or use the dashboard
torc-dash --standalone
# Navigate to Debugging tab
```

**Expected Results:**

- Job should fail within ~2-5 minutes after starting
- `parse-logs` should detect "oom-kill" or "out of memory" patterns
- `sacct` should show state as `OUT_OF_MEMORY` or `FAILED`
- Exit code should be non-zero (typically 137 for SIGKILL or 9 for OOM)

---

### resource_regroup_test/

Tests the `analyze_resource_usage` and `regroup_job_resources` MCP tools.

**Scenario:**

- 6 data-processing jobs in 2 RR groups with varying actual memory usage
- Job names and commands are opaque — the AI must analyze results to discover usage patterns
- AI identifies natural clusters and proposes better resource groupings

**Usage:**

```bash
torc run tests/workflows/resource_regroup_test/workflow.yaml
# Then use MCP tools: analyze_resource_usage → regroup_job_resources
```

See `resource_regroup_test/README.md` for detailed instructions.

---

## Watcher Test Workflows

The following directories contain complete test scenarios for `torc watch` functionality.

### recovery_hook_test/

Tests the `--recovery-hook` feature of `torc watch --recover`.

**Scenario:**

- 5 work jobs + 1 postprocess job
- `work_3` fails because a required file is missing
- Recovery hook script creates the missing file
- Workflow succeeds on retry

**Usage:**

```bash
cd tests/workflows/recovery_hook_test
# Edit workflow.yaml to set your Slurm account
torc submit-slurm --account <account> workflow.yaml
export TORC_OUTPUT_DIR=output
torc watch <workflow_id> --recover --recovery-hook "bash create_missing_file.sh"
```

See `recovery_hook_test/README.md` for detailed instructions.

### oom_auto_recovery_test/

Tests automatic OOM recovery in `torc watch --recover`.

**Scenario:**

- 10 work jobs that request 10GB memory but try to allocate 30GB
- Jobs fail with OOM
- Watcher detects OOM and increases memory (10GB → 15GB → 22GB → 33GB)
- Eventually jobs get enough memory and succeed

**Usage:**

```bash
cd tests/workflows/oom_auto_recovery_test
# Edit workflow.yaml to set your Slurm account
chmod +x allocate_memory.sh
torc submit-slurm --account <account> workflow.yaml
torc watch <workflow_id> --recover --max-retries 5
```

See `oom_auto_recovery_test/README.md` for detailed instructions.

### timeout_auto_recovery_test/

Tests automatic timeout recovery in `torc watch --recover`.

**Scenario:**

- 2 jobs with 5 minute runtime specified
- `job_fast` completes in 1 minute (succeeds)
- `job_slow` runs for 10 minutes (exceeds walltime, gets killed)
- Watcher detects timeout and increases runtime (5min → 7.5min → 11.25min)
- Eventually job gets enough time and succeeds

**Usage:**

```bash
cd tests/workflows/timeout_auto_recovery_test
# Edit workflow.yaml to set your Slurm account
torc submit-slurm --account <account> workflow.yaml
torc watch <workflow_id> --recover --max-retries 3
```

See `timeout_auto_recovery_test/README.md` for detailed instructions.
