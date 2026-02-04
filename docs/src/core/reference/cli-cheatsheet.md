<div class="cheatsheet">

# Torc CLI Cheat Sheet

## Quick Start

| Command                                        | Description                                    |
| ---------------------------------------------- | ---------------------------------------------- |
| `torc workflows create <spec>`                 | Create from spec file                          |
| `torc run <spec.yaml>`                         | Create workflow from spec and run locally      |
| `torc submit <spec.yaml>`                      | Create and submit to scheduler (needs actions) |
| `torc submit-slurm --account ACCT <spec.yaml>` | Auto-generate Slurm schedulers and submit      |
| `torc watch <id>`                              | Monitor workflow until completion              |
| `torc watch <id> --recover`                    | Monitor and auto-recover from failures         |
| `torc-dash`                                    | Launch web dashboard                           |
| `torc tui`                                     | Launch interactive terminal UI                 |

## Managing Workflows

| Command                      | Description                    |
| ---------------------------- | ------------------------------ |
| `torc workflows list`        | List your workflows            |
| `torc workflows status <id>` | Get job counts by status       |
| `torc workflows get <id>`    | Get workflow details           |
| `torc workflows cancel <id>` | Cancel workflow and Slurm jobs |
| `torc workflows delete <id>` | Delete workflow                |

## Job Management

| Command                              | Description         |
| ------------------------------------ | ------------------- |
| `torc jobs list <wf_id>`             | List all jobs       |
| `torc jobs list -s ready <wf_id>`    | List jobs by status |
| `torc jobs get <job_id>`             | Get job details     |
| `torc results list <wf_id>`          | List job results    |
| `torc results list --failed <wf_id>` | List failed jobs    |

## Recovery & Diagnostics

| Command                                        | Description                                   |
| ---------------------------------------------- | --------------------------------------------- |
| `torc recover <id>`                            | One-shot recovery (diagnose + fix + resubmit) |
| `torc watch <id> --recover --auto-schedule`    | Full production recovery mode                 |
| `torc workflows sync-status <id>`              | Fix orphaned jobs (stuck in "running")        |
| `torc reports check-resource-utilization <id>` | Check memory/CPU/time usage                   |
| `torc reports summary <id>`                    | Workflow completion summary                   |
| `torc reports results <id>`                    | JSON report of job results with log paths     |
| `torc slurm sacct <wf_id>`                     | Get Slurm accounting data                     |

## Remote Workers

| Command                                     | Description                              |
| ------------------------------------------- | ---------------------------------------- |
| `torc remote add-workers <wf_id> <host>...` | Add remote workers to a workflow         |
| `torc remote list-workers <wf_id>`          | List remote workers for a workflow       |
| `torc remote run <wf_id>`                   | Start workers on remote machines via SSH |
| `torc remote status <wf_id>`                | Check status of remote workers           |
| `torc remote stop <wf_id>`                  | Stop workers on remote machines          |
| `torc remote collect-logs <wf_id>`          | Collect logs from remote workers         |

## Events & Logs

| Command                       | Description                 |
| ----------------------------- | --------------------------- |
| `torc events monitor <wf_id>` | Monitor events in real-time |
| `torc logs analyze <wf_id>`   | Analyze logs for errors     |

## Global Options

| Option              | Description                        |
| ------------------- | ---------------------------------- |
| `--url <URL>`       | Server URL (or set `TORC_API_URL`) |
| `-f json`           | Output as JSON instead of table    |
| `--log-level debug` | Enable debug logging               |

</div>
