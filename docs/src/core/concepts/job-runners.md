# Job Runners

Job runners are worker processes that execute jobs on compute resources.

## Job Runner Modes

Torc supports three execution modes:

1. **Local Runner** (`torc run`) - Runs jobs on the local machine with resource tracking
2. **HPC/Slurm Runner** (`torc slurm generate` + `torc submit`) - Runs jobs on Slurm-allocated
   compute nodes
3. **Remote Workers** (`torc remote run`) - Distributes jobs across SSH-accessible machines

### Local Runner

The local runner executes jobs directly on the current machine. Start it with:

```console
torc run <workflow-id>
```

### HPC/Slurm Runner

For HPC clusters, jobs run on Slurm-allocated compute nodes. The `torc-slurm-job-runner` binary is
launched by Slurm on each allocated node and polls the server for work.

### Remote Workers

Remote workers enable distributed execution without a scheduler. The `torc remote run` command
SSH-es into multiple machines and starts a `torc run` process on each:

```console
torc remote run workers.txt <workflow-id>
```

Each remote worker runs as a detached process and polls the server for jobs, just like the local
runner. The server coordinates job distribution to prevent double-allocation.

On Unix systems, runners also use a `SIGCHLD`-driven wakeup path so local subprocess exits can be
observed promptly instead of always waiting for the full poll interval.

## Job Allocation Strategies

The job runner supports two different strategies for retrieving and executing jobs:

### Resource-Based Allocation (Default)

**Used when**: `--max-parallel-jobs` is NOT specified

**Behavior**:

- Retrieves jobs from the server via the command `claim_jobs_based_on_resources`
- Server filters jobs based on available compute node resources (CPU, memory, GPU)
- Only returns jobs that fit within the current resource capacity
- Prevents resource over-subscription and ensures jobs have required resources
- Defaults to requiring one CPU and 1 MB of memory for each job.

**Use cases**:

- When you want parallelization based on one CPU per job.
- When you have heterogeneous jobs with different resource requirements and want intelligent
  resource management.

**Example 1: Run jobs at queue depth of num_cpus**:

```yaml
parameters:
  i: "1:100"
jobs:
  - name: "work_{i}"
    command: bash my_script.sh {i}
    use_parameters:
    - i
```

**Example 2: Resource-based parallelization**:

```yaml
resource_requirements:
  - name: "work_resources"
    num_cpus: 32
    memory: "200g"
    runtime: "PT4H"
    num_nodes: 1

parameters:
  i: "1:100"
jobs:
  - name: "work_{i}"
    command: bash my_script.sh {i}
    resource_requirements: work_resources
    use_parameters:
    - i
```

### Simple Queue-Based Allocation

**Used when**: `--max-parallel-jobs` is specified

**Behavior**:

- Retrieves jobs from the server via the command `claim_next_jobs`
- Server returns the next N ready jobs from the queue (up to the specified limit)
- Ignores job resource requirements completely
- Simply limits the number of concurrent jobs

**Use cases**: When all jobs have similar resource needs or when the resource bottleneck is not
tracked by Torc, such as network or storage I/O. This is the only way to run jobs at a queue depth
higher than the number of CPUs in the worker.

**Example**:

```bash
torc run $WORKFLOW_ID \
  --max-parallel-jobs 10 \
  --output-dir ./results
```

## Job Runner Workflow

A worker has three phases: a short setup phase that runs once, a per-iteration loop that drives
execution, and a teardown phase that runs once on exit. Most of the runner's complexity lives in the
loop, where each iteration polls the server, reaps subprocesses, claims new work, and checks a
handful of exit conditions.

### Worker Lifecycle

