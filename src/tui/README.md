# Torc TUI - Terminal User Interface

A full-featured terminal user interface for managing Torc workflows, designed for HPC users working
in terminal-over-SSH environments. Built with [ratatui](https://ratatui.rs/).

## Quick Start

```bash
# Make sure the Torc server is running
torc-server run

# Launch the TUI
torc tui
```

## Features

### Workflow Management

- **Create workflows** from YAML, JSON, or JSON5 spec files
- **Initialize** workflows to set up job dependencies
- **Run** workflows locally or **submit** to HPC schedulers (Slurm)
- **Cancel**, **reset**, or **delete** workflows
- All destructive actions require confirmation

### Job Management

- **View job details** including full command and status
- **View job logs** (stdout/stderr) with search and navigation
- **Cancel**, **terminate**, or **retry** individual jobs
- Color-coded job status for quick visual scanning

### Real-time Monitoring

- **Auto-refresh** toggle for live updates (30-second interval)
- **Manual refresh** with `r` key
- **Status bar** with operation feedback
- **Color-coded results** showing pass/fail status

### Navigation

- **Two-pane interface**: Workflows list and detail view
- **Tabbed detail views**: Jobs, Files, Events, Results, DAG
- **Column filtering** for all table views
- **Keyboard-driven** interface optimized for SSH

---

## Tutorial

### 1. Starting the TUI

First, ensure the Torc server is running:

```bash
# Start the server
torc-server run

# In another terminal, set server URL (optional)
export TORC_API_URL="http://localhost:8080/torc-service/v1"

# Launch the TUI
torc tui
```

When the TUI starts, you'll see the main interface:

```
┌─ Torc Management Console ────────────────────────────────────────┐
│ ?: help | n: new | i: init | x: run | s: submit | d: delete ...│
└──────────────────────────────────────────────────────────────────┘
┌─ Server ─────────────────────────────────────────────────────────┐
│ http://localhost:8080/torc-service/v1  (press 'u' to change)    │
└──────────────────────────────────────────────────────────────────┘
┌─ User Filter ────────────────────────────────────────────────────┐
│ Current: yourname  (press 'w' to change, 'a' for all users)     │
└──────────────────────────────────────────────────────────────────┘
┌─ Workflows [FOCUSED] ────────────────────────────────────────────┐
│ >> 1  | my-workflow    | yourname | Example workflow            │
│    2  | data-pipeline  | yourname | Data processing pipeline    │
└──────────────────────────────────────────────────────────────────┘
```

### 2. Basic Navigation

| Key       | Action                                                             |
| --------- | ------------------------------------------------------------------ |
| `↑` / `↓` | Move up/down in the current table                                  |
| `←` / `→` | Switch focus between Workflows and Details panes                   |
| `Tab`     | Switch between detail tabs (Jobs → Files → Events → Results → DAG) |
| `Enter`   | Load details for selected workflow                                 |
| `q`       | Quit (or close popup/dialog)                                       |
| `?`       | Show help popup with all keybindings                               |

**Try it:** Use arrow keys to select a workflow, press `Enter` to load its details.

### 3. Creating a Workflow

Press `n` to create a new workflow from a spec file:

```
┌─ Create Workflow from Spec File ─────────────────────────────────┐
│ Workflow spec file: ~/workflows/my-workflow.yaml_                │
│ Enter: create | Esc: cancel (supports ~, YAML/JSON/JSON5)        │
└──────────────────────────────────────────────────────────────────┘
```

1. Press `n` to start
2. Type the path to your workflow spec file (supports `~` for home directory)
3. Press `Enter` to create the workflow
4. Press `Esc` to cancel

The status bar will show success or any errors.

### 4. Workflow Actions

Select a workflow and use these keys:

| Key | Action        | Description                                     |
| --- | ------------- | ----------------------------------------------- |
| `i` | Initialize    | Set up job dependencies, mark ready jobs        |
| `I` | Re-initialize | Reset and re-initialize (clears existing state) |
| `R` | Reset         | Reset all job statuses                          |
| `x` | Run           | Run workflow locally                            |
| `s` | Submit        | Submit to HPC scheduler (Slurm)                 |
| `W` | Watch         | Watch workflow with recovery                    |
| `C` | Cancel        | Cancel running workflow                         |
| `d` | Delete        | Delete workflow (destructive!)                  |

All destructive actions show a confirmation dialog:

```
┌─ Delete Workflow ────────────────────────────────────────────────┐
│                                                                  │
│        DELETE workflow 'my-workflow'?                            │
│        This action cannot be undone!                             │
│                                                                  │
│              y: Yes | n: No | Esc: cancel                        │
└──────────────────────────────────────────────────────────────────┘
```

### 5. Viewing Jobs

Press `→` to focus the Details pane, then navigate to the Jobs tab:

```
┌─ Jobs [FOCUSED] - Enter: details, l: logs, c: cancel... ────────┐
│ ID    | Name          | Status      | Command                   │
│ >> 1  | preprocess    | Completed   | python preprocess.py      │
│    2  | train-model   | Running     | python train.py           │
│    3  | evaluate      | Blocked     | python evaluate.py        │
└──────────────────────────────────────────────────────────────────┘
```

**Status colors:**

- 🟢 **Green**: Completed
- 🟡 **Yellow**: Running
- 🔴 **Red**: Failed
- 🟣 **Magenta**: Canceled/Terminated
- 🔵 **Blue**: Pending/Scheduled
- ⚪ **Cyan**: Ready
- ⬛ **Gray**: Blocked

### 6. Job Details and Logs

Select a job and press `Enter` to see details:

```
┌─ Job Details: train-model ───────────────────────────────────────┐
│ ID: 2                                                            │
│ Status: Running                                                  │
│                                                                  │
│ Command:                                                         │
│ python train.py --epochs 100 --batch-size 32                     │
│                                                                  │
│ Press q or Esc to close, l to view logs                          │
└──────────────────────────────────────────────────────────────────┘
```

Press `l` to view logs:

```
┌─ Logs: train-model ──────────────────────────────────────────────┐
│  stdout  |  stderr   (Tab to switch)                             │
├──────────────────────────────────────────────────────────────────┤
│   1  Epoch 1/100: loss=0.532                                     │
│   2  Epoch 2/100: loss=0.421                                     │
│   3  Epoch 3/100: loss=0.387                                     │
│   4  Epoch 4/100: loss=0.352                                     │
│   ...                                                            │
├──────────────────────────────────────────────────────────────────┤
│ Path: output/job_stdio/job_wf1_j2_r1.o                           │
│ q: close | /: search | n/N: next/prev | g/G: top/bottom | y: path│
└──────────────────────────────────────────────────────────────────┘
```

**Log viewer controls:**

| Key             | Action                           |
| --------------- | -------------------------------- |
| `Tab`           | Switch between stdout and stderr |
| `↑` / `↓`       | Scroll one line                  |
| `PgUp` / `PgDn` | Scroll 20 lines                  |
| `g`             | Jump to top                      |
| `G`             | Jump to bottom                   |
| `/`             | Start search                     |
| `n`             | Next search match                |
| `N`             | Previous search match            |
| `y`             | Show file path in status bar     |
| `q` / `Esc`     | Close log viewer                 |

### 7. Job Actions

From the Jobs tab, select a job and use:

| Key | Action    | When to use                              |
| --- | --------- | ---------------------------------------- |
| `c` | Cancel    | Stop a pending or running job gracefully |
| `t` | Terminate | Force-stop a running job                 |
| `y` | Retry     | Re-queue a failed job                    |

### 8. Viewing Files

Switch to the Files tab and press `Enter` on a file to view its contents:

```
┌─ File: config.yaml ──────────────────────────────────────────────┐
│   1  name: my-workflow                                           │
│   2  description: Example workflow                               │
│   3                                                              │
│   4  jobs:                                                       │
│   5    - name: preprocess                                        │
│   ...                                                            │
├──────────────────────────────────────────────────────────────────┤
│ Path: /home/user/workflows/config.yaml                           │
│ q: close | /: search | n/N: next/prev | g/G: top/bottom | y: path│
└──────────────────────────────────────────────────────────────────┘
```

**File viewer controls:**

| Key             | Action                       |
| --------------- | ---------------------------- |
| `↑` / `↓`       | Scroll one line              |
| `PgUp` / `PgDn` | Scroll 20 lines              |
| `g`             | Jump to top                  |
| `G`             | Jump to bottom               |
| `/`             | Start search                 |
| `n`             | Next search match            |
| `N`             | Previous search match        |
| `y`             | Show file path in status bar |
| `q` / `Esc`     | Close file viewer            |

**Notes:**

- Files up to 1MB can be viewed
- Binary files show a hex dump preview
- Files that don't exist show a helpful error message

### 9. Filtering Tables

Press `f` while focused on any detail table to filter:

```
┌─ Filter Input ───────────────────────────────────────────────────┐
│ Status: failed_ | Tab: change column | Enter: apply | Esc: cancel│
└──────────────────────────────────────────────────────────────────┘
```

1. Press `f` to start filtering
2. Press `Tab` to change the filter column (e.g., Status, Name, Command)
3. Type your filter text
4. Press `Enter` to apply
5. Press `c` to clear the filter

### 10. Auto-Refresh

Press `A` to toggle auto-refresh:

```
┌─ Torc Management Console - Auto-refresh enabled (30s interval) ──┐
│ ?: help | n: new | i: init | x: run | ... | A: auto-refresh [ON]│
└──────────────────────────────────────────────────────────────────┘
```

When enabled, the workflow list and details refresh every 30 seconds.

### 11. DAG Visualization

Switch to the DAG tab to see job dependencies:

```
┌─ Job DAG ────────────────────────────────────────────────────────┐
│   [✓] preprocess (id: 1)                                         │
│      ↓↓↓                                                         │
│   [▶] train-model (id: 2)                                        │
│      ↓↓↓                                                         │
│   [◦] evaluate (id: 3)                                           │
└──────────────────────────────────────────────────────────────────┘
```

**Status indicators:**

- `✓` Completed (green)
- `▶` Running (yellow)
- `✗` Failed (red)
- `○` Canceled (magenta)
- `◦` Other (cyan)

---

## Complete Keyboard Reference

### Global Keys

| Key         | Action                        |
| ----------- | ----------------------------- |
| `q`         | Quit / Close popup            |
| `?`         | Show help popup               |
| `r`         | Refresh current view          |
| `A`         | Toggle auto-refresh           |
| `↑` / `↓`   | Navigate table rows           |
| `←` / `→`   | Switch focus between panes    |
| `Tab`       | Next detail tab               |
| `Shift+Tab` | Previous detail tab           |
| `Enter`     | Load details / Confirm action |

### Workflow Actions

| Key | Action                             |
| --- | ---------------------------------- |
| `n` | Create new workflow from spec file |
| `i` | Initialize workflow                |
| `I` | Re-initialize workflow             |
| `R` | Reset workflow status              |
| `x` | Run workflow locally               |
| `s` | Submit workflow to scheduler       |
| `W` | Watch workflow (recovery)          |
| `C` | Cancel workflow                    |
| `d` | Delete workflow                    |

### Job Actions (Jobs tab only)

| Key     | Action           |
| ------- | ---------------- |
| `Enter` | View job details |
| `l`     | View job logs    |
| `c`     | Cancel job       |
| `t`     | Terminate job    |
| `y`     | Retry failed job |
| `f`     | Filter jobs      |

### File Actions (Files tab only)

| Key     | Action             |
| ------- | ------------------ |
| `Enter` | View file contents |
| `f`     | Filter files       |

### Log Viewer

| Key             | Action               |
| --------------- | -------------------- |
| `Tab`           | Switch stdout/stderr |
| `/`             | Search               |
| `n`             | Next match           |
| `N`             | Previous match       |
| `g`             | Jump to top          |
| `G`             | Jump to bottom       |
| `↑` / `↓`       | Scroll one line      |
| `PgUp` / `PgDn` | Scroll 20 lines      |
| `y`             | Show file path       |
| `q` / `Esc`     | Close                |

### File Viewer

| Key             | Action          |
| --------------- | --------------- |
| `/`             | Search          |
| `n`             | Next match      |
| `N`             | Previous match  |
| `g`             | Jump to top     |
| `G`             | Jump to bottom  |
| `↑` / `↓`       | Scroll one line |
| `PgUp` / `PgDn` | Scroll 20 lines |
| `y`             | Show file path  |
| `q` / `Esc`     | Close           |

### Server Management

| Key | Action             |
| --- | ------------------ |
| `S` | Start torc-server  |
| `K` | Stop/Kill server   |
| `O` | Show server output |

### Connection Settings

| Key | Action                |
| --- | --------------------- |
| `u` | Change server URL     |
| `w` | Change user filter    |
| `a` | Toggle show all users |

---

## Server Management

The TUI can start and manage a `torc-server` instance directly:

1. **Starting a server**: Press `S` to start a server on the port from your configured URL
2. **Viewing output**: Press `O` to see the server's output in real-time
3. **Stopping**: Press `K` to stop the running server

The Connection bar shows the server status:

- `●` (green) - Server is running (managed by TUI)
- `○` (yellow) - Server was started but has stopped
- No indicator - Server not managed by TUI (external server)

**Note**: The TUI looks for `torc-server` in:

1. Same directory as the `torc` binary
2. Current working directory
3. System PATH

---

## Configuration

The TUI respects Torc's layered configuration system. Settings are loaded with this priority
(highest to lowest):

1. Interactive changes in TUI (press `u` to change server URL)
2. Environment variables (`TORC_CLIENT__API_URL`)
3. Local config file (`./torc.toml`)
4. User config file (`~/.config/torc/config.toml`)
5. System config file (`/etc/torc/config.toml`)
6. Default values

### Setting Up Configuration

Use the CLI to create a user configuration file:

```bash
# Create user config interactively
torc config init --user

# Show current configuration
torc config show
```

### Environment Variables

| Variable               | Description             | Default                                 |
| ---------------------- | ----------------------- | --------------------------------------- |
| `TORC_CLIENT__API_URL` | Torc server API URL     | `http://localhost:8080/torc-service/v1` |
| `USER`                 | Default username filter | Current system user                     |

### Log File Locations

Job logs are stored in the `output` directory by default:

```
output/
└── job_stdio/
    ├── job_{workflow_id}_{job_id}_{run_id}.o  # stdout
    └── job_{workflow_id}_{job_id}_{run_id}.e  # stderr
```

---

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

### Job actions not working

- Ensure you have the correct permissions
- Check that the server is responsive (press `r` to refresh)
- Some actions only apply to certain job states (e.g., can't retry a completed job)

### Screen rendering issues

- Ensure your terminal supports UTF-8 and 256 colors
- Try resizing your terminal window
- Press `r` to force a refresh

---

## Architecture

The TUI is organized into these modules:

```
src/tui/
├── mod.rs          # Entry point and event loop
├── app.rs          # Application state and business logic
├── api.rs          # Synchronous API client
├── ui.rs           # UI rendering with ratatui
├── dag.rs          # DAG layout computation
└── components.rs   # Reusable UI components
                    # (dialogs, log viewer, status bar)
```

The TUI is completely synchronous, using the blocking reqwest client—no async runtime overhead,
perfect for resource-constrained HPC environments.

---

## Comparison with torc-dash

| Feature           | TUI (`torc tui`)     | Web (`torc-dash`)    |
| ----------------- | -------------------- | -------------------- |
| Environment       | Terminal/SSH         | Web browser          |
| Startup           | Instant              | ~2 seconds           |
| Dependencies      | None (single binary) | None (single binary) |
| Workflow actions  | ✅                   | ✅                   |
| Job actions       | ✅                   | ✅                   |
| Log viewing       | ✅                   | ✅                   |
| DAG visualization | Text-based           | Interactive graph    |
| Resource plots    | Planned              | ✅                   |
| Event monitoring  | Planned              | ✅                   |
| File preview      | Via logs             | ✅                   |

Choose the **TUI** for: SSH sessions, HPC environments, quick operations, low-bandwidth connections.

Choose **torc-dash** for: Rich visualizations, file previews, resource plots, team dashboards.
