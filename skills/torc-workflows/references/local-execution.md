# Local execution

## Contents

- [Standalone versus shared server](#standalone-versus-shared-server)
- [torc run](#torc-run)
- [torc exec](#torc-exec)
- [Resource-based parallelism](#resource-based-parallelism)
- [Output directory layout](#output-directory-layout)

## Standalone versus shared server

Every Torc command needs a server. There are three ways to get one.

| Approach                           | Command                           | Server lifetime    |
| ---------------------------------- | --------------------------------- | ------------------ |
| Long-running server                | `torc-server run` then `torc ...` | Until stopped      |
| Ephemeral standalone               | `torc -s ...`                     | The single command |
| Ephemeral standalone, RAM database | `torc -s --in-memory ...`         | The single command |

`-s`/`--standalone` spawns `torc-server` on `127.0.0.1` with an auto-assigned port, points the
client at it, and shuts it down when the command exits. The database at `--db` (default
`./torc_output/torc.db`) survives, so later standalone commands against the same `--db` can inspect
the workflow:

```bash
torc -s run workflow.yaml -o out
torc -s results list 1
torc -s status 1
```

Standalone mode requires the `torc-server` binary on `PATH` or at `--torc-server-bin` /
`TORC_SERVER_BIN`. It is not built by the default `cargo install torc`; install with the
`server-bin` feature or download a release archive.

`--in-memory` keeps the database in RAM and snapshots it to `--db` just before shutdown. It is Unix
only and rejects the flag elsewhere. Use it when the output directory lives on a slow shared
filesystem (Lustre, GPFS, NFS). The trade-off is real: if the process is killed, everything since
the last snapshot is lost. `--snapshot-interval-seconds` adds periodic snapshots, which briefly
serialize against writes.

For a smoke test, combine both and point at a scratch directory:

```bash
torc -s --in-memory run workflow.yaml --max-parallel-jobs 1 -o "$(mktemp -d)"
```

## torc run

`torc run <spec-or-id>` creates the workflow when given a path (or `-` for stdin), initializes it,
and runs a local job runner in the foreground until the workflow completes or the runner's limits
are reached.

```bash
torc run workflow.yaml                     # create from spec and run
torc run 123                               # run an existing workflow
cat workflow.yaml | torc run -             # spec from stdin
torc run workflow.yaml -o /scratch/out     # output directory
torc run workflow.yaml --max-parallel-jobs 4
torc run workflow.yaml --num-cpus 8 --memory-gb 32 --num-gpus 2
torc run workflow.yaml --time-limit PT1H   # or --end-time 2026-03-14T15:00:00Z
```

Behavior worth knowing:

- **Exit status ignores job failures.** `torc run` exits 0 even when jobs failed. The final log line
  reports `had_failures=true`, and the runner log records it, but the status is not propagated.
  Verify with `torc status <id>` or `torc results list <id> --failed`.
- **Logs go to stdout.** In table format the runner writes its log lines to stdout and to the runner
  log file. With `-f json` they go to stderr instead. Redirect accordingly when capturing output.
- **The runner may wait.** After the last job finishes, the runner honors
  `compute_node_wait_for_new_jobs_seconds` before exiting, so a short workflow can appear to hang
  for up to that long. It is 90 seconds by default in local runs.
- **`--time-limit` and `--end-time`** stop the runner, not the workflow. Jobs already running are
  terminated according to `execution_config`; remaining jobs stay ready for the next runner.
- **`--skip-checks`** bypasses validation such as scheduler node requirements.

Running the same workflow ID from several machines is how Torc distributes work: each runner claims
ready jobs from the server. Nothing coordinates the output directory, so give shared-filesystem
runners the same `-o` path and let the embedded hostname keep runner logs distinct.

## torc exec

`torc exec` synthesizes a workflow from inline commands. Use it for ad-hoc monitoring or a parallel
batch when writing a spec file adds nothing.

```bash
torc -s exec -c 'bash long_script.sh'
torc -s exec -- bash long_script.sh --flag value        # everything after -- is one command
torc -s exec -c 'a' -c 'b' -c 'c' -j 2                  # parallelism cap
torc -s exec -C commands.txt -j 4                       # one command per line
ls *.fastq | sed 's|^|bash align.sh |' | torc -s exec -C - -j 8
torc -s exec -c 'run.sh {i}' --param i=1:100 -j 8       # parameter sweep
torc exec --dry-run -c 'run.sh {i}' --param i=1:3       # print the expanded spec, create nothing
```

- `--param NAME=VALUE` accepts `1:10` ranges, `[a,b,c]` lists, `@file.txt`, or a literal.
  `--link zip` pairs parameters element-wise instead of the Cartesian default.
- `--monitor summary|time-series|off` sets per-job monitoring. `time-series` is required for
  `--generate-plots`, which writes HTML under `torc_output/resource_utilization/`.
- **`torc exec` exits 1 when any job fails or is terminated.** This is the opposite of `torc run`,
  and makes `exec` the better choice inside scripts and CI.
- The synthesized workflow is a normal workflow. Inspect it afterwards with `torc -s results list`
  or `torc -s jobs list <id>`; only the standalone server was short-lived.

## Resource-based parallelism

Without `--max-parallel-jobs`, the runner packs jobs against detected CPU, memory, and GPU, honoring
each job's resource requirements. Override the detected capacity with `--num-cpus`, `--memory-gb`,
and `--num-gpus`, which is how you keep a runner from taking over a shared machine.

In direct mode with `execution_config.limit_resources` (default true), the runner also monitors
running jobs and kills those that exceed their limits, so an under-specified `memory` shows up as a
killed job rather than a swapping machine.

Defaults for these flags can live in `[client.run]` in a config file; see the `torc-config` skill.

## Output directory layout

`-o`/`--output-dir` defaults to `torc_output` (configurable via `client.run.output_dir`). A local
run produces:

```text
<output-dir>/
  job_runner_<hostname>_wf<id>_r<run>.log
  job_stdio/
    job_wf<id>_j<job>_r<run>_a<attempt>.o
    job_wf<id>_j<job>_r<run>_a<attempt>.e
  resource_utilization/        # only with --generate-plots
  offline_journal/             # only after an offline drain
```

The same path must be passed to later commands that read logs
(`torc results list --include-logs -o ...`, `torc logs bundle --output-dir ...`), otherwise they
warn that log files are missing. See the `torc-debug` skill for the full log map.