```mermaid
flowchart TD
    Start([Start worker]) --> Init[Version check<br/>+ RO-Crate provenance entities]
    Init --> StartActions[Run on_workflow_start<br/>+ on_worker_start actions]
    StartActions --> Loop[[Per-iteration loop<br/>see chart below]]
    Loop --> WorkerComplete[Run on_worker_complete actions<br/><i>skipped if offline</i>]
    WorkerComplete --> Monitor[Shutdown resource monitor<br/>+ generate plots if requested]
    Monitor --> Deactivate[deactivate_compute_node<br/><i>skipped if offline</i>]
    Deactivate --> End([End])

    style Start fill:#10b981,stroke:#059669,color:#fff
    style End fill:#ef4444,stroke:#dc2626,color:#fff
    style Loop fill:#8b5cf6,stroke:#7c3aed,color:#fff
    style Init fill:#3b82f6,stroke:#2563eb,color:#fff
    style StartActions fill:#3b82f6,stroke:#2563eb,color:#fff
    style WorkerComplete fill:#3b82f6,stroke:#2563eb,color:#fff
    style Monitor fill:#3b82f6,stroke:#2563eb,color:#fff
    style Deactivate fill:#3b82f6,stroke:#2563eb,color:#fff
```

While the worker is in offline-drain mode, `on_worker_complete` and `deactivate_compute_node` are
both skipped — the server is unreachable, so the node is left active and the journal is replayed via
`torc workflows reconcile` once connectivity returns.

### Per-Iteration Logic

```mermaid
flowchart TD
    Iter([Loop iteration]) --> OfflineCheck{Offline?}
    OfflineCheck -->|Yes| TryResume[maybe_try_resume<br/>throttled to drain_ping_interval]
    OfflineCheck -->|No| CheckStatus[Check workflow status]
    TryResume --> ReapJobs
    CheckStatus --> IsComplete{Workflow complete<br/>or canceled?}
    IsComplete -->|Yes| WorkflowDone[Run on_workflow_complete actions]
    WorkflowDone --> Exit([Exit loop])
    IsComplete -->|No| ReapJobs[check_job_status<br/>reap completions, free resources]
    ReapJobs --> OomCheck{Direct mode +<br/>resource limits?}
    OomCheck -->|Yes| OomHandler[handle_oom_violations<br/>SIGKILL offenders,<br/>report as failed]
    OomCheck -->|No| DrainDone{Offline AND<br/>no running jobs?}
    OomHandler --> DrainDone
    DrainDone -->|Yes| DrainExit[Log reconcile hint]
    DrainExit --> Exit
    DrainDone -->|No| OnlineForClaim{Online?}
    OnlineForClaim -->|No| Wait
    OnlineForClaim -->|Yes| ExecActions[check_and_execute_actions<br/>on_jobs_ready, on_jobs_complete]
    ExecActions --> ClaimBranch{Allocation strategy?}
    ClaimBranch -->|Resource-based| ClaimResources[claim_jobs_based_on_resources<br/>per-node CPU/memory/GPU tracking]
    ClaimBranch -->|Queue-based| MinTimeGate{Remaining time ≥<br/>compute_node_min_time?}
    MinTimeGate -->|Yes| ClaimQueue[claim_next_jobs<br/>up to max-parallel-jobs]
    MinTimeGate -->|No| Wait
    ClaimResources --> StartJobs[Start claimed jobs<br/>start_job + AsyncCliCommand<br/>+ GPU round-robin assignment]
    ClaimQueue --> StartJobs
    StartJobs --> Wait[Wait: adaptive backoff with<br/>SIGCHLD-aware wakeup;<br/>reset to base on progress,<br/>pinned to base while offline]
    Wait --> Sigterm{SIGTERM<br/>received?}
    Sigterm -->|Yes| TermJobs[terminate_jobs:<br/>SIGTERM + lead, then SIGKILL]
    TermJobs --> Exit
    Sigterm -->|No| Deadline{end_time reached?<br/>direct mode applies a<br/>pre-deadline window}
    Deadline -->|Yes| TermJobs
    Deadline -->|No| IdleExit{Idle past threshold<br/>and no pending actions?}
    IdleExit -->|Yes| Exit
    IdleExit -->|No| Iter

    style Iter fill:#10b981,stroke:#059669,color:#fff
    style Exit fill:#ef4444,stroke:#dc2626,color:#fff
    style OfflineCheck fill:#f59e0b,stroke:#d97706,color:#fff
    style IsComplete fill:#f59e0b,stroke:#d97706,color:#fff
    style OomCheck fill:#f59e0b,stroke:#d97706,color:#fff
    style DrainDone fill:#f59e0b,stroke:#d97706,color:#fff
    style OnlineForClaim fill:#f59e0b,stroke:#d97706,color:#fff
    style ClaimBranch fill:#f59e0b,stroke:#d97706,color:#fff
    style MinTimeGate fill:#f59e0b,stroke:#d97706,color:#fff
    style Sigterm fill:#f59e0b,stroke:#d97706,color:#fff
    style Deadline fill:#f59e0b,stroke:#d97706,color:#fff
    style IdleExit fill:#f59e0b,stroke:#d97706,color:#fff
    style TryResume fill:#3b82f6,stroke:#2563eb,color:#fff
    style CheckStatus fill:#3b82f6,stroke:#2563eb,color:#fff
    style ReapJobs fill:#3b82f6,stroke:#2563eb,color:#fff
    style OomHandler fill:#3b82f6,stroke:#2563eb,color:#fff
    style WorkflowDone fill:#3b82f6,stroke:#2563eb,color:#fff
    style DrainExit fill:#3b82f6,stroke:#2563eb,color:#fff
    style ExecActions fill:#3b82f6,stroke:#2563eb,color:#fff
    style StartJobs fill:#3b82f6,stroke:#2563eb,color:#fff
    style TermJobs fill:#3b82f6,stroke:#2563eb,color:#fff
    style Wait fill:#6b7280,stroke:#4b5563,color:#fff
    style ClaimResources fill:#8b5cf6,stroke:#7c3aed,color:#fff
    style ClaimQueue fill:#ec4899,stroke:#db2777,color:#fff
```

