# Terminal User Interface (TUI)

The Torc TUI provides a full-featured terminal interface for managing workflows, designed for HPC
users working in terminal-over-SSH environments.

## Quick Start

```bash
# Option 1: Connect to an existing server
torc-server run &   # Start server in background
torc tui            # Launch the TUI

# Option 2: Standalone mode (auto-starts server)
torc tui --standalone

# Option 3: Start TUI without server (manual connection)
torc tui            # Shows warning, use 'S' to start server
```

### Standalone Mode

Use `--standalone` for single-machine development or testing:

```bash
# Basic standalone mode
torc tui --standalone

# Custom port
torc tui --standalone --port 8090

# Custom database location
torc tui --standalone --database /path/to/workflows.db
```

In standalone mode, the TUI automatically starts a `torc-server` process with the specified
configuration.

## Features

- **Workflow Management**: Create, initialize, run, submit, cancel, reset, and delete workflows
- **Job Management**: View details, logs, cancel, terminate, or retry jobs
- **Real-time Monitoring**: Auto-refresh, manual refresh, color-coded status
- **Live Event Streaming**: Server-Sent Events (SSE) for real-time job and compute node events
- **Server Management**: Start/stop torc-server directly from the TUI
- **File Viewing**: Preview workflow files with search and navigation
- **DAG Visualization**: Text-based dependency graph

## Interface Overview

When the TUI starts, you'll see:

```
┌─ Torc Management Console ────────────────────────────────────────┐
│ ?: help | n: new | i: init | I: reinit | R: reset | x: run ...  │
└──────────────────────────────────────────────────────────────────┘
┌─ Server ─────────────────────────────────────────────────────────┐
│ http://localhost:8080/torc-service/v1  S: start | K: stop | O: output │
└──────────────────────────────────────────────────────────────────┘
┌─ User Filter ────────────────────────────────────────────────────┐
│ Current: yourname  (press 'w' to change, 'a' for all users)     │
└──────────────────────────────────────────────────────────────────┘
┌─ Workflows [FOCUSED] ────────────────────────────────────────────┐
│ >> 1  | my-workflow    | yourname | Example workflow            │
│    2  | data-pipeline  | yourname | Data processing pipeline    │
└──────────────────────────────────────────────────────────────────┘
```

## Basic Navigation

| Key             | Action                                                             |
| --------------- | ------------------------------------------------------------------ |
| `↑` / `↓`       | Move up/down in the current table                                  |
| `PgUp` / `PgDn` | Page through rows (10 at a time)                                   |
| `g` / `G`       | Jump to top / bottom of the current table                          |
| `←` / `→`       | Switch focus between Workflows and Details panes                   |
| `Tab`           | Switch between detail tabs (Jobs → Files → Events → Results → DAG) |
| `Enter`         | Load details for selected workflow                                 |
| `q`             | Quit (or close popup/dialog)                                       |
| `?`             | Show context-aware help popup (scoped to current pane / tab)       |

## Workflow Actions

Select a workflow and use these keys:

| Key | Action        | Description                                             |
| --- | ------------- | ------------------------------------------------------- |
| `n` | New           | Create workflow from spec file                          |
| `i` | Initialize    | Set up job dependencies, mark ready jobs                |
| `I` | Re-initialize | Reset and re-initialize (prompts if output files exist) |
| `R` | Reset         | Reset all job statuses                                  |
| `x` | Run           | Run workflow locally (shows real-time output)           |
| `s` | Submit        | Submit to HPC scheduler (Slurm)                         |
| `C` | Cancel        | Cancel running workflow                                 |
| `d` | Delete        | Delete workflow (destructive!)                          |

All destructive actions show a confirmation dialog.

### Handling Existing Output Files

When initializing or re-initializing a workflow, if existing output files are detected, the TUI will
show a confirmation dialog listing the files that will be deleted. Press `y` to proceed with
`--force` or `n` to cancel.

## Job Management

Navigate to the Jobs tab (`→` then `Tab` if needed) to manage individual jobs:

| Key         | Action                                      |
| ----------- | ------------------------------------------- |
| `Enter`     | View job details                            |
| `l`         | View job logs (stdout/stderr)               |
| `C`         | Cancel job                                  |
| `t`         | Terminate job                               |
| `y`         | Retry failed job                            |
| `1`/`2`/`3` | Sort by ID / Name / Status (cycles ↓ ↑ off) |

### Job Status Colors

- **Green**: Completed
- **Yellow**: Running
- **Red**: Failed
- **Magenta**: Canceled/Terminated
- **Blue**: Pending/Scheduled
- **Cyan**: Ready
- **Gray**: Blocked

## Log Viewer

Press `l` on a job to view its logs:

| Key             | Action                           |
| --------------- | -------------------------------- |
| `Tab`           | Switch between stdout and stderr |
| `↑` / `↓`       | Scroll one line                  |
| `PgUp` / `PgDn` | Scroll 20 lines                  |
| `g` / `G`       | Jump to top / bottom             |
| `/`             | Start search                     |
| `n` / `N`       | Next / previous search match     |
| `q`             | Close log viewer                 |

## File Viewer

Navigate to the Files tab and press `Enter` on a file to view its contents. The file viewer
supports:

