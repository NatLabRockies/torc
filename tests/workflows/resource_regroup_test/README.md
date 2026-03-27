# Resource Regrouping Test

This test verifies the `analyze_resource_usage` and `regroup_job_resources` MCP tools. It runs a
workflow where the initial resource groups don't match actual usage patterns, then uses an AI to
discover better groupings from the results data.

## Workflow Description

**6 data-processing jobs** split across **2 resource groups** (`standard`, `high_mem`). Each job
processes a different data chunk with varying memory requirements. The actual memory usage per chunk
is defined in `workloads.conf` and is not obvious from the job names or commands.

The AI must call `analyze_resource_usage` to see actual peak memory per job and decide how to
regroup.

## Test Procedure

### 1. Run the workflow locally

```bash
torc run tests/workflows/resource_regroup_test/workflow.yaml
```

Note the workflow ID. Wait for all 6 jobs to complete (~15-20 seconds each).

### 2. Verify jobs completed

```bash
torc workflows status <workflow_id>
```

### 3. Ask the AI to analyze and regroup

> "Analyze resource usage for workflow `<id>` and suggest better resource groupings."

The AI should:

1. Call `analyze_resource_usage(workflow_id=<id>)` to see actual peak memory per job
2. Identify natural clusters based on the data
3. Propose regrouping with `regroup_job_resources(dry_run=true)`
4. After user approval, apply with `regroup_job_resources(dry_run=false)`

### 4. Verify regrouping

```bash
torc workflows check-resources <workflow_id>
```

## Files

- `workflow.yaml` - Workflow specification with 6 jobs in 2 RR groups
- `process_data.sh` - Script that processes a data chunk (memory usage varies per chunk)
- `workloads.conf` - Configuration mapping chunk indices to memory requirements
- `README.md` - This file
