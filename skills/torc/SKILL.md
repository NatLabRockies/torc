---
name: torc
description: >
  Author, run, optimize, inspect, and debug Torc workflows through the torc CLI or the Torc MCP
  server. Use for workflow specs in YAML/JSON5/JSON/KDL, local runs, torc exec, Slurm submission and
  allocation sizing, node packing and parallel-jobs-per-node decisions, remote SSH workers, reruns
  and recovery, status/results/log queries, JSON and CSV scripting, MCP tools, configuration, and log
  levels. Do not use for generic Slurm, SSH, or shell questions with no Torc workflow involved, or
  for contributing to the Torc codebase itself.
---

# Torc

Torc turns a declarative spec into a dependency graph of jobs. The server owns workflow state in
SQLite. Runners claim and execute jobs. Use the least costly execution mode that meets the request.
Do not progress through every mode or launch work just because it is possible.

## Find the relevant guidance

Read the smallest relevant reference. Do not preload the reference library. Follow links only when a
task needs them.

| Task                                                       | Reference                                                         |
| ---------------------------------------------------------- | ----------------------------------------------------------------- |
| Write, review, or validate a workflow spec                 | `references/spec-authoring.md`                                    |
| Choose an execution mode                                   | `references/execution-modes.md`                                   |
| Run locally or use `torc exec`                             | `references/local-execution.md`                                   |
| Generate schedulers, submit, or diagnose Slurm allocations | `references/slurm.md`                                             |
| Tune packing, walltime, or allocation count                | `references/optimization.md`                                      |
| Configure SSH workers                                      | `references/remote-workers.md`                                    |
| Query workflow, job, result, dependency, or resource state | `references/query-map.md`                                         |
| Watch live work or plot resources                          | `references/live-monitoring.md`                                   |
| Diagnose failures or stalled work                          | `references/failure-analysis.md`, `references/stuck-workflows.md` |
| Rerun, reset, or recover work                              | `references/rerun-and-recovery.md`                                |
| Find, collect, or analyze logs                             | `references/log-map.md`                                           |
| Script Torc output                                         | `references/scripting.md`                                         |
| Exit codes, streams, prompts, pagination, or auth          | `references/command-behavior.md`                                  |
| Configure the CLI, server, or logging                      | `references/settings.md`, `references/logging.md`                 |
| Troubleshoot server access or outages                      | `references/connectivity.md`                                      |
| Define an HPC profile                                      | `references/hpc-profiles.md`                                      |
| Use Torc from MCP                                          | `references/mcp-tools.md`                                         |
| Find CLI commands and behavior not shown by help           | `references/cli-reference.md`                                     |

For current CLI syntax and flags, use `torc <command> --help`. Use `torc <group> --help` to list a
group's commands. Consult `cli-reference.md` for hidden command groups and behavior that help does
not explain. For MCP, use the connected tool schema and read `mcp-tools.md` for capability
boundaries and mutation procedures.

## Agent workflow

1. Classify the request. Distinguish authoring, inspection, execution, tuning, recovery, and
   configuration. Do not turn a question or diagnosis into a workflow mutation.
2. Choose the available surface. When Torc MCP tools are connected, use them for capabilities they
   provide. Do not shell out for the same operation. Use the CLI for starting work, standalone
   scheduler generation/submission, remote workers, and CLI configuration. MCP spec creation can
   include scheduler generation but cannot submit allocations. If MCP is unavailable, use the CLI.
3. Establish the target and evidence. For server operations, identify the configured endpoint and an
   explicit workflow ID. Inspect current state before proposing a change. Use `torc ping` to check
   connectivity when needed. Standalone local validation does not require a shared server. Prefer
   read-only queries over asking the user for facts the available tools can retrieve.
4. Ask only for missing inputs. Resolve local versus Slurm, account, paths, and commands from the
   request or current configuration where possible. Ask when a necessary choice cannot be inferred
   or safely discovered. Never invent payload commands or assume placeholder commands are runnable.
5. Take the smallest appropriate step. Validate a spec offline with `torc create --dry-run` or the
   MCP `validate` action before creating it. Preview a proposed change with `--dry-run` when
   supported. For MCP mutations, follow `mcp-tools.md`. Show the impact and get confirmation where
   required. Verify the exact workflow and side effects before forced resets or irreversible
   actions. Smoke-test unvalidated payloads in scratch output before scaling. Do not run a test when
   the user only asked for a draft or inspection.
6. Verify the requested outcome. Check server state after execution or recovery. Use explicit IDs
   and JSON for scripted queries. Report the relevant command/tool calls, observed result, and
   remaining blocker. Distinguish observations from inferences and keep the report proportional to
   the request.

## Non-obvious behavior

- `torc run` exits 0 even when jobs fail. Its status describes the runner, not the workload. Verify
  job outcomes with `torc status <id>` or `torc results list <id> --failed`. `torc exec` and
  `torc watch` have meaningful non-zero failure statuses.
- Runner logs from `torc run`, `torc exec`, and `torc watch` go to stderr and a runner log file.
  Structured command output stays on stdout. Use `references/command-behavior.md` for stream and
  exit-code details.
- Pass workflow IDs explicitly. Commands that permit an omitted ID may prompt on a TTY or fail
  without one. Interactive single-workflow selection can still choose the wrong target.
- Resource requirements describe one job's needs on one node. They determine packing and Slurm
  sizing. Use `references/optimization.md` for the calculations.
- `torc submit` needs a `schedule_nodes` action. A spec without one is local-only. Validate with
  `torc create --dry-run` before submitting.

## Execution modes

Use standalone `torc -s` for local, short-lived-server work. Use a shared server for durable history
or multiple runners, remote workers for SSH-reachable machines without a scheduler, and Slurm where
a scheduler manages compute. Login nodes are for validation and submission, not payload smoke tests.
See `references/execution-modes.md` for trade-offs and `references/local-execution.md` or
`references/slurm.md` for the selected path.