Each iteration runs the following steps and either continues or exits via one of the termination
branches above:

1. **Resume probe** (offline only) — `maybe_try_resume` pings the server at most once per
   `drain_ping_interval`. If it responds, flush the journal and return to normal operation.
2. **Workflow status check** (online only) — exit on `is_complete` or `is_canceled`. If the server
   is unreachable past the retry window, enter offline-drain mode (see below).
3. **Reap completions** — `check_job_status` collects exit codes, reports completions to the server
   (or to the local journal while offline), and frees per-node resources.
4. **OOM enforcement** (direct mode with resource limits) — kill jobs that exceeded their memory
   reservation and report them as failed.
5. **Drain-complete exit** — in offline mode, once all running jobs have finished, log the reconcile
   hint and exit; the journal will be replayed once the server is back.
6. **Workflow actions** (online only) — execute pending `on_jobs_ready` and `on_jobs_complete`
   actions (e.g., schedule new Slurm allocations).
7. **Claim new jobs** (online only) — resource-based mode filters by per-node CPU/memory/GPU
   capacity; queue-based mode claims up to `--max-parallel-jobs`, but only if remaining time exceeds
   `compute_node_min_time_for_new_jobs_seconds`.
8. **Start jobs** — call `start_job`, assign a GPU device if needed, then spawn the command via
   `AsyncCliCommand`, recording stdout/stderr to per-job log files.
9. **Wait** — sleep with `SIGCHLD`-aware wakeup so subprocess exits are observed promptly. Adaptive
   backoff doubles the interval toward `claim_backoff_max_secs` when an iteration produces no
   progress; any completion, claim, or early wakeup resets it to the base interval. Pinned to the
   base interval while offline so the drain-ping cadence is preserved.
10. **Exit checks** — break out on `SIGTERM`, on the end-of-allocation deadline (direct mode applies
    a pre-deadline window so `SIGTERM` has time to land before `SIGKILL`), or after an idle interval
    past `compute_node_wait_for_new_jobs_seconds` with no pending actions that could add capacity.