- Files up to 1MB
- Binary files show a hex dump preview
- Same navigation keys as the log viewer

## Filtering and Sorting

### Filtering

| Key | Action                                                            |
| --- | ----------------------------------------------------------------- |
| `f` | Open the filter input for the focused pane (Workflows or Details) |
| `=` | Filter to the selected row's value on a sensible primary column   |
| `c` | Clear the active filter on the focused pane                       |

When a filter is active, the table title shows it in magenta so you don't lose track of why rows are
missing — for example, `▶ Jobs │ filter: Status~failed`.

The `=` shortcut maps the focused pane to a primary column and applies the filter using the selected
row's value: Workflows → User, Jobs → Status, Results → Status, Events → Event Type, Files → Name,
Compute Nodes → Hostname, Scheduled Nodes → Status. Use it to quickly narrow down to "rows like this
one."

### Sorting

The Jobs and Results tables support cycling sort indicators in their column headers.

| Tab     | Key | Action                               |
| ------- | --- | ------------------------------------ |
| Jobs    | `1` | Sort by ID (cycles ↓ ↑ off)          |
| Jobs    | `2` | Sort by Name (cycles ↓ ↑ off)        |
| Jobs    | `3` | Sort by Status (cycles ↓ ↑ off)      |
| Results | `m` | Sort by Peak Memory (cycles ↓ ↑ off) |
| Results | `p` | Sort by Peak CPU % (cycles ↓ ↑ off)  |

Selection is preserved across sort changes and refreshes — re-sorting the table keeps the row you
were looking at highlighted.

## Results Tab

The Results tab shows execution results for completed and failed jobs:

| Column     | Meaning                                        |
| ---------- | ---------------------------------------------- |
| ID         | Result row ID                                  |
| Job ID     | The job that produced this result              |
| Run        | Workflow run number                            |
| Attempt    | Attempt number for this job                    |
| Return     | Process exit code (0 green, non-zero red)      |
| Status     | Final job status                               |
| Peak Mem   | Highest sampled memory usage                   |
| Peak CPU % | Highest sampled CPU% across the job's lifetime |

Use `m` and `p` to surface jobs that exceeded their resource allocation (sort descending and the
worst offenders rise to the top).

## Events Tab (SSE Live Streaming)

The Events tab shows real-time events from the server using Server-Sent Events (SSE). When you
select a workflow and navigate to the Events tab, the TUI establishes a live connection to receive
events as they occur.

**Event types displayed:**

- `job_started` / `job_completed` / `job_failed` - Job lifecycle events
- `compute_node_started` / `compute_node_stopped` - Worker node lifecycle
- `workflow_started` / `workflow_reinitialized` - Workflow initialization events
- `scheduler_node_created` - Slurm scheduler events

**Features:**

- Events appear instantly without polling
- Newest events shown at the top
- Filter by Event Type or Data content using `f`
- Title shows "[SSE Live]" indicator when connected

## Server Management

The TUI can start and manage a `torc-server` instance:

| Key | Action             |
| --- | ------------------ |
| `S` | Start torc-server  |
| `K` | Stop/Kill server   |
| `O` | Show server output |

The server status indicator in the connection bar shows:

- `●` (green): Server is running (managed by TUI)
- `○` (yellow): Server was started but has stopped
- No indicator: External server (not managed by TUI)

## Connection Settings

| Key | Action                |
| --- | --------------------- |
| `u` | Change server URL     |
| `w` | Change user filter    |
| `a` | Toggle show all users |

## Auto-Refresh

Press `A` to toggle auto-refresh (30-second interval). When enabled, the workflow list and details
refresh automatically.

## Configuration

The TUI respects Torc's layered configuration system:

1. Interactive changes in TUI (press `u` to change server URL)
2. Environment variables (`TORC_CLIENT__API_URL`)
3. Local config file (`./torc.toml`)
4. User config file (`~/.config/torc/config.toml`)
5. System config file (`/etc/torc/config.toml`)
6. Default values

## Troubleshooting

### "Could not connect to server"

1. Ensure the Torc server is running: `torc-server run`
2. Check the server URL: press `u` to update if needed
3. Verify network connectivity

### "No log content available"

Logs may not be available if:

- The job hasn't run yet
- You're on a different machine than where jobs ran
- The output directory is in a different location

### Screen rendering issues

- Ensure your terminal supports UTF-8 and 256 colors
- Try resizing your terminal window
- Press `r` to force a refresh

## TUI vs Web Dashboard

| Feature           | TUI (`torc tui`)     | Web (`torc-dash`)    |
| ----------------- | -------------------- | -------------------- |
| Environment       | Terminal/SSH         | Web browser          |
| Startup           | Instant              | ~2 seconds           |
| Dependencies      | None (single binary) | None (single binary) |
| Workflow actions  | Yes                  | Yes                  |
| Job actions       | Yes                  | Yes                  |
| Log viewing       | Yes                  | Yes                  |
| DAG visualization | Text-based           | Interactive graph    |
| Resource plots    | Planned              | Yes                  |

**Choose the TUI for**: SSH sessions, HPC environments, quick operations, low-bandwidth connections.

**Choose torc-dash for**: Rich visualizations, resource plots, team dashboards.
