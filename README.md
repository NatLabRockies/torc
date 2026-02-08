# Torc workflow management system

**Distributed workflow orchestration for complex computational pipelines**

Torc is a workflow management system designed for running large-scale computational workflows with
complex dependencies on local machines and HPC clusters. It uses a client-server architecture with a
centralized SQLite database for state management and coordination.

[![License](https://img.shields.io/badge/license-BSD%203--Clause-blue.svg)](LICENSE)

## Project Status

We recently re-developed this software package in Rust with SQLite as the backend database,
increasing portability and stability. We also added many new features. The code is tested and ready
for user adoption. Interfaces are somewhat stable. Over the next 1-2 months we would like to receive
user feedback and will consider changes. Our goal is to have a 1.0 release by March 2026.

Please post comments or new ideas for Torc in the
[discussions](https://github.com/NREL/torc/discussions).

## Features

- **Declarative Workflow Specifications** - Define workflows in YAML, JSON5, JSON, or KDL
- **Automatic Dependency Resolution** - Dependencies inferred from file and data relationships
- **Job Parameterization** - Create parameter sweeps and grid searches with simple syntax
- **Distributed Execution** - Run jobs across multiple compute nodes with resource tracking
- **Slurm Integration** - Native support for HPC cluster job submission
- **Automatic Failure Recovery** - Detect OOM/timeout failures and retry with adjusted resources
- **Workflow Resumption** - Restart workflows after failures without losing progress
- **Change Detection** - Automatically detect input changes and re-run affected jobs
- **Resource Management** - Track CPU, memory, and GPU usage across all jobs
- **AI-Assisted Management** - Use Claude Code or GitHub Copilot to create, debug, and manage
  workflows through natural language
- **RESTful API** - Complete OpenAPI-specified REST API for integration

## Quick Start

### Installation

Download precompiled binaries from the [releases page](https://github.com/NREL/torc/releases),
install from crates.io, or build from source:

```bash
# Install from crates.io (CLI only)
cargo install torc

# Install all binaries (CLI, server, dashboard, MCP server, Slurm runner)
cargo install torc --features "server-bin,mcp-server,dash,slurm-runner"

# Or build from source
cargo build --all-features --release
```

**macOS users**: The precompiled binaries are not signed with an Apple Developer certificate. macOS
Gatekeeper will block them by default. To allow the binaries to run, remove the quarantine attribute
after downloading:

```bash
xattr -cr /path/to/torc*
```

Alternatively, you can right-click each binary and select "Open" to add a security exception.

The unified `torc` CLI provides all workflow management, execution, and monitoring capabilities.

### Basic Usage

```bash
# 1. Start the Torc server
torc-server run
# Or with options:
torc-server run --url localhost --port 8080 --threads 8 --database path/to/db.sqlite

# 2. Use the unified CLI for all client operations
# Create a workflow from a specification file
torc workflows create my_workflow.yaml

# Run jobs locally
torc run <workflow_id>

# Monitor workflows with the interactive TUI
torc tui

# List workflows
torc workflows list

# View job status
torc jobs list <workflow_id>

# Generate resource usage plots
torc plot-resources output/resource_metrics.db
```

For detailed documentation, see the [docs](docs/) directory.

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

- **Workflow Management**: `torc workflows <subcommand>`
- **Job Management**: `torc jobs <subcommand>`
- **File Management**: `torc files <subcommand>`
- **Local Execution**: `torc run <workflow_spec_or_id>`
- **Interactive TUI**: `torc tui`
- **Resource Visualization**: `torc plot-resources <db_path>`
- **Reports**: `torc reports <subcommand>`

**Global Options**:

- `--url <URL>` - Specify Torc server URL (or use `TORC_API_URL` env var)
- `-f, --format <FORMAT>` - Output format: `table` or `json`

Additional binaries are available via feature flags (see installation docs):

- `torc-server` - REST API server (run separately from the unified CLI)

## Why develop another workflow management tool?

Since there are so many open source workflow management tools available, some may ask, "why develop
another?" We evaluated many of them, including [Nextflow](https://www.nextflow.io/),
[Snakemake](https://snakemake.github.io/), and [Pegasus](https://pegasus.isi.edu/). Those are
excellent tools and we took inspiration from them. However, they did not fully meet our needs and it
wasn't difficult to create exactly what we wanted.

Here are the features of Torc that we think differentiate it from other tools:

- Simple execution on local computers. Many tools require advanced setup and management. Torc
  provides precompiled binaries for each supported platform.

- Node packing on HPC compute nodes

  A Torc worker can maintain a maximum queue depth of jobs on a compute node until the allocation
  runs out of time. Users can start workers on any number single-node or multi-node allocations.

  Users that are not savvy with Bash, Slurm, or workflows can easily distribute many jobs across
  nodes.

- Torc API Server

  Torc provides a server that implements an API conforming to an
  [OpenAPI specification](https://swagger.io/specification/), providing automatic client library
  generation. We use both Python and Julia clients to build and manage workflows. Users can monitor
  workflows through Torc-provided CLI and TUI applications or develop their own scripts.

- Debugging errors

  We run large numbers of simulations on untested input data. Many of them fail. Torc provides
  automatic resource monitoring, log collection, and detailed error reporting through raw text,
  tables, and formatted JSON. Torc makes it easy for users to rerun failed jobs after applying
  fixes.

- Traceability

  All workflows and results are stored in a database, tracked by user and other metadata.

## License

Torc is released under a BSD 3-Clause [license](https://github.com/NREL/torc/blob/main/LICENSE).

## Software Record

This package is developed under NREL Software Record SWR-24-127.
