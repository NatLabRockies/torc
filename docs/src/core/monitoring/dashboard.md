# Web Dashboard (torc-dash)

The Torc Dashboard (`torc-dash`) provides a modern web-based interface for monitoring and managing
workflows, offering an intuitive alternative to the command-line interface.

## Overview

`torc-dash` is a Rust-based web application that allows you to:

- **Monitor workflows and jobs** with real-time status updates
- **Create and run workflows** by uploading specification files (YAML, JSON, JSON5, KDL)
- **Visualize workflow DAGs** with interactive dependency graphs
- **Debug failed jobs** with integrated log file viewer
- **Generate resource plots** from time series monitoring data
- **Manage torc-server** start/stop in standalone mode
- **Live event streaming** via Server-Sent Events (SSE) for real-time job and compute node events

## Installation

### Building from Source

`torc-dash` is built as part of the Torc workspace:

```bash
# Build torc-dash
cargo build --release --features dash --bin torc-dash

# Binary location
./target/release/torc-dash
```

### Prerequisites

- A running `torc-server` (or use `--standalone` mode to auto-start one)
- The `torc` CLI binary in your PATH (for workflow execution features)

## Running the Dashboard

### Quick Start (Standalone Mode)

The easiest way to get started is standalone mode, which automatically starts `torc-server`:

```bash
torc-dash --standalone
```

This will:

1. Start `torc-server` on an automatically-detected free port
2. Start the dashboard on http://127.0.0.1:8090
3. Configure the dashboard to connect to the managed server

Open the URL torc-dash logs at startup, such as
`Dashboard available at http://127.0.0.1:8090/?token=c1b6…`. The token keeps other users on the same
machine out of the dashboard's API; the browser remembers it, so later visits to
http://127.0.0.1:8090 work without it until torc-dash restarts with a new token.

### Connecting to an Existing Server

If you already have `torc-server` running:

```bash
# Use default API URL (http://localhost:8080/torc-service/v1)
torc-dash

# Specify custom API URL
torc-dash --api-url http://myserver:9000/torc-service/v1

# Or use environment variable
export TORC_API_URL="http://myserver:9000/torc-service/v1"
torc-dash
```

### Command-Line Options

```
Options:
  -p, --port <PORT>           Dashboard port [default: 8090]
      --host <HOST>           Dashboard host [default: 127.0.0.1]
      --socket <PATH>         Listen on a UNIX domain socket instead of TCP (unix only)
      --allowed-hosts <HOSTS> Extra hostnames the dashboard may be reached by, comma-separated
                              [env: TORC_DASH_ALLOWED_HOSTS]
      --no-token              Serve the API without a token (only behind an authenticating proxy)
  -a, --api-url <API_URL>     Torc server API URL [default: http://localhost:8080/torc-service/v1]
      --torc-bin <PATH>       Path to torc CLI binary [default: torc]
      --torc-server-bin       Path to torc-server binary [default: torc-server]
      --standalone            Auto-start torc-server alongside dashboard
      --server-port <PORT>    Server port in standalone mode (0 = auto-detect) [default: 0]
      --database <PATH>       Database path for standalone server
      --torc-mcp-server-bin        Path to torc-mcp-server [default: torc-mcp-server]

  AI Chat options:
      --llm-provider               LLM provider: anthropic, openai, ollama, github [env: LLM_PROVIDER]
      --anthropic-api-key          Anthropic API key [env: ANTHROPIC_API_KEY]
      --anthropic-foundry-api-key  Foundry API key [env: ANTHROPIC_FOUNDRY_API_KEY]
      --anthropic-foundry-resource Foundry resource [env: ANTHROPIC_FOUNDRY_RESOURCE]
      --anthropic-base-url         Override API base URL [env: ANTHROPIC_BASE_URL]
      --anthropic-auth-token       Bearer token, e.g. for LiteLLM [env: ANTHROPIC_AUTH_TOKEN]
      --anthropic-auth-header      Override auth header name [env: ANTHROPIC_AUTH_HEADER]
      --anthropic-model            Claude model [default: claude-sonnet-4-20250514]
      --openai-api-key             OpenAI API key [env: OPENAI_API_KEY]
      --openai-base-url            OpenAI API URL [default: https://api.openai.com/v1]
      --openai-model               OpenAI model [default: gpt-4o] [env: OPENAI_MODEL]
      --ollama-base-url            Ollama API URL [default: http://localhost:11434/v1]
      --ollama-model               Ollama model [default: llama3.2] [env: OLLAMA_MODEL]
      --github-token               GitHub token for GitHub Models [env: GITHUB_TOKEN]
      --github-models-base-url     GitHub Models URL [default: https://models.inference.ai.azure.com]
      --github-models-model        GitHub Models model [default: gpt-4o] [env: GITHUB_MODELS_MODEL]
```

