# Slurm Allocation Strategies

When submitting a workflow with many jobs to Slurm, you must decide how to split work across
allocations. The `torc slurm plan-allocations` command (or the `plan_allocations` MCP tool for AI
assistants) analyzes your workflow and cluster state to recommend a strategy.

## The Core Tradeoff: Single Large vs Many Small

Given N nodes worth of work, there are two extremes:

| Strategy                 | Description                           | Pros                                                                                                | Cons                                                                                    |
| ------------------------ | ------------------------------------- | --------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| **1 x N** (single large) | One allocation requesting all N nodes | Slurm prioritizes larger jobs; all work completes in one walltime window; no fair-share degradation | Must wait for N nodes to be available simultaneously                                    |
| **N x 1** (many small)   | N separate single-node allocations    | First jobs start as soon as any node is free                                                        | Fair-share degrades as allocations start; last jobs may wait much longer than the first |

### When Single Large Wins

- **Slurm backfill priority**: Slurm's scheduler reserves nodes for large pending jobs. A 167-node
  request gets a reserved slot in the queue, while 167 individual jobs compete with everyone.
- **Fair-share preservation**: A single allocation consumes your fair-share budget once. Many small
  allocations drain it progressively, causing later jobs to lose priority.
- **Deterministic completion**: All jobs start processing simultaneously and finish within one
  walltime window.
- **Busy clusters**: Counter-intuitively, a fully loaded cluster often favors large allocations
  because Slurm will schedule the large job as a block when enough nodes free up, rather than
  letting small jobs trickle through.

### When Many Small Wins

- **Extremely long queues**: If the cluster is oversubscribed for weeks, small jobs may fit into
  backfill gaps that a large allocation cannot.
- **Partial results needed**: If you need some results quickly rather than waiting for all of them.
- **Near partition limits**: If your ideal node count exceeds `max_nodes_per_user`, you cannot
  request a single allocation that large.

## Using `sbatch --test-only`

The `plan-allocations` command runs `sbatch --test-only` to ask Slurm's scheduler when each strategy
would start, without actually submitting jobs. For a plan with K nodes per allocation and N total
allocations:

```bash
# Single large: when would all K*N nodes start together?
sbatch --test-only --nodes=<K*N> --time=04:00:00 --account=myproject --wrap="hostname"

# Many small: when would one K-node allocation start?
sbatch --test-only --nodes=<K> --time=04:00:00 --account=myproject --wrap="hostname"
```

When no partition is explicitly configured, the `--partition` flag is omitted so Slurm uses its
default partition.

The single-large estimated start + walltime gives the completion time directly. The many-small
estimate is **optimistic** — it only predicts when the _first_ allocation would start. Later
allocations will be delayed by fair-share degradation.

### Fair-Share Degradation Estimate

The tool estimates the last small allocation's completion as:

```
last_completion ≈ first_wait × min(N, 10) + walltime
```

This is a rough approximation. The actual degradation depends on your account's fair-share balance,
other users' activity, and the scheduler's configuration.

## Interpreting Results

Example output:

```
Recommendations
===============
  "work_resources": 1 allocation(s) x 167 node(s) [single]
    sbatch --test-only: large (167 nodes) completes in ~4h 30min,
    faster than 167 small allocations (~6h 30min).
    Slurm prioritizes larger allocations

    Scheduler Estimate (sbatch --test-only):
      Single large (167 nodes): start in ~30min, complete in ~4h 30min
      Many small  (1 node):     start in ~5min, complete in ~4h 5min
        Note: estimate is for first job only; later jobs delayed by fair-share
```

Key things to check:

- **Large completion vs small completion**: The tool accounts for fair-share degradation in its
  recommendation, but review the raw estimates yourself.
- **Wait time for large**: If the large allocation won't start for hours while small jobs start
  immediately, small may still be better for partial results.
- **Dependency depth**: A DAG with deep dependency chains cannot exploit N-node parallelism fully.
  Check `max_parallelism` in the workflow analysis — if it's much less than `ideal_nodes`, you may
  need fewer nodes than calculated.

## Skipping the Probes

Use `--skip-test-only` to rely on heuristics only (faster, no `sbatch` calls):

```bash
torc slurm plan-allocations --account myproject --skip-test-only workflow.yaml
```

Use `--offline` to skip all cluster queries (sinfo, squeue, sbatch):

```bash
torc slurm plan-allocations --account myproject --offline workflow.yaml
```

## Applying the Recommendation

After reviewing the recommendation:

```bash
# Single large allocation (1 x N nodes)
torc slurm generate --account myproject --single-allocation workflow.yaml

# Many small allocations (N x 1 node, the default)
torc slurm generate --account myproject workflow.yaml
```

## Using with AI Assistants

The `plan_allocations` MCP tool provides the same analysis through AI assistants (Claude Code,
GitHub Copilot). The assistant can analyze your workflow spec and present the recommendation
conversationally:

```
> I have a workflow with 167 parallel jobs. Should I use one big Slurm
> allocation or many small ones? My account is "myproject".

⏺ torc - plan_allocations (MCP)(spec_json: {...}, account: "myproject")
  ⎿  { "recommendations": [...], "cluster_state": [...], ... }

⏺ Based on the current cluster state and sbatch --test-only probes:

  Recommendation: Single large allocation (1 x 167 nodes)

  | Strategy    | Estimated Start | Estimated Completion |
  |-------------|-----------------|----------------------|
  | Single large| ~30 min         | ~4h 30min            |
  | Many small  | ~5 min (first)  | ~6h 30min (last)     |

  The single large allocation completes ~2 hours faster because Slurm
  prioritizes it in the backfill queue, and you avoid fair-share
  degradation from 167 separate allocations.

  Shall I generate the workflow with a single allocation?
```

The tool accepts a workflow spec as a JSON object, the Slurm account, and optional partition and HPC
profile overrides. Use `skip_test_only: true` to skip the sbatch probes for faster results based on
heuristics only.

See [Configuring AI Assistants](../tools/ai-assistants.md) for setup instructions.
