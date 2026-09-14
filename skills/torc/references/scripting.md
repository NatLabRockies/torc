# Scripting Torc output

## Contents

- [JSON envelope](#json-envelope)
- [jq patterns](#jq-patterns)
- [Nushell patterns](#nushell-patterns)
- [CSV export](#csv-export)
- [Wait loops](#wait-loops)
- [Pitfalls](#pitfalls)

## JSON envelope

`-f json` shapes differ by command family:

- **List commands** return an object with an `items` array: `jobs list`, `results list`,
  `workflows list`, `files list`, `events list`, and so on. Always index through `.items`.
- **Single-record and report commands** return a flat object: `status`, `workflows get`, `jobs get`,
  `workflows is-complete`, `workflows diagnose`.
- **`results list --include-logs`** returns a report object with workflow fields plus `items`.

Field names to expect:

```text
results[]  id, job_id, job_name, workflow_id, run_id, attempt_id, return_code, status,
           completion_time, exec_time_minutes, compute_node_id,
           peak_memory_bytes, avg_memory_bytes, peak_cpu_percent, avg_cpu_percent
jobs[]     id, name, command, status, priority, workflow_id, attempt_id,
           resource_requirements_id, failure_handler_id, scheduler_id, invocation_script,
           cancel_on_blocking_job_failure, supports_termination
status     jobs_by_status{...}, is_complete, is_canceled, active_compute_nodes,
           active_scheduled_nodes, pending_scheduled_nodes, runtime_blocked_ready_jobs,
           total_exec_time_formatted
```

JSON `status` values are lowercase (`failed`, `completed`); the table renderer capitalizes them
(`Failed`, `Completed`) and `--include-logs` reports capitalized names too. Match on the value from
the exact command you are parsing, not on what another view displayed.

Memory in JSON is bytes; the table pre-formats it as `MB`. CPU is a percentage that can exceed 100
for multi-threaded jobs.

## jq patterns

```bash
# Failure count from the summary
torc -f json status "$WF" | jq '.jobs_by_status.failed'

# Failed job names and exit codes
torc -f json results list "$WF" --failed \
  | jq -r '.items[] | "\(.job_id)\t\(.job_name)\texit=\(.return_code)"'

# Distribution of exit codes
torc -f json results list "$WF" | jq '[.items[].return_code] | group_by(.) | map({code: .[0], n: length})'

# Slowest ten jobs
torc -f json results list "$WF" \
  | jq -r '.items | sort_by(-.exec_time_minutes)[:10][] | "\(.exec_time_minutes)\t\(.job_name)"'

# Peak memory in GB, descending
torc -f json results list "$WF" \
  | jq -r '.items | sort_by(-.peak_memory_bytes)[] | "\(.peak_memory_bytes/1073741824 | .*100|round/100)\t\(.job_name)"'

# The command to reproduce a failure
torc -f json jobs get "$JOB" | jq -r '.command'

# stderr path for a named job
torc -f json results list "$WF" --include-logs -o "$OUT" \
  | jq -r --arg n my_job '.items[] | select(.job_name == $n) | .job_stderr'
```

## Nushell patterns

Nushell is shipped-in-docs alternative to `jq` and reads better for interactive filtering:

```nu
torc -f json jobs list 123 | from json | get items | where status == "failed"
torc -f json results list 123 | from json | get items | sort-by exec_time_minutes --reverse | first 10
torc -f json results list 123 | from json | get items | group-by return_code | transpose code rows
```

## CSV export

`-f csv` works for list commands and is the fastest path to a spreadsheet:

```bash
torc -f csv results list "$WF" > results.csv
torc -f csv jobs list "$WF" -x command -x priority > jobs.csv
```

`-x/--exclude` drops columns (repeatable, case-insensitive) and is available on `jobs list`; other
list commands do not take it. Single-record commands and multi-section reports reject `csv` with
exit 1; use `-f json` for those.

## Wait loops

`torc watch <id>` already polls and exits non-zero on failure, so prefer it over a hand-rolled loop.
When a custom loop is genuinely needed:

```bash
until torc -f json workflows is-complete "$WF" | jq -e '.is_complete' >/dev/null; do
  sleep 30
done

failed=$(torc -f json status "$WF" | jq '.jobs_by_status.failed + .jobs_by_status.terminated')
if [ "$failed" -gt 0 ]; then
  torc -f json results list "$WF" --failed | jq -r '.items[] | "\(.job_name) exit=\(.return_code)"'
  exit 1
fi
```

Two details this handles that a naive loop does not: `is-complete` is the cheap predicate, and
`torc run` exiting 0 does not mean the jobs succeeded, so the failure check must come from server
state.

## Pitfalls

| Pitfall                                    | Consequence                                                                  |
| ------------------------------------------ | ---------------------------------------------------------------------------- |
| Omitting the workflow ID                   | A selection table is printed on stdout, breaking JSON parsing; exit 1 on EOF |
| Assuming `results list` covers all runs    | Only the current `run_id` is returned; add `--all-runs`                      |
| Trusting `torc run`'s exit status          | It exits 0 with failed jobs; check `status` or `results --failed`            |
| Matching capitalized statuses against JSON | JSON uses lowercase; the table capitalizes                                   |
| Parsing runner output from `torc run`      | Runner logs go to stdout in table mode; use `-f json` to move them to stderr |
| Wrong `-o` with `--include-logs`           | Log paths resolve but the files do not exist; warnings go to stderr          |
| Fetching everything then filtering locally | Extra round trips; use server-side filters                                   |
