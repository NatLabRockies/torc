# Workflow Recovery

Torc provides mechanisms for recovering workflows when Slurm allocations are preempted or fail
before completing all jobs. The `torc slurm regenerate` command creates new schedulers and
allocations for pending jobs.

## The Recovery Problem

When running workflows on Slurm, allocations can fail or be preempted before all jobs complete. This
leaves workflows in a partial state with:

1. **Ready/uninitialized jobs** - Jobs that were waiting to run but never got scheduled
2. **Blocked jobs** - Jobs whose dependencies haven't completed yet
3. **Orphaned running jobs** - Jobs still marked as "running" in the database even though their
   Slurm allocation has terminated

Simply creating new Slurm schedulers and submitting allocations isn't enough because:

1. **Orphaned jobs block recovery**: Jobs stuck in "running" status prevent the workflow from being
   considered complete, blocking recovery precondition checks
2. **Duplicate allocations**: If the workflow had `on_workflow_start` actions to schedule nodes,
   those actions would fire again when the workflow is reinitialized, creating duplicate allocations
3. **Missing allocations for blocked jobs**: Blocked jobs will eventually become ready, but there's
   no mechanism to schedule new allocations for them

## Orphan Detection

Before recovery can proceed, orphaned jobs must be detected and their status corrected. This is
handled by the **orphan detection** module (`src/client/commands/orphan_detection.rs`).

### How It Works

The orphan detection logic checks for three types of orphaned resources:

1. **Active allocations with terminated Slurm jobs**: ScheduledComputeNodes marked as "active" in
   the database, but whose Slurm job is no longer running (verified via `squeue`)

2. **Pending allocations that disappeared**: ScheduledComputeNodes marked as "pending" whose Slurm
   job no longer exists (cancelled or failed before starting)

3. **Running jobs with no active compute nodes**: Jobs marked as "running" but with no active
   compute nodes to process them (fallback for non-Slurm cases)

```mermaid
flowchart TD
    A[Start Orphan Detection] --> B[List active ScheduledComputeNodes]
    B --> C{For each Slurm allocation}
    C --> D[Check squeue for job status]
    D --> E{Job still running?}
    E -->|Yes| C
    E -->|No| F[Find jobs on this allocation]
    F --> G[Mark jobs as failed]
    G --> H[Update ScheduledComputeNode to complete]
    H --> C
    C --> I[List pending ScheduledComputeNodes]
    I --> J{For each pending allocation}
    J --> K[Check squeue for job status]
    K --> L{Job exists?}
    L -->|Yes| J
    L -->|No| M[Update ScheduledComputeNode to complete]
    M --> J
    J --> N[Check for running jobs with no active nodes]
    N --> O[Mark orphaned jobs as failed]
    O --> P[Done]

    style A fill:#4a9eff,color:#fff
    style B fill:#4a9eff,color:#fff
    style C fill:#6c757d,color:#fff
    style D fill:#4a9eff,color:#fff
    style E fill:#6c757d,color:#fff
    style F fill:#4a9eff,color:#fff
    style G fill:#dc3545,color:#fff
    style H fill:#4a9eff,color:#fff
    style I fill:#4a9eff,color:#fff
    style J fill:#6c757d,color:#fff
    style K fill:#4a9eff,color:#fff
    style L fill:#6c757d,color:#fff
    style M fill:#4a9eff,color:#fff
    style N fill:#4a9eff,color:#fff
    style O fill:#dc3545,color:#fff
    style P fill:#28a745,color:#fff
```

### Integration Points

Orphan detection is integrated into two commands:

1. **`torc recover`**: Runs orphan detection automatically as the first step before checking
   preconditions. This ensures that orphaned jobs don't block recovery.

2. **`torc workflows sync-status`**: Standalone command to run orphan detection without triggering a
   full recovery. Useful for debugging or when you want to clean up orphaned jobs without submitting
   new allocations.

