# Remote workers

Run a workflow across SSH-reachable machines with no scheduler. Workers are ordinary Torc job
runners started over SSH; the server still coordinates job claiming.

## Contents

- [When to use this mode](#when-to-use-this-mode)
- [Worker file format](#worker-file-format)
- [Lifecycle](#lifecycle)
- [Reachability of the server](#reachability-of-the-server)
- [Platform differences](#platform-differences)
- [Failure signatures](#failure-signatures)

## When to use this mode

Choose remote workers for ad-hoc clusters of workstations or cloud VMs, environments with no
scheduler, and rehearsing a distributed run before moving it to Slurm. Use Slurm when a scheduler
exists: it handles queueing, accounting, and preemption that this mode does not.

## Worker file format

One host per line, `[user@]hostname[:port]`, with `#` comments. Each host may appear once;
duplicates are an error.

```text
worker1.example.com
alice@worker2.example.com
admin@192.168.1.10:2222
10.0.0.5
[2001:db8::1]
[::1]:2222
```

SSH runs with `ConnectTimeout=30`, `BatchMode=yes` (key auth only, no password prompts), and
`StrictHostKeyChecking=accept-new`. For anything else (custom identity file, jump host, non-default
user), define a `Host` alias in `~/.ssh/config` and list the alias in the worker file.

## Lifecycle

Workers are stored in the database against the workflow, so they are configured once and reused.

```bash
torc remote add-workers <id> worker1 alice@worker2 admin@10.0.0.5:2222
torc remote add-workers-from-file workers.txt <id>
torc remote list-workers <id>
torc remote remove-worker worker1 <id>

torc remote run <id>                        # start detached workers on every stored host
torc remote run <id> --workers workers.txt  # add hosts and start in one step
torc remote status <id>                     # which workers are still running
torc remote stop <id>                       # SIGTERM; --force sends SIGKILL
torc remote collect-logs <id> -l ./logs     # pull logs locally; --delete removes them remotely
torc remote delete-logs <id>                # remove remote output dir without collecting
```

Useful `run` options: `-o/--output-dir` (remote output directory, default `torc_output`),
`--max-parallel-jobs`, `--num-cpus`, `--memory-gb`, `--num-gpus` (all auto-detected per host when
omitted), `-p/--poll-interval`, and `--max-parallel-ssh` (default 10, shared by the other
subcommands).

`torc remote run` starts workers detached so they survive SSH disconnection, then returns. It does
not block until completion. Monitor with `torc remote status`, `torc status`, or `torc watch`.

Workers exit when the workflow completes or is canceled.

## Reachability of the server

Every worker talks HTTP to the Torc server directly. The `--url` / `TORC_API_URL` value must resolve
and be reachable **from the workers' network**, not just from the machine issuing the commands. A
`localhost` URL works only when the server runs on that same host, so a standalone server on a
laptop cannot serve remote workers.

Before debugging job payloads, confirm:

1. The server is reachable from a worker: `ssh worker1 'curl -sf "$TORC_API_URL/ping"'` or an
   equivalent probe with the explicit URL.
2. Every worker has the same `torc` version as the client. `torc remote run` verifies this and fails
   with a per-host report. `--skip-version-check` is a diagnosis aid, not a fix.
3. `torc` is on each host's `PATH` for the non-interactive SSH session.

## Platform differences

Torc probes each host once and adapts. POSIX hosts (Linux, macOS, BSD) are driven through `bash`
(`nohup ... & disown`, `pgrep`, `kill`, `tar`) so `bash` must be on `PATH`. Windows hosts are driven
through PowerShell `-EncodedCommand` payloads (`Win32_Process.Create`, `Get-Process`, `taskkill`,
`tar`), which works whether the OpenSSH default shell is `cmd.exe` or PowerShell.

Windows caveats:

- `torc remote stop` is always a forced stop on Windows; there is no SIGTERM equivalent for a
  detached process. Design for checkpoint/restart instead of graceful shutdown.
- `collect-logs` needs `tar.exe` (built into Windows 10 1803+).
- Keep job commands portable: forward-slash paths, tools that exist on the target OS, no Bash-only
  heredocs or cleanup.

## Failure signatures

| Message                                       | Cause and fix                                                                    |
| --------------------------------------------- | -------------------------------------------------------------------------------- |
| `No workers configured for workflow <id>`     | Run `add-workers` or pass `--workers`                                            |
| `Version mismatch: local=X, worker=Y`         | Install matching Torc on every host                                              |
| `SSH connection failed ... Permission denied` | Key auth is not set up; verify with `ssh <host> true`                            |
| `Process died immediately. Last log: ...`     | Worker could not reach the server: wrong `--url`, firewall, or server down       |
| Workers start but claim nothing               | Workflow not initialized, no ready jobs, or requirements exceed worker resources |
| `Could not determine the remote shell`        | Host is neither POSIX nor PowerShell-capable                                     |

For the "claim nothing" case, check in this order: `torc status <id>` (is it initialized, are jobs
ready), `torc jobs list <id> -s ready`, then whether each ready job's resource requirements fit the
per-worker CPU/memory/GPU values.
