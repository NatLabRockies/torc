# Log map

Every path below is relative to the output directory passed to the run (`-o`/`--output-dir`, default
`torc_output`). Passing the wrong directory to a log-reading command is the most common cause of
"logs are missing".

## Contents

- [Finding paths without guessing](#finding-paths-without-guessing)
- [Filename formats](#filename-formats)
- [Which file answers which question](#which-file-answers-which-question)
- [Grepping structured log lines](#grepping-structured-log-lines)
- [Bundling and pattern analysis](#bundling-and-pattern-analysis)
- [When a log does not exist](#when-a-log-does-not-exist)

## Finding paths without guessing

```bash
torc results list <id> --failed --include-logs -o <output-dir> > report.json
jq -r '.items[] | "\(.job_name)\t\(.job_stderr)"' report.json
```

The report resolves every log path for each result and warns on stderr about files it cannot find.
That is faster and safer than assembling filenames by hand, since the names embed run and attempt
IDs.

Both the local and Slurm cases are covered: Slurm results additionally carry `slurm_stdout`,
`slurm_stderr`, `slurm_env_log`, and `dmesg_log`.

## Filename formats

```text
<output-dir>/
  job_stdio/
    job_wf<wf>_j<job>_r<run>_a<attempt>.o        # stdout (separate mode)
    job_wf<wf>_j<job>_r<run>_a<attempt>.e        # stderr (separate mode)
    job_wf<wf>_j<job>_r<run>_a<attempt>.log      # combined mode
  job_runner_<hostname>_wf<wf>_r<run>.log        # local runner
  job_runner_slurm_wf<wf>_sl<slurm>_n<node>_pid<pid>.log
  watch_<hostname>_wf<wf>.log                    # torc watch
  slurm_output_wf<wf>_sl<slurm>.o
  slurm_output_wf<wf>_sl<slurm>.e
  slurm_env_wf<wf>_sl<slurm>_n<node>_pid<pid>.log
  dmesg_slurm_wf<wf>_sl<slurm>_n<node>_pid<pid>.log
  resource_utilization/
    resource_metrics_wf<wf>_h<hostname>_r<run>.db          # local runner
    resource_metrics_wf<wf>_sl<slurm>_n<node>_p<pid>.db    # Slurm runner
  offline_journal/
    offline_results_wf<wf>_r<run>_<label>.db
```

Four identifiers appear throughout: `wf` workflow, `j` job, `r` run (bumped by
`torc workflows reinit`), `a` attempt (bumped by failure-handler retries). Slurm files use the Slurm
job ID, node ID, and task PID instead of job and attempt, so correlate them through the runner log
or `torc slurm sacct`.

## Which file answers which question

| Question                                           | File                                           |
| -------------------------------------------------- | ---------------------------------------------- |
| What did my program print or raise?                | job `.e` first, then `.o` (or `.log` combined) |
| Did Torc start and finish the job, and when?       | job runner log                                 |
| Why did the runner claim or skip jobs?             | job runner log                                 |
| Did the allocation itself fail?                    | `slurm_output_*.e`                             |
| What Slurm environment did the runner see?         | `slurm_env_*.log`                              |
| Was the kernel involved (OOM killer, hardware)?    | `dmesg_slurm_*.log` (written on failure)       |
| How much CPU and memory did the job actually use?  | resource metrics DB, or `torc results list`    |
| Which completions were journaled during an outage? | `offline_journal/*.db`                         |

`slurm_env_*.log` files are excluded from error analysis automatically; they are configuration
dumps, not error logs.

## Grepping structured log lines

Torc log messages use `key=value` pairs, so plain `grep` is effective:

```bash
grep -r "workflow_id=123" <output-dir>/
grep -r "job_id=456" <output-dir>/ -C 2
grep -rE "workflow_id=123.*job_id=456" <output-dir>/
grep -r "job_id=456" <output-dir>/ | grep "attempt_id="
grep -r "compute_node_id=789" <output-dir>/
```

Lifecycle lines worth knowing:

```bash
grep -r "Job started workflow_id=" <output-dir>/
grep -r "Job completed workflow_id=" <output-dir>/
grep -r "Job completed workflow_id=" <output-dir>/ | grep -v "return_code=0 "
```

The runner also logs a startup line with the client and server versions, API versions, detected
resources, compute-node rules, poll interval, `max_parallel_jobs`, and execution mode. It is the
fastest way to confirm what a runner actually believed about its environment.

## Bundling and pattern analysis

```bash
torc logs bundle <id> -o <output-dir> --bundle-dir ./bundles
torc logs analyze ./bundles/wf<id>.tar.gz
torc logs analyze <output-dir> --workflow-id <id>
```

`bundle` collects job stdio, runner logs, Slurm outputs, Slurm environment logs, dmesg logs, and
bundle metadata into `wf<id>.tar.gz`. `analyze` accepts either the tarball or a directory;
`--workflow-id` is required when a directory holds several workflows.

`analyze` matches a fixed set of regex patterns, reporting the file, line, severity, and type for
each hit: Missing Output Files, Slurm Error (`slurmstepd`, `CANCELLED`, `TIMEOUT`, `OUT_OF_MEMORY`),
OOM Killed, Timeout, Segmentation Fault, Permission Denied, File Not Found, Disk Full, Connection
Error, Rust Panic, Python Exception, and a catch-all Generic Error reported at warning severity.
`INFO` lines are ignored for the Slurm Error and Generic Error patterns to cut false positives.

`torc slurm parse-logs --workflow-id <id> <output-dir>` does the same for Slurm stdout/stderr and
correlates hits back to affected Torc jobs.

Treat both as pattern matchers. "No errors detected" means no known pattern matched, not that the
job was fine.

## When a log does not exist

Work through these in order:

1. **Wrong output directory.** Confirm with
   `find . -name 'job_wf*_j*_r*.o' -o -name 'job_runner_*.log'`, then pass that directory as `-o`.
2. **Wrong run.** `run_id` in the filename comes from the run that produced it; a `reinit` since
   then moved you forward. Use `torc results list <id> --all-runs`.
3. **Configured away.** `execution_config.stdio` (or a per-job `stdio`) with `no_stdout`,
   `no_stderr`, or `none` suppresses files, and `delete_on_success: true` removes them for jobs that
   exited 0.
4. **The job never started.** A `canceled` job produces no stdio at all; check status before hunting
   for files.
5. **Different machine.** On Slurm or remote workers the logs are written where the job ran. Use a
   shared filesystem for `-o`, or collect them: `torc remote collect-logs <id> -l ./logs`.
