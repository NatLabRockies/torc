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

A persistent runner ignores the idle timeout. It exits when the workflow is complete or canceled, or
when its `--end-time` or `--time-limit` is reached.

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
```

`torc jobs update` can set or replace a label but cannot remove one.

## Things to Watch For

- **A label that no runner uses strands the job.** It stays `ready` and the workflow never
  completes. Torc does not check that a runner exists for each label, so watch for typos.
- **Slurm runners launched by Torc have no label.** Leave the jobs meant for them unlabeled.
- **`torc watch --auto-schedule` does not consider labels.** It can request Slurm allocations for
  ready labeled jobs that no Slurm runner will claim.
