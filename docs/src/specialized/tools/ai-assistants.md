# Configuring AI Assistants

Complete guide for configuring AI assistants (Claude Code, GitHub Copilot) to work with Torc.

## Overview

Torc provides an MCP (Model Context Protocol) server that enables AI assistants to interact with
workflows. The `torc-mcp-server` binary acts as a bridge between AI assistants and the Torc HTTP
API.

## Available Tools

The AI assistant has access to these Torc operations:

| Tool                         | Description                                              |
| ---------------------------- | -------------------------------------------------------- |
| `get_workflow_status`        | Get workflow info with job counts by status              |
| `get_job_details`            | Get detailed job info including resource requirements    |
| `get_job_logs`               | Read stdout/stderr from job log files                    |
| `list_failed_jobs`           | List all failed jobs in a workflow                       |
| `list_jobs_by_status`        | Filter jobs by status                                    |
| `check_resource_utilization` | Analyze resource usage and detect OOM/timeout issues     |
| `update_job_resources`       | Modify job resource requirements                         |
| `analyze_resource_usage`     | Per-job resource data grouped by RR for cluster analysis |
| `regroup_job_resources`      | Create new RR groups and reassign jobs (dry_run support) |
| `restart_jobs`               | Reset and restart failed jobs                            |
| `resubmit_workflow`          | Regenerate Slurm schedulers and submit new allocations   |
| `cancel_jobs`                | Cancel specific jobs                                     |
| `create_workflow_from_spec`  | Create a workflow from JSON specification                |

## Environment Variables

| Variable          | Description                            | Default                                 |
| ----------------- | -------------------------------------- | --------------------------------------- |
| `TORC_API_URL`    | Torc server URL                        | `http://localhost:8080/torc-service/v1` |
| `TORC_OUTPUT_DIR` | Directory containing job logs          | `output`                                |
| `TORC_PASSWORD`   | Password for authentication (optional) | —                                       |

---

## Claude Code Configuration

### Configuration Scopes

Claude Code supports MCP configuration at three scopes:

| Scope       | File                             | Use Case                                  |
| ----------- | -------------------------------- | ----------------------------------------- |
| **Project** | `.mcp.json` in project root      | Team-shared configuration (commit to git) |
| **Local**   | `.mcp.json` with `--scope local` | Personal project settings (gitignored)    |
| **User**    | `~/.claude.json`                 | Cross-project personal tools              |

### CLI Commands

```bash
# Add the Torc MCP server
claude mcp add torc \
  --scope project \
  -e TORC_API_URL=http://localhost:8080/torc-service/v1 \
  -e TORC_OUTPUT_DIR=/path/to/your/output \
  -- /path/to/torc-mcp-server

# List configured MCP servers
claude mcp list

# Get details about the torc server
claude mcp get torc

# Remove the MCP server
claude mcp remove torc
```

### Manual Configuration

Create or edit `.mcp.json` in your project root:

```json
{
  "mcpServers": {
    "torc": {
      "command": "/path/to/torc-mcp-server",
      "env": {
        "TORC_API_URL": "http://localhost:8080/torc-service/v1",
        "TORC_OUTPUT_DIR": "/path/to/your/output"
      }
    }
  }
}
```

### Environment Variable Expansion

You can use environment variable expansion in `.mcp.json`:

```json
{
  "mcpServers": {
    "torc": {
      "command": "/path/to/torc-mcp-server",
      "env": {
        "TORC_API_URL": "${TORC_API_URL:-http://localhost:8080/torc-service/v1}",
        "TORC_OUTPUT_DIR": "${TORC_OUTPUT_DIR:-./output}"
      }
    }
  }
}
```

---

## VS Code + GitHub Copilot Configuration

### Prerequisites

- VS Code 1.99 or later
- GitHub Copilot extension installed
- GitHub Copilot subscription (Business, Enterprise, Pro, or Pro+)

### Configuration

Create `.vscode/mcp.json` in your project root:

```json
{
  "servers": {
    "torc": {
      "command": "/path/to/torc-mcp-server",
      "env": {
        "TORC_API_URL": "http://localhost:8080/torc-service/v1",
        "TORC_OUTPUT_DIR": "./output"
      }
    }
  }
}
```

