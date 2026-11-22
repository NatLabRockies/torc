# Server API Handler

The server is a Rust async web service built with Tokio and uses:

- **Multi-threaded Tokio runtime** for concurrent request handling
- **Modular API structure** with separate modules per resource type (`workflows.rs`, `jobs.rs`,
  `files.rs`, etc.)
- **OpenAPI-generated types** for consistent API contracts
- **Database-level locking** (`BEGIN IMMEDIATE TRANSACTION`) for critical sections

## Key Endpoints

The server implements these key endpoints:

- `POST /workflows` - Create new workflows
- `POST /workflows/{id}/initialize_jobs` - Build dependency graph and mark jobs ready
- `POST /workflows/{id}/claim_next_jobs` - Thread-safe job allocation to workers
- `POST /jobs/{id}/manage_status_change` - Update job status with cascade effects
- `POST /workflows/{id}/process_changed_job_inputs` - Detect changed inputs and reset jobs
- `POST /workflows/{id}/file_stats` - Compare a client's file stat snapshot with stored mtimes and
  reset jobs with changed inputs or missing outputs, plus their downstream jobs

## Thread Safety

The `claim_next_jobs` endpoint uses database-level write locks to prevent multiple workers from
double-allocating jobs to different clients. This is critical for maintaining consistency in
distributed execution.

## API Organization

Each resource type (workflows, jobs, files, events, etc.) has its own module in
`server/src/bin/server/api/`, keeping the codebase organized and maintainable. The main routing
logic delegates to these specialized modules.
