# Tutorial: AI-Assisted Workflow Management

This tutorial shows how to use AI assistants to manage Torc workflows using natural language.

## What You'll Learn

- Set up an AI assistant to work with Torc
- Create and manage workflows through conversation
- Debug failures by asking questions

## Prerequisites

- Torc installed and server running
- [Claude Code](https://claude.ai/code) or VS Code with GitHub Copilot

## What Can AI Assistants Do?

With Torc's AI integration, you can manage workflows using natural language:

| Task               | Example                                                 |
| ------------------ | ------------------------------------------------------- |
| Create workflows   | _"Create a workflow with 10 parallel jobs"_             |
| Check status       | _"What's the status of workflow 42?"_                   |
| Debug failures     | _"Why did a job in workflow 5 fail?"_                   |
| Fix problems       | _"Restart the failed jobs with doubled memory"_         |
| Investigate        | _"Check if any jobs exceeded their memory limits"_      |
| Optimize resources | _"Analyze usage and suggest better resource groupings"_ |
| Plan allocations   | _"Should I use one big allocation or many small ones?"_ |

---

## Quick Setup: Claude Code

**Step 1:** Add the Torc MCP server to your project:

```bash
claude mcp add torc \
  --scope project \
  -e TORC_API_URL=http://localhost:8080/torc-service/v1 \
  -e TORC_OUTPUT_DIR=./output \
  -- /path/to/torc-mcp-server
```

**Step 2:** Verify it's configured:

```bash
claude mcp list
```

**Step 3:** Start a new Claude Code session and try:

> "What workflows exist?"

---

## Quick Setup: VS Code + Copilot

**Step 1:** Create `.vscode/mcp.json` in your project:

```json
{
  "servers": {
    "torc": {
      "command": "/path/to/torc-mcp-server",
      "env": {
        "TORC_API_URL": "http://localhost:8080/torc-service/v1",
        "TORC_OUTPUT_DIR": "./output"
      }
    }
  }
}
```

**Step 2:** In Copilot Chat, use Agent Mode and try:

> "What workflows exist?"

---

## Example: Creating a Workflow

```
> Create a workflow with 3 stages: a preprocess script, 10 work jobs that each take an index, and a postprocess job. The work jobs each require
 10 CPUs and 20 GB of memory. The other jobs have trivial resource requirements.

⏺ I'll create a workflow spec file with the 3-stage fan-out/fan-in pattern you described.

⏺ Created the workflow spec at ./workflow.json with:

  Structure:
  - Stage 1: preprocess - runs first, outputs preprocess_output.txt
  - Stage 2: work_0 through work_9 - 10 parallel jobs, each reads preprocess output
  - Stage 3: postprocess - waits for all work jobs to complete (fan-in pattern)

  Resource Requirements:
  | Name  | CPUs | Memory | Runtime |
  |-------|------|--------|---------|
  | small | 1    | 1g     | 1 hour  |
  | large | 10   | 20g    | 2 hours |

  Next steps:
  1. Edit the spec to replace placeholder commands with your actual scripts
  2. Run locally: torc run workflow.json
  3. Or submit to Slurm: torc submit workflow.json (requires adding an account)
```

You can create much more complex workflows. Output is not shown, but this prompt will create a
workflow with job dependencies defined implicitly through job-file relationships:

```
Create a workflow with four stages of jobs. There are 3 initial jobs each dependent on an input file. Each of those jobs fans out into 10 work jobs through
  output files. Each of those jobs creates a file. In the third stage there is a script that aggregates each of the 10 jobs into one file. The final
  postprocess job aggregates those three files into one. I will run it with Slurm: account=my_account. The work jobs require 10 cpus and 30 GB of
  memory. The other jobs have trivial resource requirements.
```

---

## Example: Adjusting jobs' resource requirements

Suppose that jobs fail because they use too much CPU or memory.

```
> Check the resource utilization for workflow 20

⏺ torc - check_resource_utilization (MCP)(workflow_id: 20)
  ⎿  {
       "over_utilization_count": 1,
       "run_id": null,
     … +17 lines (ctrl+o to expand)

⏺ Resource Utilization for Workflow 20

  Summary: 3 job results analyzed, 1 resource violation found

  CPU Over-Utilization

  | Job ID | Job Name      | Specified      | Peak Used | Over By |
  |--------|---------------|----------------|-----------|---------|
  | 264    | cpu_heavy_job | 3 cores (300%) | 602.5%    | +100.8% |

  The cpu_heavy_job used approximately 6 CPU cores at peak, but was only allocated 3 cores.

  Would you like me to update the resource requirements for this job? I can increase the CPU allocation to 7 cores to provide headroom for future runs.

> yes

⏺ torc - update_job_resources (MCP)(job_id: 264, num_cpus: 7)
  ⎿  {
       "job_id": 264,
       "resource_requirements_id": 45,
     … +8 lines (ctrl+o to expand)

⏺ Done. Updated cpu_heavy_job (job 264) resources:

  | Setting | Value      |
  |---------|------------|
  | CPUs    | 7 (was 3)  |
  | Memory  | 1g         |
  | Runtime | 15 minutes |

  The job is now configured with enough CPU capacity for future runs.
```

---

## Example: Regrouping resources based on actual usage

When jobs are initially assigned to broad resource groups, actual usage may reveal that a finer
grouping would be more efficient. The AI can analyze per-job resource measurements and propose new
groups.

In this example, 6 data-processing jobs are split across 2 resource groups (`standard` and
`high_mem`), but actual memory usage falls into 3 distinct clusters.

```
> Analyze resource usage for workflow 42 and suggest better groupings.

⏺ torc - analyze_resource_usage (MCP)(workflow_id: 42)
  ⎿  {
       "workflow_id": 42,
       "total_jobs": 6,
       "total_jobs_with_results": 6,
       "resource_groups": [ ...
     … +80 lines (ctrl+o to expand)

⏺ Resource Usage Analysis for Workflow 42

  Current grouping: 2 groups (standard, high_mem), both configured with 2g memory.

  Actual peak memory by job:

  | Job              | Current RR | Peak Memory |
  |------------------|------------|-------------|
  | process_chunk_0  | standard   | 210 MB      |
  | process_chunk_1  | standard   | 525 MB      |
  | process_chunk_2  | standard   | 210 MB      |
  | process_chunk_3  | high_mem   | 1050 MB     |
  | process_chunk_4  | high_mem   | 1049 MB     |
  | process_chunk_5  | high_mem   | 524 MB      |

  I can see 3 natural clusters:
  - ~200 MB: process_chunk_0, process_chunk_2
  - ~500 MB: process_chunk_1, process_chunk_5
  - ~1000 MB: process_chunk_3, process_chunk_4

  The current 2-group split doesn't match these clusters — chunk_1 (500 MB) is
  in the "standard" group alongside 200 MB jobs, and chunk_5 (500 MB) is in
  "high_mem" alongside 1000 MB jobs.

  Would you like me to propose a 3-group regrouping with appropriate memory
  allocations?

> yes

⏺ torc - regroup_job_resources (MCP)(
    workflow_id: 42,
    dry_run: true,
    groups: [
      { name: "small", memory: "300m", num_cpus: 1, runtime: "PT5M",
        job_ids: [301, 303] },
      { name: "medium", memory: "700m", num_cpus: 1, runtime: "PT5M",
        job_ids: [302, 306] },
      { name: "large", memory: "1300m", num_cpus: 1, runtime: "PT5M",
        job_ids: [304, 305] }
    ])
  ⎿  {
       "workflow_id": 42,
       "dry_run": true,
       "groups": [ ...
     … +40 lines (ctrl+o to expand)

⏺ Regrouping Preview (Dry Run)

  | Group  | Memory | Jobs                             |
  |--------|--------|----------------------------------|
  | small  | 300m   | process_chunk_0, process_chunk_2 |
  | medium | 700m   | process_chunk_1, process_chunk_5 |
  | large  | 1300m  | process_chunk_3, process_chunk_4 |

  Each group has ~30% headroom above peak usage.
  New RR records will be created; existing RRs are not modified.

  Would you like me to apply this regrouping?

> yes

⏺ torc - regroup_job_resources (MCP)(workflow_id: 42, dry_run: false, groups: [...])
  ⎿  {
       "success": true,
       "total_jobs_updated": 6,
     … +20 lines (ctrl+o to expand)

⏺ Regrouping Complete ✓

  Created 3 new resource requirement groups and reassigned all 6 jobs:
  - small (300m): 2 jobs
  - medium (700m): 2 jobs
  - large (1300m): 2 jobs

  On the next run, each job will request only the memory it actually needs.
```

---

## Example: Planning Slurm allocation strategy

When submitting a workflow to Slurm, the AI can analyze the cluster state and recommend whether to
use a single large allocation or many small ones.

```
> I'm about to submit this workflow to Slurm. Should I use one big
> allocation or many small ones? My account is "climate_sim".

⏺ torc - plan_allocations (MCP)(spec_json: {...}, account: "climate_sim")
  ⎿  { "recommendations": [...], "cluster_state": [...], ... }

⏺ Allocation Strategy Analysis

  Workflow: 167 parallel work jobs, 1 preprocess, 1 postprocess
  Cluster: short partition — 42 idle, 180 allocated, 12 pending jobs

  Recommendation: Single large allocation (1 x 167 nodes)

  | Strategy     | Est. Start | Est. Completion |
  |--------------|------------|-----------------|
  | Single large | ~30 min    | ~4h 30min       |
  | Many small   | ~5 min     | ~6h 30min       |

  Reason: Slurm's backfill scheduler gives the large allocation a
  reserved slot. Many small allocations would start faster initially,
  but fair-share degradation delays the last jobs by ~2 hours.

  Your DAG has max_parallelism=167, so all work jobs can run
  simultaneously — a single allocation fully utilizes all nodes.

> Go ahead and generate it with the single allocation.

⏺ Running: torc slurm generate --account climate_sim --single-allocation workflow.yaml
  ⎿  Generated scheduler plan with 1 allocation x 167 nodes.

⏺ Done. Submit with: torc submit workflow.yaml
```

For background on the tradeoffs, see [Allocation Strategies](../hpc/allocation-strategies.md).

---

## Example: Recovering a failed workflow that timed out

The workflow in `tests/workflows/timeout_auto_recovery_test` is used to test torc's auto-recovery of
walltime timeouts. In this example, the workflow was left in a failed state.

Here is a conversation in Claude Code after running the workflow:

```
> analyze the logs for workflow 6

Workflow 6 Log Analysis Summary

  Workflow: timeout_auto_recovery_test - Test workflow for automatic timeout recovery

  Job Results

  | Job | Name       | Status        | Runtime | Return Code  |
  |-----|------------|---------------|---------|--------------|
  | 31  | (fast job) | ✅ Completed  | 1.0 min | 0            |
  | 32  | job_slow   | ⛔ Terminated | 5.9 min | -9 (SIGKILL) |

  Terminated Job Details (job_slow)

  The job was killed by signal 9 (SIGKILL) after running for ~6 minutes, which matches the expected behavior for a timeout test.

  stdout log shows the job ran through minute 6 before being killed:
  Slow job starting at Sat Dec 27 15:40:59 MST 2025
  This job will run for 10 minutes (exceeds 5 min runtime and 8 min walltime)
  It should be killed by Slurm and recovered by the watcher
  Minute 1 of 10...
  Minute 2 of 10...
  ...
  Minute 6 of 10...

  stderr log: Empty (no errors written)

  Conclusion

  The timeout auto-recovery test is working as expected:
  - The slow job exceeded its 5-minute runtime limit
  - It was killed by the system (SIGKILL, return code -9) at ~6 minutes
  - Peak memory usage was ~4.1 MB (no memory issues)
```

---

## Example: Debugging a failed workflow with over-consumption of memory

The workflow in `tests/workflows/oom_auto_recovery_test` is used to test torc's diagnosis of memory
over-consumption errors.

Here is a conversation in Claude Code after running the workflow:

> Analyze the logs for workflow 16

● torc - analyze_workflow_logs (MCP)(workflow_id: 16) ⎿  { "error_count": 0, "errors_by_type": [], …
+7 lines (ctrl+o to expand)

● torc - get_workflow_status (MCP)(workflow_id: 16) ⎿  { "description": "Test workflow with an OOM",
"job_counts_by_status": { … +8 lines (ctrl+o to expand)

● The workflow has 3 failed jobs out of 10 total, but the log analyzer didn't find error patterns.
Let me dig deeper:

● torc - list_failed_jobs (MCP)(workflow_id: 16) ⎿  { "failed_job_count": 3, "failed_jobs": [ … +18
lines (ctrl+o to expand)

● torc - check_resource_utilization (MCP)(workflow_id: 16, include_failed: true) ⎿  { "failed_jobs":
[ { … +137 lines (ctrl+o to expand)

● Workflow 16 Analysis Summary

Workflow: oom_test - "Test workflow with an OOM" Status: 7 completed, 3 failed (10 total jobs)

Root Cause: Out of Memory (OOM)

All 3 failed jobs were killed due to OOM (return code 137):

| Job ID | Name   | Peak Memory        | Configured | Over-Utilization |
| ------ | ------ | ------------------ | ---------- | ---------------- |
| 103    | work_1 | N/A (killed early) | 10g        | SIGKILL 137      |
| 109    | work_7 | 29.91 GB           | 10g        | +199.1%          |
| 110    | work_8 | 26.89 GB           | 10g        | +168.9%          |

Critical Warning: More Failures Expected

9 jobs exceeded their 10 GB memory limit - including 6 jobs that happened to complete but used ~30
GB each:

- work_2 through work_6, work_9, work_10 all peaked at ~30.01 GB (+200.1% over limit)
- These completed due to available system memory but are also misconfigured

Recommendation

1. Increase memory allocation to 32g for all work jobs
2. Use automated recovery - I can run a dry-run of the recovery tool to preview fixes

Would you like me to run recover_workflow with dry_run=true to preview the recovery actions?

> yes

● torc - recover_workflow (MCP)(workflow_id: 16, dry_run: true) ⎿  { "dry_run": true,
"memory_multiplier": 1.5, … +9 lines (ctrl+o to expand)

● Recovery Preview (Dry Run)

Proposed Actions:

- 3 failed jobs would have memory increased (1.5x multiplier: 10g → 15g)
- Failed jobs would be reset to ready status
- Slurm schedulers would be regenerated

⚠️ Important Consideration

The default 1.5x memory multiplier would increase memory from 10g → 15g, but the jobs actually
peaked at ~30 GB. This means:

- 15g may still cause OOM failures
- A 3.5x multiplier (10g → 35g) would be safer based on actual usage

Would you like me to proceed with:

1. Default recovery (1.5x = 15g) - may require another recovery cycle
2. Custom recovery with a higher memory multiplier (e.g., 3.5x = 35g)

> 2

● torc - recover_workflow (MCP)(workflow_id: 16, dry_run: true, memory_multiplier: 3.5) ⎿  {
"dry_run": true, "memory_multiplier": 3.5, … +9 lines (ctrl+o to expand)

● Recovery Preview (3.5x Memory Multiplier)

Proposed Actions:

- 3 failed jobs would have memory increased: 10g → 35g
- Failed jobs would be reset to ready status
- Slurm schedulers would be regenerated

This should safely cover the ~30 GB peak memory usage observed.

Would you like me to proceed with these recovery actions?

> yes

● torc - recover_workflow (MCP)(workflow_id: 16, dry_run: false, memory_multiplier: 3.5) ⎿  {
"dry_run": false, "memory_multiplier": 3.5, … +7 lines (ctrl+o to expand)

● Recovery Complete ✓

Actions taken:

- 3 jobs had memory increased from 10g → 35g
- Failed jobs reset to ready status
- Slurm schedulers regenerated and submitted

● torc - get_workflow_status (MCP)(workflow_id: 16) ⎿  { "description": "Test workflow with an OOM",
"job_counts_by_status": { … +8 lines (ctrl+o to expand)

● Current Status:

| Status    | Count |
| --------- | ----- |
| Completed | 7     |
| Ready     | 3     |

The 3 previously failed jobs (work_1, work_7, work_8) are now ready to run with 35g memory. Slurm
allocations have been submitted and will execute these jobs automatically.

---

## What You Learned

- How to configure AI assistants with Torc (one command for Claude Code, one file for VS Code)
- How to create workflows through conversation
- How to debug and fix failures using natural language

## Next Steps

- [Configuring AI Assistants](./ai-assistants.md) — Full configuration options, all tools,
  troubleshooting
- [Automatic Failure Recovery](./automatic-recovery.md) — Use `torc watch` for automated recovery
- [Configuration Files](./configuration.md) — Set up Torc configuration
