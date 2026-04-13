# Server API Modules

This directory contains the resource-focused server implementations that talk to the database and
return generated response enums. They are the lower-level building blocks behind the live HTTP
transport in `src/server/http_transport.rs`.

## Current Layout

- `mod.rs`: shared utilities, query helpers, and re-exports
- `access_groups.rs`: access control and workflow-group membership data operations
- `compute_nodes.rs`, `events.rs`, `files.rs`, `results.rs`, `user_data.rs`: artifact/resource APIs
- `jobs.rs`, `workflow_actions.rs`, `workflows.rs`: workflow orchestration APIs
- `schedulers.rs`, `resource_requirements.rs`, `remote_workers.rs`, `slurm_stats.rs`: scheduling
  APIs
- `failure_handlers.rs`, `ro_crate.rs`: specialized workflow support APIs

## Ownership Model

- Traits in this directory define resource-level APIs such as `FilesApi<C>` or `JobsApi<C>`.
- `src/server/api_contract.rs` groups those traits into higher-level domain contracts and keeps a
  narrow compatibility trait for the still-large top-level server implementation.
- `src/server/http_server.rs` owns authorization, broadcast side effects, and other cross-resource
  policy before delegating into these modules.

## Shared Utilities

- `ApiContext`: shared database pool wrapper
- `database_error_with_msg`, `database_lock_aware_error`: standard server-side database error
  mapping
- `SqlQueryBuilder`: helper for query construction
- `MAX_RECORD_TRANSFER_COUNT`: global ceiling for list endpoints

## Guidance

- Add new persistence-heavy behavior in the relevant module here.
- Keep authorization and cross-resource orchestration in `src/server/http_server.rs`.
- Keep transport-only HTTP parsing and response shaping in `src/server/http_transport.rs`.
- If a new module trait is added here, update `src/server/api_contract.rs` so the domain-level
  contracts stay aligned.
