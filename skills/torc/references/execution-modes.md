# Execution modes

Three ways to execute the same workflow. The spec is largely portable between them; what changes is
who provides the compute and who tracks it.

## Contents

- [Choosing a mode](#choosing-a-mode)
- [Local](#local)
- [Remote workers](#remote-workers)
- [Slurm](#slurm)
- [Local versus remote workers](#local-versus-remote-workers)
- [Remote workers versus Slurm](#remote-workers-versus-slurm)
- [Sizing a worker pool](#sizing-a-worker-pool)
- [Mixing modes](#mixing-modes)
- [Portability across modes](#portability-across-modes)

## Choosing a mode

| Question                                                | Answer           |
| ------------------------------------------------------- | ---------------- |
| Does a scheduler manage the machines?                   | Slurm            |
| Do you have several machines you can SSH into?          | Remote workers   |
| Is one machine enough?                                  | Local            |
| Are you validating a spec or debugging a payload?       | Local, with `-s` |
| Do you need queueing, accounting, walltime, or requeue? | Slurm            |

Escalate only when the current mode runs out of capacity. A spec that works locally usually works on
workers and Slurm with no job-level changes.

## Local

```bash
torc -s run workflow.yaml -o out           # ephemeral server, nothing to set up
torc run workflow.yaml                      # shared server, durable history
torc run 123 --num-cpus 8 --memory-gb 32    # cap what the runner claims
torc -s exec -c 'bash job.sh' -j 4          # ad-hoc batch, no spec file
```

One runner, one machine, resource-aware packing against detected CPU, memory, and GPU. Override the
detected capacity with `--num-cpus`, `--memory-gb`, and `--num-gpus` to leave headroom on a shared
workstation.

Local is the right place to validate a spec, reproduce a payload failure, and measure real resource
peaks before committing to allocations. See `local-execution.md`.

## Remote workers

```bash
torc remote add-workers <id> worker1 alice@worker2 admin@10.0.0.5:2222
torc remote run <id> -o /data/torc_output
torc remote status <id>
torc remote collect-logs <id> -l ./logs
```

Torc SSHes into each host and starts a detached `torc run`. Each worker is an ordinary runner
polling the same server, and the server prevents double-claiming. Workers survive SSH disconnection
and exit when the workflow completes or is canceled.

No scheduler, no queue, no walltime. You choose the machines; Torc does not acquire them. See
`remote-workers.md`.

## Slurm

```bash
torc slurm generate --account <acct> workflow.yaml -o gen.yaml
torc submit gen.yaml -o /scratch/$USER/torc-output
```

Torc requests allocations through `sbatch` and runs a worker inside each one. It brings queueing,
fair-share accounting, walltime enforcement, and node-packing arithmetic derived from resource
requirements. See `slurm.md` for submission and `optimization.md` for sizing.

## Local versus remote workers

They are the same runner. The differences are operational.

| Aspect              | Local                   | Remote workers                                        |
| ------------------- | ----------------------- | ----------------------------------------------------- |
| Concurrency         | One machine's resources | Sum across hosts                                      |
| Server URL          | `localhost` is fine     | Must be reachable **from the workers**                |
| Standalone `-s`     | Works                   | Cannot serve workers (binds `127.0.0.1`, random port) |
| Output directory    | One local path          | Per-host path; logs land on each host                 |
| Version skew        | Not possible            | Checked per host; mismatches fail the start           |
| Foreground          | Blocks until done       | Returns immediately; workers detach                   |
| Failure of one host | Whole run stops         | Others keep claiming                                  |
| Prerequisites       | None                    | Key-based SSH, `torc` on `PATH`, `bash` or PowerShell |
| Log collection      | Already local           | `torc remote collect-logs`                            |

Three consequences worth internalizing:

- **A standalone server cannot back remote workers.** `-s` binds loopback on an auto-assigned port.
  Remote workers need a real server on a routable address, and `TORC_API_URL` must include
  `/torc-service/v1` and resolve from the workers' network.
- **Output directories are per host.** Without a shared filesystem each host writes its own logs and
  its own resource-metrics database, so collect them before deleting anything.
- **Remote resource flags are uniform.** `--num-cpus`, `--memory-gb`, `--num-gpus`, and
  `--max-parallel-jobs` on `torc remote run` are passed identically to every worker. Omit them so
  each host auto-detects; for a genuinely heterogeneous pool, either omit them or start the small
  hosts in a separate invocation.

## Remote workers versus Slurm

| Aspect             | Remote workers        | Slurm                    |
| ------------------ | --------------------- | ------------------------ |
| Scheduler required | No                    | Yes                      |
| Node acquisition   | Manual, by hostname   | Automatic, by allocation |
| Walltime limits    | None                  | Enforced                 |
| Queueing/priority  | None                  | Yes                      |
| Fault tolerance    | Limited               | Full, including requeue  |
| Accounting         | None                  | `sacct`, fair-share      |
| Best for           | Ad-hoc pools, testing | Production HPC           |

Where a scheduler exists, use it: Torc's packing arithmetic, allocation planning, walltime handling,
OOM/timeout recovery, and orphan detection are all built around it. Remote workers are for cloud
VMs, workstation pools, and rehearsing distribution before moving to Slurm.

Note that runtime-based packing behaves differently. A Slurm worker knows its allocation end time
and stops claiming when remaining walltime cannot fit the next job's `runtime`; a remote or local
worker has no end time unless you pass `--time-limit` or `--end-time`, so nothing is
runtime-blocked.

## Sizing a worker pool

For remote workers, concurrency comes from what each host declares and what jobs require, so the
same per-node arithmetic applies:

```text
concurrent_jobs_per_host = max(1, min(host_cpus / job_cpus, host_mem / job_mem, host_gpus / job_gpus))
total_concurrency        = sum over hosts
```

Practical steps:

1. Measure a representative job locally with monitoring enabled and read the real peak from
   `torc results list` or `torc workflows check-resources`.
2. Declare requirements from the measurement, not from the largest host.
3. Let each host auto-detect its capacity, then confirm placement with `torc jobs running <id>`.
4. Add hosts only while the ready-job width justifies them; check with
   `torc workflows execution-plan`.

Reserve headroom on any host you also use interactively, and remember that a job whose thread count
exceeds its declared `num_cpus` will oversubscribe every host it is packed onto.

## Mixing modes

A workflow is not bound to one mode. Any number of runners can serve the same workflow ID
concurrently, and the server arbitrates claims:

```bash
# Slurm allocations doing the bulk of the work
torc submit 123

# Plus a workstation contributing to the same workflow
torc run 123 --num-cpus 8 --memory-gb 32

# Plus a queue-depth runner for a swarm of tiny jobs
torc run 123 --max-parallel-jobs 50
```

This is useful for draining a long tail, or for adding a high-memory machine that the cluster does
not have. Give runners on a shared filesystem the same `-o` path; runner log filenames embed the
hostname, so they will not collide.

When a job must only run on particular resources, bind it to a scheduler and set
`client.slurm.strict_scheduler_match = true`; otherwise an idle worker will claim it.

## Portability across modes

What travels unchanged: jobs, commands, dependencies, files, user data, parameters, and failure
handlers.

What is mode-specific:

- `slurm_schedulers`, `slurm_defaults`, and `schedule_nodes` actions are Slurm-only. A local-only
  workflow needs none of them, and `torc submit` is not usable without a `schedule_nodes` action.
- `resource_requirements` are optional locally but required for Slurm scheduler generation, and they
  are what makes packing correct in every mode.
- `execution_config.mode` selects `direct` or `slurm` execution; direct-mode fields such as
  `limit_resources` and `termination_signal` are rejected under `mode: slurm`.
- Paths must exist on whichever machine runs the job. Prefer absolute paths or
  `TORC_WORKFLOW_SUBMISSION_DIR`, and keep cross-platform specs to forward slashes and tools present
  on the target OS.
- Module loads, conda activation, and interpreter selection belong in an `invocation_script` ending
  in `exec "$@"`, not in `env`, whose values are literal strings.
