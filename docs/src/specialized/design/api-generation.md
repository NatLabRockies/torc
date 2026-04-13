# API Generation Architecture

This document describes how Torc's HTTP API contract and generated clients are produced.

## Overview

Torc uses a Rust-owned OpenAPI workflow:

- The HTTP contract is defined in Rust by `src/openapi_spec.rs`, live handlers in
  `src/server/live_router.rs`, and `src/models.rs`.
- The checked-in OpenAPI artifacts are `api/openapi.yaml` and `api/openapi.codegen.yaml`.
- The live server transport is implemented in Rust under `src/server/http_transport.rs`.
- The Rust client used by the CLI/TUI/dashboard is generated into `src/client/apis/` using
  checked-in OpenAPI Generator template overrides so it matches Torc's canonical model layer and
  client conventions.
- The Python and Julia clients are generated from the checked-in OpenAPI artifact.

## Source Of Truth

The main ownership layers are:

- `src/models.rs`
  - Canonical API models used by the Rust-owned contract.
- `src/openapi_spec.rs`
  - OpenAPI document definition, emission, and parity checks.
- `src/server/live_router.rs`
  - Live typed handlers that also own route metadata and operation IDs.
- `src/server/http_transport.rs` and `src/server/http_transport/*.rs`
  - Live HTTP transport implementation for the server.
- `src/server/api_contract.rs`
  - Rust server contract traits.

The HTTP API version is single-sourced in:

- `src/api_version.rs`

That constant feeds:

- server version reporting
- client/server version checks
- emitted OpenAPI document version
- external client generation package metadata

## OpenAPI Artifacts

The checked-in files in `api/` are:

- `api/openapi.yaml`
  - Promoted OpenAPI artifact used for external client generation.
- `api/openapi.codegen.yaml`
  - Rust-emitted artifact kept alongside the promoted spec.

These should match the Rust emitter output. The parity check enforces that.

## Server Creation

The live server is built from:

- `src/server/http_server.rs`
  - top-level server wiring and shared state
- `src/server/http_transport.rs`
  - HTTP route dispatch and request/response mapping
- `src/server/http_transport/*.rs`
  - domain-specific transport handlers
- `src/server/api/*.rs`
  - lower-level server business logic modules

## Rust Client Creation

The Rust client under `src/client/apis/` is generated from `api/openapi.yaml`, but it does not own
its own model layer.

Relevant files:

- `api/regenerate_rust_client.sh`
  - Generates a sync Rust client into a temporary directory.
  - Passes `api/openapi-generator-templates/rust/` to OpenAPI Generator.
  - Copies only the generated grouped API modules, not the generated `models/` tree.
- `api/openapi-generator-templates/rust/`
  - Checked-in template overrides for the generated Rust client surface.
  - Must remain version-controlled because regeneration uses them as a required input.
  - Encodes Torc-specific generation behavior such as importing `crate::models`, using the shared
    blocking client helpers, and applying auth consistently.
- `src/client/apis/configuration.rs`
  - Hand-owned blocking client configuration used by the generated Rust API modules.
  - Carries Torc-specific conventions like TLS settings, cookie handling, and auth application.

The surrounding Rust client logic is hand-written in:

- `src/client/`

That includes things like:

- `src/client/version_check.rs`
- `src/client/commands/`
- `src/client/workflow_spec.rs`

The generated Rust client is one layer inside a larger hand-owned Rust client stack. The canonical
Rust API models remain in `src/models.rs`.

The generated Rust client does **not** own a second Rust model layer. Generated `models.rs` output
is discarded. The repo keeps exactly one canonical Rust API model surface in `src/models.rs`.

The generated Rust client is tag-grouped. Current generated modules include domains such as:

- `src/client/apis/workflows_api.rs`
- `src/client/apis/jobs_api.rs`
- `src/client/apis/access_control_api.rs`
- `src/client/apis/workflow_actions_api.rs`
- `src/client/apis/remote_workers_api.rs`
- `src/client/apis/ro_crate_entities_api.rs`
- `src/client/apis/system_api.rs`

The rest of the Rust codebase now calls those grouped generated modules directly.

## Python And Julia Client Creation

The Python and Julia clients are generated from `api/openapi.yaml`.

Relevant files:

- `api/regenerate_clients.sh`
  - Regenerates the Rust, Python, and Julia clients from the selected spec.
- `api/sync_openapi.sh`
  - Main developer entrypoint for emitting, checking, promoting, and regenerating clients.

Generated output locations:

- Python:
  - `python_client/src/torc/openapi_client/`
- Julia:
  - `julia_client/Torc/src/api/`
  - `julia_client/julia_client/docs/`

There is also a hand-written Python compatibility/helper layer in:

- `python_client/src/torc/api.py`

That layer exists to provide a stable high-level interface over the tag-grouped generated Python
APIs.

## Developer Workflow

Emit the Rust-owned OpenAPI spec:

```bash
bash api/sync_openapi.sh emit
```

Verify the checked-in artifacts match the Rust emitter:

```bash
bash api/sync_openapi.sh check
```

Promote the Rust-emitted spec into `api/openapi.yaml`:

```bash
bash api/sync_openapi.sh promote
```

Regenerate Rust, Python, and Julia clients:

```bash
bash api/sync_openapi.sh clients
```

This regenerates the Rust client from the selected spec using the checked-in template overrides,
then regenerates the Python and Julia clients from the same spec.

To iterate on clients against the Rust-emitted contract before promotion:

```bash
bash api/sync_openapi.sh emit
bash api/sync_openapi.sh check
bash api/sync_openapi.sh clients --use-rust-spec
```

Run the full sync flow:

```bash
bash api/sync_openapi.sh all --promote
```

## Why This Layout Exists

This arrangement gives Torc one authoritative contract pipeline:

- The server contract is owned in normal Rust code.
- The OpenAPI artifact is emitted deterministically from that Rust code.
- External clients are generated from the emitted artifact.
- Rust generator customization is expressed in checked-in templates, not a post-generation patch
  overlay.
- Generated files are downstream artifacts, not the source of truth.