### The `torc watch` Command

The `torc watch` command also performs orphan detection during its polling loop. When it detects
that no valid Slurm allocations exist (via a quick `squeue` check), it runs the full orphan
detection logic to clean up any orphaned jobs before checking if the workflow can make progress.

## Recovery Actions

The recovery system uses **ephemeral recovery actions** to solve these problems.

### How It Works

When `torc slurm regenerate` runs:

```mermaid
flowchart TD
    A[torc slurm regenerate] --> B[Fetch pending jobs]
    B --> C{Has pending jobs?}
    C -->|No| D[Exit - nothing to do]
    C -->|Yes| E[Build WorkflowGraph from pending jobs]
    E --> F[Mark existing schedule_nodes actions as executed]
    F --> G[Group jobs using scheduler_groups]
    G --> H[Create schedulers for each group]
    H --> I[Update jobs with scheduler assignments]
    I --> J[Create on_jobs_ready recovery actions for deferred groups]
    J --> K{Submit allocations?}
    K -->|Yes| L[Submit Slurm allocations]
    K -->|No| M[Done]
    L --> M

    style A fill:#4a9eff,color:#fff
    style B fill:#4a9eff,color:#fff
    style C fill:#6c757d,color:#fff
    style D fill:#6c757d,color:#fff
    style E fill:#4a9eff,color:#fff
    style F fill:#4a9eff,color:#fff
    style G fill:#4a9eff,color:#fff
    style H fill:#4a9eff,color:#fff
    style I fill:#4a9eff,color:#fff
    style J fill:#ffc107,color:#000
    style K fill:#6c757d,color:#fff
    style L fill:#ffc107,color:#000
    style M fill:#28a745,color:#fff
```

### Step 1: Mark Existing Actions as Executed

All existing `schedule_nodes` actions are marked as executed using the `claim_action` API. This
prevents them from firing again and creating duplicate allocations:

```mermaid
sequenceDiagram
    participant R as regenerate
    participant S as Server
    participant A as workflow_action table

    R->>S: get_workflow_actions(workflow_id)
    S-->>R: [action1, action2, ...]

    loop For each schedule_nodes action
        R->>S: claim_action(action_id)
        S->>A: UPDATE executed=1, executed_at=NOW()
    end
```

### Step 2: Group Jobs Using WorkflowGraph

The system builds a `WorkflowGraph` from pending jobs and uses `scheduler_groups()` to group them by
`(resource_requirements, has_dependencies)`. This aligns with the behavior of `torc create-slurm`:

- **Jobs without dependencies**: Can be scheduled immediately with `on_workflow_start`
- **Jobs with dependencies** (deferred): Need `on_jobs_ready` recovery actions to schedule when they
  become ready

```mermaid
flowchart TD
    subgraph pending["Pending Jobs"]
        A[Job A: no deps, rr=default]
        B[Job B: no deps, rr=default]
        C[Job C: depends on A, rr=default]
        D[Job D: no deps, rr=gpu]
    end

    subgraph groups["Scheduler Groups"]
        G1[Group 1: default, no deps<br/>Jobs: A, B]
        G2[Group 2: default, has deps<br/>Jobs: C]
        G3[Group 3: gpu, no deps<br/>Jobs: D]
    end

    A --> G1
    B --> G1
    C --> G2
    D --> G3

    style A fill:#4a9eff,color:#fff
    style B fill:#4a9eff,color:#fff
    style C fill:#ffc107,color:#000
    style D fill:#17a2b8,color:#fff
    style G1 fill:#28a745,color:#fff
    style G2 fill:#28a745,color:#fff
    style G3 fill:#28a745,color:#fff
```

### Step 3: Create Recovery Actions for Deferred Groups

For groups with `has_dependencies = true`, the system creates `on_jobs_ready` recovery actions.
These actions:

- Have `is_recovery = true` to mark them as ephemeral
- Use a `_deferred` suffix in the scheduler name
- Trigger when the blocked jobs become ready
- Schedule additional Slurm allocations for those jobs

```mermaid
flowchart LR
    subgraph workflow["Original Workflow"]
        A[Job A: blocked] --> C[Job C: blocked]
        B[Job B: blocked] --> C
    end

    subgraph actions["Recovery Actions"]
        RA["on_jobs_ready: schedule_nodes<br/>job_ids: (A, B)<br/>is_recovery: true"]
        RC["on_jobs_ready: schedule_nodes<br/>job_ids: (C)<br/>is_recovery: true"]
    end

    style A fill:#6c757d,color:#fff
    style B fill:#6c757d,color:#fff
    style C fill:#6c757d,color:#fff
    style RA fill:#ffc107,color:#000
    style RC fill:#ffc107,color:#000
```

## Recovery Action Lifecycle

Recovery actions are ephemeral - they exist only during the recovery period:

```mermaid
stateDiagram-v2
    [*] --> Created: regenerate creates action
    Created --> Executed: Jobs become ready, action triggers
    Executed --> Deleted: Workflow reinitialized
    Created --> Deleted: Workflow reinitialized

    classDef created fill:#ffc107,color:#000
    classDef executed fill:#28a745,color:#fff
    classDef deleted fill:#6c757d,color:#fff

    class Created created
    class Executed executed
    class Deleted deleted
```

When a workflow is reinitialized (e.g., after resetting jobs), all recovery actions are deleted and
original actions are reset to their initial state. This ensures a clean slate for the next run.

## Database Schema

Recovery actions are tracked using the `is_recovery` column in the `workflow_action` table:

| Column        | Type    | Description                            |
| ------------- | ------- | -------------------------------------- |
| `is_recovery` | INTEGER | 0 = normal action, 1 = recovery action |

### Behavior Differences

| Operation                           | Normal Actions        | Recovery Actions        |
| ----------------------------------- | --------------------- | ----------------------- |
| On `reset_actions_for_reinitialize` | Reset `executed` to 0 | Deleted entirely        |
| Created by                          | Workflow spec         | `torc slurm regenerate` |
| Purpose                             | Configured behavior   | Temporary recovery      |

## Usage

```bash
# Regenerate schedulers for pending jobs
torc slurm regenerate <workflow_id> --account <account>

# With automatic submission
torc slurm regenerate <workflow_id> --account <account> --submit

# Using a specific HPC profile
torc slurm regenerate <workflow_id> --account <account> --profile kestrel
```

## Implementation Details

The recovery logic is implemented in:

- `src/client/commands/orphan_detection.rs`: Shared orphan detection logic used by `recover`,
  `watch`, and `workflows sync-status`
- `src/client/commands/recover.rs`: Main recovery command implementation
- `src/client/commands/slurm.rs`: `handle_regenerate` function for Slurm scheduler regeneration
- `src/client/workflow_graph.rs`: `WorkflowGraph::from_jobs()` and `scheduler_groups()` methods
- `src/server/api/workflow_actions.rs`: `reset_actions_for_reinitialize` function
- `migrations/20251225000000_add_is_recovery_to_workflow_action.up.sql`: Schema migration

Key implementation notes:

1. **WorkflowGraph construction**: A `WorkflowGraph` is built from pending jobs using `from_jobs()`,
   which reconstructs the dependency structure from `depends_on_job_ids`
2. **Scheduler grouping**: Jobs are grouped using `scheduler_groups()` by
   `(resource_requirements, has_dependencies)`, matching `create-slurm` behavior
3. **Deferred schedulers**: Groups with dependencies get a `_deferred` suffix in the scheduler name
4. **Allocation calculation**: Number of allocations is based on job count and resources per node
5. **Recovery actions**: Only deferred groups (jobs with dependencies) get `on_jobs_ready` recovery
   actions
