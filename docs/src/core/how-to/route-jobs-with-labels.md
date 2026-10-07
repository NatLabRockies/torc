# Route Jobs with Labels

Use a job `label` when some jobs must run on particular machines and the rest must stay off them.

This is useful when:

- One step needs software that exists only on a dedicated machine, such as a Windows VM
- Most jobs run on short-lived Slurm compute nodes, but a few must run on a long-lived server
- Several kinds of runners share one workflow and each should take only its own work

## How Labels Work

A job has an optional `label`. A runner has an optional `--label`. When a runner asks for work, the
server returns only jobs whose label is identical to the runner's.

| Runner started with | Jobs it claims                     |
| ------------------- | ---------------------------------- |
| no `--label`        | jobs with no label                 |
| `--label windows`   | jobs with `label: windows`         |
| `--label gpu`       | jobs with `label: gpu` (no others) |

The match is exact and enforced by the server, so a runner cannot claim another runner's jobs by
mistake. A labeled job must still fit the runner's resources, and `priority` still orders the jobs a
runner is allowed to claim.

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
    label: windows
    depends_on_regexes:
      - "^simulate_\\d+$"
```

The `simulate_*` jobs have no label, so ordinary runners take them. Only a runner started with
`--label windows` takes `postprocess`. A complete example is in `examples/yaml/labeled_jobs.yaml`.

In KDL, add `label "windows"` inside the `job` block.

## Start a Labeled Runner

```console
torc run <workflow_id> --label windows
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
torc run <workflow_id> --label windows --persistent
```

A persistent runner ignores the idle timeout while the workflow still has unfinished jobs with its
label (or unlabeled jobs, for a runner with no `--label`). Once none remain it exits like any other
runner. It also exits when the workflow is complete or canceled, or when its `--end-time` or
`--time-limit` is reached.

## See Which Runners Serve Which Labels

Each runner records the label it was started with on its compute node:

```console
torc compute-nodes list <workflow_id>
```

The `Label` column is empty for runners that claim unlabeled jobs.

`torc status <workflow_id>` warns when ready jobs carry a label that no active runner was started
with, and `-f json` reports the counts per label in `unserved_ready_labels`:

```text
⚠ 2 ready job(s) with label 'windows' but no active runner has that label
```

## Label Dynamically Spawned Jobs

Jobs added at runtime by an orchestrator accept the same field. With the Python client:

```python
SpawnJobModel(
    name="postprocess_i03",
    command="postprocess.exe 3",
    resource_requirements="small",
    label="windows",
)
```

See [Dynamic Jobs](../tutorials/dynamic-jobs.md) for the orchestrator pattern.

## Check or Change a Label

```console
torc jobs get <job_id>
torc jobs update <job_id> --label windows
torc jobs update <job_id> --clear-label
```

`--clear-label` removes the label, so runners without `--label` claim the job. Through the API,
update the job with an empty `label` to clear it.

A label must be non-empty with no leading or trailing whitespace. Labels are matched exactly,
including case.

## Things to Watch For

- **A label that no runner uses strands the job.** It stays `ready` and the workflow never
  completes. `torc status` warns about ready jobs whose label has no active runner, so check it if a
  workflow stalls.
- **Slurm runners launched by Torc have no label.** Leave the jobs meant for them unlabeled. Labeled
  jobs are left out when Torc sizes Slurm allocations (`torc slurm generate`,
  `torc slurm regenerate`, `torc watch --auto-schedule`).
- **Parameters work in labels.** `label: "{os}"` expands along with the job name and command.