## Surviving Server Outages (Offline Drain)

Job runners need the server to claim work, report completions, and unblock dependents. But a running
job is an ordinary subprocess on the compute node — it does not need the server to keep running.
Killing in-flight jobs because the server is briefly unreachable would throw away expensive compute
(imagine a 5-day job killed on the 4th night by a server outage that can be repaired the next day).

To avoid this, every API call first retries for up to
`compute_node_wait_for_healthy_database_minutes` (default 20). If the server is still unreachable
after that window, the runner enters **offline-drain mode** instead of exiting:

1. **Stop claiming new jobs.** Claiming requires a server-side write lock that prevents two nodes
   from grabbing the same job, so no new work can start while the server is down.
2. **Let running jobs finish.** The runner keeps monitoring its subprocesses to completion.
3. **Journal results locally.** Each completion is written to a per-node SQLite file under
   `<output_dir>/offline_journal/`, named `offline_results_wf<workflow_id>_r<run_id>_<label>.db`.
4. **Watch for recovery.** The runner pings the server every `drain_ping_interval_secs` (default
   120). If the server comes back **while jobs are still running**, the runner flushes the journal,
   brings the server's view up to date, and resumes normal operation — claiming new work again with
   no lost results.
5. **Exit when drained.** If all running jobs finish while the server is still down, the runner
   writes its final results to the journal and exits.

This behavior is on by default. It can be tuned (or disabled in favor of the legacy kill-and-exit
behavior) via the [`[client.offline]`](../reference/configuration.md) configuration section.

```mermaid
flowchart TD
    Down{Server unreachable<br/>past retry window?} -->|No| Normal[Normal loop]
    Down -->|Yes| Drain[Offline drain:<br/>stop claiming, journal completions]
    Drain --> Ping{Server back<br/>while jobs running?}
    Ping -->|Yes| Flush[Flush journal,<br/>resume normal loop]
    Ping -->|No| AllDone{All running<br/>jobs finished?}
    AllDone -->|No| Drain
    AllDone -->|Yes| Exit([Write journal, exit])
    Flush --> Normal

    style Down fill:#f59e0b,stroke:#d97706,color:#fff
    style Ping fill:#f59e0b,stroke:#d97706,color:#fff
    style AllDone fill:#f59e0b,stroke:#d97706,color:#fff
    style Drain fill:#3b82f6,stroke:#2563eb,color:#fff
    style Flush fill:#3b82f6,stroke:#2563eb,color:#fff
    style Normal fill:#10b981,stroke:#059669,color:#fff
    style Exit fill:#ef4444,stroke:#dc2626,color:#fff
```

### Reconciling Journals After an Outage

When runners exit while the server is down (for example, a long outage where every node finishes its
work before the server returns), replay their journals once the server is healthy:

```bash
# Reconcile every node's journal for workflow 42, run 1, found under the current directory
torc workflows reconcile 42 1

# Point at the shared output root used by all compute nodes
torc workflows reconcile 42 1 --base-dir /scratch/run42
```

`torc workflows reconcile` discovers **all** journal files for that `(workflow_id, run_id)` beneath
the base directory — so a 1000-node run is recovered with one command, not one per node — and
replays the completions to the server in bulk. The `run_id` is shown by `torc workflows status 42`
and encoded in each journal's file name.

Replay is idempotent and safe. The server validates each completion's `run_id` against the
workflow's current generation, so completions from a superseded run (for example, after a manual
restart) are rejected rather than applied; `torc workflows reconcile` reports these as rejected and
exits without error.

## Resource Management (Resource-Based Allocation Only)

When using resource-based allocation (default), the local job runner tracks:

- Number of CPUs in use
- Memory allocated to running jobs
- GPUs in use
- Job runtime limits

When a ready job is retrieved, the runner checks if sufficient resources are available before
executing it.