## Features

The sidebar switches between tabs. Tabs that work on one workflow (Details, Graph, Events,
Debugging, Resources) share a **workflow picker** in the top bar. Data refreshes automatically
(every 30 seconds by default; pauses while the window is hidden) or on demand with **Refresh**.

Keyboard shortcuts (press `?` in the dashboard for the list):

| Key     | Action                              |
| ------- | ----------------------------------- |
| `1`–`8` | Switch tabs                         |
| `E`     | Spec editor                         |
| `N`     | New workflow                        |
| `R`     | Refresh                             |
| `G`     | Graph                               |
| `⌫`     | Back from the graph                 |
| `T`     | Cycle theme (system / light / dark) |
| `/`     | Focus the current tab's filter      |
| `Esc`   | Close a dialog or leave a field     |

### Workflows Tab

- **Workflow list** with ID, name, user, project, description, and creation time. Filter by text or
  access group, or show workflows from all users. Click a row to open its details.
- **Actions** per workflow: run locally, submit to the scheduler, initialize, reinitialize, reset
  status, cancel, recover, sync status, execution plan, export, and delete. Select several workflows
  to delete them together.
- **Import** a workflow from a file on the server or an uploaded file.
- **Run output** streams into a panel at the bottom of the window while you keep using the
  dashboard. Stopping the panel stops the output, not the run; use **Cancel** to stop the run.

#### Creating Workflows

Click **Create workflow** (or press `N`) and choose a source:

1. **Upload file**: drag and drop or click to select a YAML, JSON, JSON5, or KDL specification
2. **File path**: a path on the machine running `torc-dash`
3. **Paste spec**: paste a specification; the format is detected automatically
4. **Wizard**: build a workflow step by step (jobs, parameters, resources, Slurm schedulers,
   actions, monitoring) with a live preview of the generated spec

**Validate** checks the spec on the server without creating anything. By default the new workflow is
initialized after creation; if output files already exist you are asked before they are removed.

From an uploaded YAML or JSON file, **Open in spec editor** moves it to the Spec Editor tab; on the
wizard's last step, **Edit as YAML** does the same with the generated spec.

### Spec Editor Tab

A YAML editor for workflow specs, driven by the workflow-spec JSON Schema:

- **Completion**: type a field name, or press `Ctrl+Space`, for the fields allowed at the cursor
  with their types and descriptions. After `key:`, fields with fixed values (such as
  `execution_config.mode` or booleans) list those values.
- **Hover** a field name for its documentation.
- **Problems are underlined as you type**: YAML syntax errors, unknown fields, missing required
  fields, and wrong value types. **Validate** runs the full server-side check
  (`torc create
  --dry-run`), which also catches problems the schema can't express, such as
  references to undefined jobs or files.
- **New from template** starts from a blank spec or one of a few examples from `examples/yaml`
  (basics, parameter sweeps and pipelines, Slurm, retries), with a preview.
- **Insert** adds a building block (job, parameterized job, file, resource requirements, Slurm
  scheduler, schedule-nodes action, failure handler, user data) to the end of its section, creating
  the section if needed.
- **Open local file** opens YAML or JSON from your computer (JSON is converted to YAML). **Open
  path** and **Save** (`Ctrl+S` / `⌘S`) read and write files on the machine running `torc-dash`;
  relative paths resolve against its working directory.
