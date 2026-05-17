# Torc

**Turn one YAML file into thousands of orchestrated jobs — on your laptop or across an HPC
cluster.**

Torc runs the messy, real workflows: parameter sweeps, hyperparameter searches, simulation
campaigns. Write the spec once, get automatic dependency resolution, resource-aware scheduling,
OOM/timeout retries, and a live TUI or Dashboard — local or Slurm, no code changes.

[![License](https://img.shields.io/badge/license-BSD%203--Clause-blue.svg)](LICENSE)

![Torc TUI watching a parameterized simulation sweep](docs/assets/tui-demo.gif)

## See it in action

A typical Torc workflow: one pre-process job, a parameterized simulation that fans out into many
runs, and a post-process job that aggregates the results.

```yaml
# simulation_sweep.yaml
jobs:
  - name: prepare_inputs
    command: python prepare.py --out=/data/config.xyz
    resource_requirements: small
    output_files: [config]

  - name: simulate_T{temp}_P{pressure:03d}
    command: ./run_sim --config=/data/config.xyz --T={temp} --P={pressure}
    resource_requirements: simulation
    depends_on: [prepare_inputs]
    input_files: [config]
    output_files: [result_T{temp}_P{pressure:03d}]
    parameters:
      temp: "250:400:50"      # 4 temperatures
      pressure: "1:101:25"    # 5 pressures → 20 simulations

  - name: summarize
    command: python summarize.py --out=/results/phase_diagram.png
    resource_requirements: small
    input_file_regexes: ["^result_T\\d+_P\\d+$"]
```

```bash
torc run simulation_sweep.yaml      # run locally
torc submit simulation_sweep.yaml   # submit to Slurm
torc tui                            # watch it live
```

One file, 22 jobs (1 setup + 20 sims + 1 summary), dependencies resolved, resources tracked,
failures retried. Widen a parameter range to scale to thousands.

## Why Torc?

We evaluated [Nextflow](https://www.nextflow.io/), [Snakemake](https://snakemake.github.io/), and
[Pegasus](https://pegasus.isi.edu/) — excellent tools, but none combined all of:

- **Zero-setup local execution.** A single precompiled binary. `torc run workflow.yaml` and go.
- **Node packing on HPC.** A single Slurm allocation hosts a deep queue of jobs until its wall clock
  runs out — no per-job submission overhead, no Bash gymnastics. Distribute hundreds of jobs across
  nodes without being a Slurm expert.
- **Resource-aware retries.** OOM and timeout failures are detected and automatically retried with
  larger resources. Stop babysitting overnight runs.
- **Debug and rerun.** Failed jobs come with collected logs, resource metrics, and structured error
  reports (text, table, or JSON). Fix the bug, rerun just the failures — no need to restart the
  whole workflow.
- **Live observability.** Interactive TUI, web dashboard, and resource plots — not just log files.
- **Traceability.** Every workflow and result is durably stored and queryable by user, project, and
  custom metadata long after the run finishes.
- **OpenAPI-first.** Generated Python and Julia clients ship in-tree; write your own in any
  language.
- **AI-native.** Build, debug, and manage workflows through Claude Code, GitHub Copilot, or the
  bundled MCP server.

## Features

- **Declarative specs** in YAML, JSON5, JSON, or KDL
- **Automatic dependency resolution** from file and data relationships
- **Parameter sweeps & grid search** via inline `{param}` templates
- **Distributed execution** with CPU/memory/GPU accounting
- **Slurm integration** with node packing
- **Automatic failure recovery** with OOM/timeout detection and bump-on-retry
- **Workflow resumption** — restart from where execution stopped
- **Change detection** — re-run only the jobs whose inputs moved
- **AI-assisted management** via Claude Code, GitHub Copilot, and an MCP server
- **REST API** with OpenAPI-generated clients

## Project Status

Recently rebuilt in Rust with SQLite — more portable, more stable, plus a lot of new features.
Tested and ready for adoption; interfaces are mostly stable. We're collecting user feedback over the
next 1–2 months and targeting a **1.0 release by July 2026**.

Ideas and bug reports are very welcome on
[GitHub Discussions](https://github.com/NatLabRockies/torc/discussions).

## Installation

```bash
# CLI only
cargo install torc

# Everything (server, dashboard, MCP server, Slurm runner)
cargo install torc --features "server-bin,mcp-server,dash,slurm-runner"

# Or build from source
cargo build --all-features --release
```

Or download a precompiled binary from the
[releases page](https://github.com/NatLabRockies/torc/releases).

> **macOS:** binaries aren't signed with an Apple Developer certificate. After downloading, clear
> the quarantine attribute with `xattr -cr /path/to/torc*`, or right-click each binary and select
> "Open" to add a security exception.

## Basic Usage

```bash
# 1. Start the server
torc-server run
# (options: --url localhost --port 8080 --threads 8 --database path/to/db.sqlite)

# 2. Create + run in one step
torc run examples/yaml/hyperparameter_sweep.yaml

# Or, explicitly:
torc create my_workflow.yaml
torc run <workflow_id>

# Watch it
torc tui

# Inspect
torc workflows list
torc jobs list <workflow_id>
torc plot-resources output/resource_metrics.db
```

For full documentation, see the [docs](docs/) directory.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         Torc Server                         │
│  ┌───────────────────────────────────────────────────────┐  │
│  │               REST API (Tokio + Axum)                 │  │
│  │    /workflows  /jobs  /files  /user_data  /results    │  │
│  └───────────────────────────┬───────────────────────────┘  │
│                              │                              │
│  ┌───────────────────────────▼───────────────────────────┐  │
│  │                SQLite Database (WAL)                  │  │
│  │    • Workflow state    • Job dependencies             │  │
│  │    • Resource tracking • Execution results            │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                               ▲
                               │ HTTP/REST
                               │
     ┌────────────┬────────────┼────────────┬────────────┐
     │            │            │            │            │
┌────▼────┐ ┌─────▼─────┐ ┌────▼────┐ ┌─────▼─────┐ ┌────▼────┐
│   CLI   │ │ Dashboard │ │   AI    │ │ Runner 1  │ │ Runner N│
│  torc   │ │ torc-dash │ │Assistant│ │(compute-1)│ │(compute)│
└─────────┘ └───────────┘ └─────────┘ └───────────┘ └─────────┘
```

## Command-Line Interface

Torc provides a unified CLI with the following commands:

- **Local Execution**: `torc run <workflow_spec_or_id>`
- **Interactive TUI**: `torc tui`
- **Workflow Management**: `torc workflows <subcommand>`
- **Job Management**: `torc jobs <subcommand>`
- **Results Management**: `torc results <subcommand>`
- **Resource Visualization**: `torc plot-resources <db_path>`

**Global Options**:

- `--url <URL>` - Specify Torc server URL (or use `TORC_API_URL` env var)
- `-f, --format <FORMAT>` - Output format: `table` or `json`

Additional binaries are available via feature flags (see installation docs):

- `torc-server` - REST API server (run separately from the unified CLI)

## License

Torc is released under a BSD 3-Clause
[license](https://github.com/NatLabRockies/torc/blob/main/LICENSE).

## Software Record

This package is developed under NLR Software Record SWR-24-127.
