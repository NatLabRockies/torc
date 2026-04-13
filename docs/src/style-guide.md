# Rust Developer Style Guide

This guide establishes coding standards, conventions, and workflows for Rust developers contributing
to Torc. Following these guidelines ensures consistency across the codebase and streamlines the
review process.

## Pre-commit Hooks

**Always rely on the pre-commit hooks provided.** The repository uses `cargo-husky` to install Git
hooks automatically. Before each commit, the following checks run:

```bash
cargo fmt -- --check    # Rust formatting
cargo clippy --all --all-targets --all-features -- -D warnings
dprint check            # Markdown formatting
```

If any check fails, the commit is blocked. Fix the issues before committing.

### Installing Pre-commit Hooks

Hooks are installed automatically when you run `cargo build` for the first time. If you need to
reinstall them manually:

```bash
cargo install cargo-husky
cargo build  # Triggers hook installation
```

## Code Formatting

### Rust Formatting (rustfmt)

All Rust code must pass `cargo fmt --check`. Run `cargo fmt` before committing to auto-format your
code.

**Key conventions enforced:**

- 4-space indentation
- Max line width of 100 characters
- Consistent brace placement
- Sorted imports

### Clippy Compliance

All code must compile without clippy warnings when run with `-D warnings`:

```bash
cargo clippy --all --all-targets --all-features -- -D warnings
```

**Common clippy lints to watch for:**

- `clippy::unwrap_used` - Prefer `expect()` with descriptive messages or proper error handling
- `clippy::clone_on_copy` - Avoid cloning Copy types
- `clippy::needless_return` - Omit unnecessary `return` keywords
- `clippy::redundant_closure` - Use method references where possible

### Markdown Formatting (dprint)

All Markdown files in `docs/` must comply with dprint formatting:

```bash
dprint check   # Verify formatting
dprint fmt     # Auto-format
```

**Critical requirement: Maximum line length of 100 characters for all Markdown files.**

The `dprint.json` configuration enforces:

```json
{
  "lineWidth": 100,
  "markdown": {
    "lineWidth": 100,
    "textWrap": "always"
  }
}
```

## Documentation Standards