- **Create workflow** creates the workflow from the editor contents, optionally initializing it.
- **Assistant** opens an AI panel that works on the editor's buffer. It uses the same AI provider as
  the AI Chat tab; set it from either one with **Provider**. Describe a workflow or a change; the
  assistant edits the spec directly, validates it with a dry run, and explains what it did. Each
  edit is selected as it lands and undoes separately with `⌘Z` / `Ctrl+Z`. The editor is read-only
  while the assistant is responding; **Stop** ends the response. The assistant doesn't create
  workflows or write files; you do that from the editor.

The editor keeps its contents in the browser between visits.

### Details Tab

A summary of the selected workflow (status counts, execution time, actions) above sub-tabs for jobs,
running jobs, results, events, files, user data, resource requirements, schedulers, compute nodes,
scheduled compute nodes, and Slurm stats.

- **Sorting**: click column headers
- **Filtering**: plain text matches any column; `field<op>value` filters a column, with `=`, `!=`,
  `>`, `<`, `>=`, `<=`, `~` (contains), and `:` (e.g. `status=failed`, `exec_time>10`, `name:work`).
  Quick-filter buttons cover common job statuses.
- **Job details**: click a job to see its command, results, logs (stdout/stderr), files, user data,
  resources, and dependencies. Other tabs open the same view.

Tables render only the visible rows, so workflows with very large job counts stay responsive.

### Graph Tab

An interactive dependency graph for the selected workflow:

- Views: jobs only, jobs and files, or jobs and user data
- Nodes are colored by job status; the legend shows counts per status
- Zoom, pan, and fit to screen; click a job to open its details
- Very large graphs (thousands of nodes) use a faster layout and ask before rendering

### Debugging Tab

Investigate jobs in the selected workflow:

