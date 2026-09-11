# Adding features

A user-facing feature is rarely finished at one interface. Decide deliberately which surfaces should
expose it, then follow each one's conventions.

## Contents

- [Interface map](#interface-map)
- [Adding a CLI command](#adding-a-cli-command)
- [CLI conventions](#cli-conventions)
- [Workflow spec fields](#workflow-spec-fields)
- [TUI, dashboard, and MCP](#tui-dashboard-and-mcp)
- [Documentation](#documentation)
- [Review checklist](#review-checklist)

## Interface map

| Interface        | Location                                                            | Reach                                |
| ---------------- | ------------------------------------------------------------------- | ------------------------------------ |
| CLI              | `src/cli.rs`, `src/client/commands/`                                | Scripting and automation             |
| HTTP API         | `src/openapi_spec.rs`, `src/server/live_router.rs`, `src/models.rs` | Everything else, plus external tools |
| OpenAPI artifact | `api/openapi.yaml`                                                  | Python and Julia clients             |
| Dashboard        | `src/bin/torc-dash.rs`, `torc-dash/static/`                         | Browser users                        |
| TUI              | `src/tui/`                                                          | Terminal users on HPC                |
| MCP server       | `src/mcp_server/tools.rs`                                           | AI assistants                        |

Anything that reads or writes server state needs the HTTP API first; the CLI, TUI, dashboard, and
MCP tools are all clients of it. A purely local capability (formatting, plotting, log parsing) can
live in the CLI alone.

## Adding a CLI command

1. Add or extend a `Subcommand` enum in `src/client/commands/<feature>.rs`.
2. Wire it into `src/cli.rs`. Note that most groups are declared `#[command(hide = true)]` and
   appear only in the grouped help headings, so a new group needs an entry in the appropriate
   heading to be discoverable at all.
3. Implement the handler with the shared helpers from `src/client/commands/`.
4. Add `after_long_help` examples. Every existing command has them and users rely on them, since
   hidden commands are otherwise hard to find.
5. Cover it with a CLI-level test using `run_cli_with_json` or `run_cli_command`.

## CLI conventions

- **Output formats.** Support `-f json` via `print_if_json` (single record) or
  `print_wrapped_if_json` (collections). `print_if_json` deliberately exits 1 on `-f csv` for
  single-record commands; keep that behavior rather than emitting degenerate CSV.
- **Tables.** Use `tabled` with `#[tabled(rename = "...")]` for headers, matching neighboring
  commands' column names.
- **Optional workflow ID.** List and query commands take `Option<i64>` and call
  `select_workflow_interactively()` when it is `None`. Follow the pattern in `jobs.rs` or
  `ro_crate.rs`. Be aware the prompt writes to stdout, so an interactive path in a JSON-capable
  command is a real hazard; keep the ID explicit wherever possible.
- **Pagination.** Any command listing server records must paginate with the `paginate_*` or `iter_*`
  helpers in `src/client/commands/pagination/`, not a single unbounded request.
- **Batching.** Prefer one batch API call over a loop of per-item calls, and never shell out to the
  `torc` binary from inside a command.
- **Errors.** Use `print_error` for API failures so messages stay consistent, and make the message
  say what failed and what to do next.
- **Logging.** Record identifiers as `workflow_id=<id> job_id=<id>`.

## Workflow spec fields

A new spec field touches more than the struct:

1. Add it to the spec types in `src/client/workflow_spec.rs`, with `serde` defaults so existing
   specs keep parsing.
2. Validate it in the spec's validation path so `torc create --dry-run` catches mistakes offline. A
   field that only fails at runtime is a poor field.
3. Support every format. YAML, JSON, JSON5, and KDL share one deserialization path, but KDL syntax
   and any nested structure deserve a test.
4. Decide how it interacts with `variables` and `parameters`, since `{name}` substitution applies to
   every string field.
5. Document it in `docs/src/core/reference/workflow-spec.md` and add a runnable example under
   `examples/yaml/`.

Backward-incompatible spec changes break stored workflows: the created workflow keeps the values it
was created with, and `reinit` re-reads the job definitions.

## TUI, dashboard, and MCP

- **TUI** (`src/tui/`): state in `app.rs`, rendering in `ui.rs`, API calls in `api.rs`. Use
  `anyhow::Result`, keep vim-style bindings consistent with existing panes, confirm destructive
  actions, and add the key to the help popup.
- **Dashboard** (`src/bin/torc-dash.rs`): Axum handlers that proxy the Torc API and return JSON;
  static assets in `torc-dash/static/`. Do not query the database directly.
- **MCP** (`src/mcp_server/tools.rs`): tools are named for the operation an assistant would ask for
  (`list_failed_jobs`, `check_resource_utilization`, `recover_workflow`). Keep them read-mostly,
  make mutations explicit, and support `dry_run` where the CLI equivalent does. `get_docs`,
  `list_examples`, and `get_example` fetch from GitHub unless `TORC_DOCS_DIR` / `TORC_EXAMPLES_DIR`
  point at local copies.

## Documentation

Documentation ships with the change, not after it:

- Reference material under `docs/src/core/reference/`.
- Task-oriented instructions under `docs/src/core/how-to/`.
- Conceptual background under `docs/src/core/concepts/`.
- Significant design decisions under `docs/src/specialized/design/`, covering problem, goals,
  solution, implementation, and alternatives.
- Add every new page to `docs/src/SUMMARY.md`.
- Update `docs/src/core/reference/cli-cheatsheet.md` when adding a CLI command.
- Run `dprint fmt` and the internal link checker.

Also consider these agent-facing surfaces when behavior changes: `AGENTS.md`, `CLAUDE.md`, and the
skills under `skills/`.

## Review checklist

Server:

- `authorize_workflow!` or `authorize_resource!` before any business logic.
- Correct status codes: 403, 404, 422, 500.
- Indexes for new query patterns; no unnecessary joins.

Client:

- `Option<i64>` workflow IDs with interactive selection.
- Pagination on anything that lists.
- `-f json` support.
- Structured log messages with record IDs.

Contract:

- `src/openapi_spec.rs` edited, artifacts regenerated, parity checks passing.
- `HTTP_API_VERSION` bumped appropriately in `src/api_version.rs`.

Overall:

- No duplicated logic; functions short enough to read in one pass.
- No repeated CLI subprocess invocations or per-item API calls where a batch exists.
- Tests for both success and failure paths, registered in the right nextest group when they share
  state.
