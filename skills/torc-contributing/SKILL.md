---
name: torc-contributing
description: >
  Develop the Torc codebase: repository layout, quality gates, tests, database migrations, the
  Rust-owned OpenAPI contract and generated clients, docs, and releases. Use for cargo fmt, clippy,
  dprint, cargo nextest, rstest and serial_test patterns, sqlx migrations, api/sync_openapi.sh,
  mdbook docs, adding a CLI command or API endpoint, or cutting a release. Do not use for operating
  workflows as an end user.
license: BSD-3-Clause
---

# Contributing to Torc

Torc is one feature-gated Rust crate at the repository root plus generated Python and Julia clients.
The HTTP API contract is owned by Rust and emitted; the checked-in spec and clients are artifacts.
Editing an artifact by hand is the most common way to break CI.

## Task router

| Task                                                     | Read                             |
| -------------------------------------------------------- | -------------------------------- |
| Run the gates, understand what CI checks                 | `references/quality-gates.md`    |
| Write or debug tests, use fixtures and serialization     | `references/testing.md`          |
| Change the HTTP API, regenerate clients, add a migration | `references/api-and-database.md` |
| Add a feature across CLI, API, TUI, dashboard, MCP       | `references/adding-features.md`  |

## Repository shape

```text
src/                     one crate, feature-gated
  bin/                   torc-server, torc-dash, torc-mcp-server, torc-slurm-job-runner, torc-htpasswd, torc-openapi
  cli.rs                 top-level clap definitions
  client/commands/       CLI command handlers
  client/apis/           generated Rust API client (do not hand-edit)
  client/               workflow_spec, workflow_manager, job_runner, hpc, remote, ...
  server/                handlers, live_router, http_transport
  openapi_spec.rs        Rust-owned OpenAPI contract source
  mcp_server/            MCP tools
  tui/                   ratatui interface
  config/                layered configuration
torc-server/migrations/  sqlx migrations
torc-dash/static/        dashboard assets
api/                     spec artifacts and sync scripts
python_client/, julia_client/  generated clients
tests/                   integration tests, tests/common/ shared utilities
docs/src/                mdbook sources
examples/                runnable workflow specs
```

Feature flags: `default = client, tui, plot_resources`; additional binaries need `server-bin`,
`mcp-server`, `dash`, or `slurm-runner`; `dist` enables everything. Use `--all-features` for
development builds and tests so feature-gated code is actually compiled.

## Before you commit

```bash
cargo fmt -- --check
cargo clippy --all --all-targets --all-features -- -D warnings
dprint check
```

These three are enforced by the `cargo-husky` pre-commit hook, which also runs `shellcheck` on every
`.sh` file when available. Hooks install on first `cargo build`.

`cargo clippy --all-features` needs a database for `sqlx` macros:

```bash
echo "DATABASE_URL=sqlite:torc.db" > .env
cargo install sqlx-cli --no-default-features --features sqlite
sqlx migrate run --source torc-server/migrations
```

## Non-negotiables

- **Never hand-edit generated output.** That means `api/openapi.yaml`, `api/openapi.codegen.yaml`,
  `src/client/apis/`, `python_client/src/torc/openapi_client/`, and `julia_client/Torc/src/api/`.
  Change `src/openapi_spec.rs` and regenerate.
- **Markdown is 100 characters, enforced.** `dprint fmt` rewraps; `dprint check` gates.
- **Tests use `rstest`, and integration tests that share a server or port use `serial_test`.**
  Serialization groups live in `.config/nextest.toml`; a new test binary that shares state must be
  added there.
- **Log messages that name database records use `workflow_id=<id> job_id=<id>`** so parsing scripts
  keep working.
- **New docs pages must be added to `docs/src/SUMMARY.md`**, and internal links are link-checked in
  CI.
- **A user-facing feature is not done at the CLI.** Check whether the HTTP API, TUI, dashboard, and
  MCP tools should expose it too; see `references/adding-features.md`.

## CI expectations

The lint workflow runs formatting, clippy with `-D warnings`, OpenAPI codegen parity
(`api/check_openapi_codegen_parity.sh`), generated-client parity
(`api/check_client_codegen_parity.sh`), `dprint check`, the doc-link checker and its own tests, and
`shellcheck`. The test workflow runs `cargo nextest run --all-features` on Unix, a reduced set on
Windows, and the Python client tests against a live server.

Reproduce a parity failure locally with `cd api && bash sync_openapi.sh check` before guessing.

## Output

Report the files changed, the gate commands run with their results, tests added or updated, whether
generated artifacts were regenerated, and anything intentionally left out of scope.
