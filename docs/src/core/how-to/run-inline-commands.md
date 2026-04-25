# How to Run Inline Commands with `torc exec`

`torc exec` turns an ad-hoc shell command (or a batch of them) into a torc workflow in one step — no
spec file, no long-running server required. It's the fastest way to benefit from torc's per-job
CPU/memory monitoring and its parallel job queue.

Two common use cases:

1. **Monitor CPU and memory** for one or more commands you'd normally run directly.
2. **Run a queue of commands in parallel**, capped at N at a time (a torc-native alternative to GNU
   Parallel).

Pair it with `-s`/`--standalone` and you don't need a running torc server — torc spawns a
short-lived local server, stores the workflow in `./torc_output/torc.db`, and shuts the server down
when the command exits. The workflow persists and can be inspected afterwards.

## Monitor a Single Command

```console
torc -s exec -c 'bash long_script.sh'
```

For a single command, you can also use shell-style syntax. Everything after `--` is treated as one
command:

```console
torc -s exec -- bash long_script.sh --flag value
```

While the job runs, torc samples CPU and memory usage. On completion it prints a summary and stores
the metrics so you can review them later:

```console
torc -s results list
```

For higher-resolution data and plots, ask for time-series sampling:

```console
torc -s exec -c 'bash long_script.sh' --monitor time-series --generate-plots
```

This produces an HTML plot of CPU and memory over time under `torc_output/resource_utilization/`.

## Run Many Commands in Parallel

Supply multiple `-c` flags (or a file) and a parallelism cap with `-j`:

```console
torc -s exec \
  -c 'bash process.sh sample1' \
  -c 'bash process.sh sample2' \
  -c 'bash process.sh sample3' \
  -j 2
```

Or load the commands from a file (one per line, `#` comments and blank lines ignored):

```console
torc -s exec -C commands.txt -j 4
```

Stdin also works — pipe a list from another tool:

```console
ls *.fastq | sed 's|^|bash align.sh |' | torc -s exec -C - -j 8
```

## Parameterize a Template

Use torc's standard `{name}` substitution via repeatable `--param NAME=VALUE` flags. Values can be
integer or float ranges, lists, a literal, or `@file.txt` to read one value per line.

### Cartesian product (default)

Every combination of every parameter:

```console
torc -s exec -c 'python train.py --lr {lr} --bs {bs}' \
  --param lr='[0.001,0.01,0.1]' \
  --param bs='[32,64,128]' \
  -j 4
```

That launches 3 × 3 = 9 jobs.

### Zipped (element-wise)

Parameters advance together — useful for paired inputs/outputs of the same length:

```console
torc -s exec -c 'curl -o {out} {url}' \
  --param url=@urls.txt \
  --param out=@outfiles.txt \
  --link zip
```

With 5 URLs and 5 output paths this produces exactly 5 jobs.

## Preview Before Running

Use `--dry-run` to print the expanded workflow spec without creating a workflow or starting a
server:

```console
torc exec --dry-run -c 'python train.py --lr {lr}' --param lr='[0.001,0.01,0.1]'
```

This is useful before launching a large sweep.

## Inspect Results Later

Because the workflow is persisted, you can come back to it:

```console
torc -s results list                # most recent run's jobs + metrics
torc -s workflows list              # every workflow in the standalone DB
torc -s jobs list <workflow_id>     # job statuses for a specific workflow
torc tui --standalone               # interactive browser
```

Give the workflow a friendly name up front to make it easier to find:

```console
torc -s exec -n hparam-sweep --description "LR sweep over ResNet50" \
  -c 'python train.py --lr {lr}' --param lr='[0.001,0.01,0.1]'
```

## Key Flags

| Flag                            | Default       | Purpose                                                  |
| ------------------------------- | ------------- | -------------------------------------------------------- |
| `-c, --command <CMD>`           | —             | Command to execute (repeatable)                          |
| `-C, --commands-file <FILE>`    | —             | Read commands from file; `-` reads stdin                 |
| `--param NAME=VALUE`            | —             | Template parameter (repeatable)                          |
| `--link product\|zip`           | `product`     | How to combine multiple params                           |
| `-j, --max-parallel-jobs N`     | unlimited     | Cap concurrent jobs                                      |
| `-n, --name <NAME>`             | `exec_<ts>`   | Workflow name                                            |
| `--description <TEXT>`          | —             | Workflow description                                     |
| `--dry-run`                     | off           | Print the expanded workflow spec without running         |
| `--monitor <MODE>`              | `summary`     | Per-job monitoring: `off`, `summary`, `time-series`      |
| `--monitor-compute-node <MODE>` | `off`         | Node-wide monitoring                                     |
| `--generate-plots`              | off           | Render HTML plots (requires time-series)                 |
| `-i, --sample-interval-seconds` | `10`          | Resource sample interval                                 |
| `--stdio <MODE>`                | spec default  | `separate`, `combined`, `no-stdout`, `no-stderr`, `none` |
| `-o, --output-dir <DIR>`        | `torc_output` | Where logs and metrics are written                       |

For the full reference, run `torc exec --help`.

## Running on HPC Compute Nodes (In-Memory Mode)

On HPC login or compute nodes where the shared filesystem (Lustre, GPFS, NFS) is intermittently
slow, the standalone server's writes to `./torc_output/torc.db` can stall request handlers and slow
the workflow as a whole. Add `--in-memory` to keep the database entirely in RAM and snapshot to disk
only at the end:

```console
torc -s --in-memory exec -C commands.txt -j 32
```

The behavior is identical from the user's perspective — `torc_output/torc.db` is created and
`torc -s results list` works as usual — but during the run, no SQLite traffic touches the shared
filesystem.

For longer-running workflows where losing recent state on parent-process death would be costly, add
periodic snapshots:

```console
torc -s --in-memory --snapshot-interval-seconds 600 exec -C commands.txt -j 32
```

See
[In-Memory Database with Snapshots](../../specialized/admin/server-deployment.md#in-memory-database-with-snapshots-advanced)
for the full reference.

## When to Use `torc run` Instead

`torc exec` is designed for ad-hoc commands. If you already have a workflow spec file (with
dependencies, resource requirements, failure handlers, etc.), use
[`torc run`](../../getting-started/quick-start-local.md) — it's the same engine, but the spec file
gives you the full workflow model.

Tip: if you pass a spec file to `exec` by mistake (e.g. `torc exec workflow.yaml`), torc detects it
and suggests `torc run workflow.yaml`.

## See Also

- [Resource Monitoring](../monitoring/resource-monitoring.md) — configuration details for the
  underlying `resource_monitor` spec
- [View Resource Plots](./view-resource-plots.md) — what `--generate-plots` produces
- [Job Parameterization](../reference/parameterization.md) — `{name}` substitution syntax