All features must be documented in Markdown in the `docs/` directory following the
[Diataxis framework](https://diataxis.fr/):

### Diataxis Categories

| Category          | Location                | Purpose                                        |
| ----------------- | ----------------------- | ---------------------------------------------- |
| **Tutorials**     | `docs/src/tutorials/`   | Learning-oriented, step-by-step lessons        |
| **How-To Guides** | `docs/src/how-to/`      | Task-oriented, problem-solving guides          |
| **Explanation**   | `docs/src/explanation/` | Understanding-oriented, conceptual discussions |
| **Reference**     | `docs/src/reference/`   | Information-oriented, technical descriptions   |

### Design Documentation

Significant design choices must be documented in `docs/src/explanation/design/`. Each design
document should cover:

- **Problem Statement**: What problem does this solve?
- **Design Goals**: What are the requirements and constraints?
- **Solution Overview**: High-level architecture description
- **Implementation Details**: Key technical decisions and trade-offs
- **Alternatives Considered**: What other approaches were evaluated?

Existing design documents include:

- `server.md` - API handler design and request processing
- `database.md` - SQLite schema and concurrency model
- `dashboard.md` - Web dashboard architecture
- `recovery.md` - Workflow recovery mechanisms
- `workflow-graph.md` - Dependency graph implementation

### Documentation Workflow

1. Write documentation alongside code changes
2. Add new pages to `docs/src/SUMMARY.md`
3. Run `dprint fmt` to ensure formatting compliance
4. Build and preview with `mdbook serve docs/`

## Testing with rstest

All code must include tests using the `rstest` library for fixtures and parameterized testing.

### Test Organization

```
tests/
├── common.rs              # Shared test utilities and fixtures
├── test_full_workflows.rs # Integration tests
├── test_job_runner.rs     # Job runner tests
└── scripts/               # Helper scripts for tests
```

### Common Patterns

**Fixture Pattern:**

```rust
use rstest::rstest;
use serial_test::serial;

mod common;
use common::{start_server, ServerProcess};

#[rstest]
#[serial]
fn test_workflow_creation(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Test code using the server fixture
}
```

**Parameterized Tests:**

```rust
#[rstest]
#[case(0, "immediate")]
#[case(60, "one_minute")]
#[case(3600, "one_hour")]
#[serial]
fn test_timeout_handling(#[case] timeout_secs: u64, #[case] description: &str) {
    // Test runs once for each case
}
```

**Shared Test Utilities (`tests/common.rs`):**

```rust
pub struct ServerProcess {
    pub config: Configuration,
    child: std::process::Child,
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        // Automatic cleanup on test completion
        let _ = self.child.kill();
    }
}

pub fn start_server() -> ServerProcess {
    let port = find_available_port();
    // Start server and wait for readiness
}
```

### Test Guidelines

1. **Use `#[serial]`** for integration tests that share resources (ports, database)
2. **Use descriptive `expect()` messages** instead of `.unwrap()`
3. **Clean up resources** using the Drop trait or explicit cleanup functions
4. **Test error conditions** not just happy paths
5. **Keep tests focused** - one behavior per test function

## HTTP API Changes

Changes to the HTTP API require updating the OpenAPI specification and regenerating client
libraries.

### Workflow

1. **Modify the OpenAPI spec:**

   ```bash
   # Update the Rust-owned API document, live handler, and models
   vim src/openapi_spec.rs
   ```

2. **Regenerate API clients:**

   ```bash
   cd api
   bash sync_openapi.sh emit
   bash sync_openapi.sh check
   bash sync_openapi.sh clients --use-rust-spec
   ```

   This regenerates:
   - Rust client: `src/client/apis/`
   - Python client: `python_client/src/torc/openapi_client/`
   - Julia client: `julia_client/Torc/src/api/`

   `clients --use-rust-spec` uses the current `api/openapi.codegen.yaml`. Run `emit` first so the
   generated clients reflect the latest Rust-owned contract, and `check` to verify the checked-in
   artifacts are still in sync.

   When the Rust-emitted spec should become the checked-in contract artifact, run:

   ```bash
   cd api
   bash sync_openapi.sh all --promote
   ```

   That updates both spec artifacts from Rust and regenerates external clients from the promoted
   contract.

3. **Update Rust generation assets as needed:**

   The Rust client and server still have legacy generated files in `src/`. Do not hand-edit those
   generated surfaces. Keep OpenAPI/client generation deterministic through `api/sync_openapi.sh`.

4. **Test all clients:**

   ```bash
   # Rust
   cargo nextest run --all-features

   # Python
   cd python_client && pytest

   # Julia
   julia --project=julia_client/Torc -e "import Pkg; Pkg.test()"
   ```

### OpenAPI Conventions

- Use descriptive `operationId` values (e.g., `create_workflow`, `list_jobs`)
- Include comprehensive request/response schemas
- Document all parameters with descriptions
- Use appropriate HTTP status codes (200, 400, 404, 500)

## Feature Implementation Across Interfaces

When implementing a user-facing feature, ensure it is exposed through the appropriate interfaces.
The following table shows where features should be implemented:

| Interface        | Location                                                            | Primary Use Case                                                         |
| ---------------- | ------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| CLI              | `src/client/commands/`                                              | Command-line automation, scripting                                       |
| HTTP API         | `src/openapi_spec.rs`, `src/server/live_router.rs`, `src/models.rs` | Rust-owned API contract source                                           |
| OpenAPI artifact | `api/openapi.yaml`                                                  | Checked-in emitted contract for Python/Julia integration, external tools |
| Dashboard        | `torc-dash/src/`                                                    | Web-based monitoring and management                                      |
| TUI              | `src/tui/`                                                          | Interactive terminal monitoring                                          |
| MCP Server       | `torc-mcp-server/src/`                                              | AI assistant integration                                                 |

### CLI Implementation

Commands are implemented using `clap` with subcommand enums:

```rust
// In src/client/commands/<feature>.rs

#[derive(Subcommand, Debug, Clone)]
pub enum FeatureCommands {
    /// Create a new resource
    Create {
        /// Name of the resource
        #[arg(short, long)]
        name: String,
    },
    /// List all resources
    List {
        #[arg(long, default_value = "table")]
        format: String,
    },
}

pub fn handle_feature_commands(
    config: &Configuration,
    command: &FeatureCommands,
    format: &str,
) {
    match command {
        FeatureCommands::Create { name } => handle_create(config, name, format),
        FeatureCommands::List { format: fmt } => handle_list(config, fmt),
    }
}
```

**CLI Conventions:**

- Support both `--format table` and `--format json` output
- Use `tabled` for table formatting with `#[tabled(rename = "...")]` for column headers
- Include pagination support via `--offset` and `--limit` flags
- Provide helpful error messages with context

### HTTP API (Python/Julia)

After updating the Rust-owned API contract and promoting the emitted spec, the Python and Julia
clients are auto-generated. Ensure:

1. All new endpoints have proper request/response schemas
2. Query parameters are documented
3. Error responses are specified
4. Run `sync_openapi.sh clients` to regenerate clients

### Dashboard (torc-dash)

The dashboard is an Axum-based web server with embedded static assets:

```rust
// In torc-dash/src/main.rs

async fn handle_feature_list(
    State(state): State<AppState>,
) -> Result<Json<Vec<Feature>>, StatusCode> {
    // Proxy request to Torc API server
    let features = state.client
        .get(&format!("{}/features", state.api_url))
        .send()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .json()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(features))
}
```

**Dashboard Conventions:**

- Proxy API requests to the Torc server
- Use Axum extractors for request handling
- Return JSON for API endpoints
- Serve static files for the frontend

### TUI (Terminal User Interface)

The TUI uses `ratatui` with a component-based architecture:

```rust
// In src/tui/app.rs

pub struct App {
    pub workflows: Vec<WorkflowModel>,
    pub selected_workflow: Option<usize>,
    pub detail_view: DetailViewType,
}

impl App {
    pub fn handle_key_event(&mut self, key: KeyEvent) -> AppAction {
        match key.code {
            KeyCode::Enter => self.select_current(),
            KeyCode::Char('r') => self.refresh_data(),
            KeyCode::Char('q') => AppAction::Quit,
            _ => AppAction::None,
        }
    }
}
```

**TUI Conventions:**

- Use `anyhow::Result` for error handling
- Separate state (`app.rs`), rendering (`ui.rs`), and API calls (`api.rs`)
- Support keyboard navigation with vim-style bindings
- Display confirmation dialogs for destructive actions

### MCP Server (AI Assistant)

The MCP server exposes tools for AI assistants:

```rust
// In torc-mcp-server/src/main.rs

pub fn get_workflow_status(
    config: &Configuration,
    workflow_id: i64,
) -> Result<CallToolResult, McpError> {
    let workflow = default_api::get_workflow(config, workflow_id)
        .map_err(|e| internal_error(format!("Failed to get workflow: {}", e)))?;

    let result = serde_json::json!({
        "workflow_id": workflow.id,
        "name": workflow.name,
        "status": workflow.status,
    });

    Ok(CallToolResult::success(vec![
        rmcp::model::Content::text(serde_json::to_string_pretty(&result).unwrap_or_default())
    ]))
}
```

**MCP Conventions:**

- Return structured JSON for tool results
- Use descriptive error messages via `McpError`
- Support common workflow operations (list, status, run, cancel)
- Keep tool descriptions clear for AI consumption

## Error Handling Strategy

### Application Code (CLI, TUI, binaries)

Use `anyhow::Result` for flexible error handling:

```rust
use anyhow::{Context, Result};

pub fn run_workflow(path: &Path) -> Result<()> {
    let spec = load_spec(path)
        .context("Failed to load workflow specification")?;

    create_workflow(&spec)
        .context("Failed to create workflow")?;

    Ok(())
}
```

### Library Code

Use typed errors with `thiserror`:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkflowError {
    #[error("Job {job_id} not found in workflow {workflow_id}")]
    JobNotFound { job_id: i64, workflow_id: i64 },

    #[error("Invalid status transition from {from} to {to}")]
    InvalidTransition { from: String, to: String },

    #[error("API error: {0}")]
    ApiError(#[from] reqwest::Error),
}
```

### Test Code

Use `.expect()` with descriptive messages:

```rust
let workflow = create_workflow(&spec)
    .expect("Test workflow creation should succeed");

let job = get_job(config, job_id)
    .expect("Job should exist after creation");
```

## Common Patterns

### Configuration Priority

CLI arguments override environment variables, which override config files:

```rust
let api_url = cli_args.url
    .or_else(|| env::var("TORC_API_URL").ok())
    .or_else(|| config.client.as_ref()?.api_url.clone())
    .unwrap_or_else(|| "http://localhost:8080/torc-service/v1".to_string());
```

### Table Display

Use the `tabled` crate for CLI table output:

```rust
use tabled::{Table, Tabled};

#[derive(Tabled)]
struct JobRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Status")]
    status: String,
}

fn display_jobs(jobs: &[JobModel]) {
    let rows: Vec<JobRow> = jobs.iter().map(|j| JobRow {
        id: j.id.unwrap_or(0),
        name: j.name.clone(),
        status: format!("{:?}", j.status),
    }).collect();

    println!("{}", Table::new(rows));
}
```

### Feature Flags

Use Cargo features to conditionally compile components:

```rust
// In Cargo.toml
[features]
default = ["client"]
client = ["dep:reqwest", "dep:clap"]
server = ["dep:sqlx", "dep:axum"]
tui = ["client", "dep:ratatui"]

// In code
#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "server")]
pub mod server;
```

### Async Runtime

Create blocking clients before spawning the async runtime to avoid nested runtime issues:

```rust
fn main() -> Result<()> {
    // Create blocking client BEFORE async runtime
    let client = reqwest::blocking::Client::new();
    let server = MyServer::new(client);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()?;

    runtime.block_on(async_main(server))
}
```

## Logging

Use `tracing` for structured logging:

```rust
use tracing::{debug, info, warn, error, instrument};

#[instrument(skip(config))]
pub fn process_job(config: &Configuration, job_id: i64) -> Result<()> {
    info!(job_id, "Processing job");

    match run_job(job_id) {
        Ok(result) => {
            debug!(job_id, ?result, "Job completed successfully");
            Ok(())
        }
        Err(e) => {
            error!(job_id, error = %e, "Job failed");
            Err(e)
        }
    }
}
```

Enable debug logging with:

```bash
RUST_LOG=debug cargo run
RUST_LOG=torc=debug,sqlx=warn cargo run  # Fine-grained control
```

## Summary Checklist

Before submitting a pull request, verify:

- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy --all --all-targets --all-features -- -D warnings` passes
- [ ] `dprint check` passes (for Markdown changes)
- [ ] All tests pass with `cargo nextest run --all-features`
- [ ] New features have tests using `rstest`
- [ ] Documentation added in appropriate Diataxis category
- [ ] Design decisions documented in `docs/src/explanation/design/` if applicable
- [ ] API changes reflected in the Rust-owned OpenAPI scaffold and promoted `api/openapi.yaml`
- [ ] Client libraries regenerated with `api/sync_openapi.sh clients`
- [ ] Feature exposed through appropriate interfaces (CLI, API, TUI, etc.)
