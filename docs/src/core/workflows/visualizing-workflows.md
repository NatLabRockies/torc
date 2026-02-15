# Visualizing Workflow Structure

Understanding how your workflow will execute—which jobs run in parallel, how dependencies create
stages, and when Slurm allocations are requested—is essential for debugging and optimization. Torc
provides several tools for visualizing workflow structure.

## Execution Plan Command

The `torc workflows execution-plan` command analyzes a workflow and displays its execution stages,
showing how jobs are grouped and when schedulers allocate resources.

### Basic Usage

```bash
# From a specification file
torc workflows execution-plan workflow.yaml

# From an existing workflow
torc workflows execution-plan <workflow_id>
```

### Example Output

For a workflow with two independent processing pipelines that merge at the end:

```
Workflow: two_subgraph_pipeline
Total Jobs: 15

▶ Stage 1: Workflow Start
  Scheduler Allocations:
    • prep_sched (slurm) - 1 allocation(s)
  Jobs Becoming Ready:
    • prep_a
    • prep_b

→ Stage 2: When jobs 'prep_a', 'prep_b' complete
  Scheduler Allocations:
    • work_a_sched (slurm) - 1 allocation(s)
    • work_b_sched (slurm) - 1 allocation(s)
  Jobs Becoming Ready:
    • work_a_{1..5}
    • work_b_{1..5}

→ Stage 3: When 10 jobs complete
  Scheduler Allocations:
    • post_a_sched (slurm) - 1 allocation(s)
    • post_b_sched (slurm) - 1 allocation(s)
  Jobs Becoming Ready:
    • post_a
    • post_b

→ Stage 4: When jobs 'post_a', 'post_b' complete
  Scheduler Allocations:
    • final_sched (slurm) - 1 allocation(s)
  Jobs Becoming Ready:
    • final

Total Stages: 4
```

### What the Execution Plan Shows

1. **Stages**: Groups of jobs that become ready at the same time based on dependency resolution
2. **Scheduler Allocations**: Which Slurm schedulers request resources at each stage (for workflows
   with Slurm configuration)
3. **Jobs Becoming Ready**: Which jobs transition to "ready" status at each stage
4. **Subgraphs**: Independent branches of the workflow that can execute in parallel

### Workflows Without Slurm Schedulers

For workflows without pre-defined Slurm schedulers, the execution plan shows the job stages without
scheduler information:

```bash
torc workflows execution-plan workflow_no_slurm.yaml
```

```
Workflow: my_pipeline
Total Jobs: 10

▶ Stage 1: Workflow Start
  Jobs Becoming Ready:
    • preprocess

→ Stage 2: When job 'preprocess' completes
  Jobs Becoming Ready:
    • work_{1..5}

→ Stage 3: When 5 jobs complete
  Jobs Becoming Ready:
    • postprocess

Total Stages: 3
```

This helps you understand the workflow topology before adding Slurm configuration with
`torc slurm generate`.

### Use Cases

- **Validate workflow structure**: Ensure dependencies create the expected execution order
- **Identify parallelism**: See which jobs can run concurrently
- **Debug slow workflows**: Find stages that serialize unnecessarily
- **Plan Slurm allocations**: Understand when resources will be requested
- **Verify auto-generated schedulers**: Check that `torc slurm generate` created appropriate staging

## DAG Visualization in the Dashboard

The [web dashboard](./dashboard.md) provides interactive DAG (Directed Acyclic Graph) visualization.

### Viewing the DAG

1. Navigate to the **Details** tab
2. Select a workflow
3. Click **View DAG** in the Visualization section

### DAG Types

The dashboard supports three DAG visualization types:

| Type                       | Description                                           |
| -------------------------- | ----------------------------------------------------- |
| **Job Dependencies**       | Shows explicit and implicit dependencies between jobs |
| **Job-File Relations**     | Shows how jobs connect through input/output files     |
| **Job-UserData Relations** | Shows how jobs connect through user data              |

### DAG Features

- **Color-coded nodes**: Jobs are colored by status (ready, running, completed, failed, etc.)
- **Interactive**: Zoom, pan, and click nodes for details
- **Layout**: Automatic hierarchical layout using Dagre algorithm
- **Legend**: Status color reference

## TUI DAG View

The terminal UI (`torc tui`) also includes DAG visualization:

1. Select a workflow
2. Press `d` to toggle the DAG view
3. Use arrow keys to navigate

## Comparing Visualization Tools

| Tool             | Best For                                                |
| ---------------- | ------------------------------------------------------- |
| `execution-plan` | Understanding execution stages, Slurm allocation timing |
| Dashboard DAG    | Interactive exploration, status monitoring              |
| TUI DAG          | Quick terminal-based visualization                      |

## Example: Analyzing a Complex Workflow

Consider a workflow with preprocessing, parallel work, and aggregation:

```bash
# First, view the execution plan
torc workflows execution-plan examples/subgraphs/subgraphs_workflow.yaml

# If no schedulers, generate them
torc slurm generate --account myproject examples/subgraphs/subgraphs_workflow_no_slurm.yaml

# View the plan again to see scheduler allocations
torc workflows execution-plan examples/subgraphs/subgraphs_workflow.yaml
```

The execution plan helps you verify that:

- Independent subgraphs are correctly identified
- Stages align with your expected execution order
- Slurm allocations are timed appropriately

## See Also

- [Web Dashboard](../monitoring/dashboard.md) — Full dashboard documentation
- [Slurm Workflows](../../specialized/hpc/slurm-workflows.md) — Understanding Slurm integration
- [Workflow Actions](../../specialized/design/workflow-actions.md) — How actions trigger scheduler
  allocations
- [Subgraphs Example](https://github.com/NatLabRockies/torc/tree/main/examples/subgraphs) — Complete
  example with multiple subgraphs
