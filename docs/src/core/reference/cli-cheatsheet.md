<div class="cheatsheet">

# Torc CLI Cheat Sheet

## Quick Start

| Command                     | Description                                    |
| --------------------------- | ---------------------------------------------- |
| `torc create <spec>`        | Create workflow from spec file                 |
| `torc run <spec.yaml>`      | Create workflow from spec and run locally      |
| `torc submit <spec.yaml>`   | Create and submit to scheduler (needs actions) |
| `torc status <id>`          | Workflow status and job summary                |
| `torc watch <id>`           | Monitor workflow until completion              |
| `torc watch <id> --recover` | Monitor and auto-recover from failures         |
| `torc-dash`                 | Launch web dashboard                           |
| `torc tui`                  | Launch interactive terminal UI                 |

## Workflow Lifecycle

| Command              | Description                          |
| -------------------- | ------------------------------------ |
| `torc create <spec>` | Create workflow from spec file       |
| `torc run <id>`      | Run workflow locally                 |
| `torc submit <id>`   | Submit workflow to scheduler         |
| `torc status <id>`   | Show workflow status and job summary |
| `torc cancel <id>`   | Cancel workflow and Slurm jobs       |
| `torc delete <id>`   | Delete workflow                      |

## Workflow State

| Command                            | Description                         |
| ---------------------------------- | ----------------------------------- |
| `torc workflows init <id>`         | Initialize workflow dependencies    |
| `torc workflows reinit <id>`       | Reinitialize workflow after changes |
| `torc workflows reset-status <id>` | Reset workflow and job statuses     |

## Workflow Query

| Command                   | Description          |
| ------------------------- | -------------------- |
| `torc workflows list`     | List your workflows  |
| `torc workflows get <id>` | Get workflow details |

## Job Management

| Command                           | Description         |
| --------------------------------- | ------------------- |
| `torc jobs list <id>`             | List all jobs       |
| `torc jobs list -s ready <id>`    | List jobs by status |
| `torc jobs get <job_id>`          | Get job details     |
| `torc results list <id>`          | List job results    |
| `torc results list --failed <id>` | List failed jobs    |

## Recovery & Diagnostics

| Command                                     | Description                                    |
| ------------------------------------------- | ---------------------------------------------- |
| `torc status <id>`                          | Workflow status and job summary                |
| `torc workflows check-resources <id>`       | Check memory/CPU/time usage                    |
| `torc results list <id> --include-logs`     | Job results with log paths                     |
| `torc recover <id>`                         | Interactive recovery wizard (default)          |
| `torc recover <id> --no-prompts`            | Automatic recovery (no prompts, for scripting) |
| `torc watch <id> --recover --auto-schedule` | Full production recovery mode                  |
| `torc workflows sync-status <id>`           | Fix orphaned jobs (stuck in "running")         |
| `torc workflows correct-resources <id>`     | Upscale violated + downsize over-allocated RRs |
| `torc slurm sacct <id>`                     | Get Slurm accounting data                      |
| `torc slurm stats <id>`                     | Per-job sacct stats stored in the database     |
| `torc slurm usage <id>`                     | Total compute node and CPU time consumed       |

## Remote Workers

| Command                                  | Description                              |
| ---------------------------------------- | ---------------------------------------- |
| `torc remote add-workers <id> <host>...` | Add remote workers to a workflow         |
| `torc remote list-workers <id>`          | List remote workers for a workflow       |
| `torc remote run <id>`                   | Start workers on remote machines via SSH |
| `torc remote status <id>`                | Check status of remote workers           |
| `torc remote stop <id>`                  | Stop workers on remote machines          |
| `torc remote collect-logs <id>`          | Collect logs from remote workers         |

## Events & Logs

| Command                    | Description                 |
| -------------------------- | --------------------------- |
| `torc events monitor <id>` | Monitor events in real-time |
| `torc logs analyze <id>`   | Analyze logs for errors     |

## Global Options

| Option              | Description                        |
| ------------------- | ---------------------------------- |
| `--url <URL>`       | Server URL (or set `TORC_API_URL`) |
| `-f json`           | Output as JSON instead of table    |
| `--log-level debug` | Enable debug logging               |

</div>