### Verify Setup

1. Open the Command Palette (`Ctrl+Shift+P` / `Cmd+Shift+P`)
2. Run "MCP: List Servers"
3. Verify "torc" appears in the list

### Usage

In Copilot Chat, use **Agent Mode** (`@workspace` or the agent icon) to access MCP tools.

---

## VS Code Remote SSH for HPC

For users running Torc on HPC clusters, VS Code's Remote SSH extension allows you to use Copilot
Chat with the MCP server running directly on the cluster.

### Architecture

```
┌─────────────────────┐         ┌─────────────────────────────────────┐
│  Local Machine      │   SSH   │  HPC Cluster                        │
│                     │◄───────►│                                     │
│  VS Code            │         │  torc-mcp-server ◄──► torc-server   │
│  (Copilot Chat)     │         │        ▲                            │
│                     │         │        │                            │
└─────────────────────┘         │  .vscode/mcp.json                   │
                                └─────────────────────────────────────┘
```

The MCP server runs on the HPC, communicates with the Torc server on the HPC, and VS Code proxies
requests through SSH. No ports need to be exposed to your local machine.

### Step 1: Build `torc-mcp-server` on the HPC

```bash
# On the HPC (via SSH or login node)
cd /path/to/torc
cargo build --release -p torc-mcp-server
```

### Step 2: Configure MCP in your project

Create `.vscode/mcp.json` in your project directory **on the HPC**:

```json
{
  "servers": {
    "torc": {
      "command": "/path/on/hpc/torc/target/release/torc-mcp-server",
      "env": {
        "TORC_API_URL": "http://localhost:8080/torc-service/v1",
        "TORC_OUTPUT_DIR": "./output"
      }
    }
  }
}
```

> **Important:** MCP servers configured in workspace settings (`.vscode/mcp.json`) run on the remote
> host, not your local machine.

### Step 3: Connect and use

1. Install the
   [Remote - SSH](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-ssh)
   extension
2. Connect to the HPC: `Remote-SSH: Connect to Host...`
3. Open your project folder on the HPC
4. Open Copilot Chat and use Agent Mode

### HPC-Specific Tips

- **Module systems:** If your HPC uses modules, you may need to set `PATH` in the env to include
  required dependencies
- **Shared filesystems:** Place `.vscode/mcp.json` in a project directory on a shared filesystem
  accessible from compute nodes
- **Firewalls:** The MCP server only needs to reach the Torc server on the HPC's internal network

---

## How It Works

Torc uses the Model Context Protocol (MCP), an open standard for connecting AI assistants to
external tools. The `torc-mcp-server` binary:

1. **Receives tool calls** from the AI assistant via stdio
2. **Translates them** to Torc HTTP API calls
3. **Returns results** in a format the assistant can understand

The server is stateless—it simply proxies requests to your running Torc server. All workflow state
remains in Torc's database.

---

## Security Considerations

- The MCP server has full access to your Torc server
- Consider using authentication if your Torc server is exposed
- The server can modify workflows (restart, cancel, update resources)
- Review proposed actions before they execute

---

## Troubleshooting

### Claude doesn't see the tools

- Verify the MCP server is configured: `claude mcp list`
- Check the config file is valid JSON: `cat .mcp.json | jq .`
- Check that the path to `torc-mcp-server` is correct and the binary exists
- Start a new Claude Code session (MCP servers are loaded at startup)

### "Failed to connect to server"

- Ensure your Torc server is running
- Check that `TORC_API_URL` is correct
- Verify network connectivity

### "Permission denied" or "Authentication failed"

- Set `TORC_PASSWORD` if your server requires auth
- Check that the credentials are correct

### Logs not found

- Ensure `TORC_OUTPUT_DIR` points to your job output directory
- Check that jobs have actually run (logs are created at runtime)

---

## See Also

- [AI-Assisted Workflow Management Tutorial](./ai-assistant.md)
- [Configuration Reference](../../core/reference/configuration.md)
- [HPC Deployment](../hpc/hpc-deployment.md)