1. Set the output directory where job logs are stored (default `torc_output`, relative to the
   workflow's submission directory)
2. Keep **Failed only** checked to focus on failures, or uncheck it to see all jobs
3. Select a job to read its stdout and stderr side by side (copy, or open in the file viewer)

Sub-tabs analyze Slurm logs for errors and collect `sacct` accounting data for Slurm allocations.

### Events Tab

Live workflow events via Server-Sent Events (SSE), plus stored events such as retries:

- Job, compute node, and workflow lifecycle events appear as they happen
- A status indicator shows Live or Reconnecting; reconnection is automatic
- The sidebar shows a count of new events while you are on another tab
- Click an event that refers to a job to open the job

### Resources Tab

Plot CPU and memory usage from resource monitoring databases:

1. The dashboard scans the database directory (default `torc_output/resource_utilization`) and lists
   databases for the selected workflow
2. Select databases and click **Generate plots**
3. Choose a plot: summary, CPU and memory across all jobs, or individual jobs

Requires workflows run with resource monitoring enabled; time-series plots need
`granularity: "time_series"` in the `resource_monitor` config.

### AI Chat Tab

The AI Chat tab provides an AI assistant that can interact with your workflows using natural
language. The assistant uses the Torc MCP server to access workflow data, job logs, and management
tools.

**Supported Providers:**

The dashboard supports multiple LLM providers:

- **Anthropic Claude** (direct API or Azure AI Foundry)
- **OpenAI** (GPT-4o, GPT-4o-mini, o1, etc.)
- **Ollama** (local, no API key required)
- **GitHub Models** (Azure-hosted models with GitHub token)

**Setup:**

The simplest setup is a `[dash.chat]` section in your user config file. Unlike environment
variables, it also reaches torc-dash when Torc Desktop starts it, locally or on a login node over
SSH. Create the file with `torc config init --user` (it prints the path; on macOS it is
`~/Library/Application Support/torc/config.toml`) and add, for example for a LiteLLM gateway:

```toml
[dash.chat]
provider = "custom"
base_url = "https://litellm.example.org"
auth_header = "Authorization"   # sent as "Authorization: Bearer <api_key>"
model = "claude-opus-5-5"       # a model name the gateway exposes
api_key = "sk-..."              # the key itself; then chmod 600 the file
```

When torc-dash runs on a login node, this file must be on that node. Environment variables and
flags, shown below, still work and take precedence setting by setting: an `ANTHROPIC_MODEL` in your
shell, for example, overrides `model` from the file. See
[Configuration](../reference/configuration.md#dashchat-section) for all options.

_Option 1: Anthropic Claude (Direct API)_

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
torc-dash
```

_Option 2: Microsoft Azure AI Foundry_

If you access Claude through Azure AI Foundry:

```bash
export ANTHROPIC_FOUNDRY_API_KEY="your-foundry-key"
export ANTHROPIC_FOUNDRY_RESOURCE="your-resource-name"
torc-dash
```

The dashboard constructs the Foundry endpoint automatically:
`https://{resource}.services.ai.azure.com/anthropic/v1/messages`

_Option 2b: An LLM gateway (LiteLLM)_

torc-dash reads the same variables as Claude Code, so a gateway setup that works for Claude Code
works here too. Any model the gateway routes to works, since LiteLLM serves the Anthropic Messages
API for all of them. Set the model to a name the gateway serves:

```bash
export ANTHROPIC_BASE_URL="https://litellm.example.org"   # gateway root, or ending in /v1
export ANTHROPIC_AUTH_TOKEN="sk-..."                       # sent as Authorization: Bearer
export ANTHROPIC_MODEL="claude-sonnet-4-5"                 # a model the gateway exposes
torc-dash
```

List the gateway's models with
`curl -s -H "Authorization: Bearer $ANTHROPIC_AUTH_TOKEN" "$ANTHROPIC_BASE_URL/v1/models"`.
torc-dash does not discover models itself (Claude Code's
`CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY` has no effect). To configure it in the dashboard
instead, choose **Custom via gateway (LiteLLM, proxy)** and enter the gateway URL, the token, and
the model. The dashboard remembers the URL and model (never the token) in your browser.

When torc-dash runs on a login node (for example from Torc Desktop), set these in `~/.bash_profile`
there.

_Option 3: OpenAI_

Use OpenAI's GPT models:

```bash
export OPENAI_API_KEY="sk-..."
LLM_PROVIDER=openai torc-dash

# Or specify a different model
LLM_PROVIDER=openai OPENAI_MODEL=gpt-4o-mini torc-dash
```

You can also use OpenAI-compatible services by setting a custom base URL:

```bash
LLM_PROVIDER=openai torc-dash --openai-base-url https://my-openai-proxy.example.com/v1
```

_Option 4: Ollama (Local)_

Run AI completely locally with [Ollama](https://ollama.ai). No API key required:

```bash
# First, start Ollama and pull a model
ollama pull qwen3.5:35b-a3b

# Then start torc-dash with Ollama provider
LLM_PROVIDER=ollama torc-dash

# Or specify a different model
LLM_PROVIDER=ollama OLLAMA_MODEL=qwen3.5:35b-a3b torc-dash
```

Ollama runs at `http://localhost:11434` by default. For remote Ollama servers:

```bash
LLM_PROVIDER=ollama torc-dash --ollama-base-url http://ollama-server:11434/v1
```

_Option 5: GitHub Models_

Use models hosted on GitHub Models (requires a GitHub token with `models:read` scope):

```bash
export GITHUB_TOKEN="ghp_..."
LLM_PROVIDER=github torc-dash

# Or specify a different model
LLM_PROVIDER=github GITHUB_MODELS_MODEL=Meta-Llama-3.1-70B-Instruct torc-dash
```

Available models include `gpt-4o`, `gpt-4o-mini`, `Meta-Llama-3.1-70B-Instruct`, and others. See
[GitHub Models](https://github.com/marketplace?type=models) for the full list.

**Runtime Configuration:**

You can also configure the provider through the dashboard UI without environment variables:

1. Open the AI Chat tab
2. If not configured, a setup dialog appears
3. Select your provider from the dropdown
4. Enter credentials and model as needed
5. Click "Connect"

Credentials configured this way are stored in memory only for the current session.

**MCP Server:**

You need the `torc-mcp-server` binary in your PATH (built alongside `torc-dash` when using
`--all-features` or `--features mcp-server`). If installed elsewhere, specify its location:

```bash
torc-dash --torc-mcp-server-bin /path/to/torc-mcp-server
```

**Alternative: Use Your Own AI Tool**

If you prefer to use Claude through a subscription (Claude Pro/Max) or GitHub Copilot through an
enterprise account, you can connect `torc-mcp-server` directly to your AI tool:

- **Claude Code** (Pro/Max/Team/Enterprise): Add `torc-mcp-server` as an MCP server -- see
  [AI-Assisted Workflow Management](../../specialized/tools/ai-assistant.md#quick-setup-claude-code)
- **VS Code + Copilot** (enterprise): Add `torc-mcp-server` to `.vscode/mcp.json` -- see
  [AI-Assisted Workflow Management](../../specialized/tools/ai-assistant.md#quick-setup-vs-code-copilot)

These approaches use the AI provider's own authentication and give you the same Torc tools in your
terminal or editor instead of the dashboard.

**Usage:**

- Type questions in natural language and press Enter (or click Send)
- The assistant automatically uses MCP tools to query real data from your Torc server
- If you have a workflow selected, the assistant uses it as the default context
- Tool calls are shown as collapsible sections so you can see what data the AI accessed
- Click "Clear" to reset the conversation

**Example questions:**

- "Help me create a workflow"
- "Show me the failed jobs and their error logs"
- "Check resource utilization for workflow 42"
- "Recover the failed jobs with 2x memory"
- "Create a workflow with 10 parallel jobs"

**Configuration:**

| Setting                      | Default                                 | Description                                 |
| ---------------------------- | --------------------------------------- | ------------------------------------------- |
| `LLM_PROVIDER`               | `anthropic`                             | Provider: anthropic, openai, ollama, github |
| `ANTHROPIC_API_KEY`          | (none)                                  | API key for direct Anthropic API            |
| `ANTHROPIC_FOUNDRY_API_KEY`  | (none)                                  | API key for Azure AI Foundry                |
| `ANTHROPIC_FOUNDRY_RESOURCE` | (none)                                  | Foundry resource name                       |
| `--anthropic-model`          | `claude-sonnet-4-20250514`              | Claude model to use                         |
| `OPENAI_API_KEY`             | (none)                                  | OpenAI API key                              |
| `OPENAI_MODEL`               | `gpt-4o`                                | OpenAI model to use                         |
| `--openai-base-url`          | `https://api.openai.com/v1`             | OpenAI API URL                              |
| `OLLAMA_MODEL`               | `llama3.2`                              | Ollama model to use                         |
| `--ollama-base-url`          | `http://localhost:11434/v1`             | Ollama API URL                              |
| `GITHUB_TOKEN`               | (none)                                  | GitHub token for GitHub Models              |
| `GITHUB_MODELS_MODEL`        | `gpt-4o`                                | GitHub Models model to use                  |
| `--github-models-base-url`   | `https://models.inference.ai.azure.com` | GitHub Models API URL                       |
| `--torc-mcp-server-bin`      | `torc-mcp-server`                       | Path to MCP server binary                   |

For Anthropic, at least one of `ANTHROPIC_API_KEY` or `ANTHROPIC_FOUNDRY_API_KEY` (with
`ANTHROPIC_FOUNDRY_RESOURCE`) must be set. For OpenAI, `OPENAI_API_KEY` is required. For GitHub
Models, `GITHUB_TOKEN` is required. Ollama requires no credentials (runs locally).

> **Note:** The API key is kept server-side and never sent to the browser. All AI requests are
> proxied through the `torc-dash` backend.

### Settings Tab

- **Appearance and behavior**: theme (system, light, dark), dark palette (Warm Gray, Nord, Dracula,
  Monokai, and others), auto-refresh interval, and whether to show workflows from all users by
  default. Preferences are stored in the browser.
- **Version**: dashboard and server versions, with a warning if they don't match, the server URL,
  and a **Test connection** button that shows why a connection fails
- **Server management**: start and stop `torc-server` from the dashboard (port, database path, log
  level) and view its output

The dashboard always talks to the server set with `--api-url` (or `TORC_API_URL`) when `torc-dash`
starts.

## Common Usage Patterns

### Running a Workflow

1. In the **Workflows** tab, click **Create workflow**
2. Upload or paste your specification and click **Create** (it is initialized automatically)
3. Click the run button on the workflow (or **Submit** for Slurm)
4. Watch progress in the **Details** or **Events** tab

### Debugging a Failed Workflow

1. Pick the workflow in the top bar and open the **Debugging** tab
2. Select a failed job
3. Read **stderr** for the error and **stdout** for context

### Monitoring Active Jobs

1. Open the **Details** tab and pick the workflow
2. Use the **Running** sub-tab, or the `status=running` filter on **Jobs**
3. Data refreshes automatically; press `R` to refresh immediately

## Security Considerations

1. **Network Access**: By default, binds to 127.0.0.1 (localhost only)
2. **Dashboard token**: On TCP, the API and torc-server proxy require a token, because anyone who
   can connect to the port (on a shared login node, every user) could otherwise act as you: run
   `torc`, write files, and use your torc-server login. torc-dash generates one and prints it in the
   startup URL, or takes a fixed one from the `TORC_DASH_TOKEN` environment variable (not printed;
   append `?token=<value>` to the URL yourself). Scripts send it as an `X-Torc-Dash-Token` header.
   `--no-token` turns the check off; use it only behind a proxy that authenticates users itself.
   UNIX sockets need no token.
3. **UNIX Socket (recommended for HPC)**: Use `--socket /tmp/torc-dash-$USER.sock` on shared login
   nodes. The socket file is created with 0600 permissions, restricting access to your user account.
   Connect via `ssh -L 8090:/tmp/torc-dash-$USER.sock user@login-node`.
4. **Remote Access**: Use `--host 0.0.0.0` with caution; consider a reverse proxy with HTTPS. The
   dashboard accepts requests only for `localhost`, `127.0.0.1`, `::1`, and its `--host`; add the
   name users type in the browser with `--allowed-hosts dash.example.org`. Requests for any other
   name get `403 Host '…' is not allowed`.
5. **Cross-site protection**: Other web pages open in your browser cannot call the dashboard's API;
   torc-dash refuses requests the browser marks as cross-site.
6. **Authentication**: Torc server supports htpasswd-based authentication (see
   [Authentication](../../specialized/admin/authentication.md))

## Troubleshooting

### Cannot Connect to Server

- Verify torc-server is running: `curl http://localhost:8080/torc-service/v1/workflows`
- Check the `--api-url` (or `TORC_API_URL`) that `torc-dash` was started with
- In standalone mode, check server output for startup errors

### Workflow Creation Fails

- Ensure workflow specification is valid YAML/JSON/KDL
- Check file paths are accessible from the server
- Use **Validate** in the create dialog to see the server's error message

### Resource Plots Not Showing

- Verify workflow used `granularity: "time_series"` mode
- Confirm `.db` files exist in the specified directory
- Check that database files contain data

### Standalone Mode Server Won't Start

- Verify `torc-server` binary is in PATH or specify `--torc-server-bin`
- Check if the port is already in use
- Review console output for error messages

## Desktop App

`torc-desktop` is a native app (macOS, Windows, Linux) that shows the same dashboard without a
browser or a manual SSH tunnel:

- **This computer**: starts `torc-dash` locally, pointed at your torc-server or at a new one it
  starts for you
- **Remote (SSH)**: connects to a login node, starts `torc-dash` there on a private UNIX socket, and
  forwards it automatically; password and MFA prompts appear in the app

See `torc-desktop/README.md` in the repository for building and running it.

## Architecture

`torc-dash` is a self-contained Rust binary with:

- **Axum web framework** for HTTP server
- **Embedded static assets**: the dashboard UI, built at compile time into the binary
- **API proxy** to forward requests to torc-server
- **CLI integration** for workflow operations
- **MCP client** that spawns `torc-mcp-server` as a subprocess for AI Chat

The UI is a Svelte and TypeScript app in `torc-dash/web/`, with typed API calls generated from the
OpenAPI spec, Cytoscape.js for the dependency graph, and Plotly.js for resource charts (both loaded
only when needed).

## Next Steps

- [Dashboard Deployment Tutorial](../../specialized/tools/dashboard-deployment.md) - Detailed
  deployment scenarios
- [Authentication](../../specialized/admin/authentication.md) - Secure your deployment
- [Server Deployment](../../specialized/admin/server-deployment.md) - Production server
  configuration
