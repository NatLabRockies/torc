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
cargo build --release -p torc-dash

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
  -a, --api-url <API_URL>     Torc server API URL [default: http://localhost:8080/torc-service/v1]
      --torc-bin <PATH>       Path to torc CLI binary [default: torc]
      --torc-server-bin       Path to torc-server binary [default: torc-server]
      --standalone            Auto-start torc-server alongside dashboard
      --server-port <PORT>    Server port in standalone mode (0 = auto-detect) [default: 0]
      --database <PATH>       Database path for standalone server
      --completion-check-interval-secs <SECS>  Server polling interval [default: 5]
      --torc-mcp-server-bin        Path to torc-mcp-server [default: torc-mcp-server]

  AI Chat options:
      --llm-provider               LLM provider: anthropic, openai, ollama, github [env: LLM_PROVIDER]
      --anthropic-api-key          Anthropic API key [env: ANTHROPIC_API_KEY]
      --anthropic-foundry-api-key  Foundry API key [env: ANTHROPIC_FOUNDRY_API_KEY]
      --anthropic-foundry-resource Foundry resource [env: ANTHROPIC_FOUNDRY_RESOURCE]
      --anthropic-base-url         Override API base URL [env: ANTHROPIC_BASE_URL]
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

### Workflows Tab

The main workflows view provides:

- **Workflow list** with ID, name, timestamp, user, and description
- **Create Workflow** button to upload new workflow specifications
- **Quick actions** for each workflow:
  - View details and DAG visualization
  - Initialize/reinitialize workflow
  - Run locally or submit to scheduler
  - Delete workflow

#### Creating Workflows

Click "Create Workflow" to open the creation dialog:

1. **Upload a file**: Drag and drop or click to select a workflow specification file
   - Supports YAML, JSON, JSON5, and KDL formats
2. **Or enter a file path**: Specify a path on the server filesystem
3. Click "Create" to register the workflow

### Details Tab

Explore workflow components with interactive tables:

- **Jobs**: View all jobs with status, name, command, and dependencies
- **Files**: Input/output files with paths and timestamps
- **User Data**: Key-value data passed between jobs
- **Results**: Execution results with return codes and resource metrics
- **Compute Nodes**: Available compute resources
- **Resource Requirements**: CPU, memory, GPU specifications
- **Schedulers**: Slurm scheduler configurations

Features:

- **Workflow selector**: Filter by workflow
- **Column sorting**: Click headers to sort
- **Row filtering**: Type in filter boxes (supports `column:value` syntax)
- **Auto-refresh**: Toggle automatic updates

### DAG Visualization

Click "View" on any workflow to see an interactive dependency graph:

- Nodes represent jobs, colored by status
- Edges show dependencies (file-based and explicit)
- Zoom, pan, and click nodes for details
- Legend shows status colors

### Debugging Tab

Investigate failed jobs with the integrated debugger:

1. Select a workflow
2. Configure output directory (where logs are stored)
3. Toggle "Show only failed jobs" to focus on problems
4. Click "Generate Report" to fetch results
5. Click any job row to view its log files:
   - **stdout**: Standard output from the job
   - **stderr**: Error output and stack traces
   - Copy file paths with one click

### Events Tab (SSE Live Streaming)

Monitor workflow activity in real-time using Server-Sent Events (SSE):

- **Live event streaming** - events appear instantly without polling
- **Connection status indicator** - shows Live/Reconnecting/Disconnected status
- **Event types displayed**:
  - `job_started` / `job_completed` / `job_failed` - Job lifecycle events
  - `compute_node_started` / `compute_node_stopped` - Worker node lifecycle
  - `workflow_started` / `workflow_reinitialized` - Workflow initialization events
  - `scheduler_node_created` - Slurm scheduler events
- **Clear button** to reset the event list
- **Auto-reconnect** on connection loss

### Resource Plots Tab

Visualize CPU and memory usage over time:

1. Enter a base directory containing resource database files
2. Click "Scan for Databases" to find `.db` files
3. Select databases to plot
4. Click "Generate Plots" for interactive Plotly charts

Requires workflows run with `granularity: "time_series"` in `resource_monitor` config.

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
[GitHub Models](https://github.com/marketplace/models) for the full list.

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
  [AI-Assisted Workflow Management](../../specialized/tools/ai-assistant.md#quick-setup-vs-code--copilot)

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

### Configuration Tab

#### Server Management

Start and stop `torc-server` directly from the dashboard:

- **Server Port**: Port to listen on (0 = auto-detect free port)
- **Database Path**: SQLite database file location
- **Completion Check Interval**: How often to check for job completions
- **Log Level**: Server logging verbosity

Click "Start Server" to launch, "Stop Server" to terminate.

#### API Configuration

- **API URL**: Torc server endpoint
- **Test Connection**: Verify connectivity

Settings are saved to browser local storage.

## Common Usage Patterns

### Running a Workflow

1. Navigate to **Workflows** tab
2. Click **Create Workflow**
3. Upload your specification file
4. Click **Create**
5. Click **Initialize** on the new workflow
6. Click **Run Locally** (or **Submit** for Slurm)
7. Monitor progress in the **Details** tab or **Events** tab

### Debugging a Failed Workflow

1. Go to the **Debugging** tab
2. Select the workflow
3. Check "Show only failed jobs"
4. Click **Generate Report**
5. Click on a failed job row
6. Review the **stderr** tab for error messages
7. Check **stdout** for context

### Monitoring Active Jobs

1. Open **Details** tab
2. Select "Jobs" and your workflow
3. Enable **Auto-refresh**
4. Watch job statuses update in real-time

## Security Considerations

1. **Network Access**: By default, binds to 127.0.0.1 (localhost only)
2. **UNIX Socket (recommended for HPC)**: Use `--socket /tmp/torc-dash-$USER.sock` on shared login
   nodes. The socket file is created with 0600 permissions, restricting access to your user account.
   Connect via `ssh -L 8090:/tmp/torc-dash-$USER.sock user@login-node`.
3. **Remote Access**: Use `--host 0.0.0.0` with caution; consider a reverse proxy with HTTPS
4. **Authentication**: Torc server supports htpasswd-based authentication (see
   [Authentication](./authentication.md))

## Troubleshooting

### Cannot Connect to Server

- Verify torc-server is running: `curl http://localhost:8080/torc-service/v1/workflows`
- Check the API URL in Configuration tab
- In standalone mode, check server output for startup errors

### Workflow Creation Fails

- Ensure workflow specification is valid YAML/JSON/KDL
- Check file paths are accessible from the server
- Review browser console for error details

### Resource Plots Not Showing

- Verify workflow used `granularity: "time_series"` mode
- Confirm `.db` files exist in the specified directory
- Check that database files contain data

### Standalone Mode Server Won't Start

- Verify `torc-server` binary is in PATH or specify `--torc-server-bin`
- Check if the port is already in use
- Review console output for error messages

## Architecture

`torc-dash` is a self-contained Rust binary with:

- **Axum web framework** for HTTP server
- **Embedded static assets** (HTML, CSS, JavaScript)
- **API proxy** to forward requests to torc-server
- **CLI integration** for workflow operations
- **MCP client** that spawns `torc-mcp-server` as a subprocess for AI Chat

The frontend uses vanilla JavaScript with:

- **Cytoscape.js** for DAG visualization
- **Plotly.js** for resource charts
- **Custom components** for tables and forms

## Next Steps

- [Dashboard Deployment Tutorial](../../specialized/tools/dashboard-deployment.md) - Detailed
  deployment scenarios
- [Authentication](../../specialized/admin/authentication.md) - Secure your deployment
- [Server Deployment](../../specialized/admin/server-deployment.md) - Production server
  configuration
