# AGENTS.md

This file provides guidance to coding agents working in this repository.

## Project Overview

Torc is a distributed workflow orchestration system for computational pipelines with job
dependencies, resource requirements, and distributed execution.

- Server: Rust REST API service that manages workflow state and persists data in SQLite
- Unified CLI: single `torc` binary for workflow management, job execution, TUI, and plotting
- Feature-gated binaries: `torc-server`, `torc-dash`, `torc-mcp-server`, `torc-slurm-job-runner`
- Python client: CLI and library for Python-based workflows

Core concepts:

- Workflows: top-level containers for related computational jobs
- Jobs: tasks with dependencies, resource requirements, and status tracking
- Files and user data: artifacts that establish implicit job dependencies
- Resource requirements: CPU, memory, GPU, and runtime specifications per job
- Schedulers: local or Slurm-based execution environments
- Compute nodes: available compute resources for job execution

## Code Quality Requirements

All code changes should pass these checks before commit:

```bash
cargo fmt -- --check
cargo clippy --all --all-targets --all-features -- -D warnings
dprint check
```

Key requirements:

- Rust code must compile without clippy warnings
- Markdown must comply with dprint formatting and a 100-character max line length
- If unsure, run the checks manually before finishing

For detailed style guidance, see `docs/src/style-guide.md`.

## Repository Structure

```text
torc/
├── src/
│   ├── bin/
│   ├── client/
│   │   ├── commands/
│   │   ├── apis/
│   │   ├── workflow_spec.rs
│   │   ├── workflow_manager.rs
│   │   ├── job_runner.rs
│   │   └── async_cli_command.rs
│   ├── server/
│   │   └── api/
│   ├── mcp_server/
│   ├── tui/
│   ├── run_jobs_cmd.rs
│   ├── tui_runner.rs
│   ├── plot_resources_cmd.rs
│   ├── main.rs
│   ├── lib.rs
│   └── models.rs
├── torc-server/
│   └── migrations/
├── torc-dash/
│   └── static/
├── python_client/
│   ├── src/torc/
│   └── pyproject.toml
└── examples/
```

## Component Guidance

- Client code lives in `src/client/`
- Workflow specification system lives in `src/client/workflow_spec.rs`
- Workflow manager and job runner live in `src/client/`
- Resource management and job execution live in `src/client/job_runner.rs`
- API client integration patterns live in `src/client/apis/`

## Useful Commands

```bash
# Build unified CLI
cargo build --workspace --release

# Run tests
cargo nextest run --all-features

# Run a specific test
cargo nextest run -E 'test(test_get_ready_jobs)'
```

## Architecture Notes

### Server

- Async Tokio runtime
- SQLite with write locks for safe job claiming
- Foreign key cascades for workflow deletion
- Rust-owned OpenAPI contract emitted from `src/openapi_spec.rs` and live handlers in
  `src/server/live_router.rs`
- Live HTTP transport in `src/server/http_transport.rs`

### OpenAPI Workflow

- Do not hand-edit `api/openapi.yaml` or generated client outputs.
- Emit and verify the spec from Rust with:

```bash
cd api
bash sync_openapi.sh check
```

- Promote the Rust-emitted spec and regenerate Rust, Python, and Julia clients with:

```bash
cd api
bash sync_openapi.sh all --promote
```

- Regenerate Rust, Python, and Julia clients from the current checked-in spec with:

```bash
cd api
bash sync_openapi.sh clients
```

### Documentation

- Build user docs from `docs/` with `mdbook build`
- The generated HTML under `docs/book/` should be refreshed when doc sources change

### Client

- `workflow_spec.rs`: declarative workflow definitions
- `workflow_manager.rs`: workflow lifecycle
- `job_runner.rs`: local parallel execution with resource tracking
- `async_cli_command.rs`: non-blocking subprocess execution
- `src/tui/`: interactive terminal UI

### Logging

When log messages refer to database records, use the format `"workflow_id={} job_id={}"` where
applicable.

## Testing Guidance

- Integration tests live in `tests/`
- Use `serial_test` for tests that modify shared state
- Shared test utilities live in `tests/common/`

## Important Notes

### Job Status Storage

Job statuses are stored as integers in the database:

- 0 = uninitialized
- 1 = blocked
- 2 = ready
- 3 = pending
- 4 = running
- 5 = completed
- 6 = failed
- 7 = canceled
- 8 = terminated
- 9 = disabled
- 10 = pending_failed

### Resource Formats

- Memory: strings like `"1m"`, `"2g"`, `"512k"`
- Runtime: ISO8601 durations like `"PT30M"` or `"PT2H"`
- Timestamps: Unix timestamps as float64 for file modification times

### Dependencies

- Explicit dependencies: `job_depends_on`
- Implicit dependencies: file and user data input/output relationships
- Job specs use names, which are resolved to IDs during workflow creation
