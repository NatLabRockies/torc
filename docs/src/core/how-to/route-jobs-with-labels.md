# Route Jobs with Labels

Use job `labels` when some jobs must run on particular machines and the rest must stay off them.

This is useful when:

- One step needs software that exists only on a dedicated machine, such as a Windows VM
- Most jobs run on short-lived Slurm compute nodes, but a few must run on a long-lived server
- Several kinds of runners share one workflow and each should take only its own work

## How Labels Work

A job has an optional set of `labels`. A runner has an optional set of `--label` values. When a
runner asks for work, the server returns only jobs whose every label the runner has. A job with no
labels goes only to a runner with no labels.

| Runner started with      | Jobs it claims                                                    |
| ------------------------ | ----------------------------------------------------------------- |
| no `--label`             | jobs with no labels                                               |
| `--label windows`        | jobs labeled `[windows]`                                          |
| `--label windows,gpu`    | jobs labeled `[windows]`, `[gpu]`, or `[windows, gpu]`            |
| `--label license-server` | jobs labeled `[license-server]` (not `[license-server, windows]`) |

A job's labels are requirements: it lists everything a runner must have. A runner's labels are
capabilities: it lists everything it offers. The match is enforced by the server, so a runner cannot
claim another runner's jobs by mistake. A labeled job must still fit the runner's resources, and
`priority` still orders the jobs a runner is allowed to claim.

Labels name a particular machine or capability, not a kind of hardware. To send jobs to machines
with GPUs, set `num_gpus` in the job's resource requirements instead of using a label; runners claim
those jobs based on the GPUs they have free.

## Label Jobs in a Workflow Spec

```yaml
name: labeled_jobs

resource_requirements:
  - name: small
    num_cpus: 1
    memory: 256m
    runtime: PT5M

jobs:
  - name: simulate_{i}
    command: ./simulate.sh {i}
    resource_requirements: small
    parameters:
      i: "1:4"

  - name: postprocess
    command: postprocess.exe
    resource_requirements: small
    labels: [windows]
    depends_on_regexes:
      - "^simulate_\\d+$"

  - name: render
    command: render.exe
    resource_requirements: small
    labels: [windows, gpu]
    depends_on: [postprocess]
```

The `simulate_*` jobs have no labels, so ordinary runners take them. A runner started with
`--label windows` takes `postprocess`. Only a runner that has both `windows` and `gpu` takes
`render`. A complete example is in `examples/yaml/labeled_jobs.yaml`.

In KDL, add `labels "windows" "gpu"` inside the `job` block.

## Start a Labeled Runner

```console
torc run <workflow_id> --label windows
```

Give a runner several labels by repeating the flag or separating them with commas:

```console
torc run <workflow_id> --label windows --label gpu
torc run <workflow_id> --label windows,gpu
```

Runners started without `--label`, including the Slurm runners that `torc submit` launches, claim
only unlabeled jobs.

## Keep a Runner Alive with `--persistent`

A runner normally exits when it has been idle for the workflow's
`compute_node_wait_for_new_jobs_seconds`. That suits Slurm nodes, which should release their
allocation when there is nothing to do. It does not suit a dedicated machine whose jobs become ready
only after long-running work elsewhere.

Start that runner with `--persistent`:

```console
torc run <workflow_id> --label windows,gpu --persistent
```

A persistent runner ignores the idle timeout while the workflow still has unfinished jobs it could
claim (jobs it has every label for, or unlabeled jobs for a runner with no `--label`). Once none
remain it exits like any other runner. It also exits when the workflow is complete or canceled, or
when its `--end-time` or `--time-limit` is reached.

## See Which Runners Serve Which Labels

Each runner records the labels it was started with on its compute node:

```console
torc compute-nodes list <workflow_id>
```

The `Labels` column is empty for runners that claim unlabeled jobs.

`torc status <workflow_id>` warns when ready jobs carry a label set that no active runner covers,
and `-f json` reports the counts per label set in `unserved_ready_labels`, keyed by the comma-joined
labels:

```text
⚠ 1 ready job(s) with labels 'gpu,windows' but no active runner has all of them
```

## Watch a Workflow That Mixes Slurm and Labeled Runners

`torc watch` normally exits when there are no Slurm allocations and no ready jobs, because nothing
it manages can make progress. That is wrong while a labeled runner on another machine is still
working. Add `--persistent` so it waits for the workflow to complete:

```console
torc watch <workflow_id> --persistent --auto-schedule
```

With `--auto-schedule`, watch submits new Slurm allocations when the labeled work finishes and
unblocks unlabeled jobs. Labeled ready jobs never trigger auto-scheduling.

## Label Dynamically Spawned Jobs

Jobs added at runtime by an orchestrator accept the same field. With the Python client:

```python
SpawnJobModel(
    name="postprocess_i03",
    command="postprocess.exe 3",
    resource_requirements="small",
    labels=["windows"],
)
```

See [Dynamic Jobs](../tutorials/dynamic-jobs.md) for the orchestrator pattern.

## Check or Change Labels

```console
torc jobs get <job_id>
torc jobs update <job_id> --label windows,gpu
torc jobs update <job_id> --clear-labels
```

`--label` replaces the whole set. `--clear-labels` removes every label, so runners without `--label`
claim the job. Through the API, update the job with an empty `labels` list to clear them; omit the
field to leave them unchanged.

Each label must be non-empty with no comma and no leading or trailing whitespace. Labels are matched
exactly, including case. The server stores a job's labels sorted and without duplicates, so
`[windows, gpu]` and `[gpu, windows]` are the same set.

## Things to Watch For

- **A label set that no runner covers strands the job.** It stays `ready` and the workflow never
  completes. `torc status` warns about ready jobs whose labels no active runner has, so check it if
  a workflow stalls.
- **Slurm runners launched by Torc have no labels.** Leave the jobs meant for them unlabeled.
  Labeled jobs are left out when Torc sizes Slurm allocations (`torc slurm generate`,
  `torc slurm regenerate`, `torc watch --auto-schedule`). A job that sets both `labels` and
  `scheduler` is rejected when the spec is validated.
- **Parameters work in labels.** `labels: ["{os}"]` expands along with the job name and command.
