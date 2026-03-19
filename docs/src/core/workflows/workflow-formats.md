# Workflow Specification Formats

Torc supports three workflow specification formats: YAML, JSON5, and KDL. All formats provide the
same functionality with different syntaxes to suit different preferences and use cases.

## Format Overview

| Feature                 | YAML | JSON5 | KDL |
| ----------------------- | ---- | ----- | --- |
| Parameter Expansion     | ✓    | ✓     | ✓   |
| Comments                | ✓    | ✓     | ✓   |
| Trailing Commas         | ✗    | ✓     | N/A |
| Human-Readable          | ✓✓✓  | ✓✓    | ✓✓✓ |
| Programmatic Generation | ✓✓   | ✓✓✓   | ✓   |
| Industry Standard       | ✓✓✓  | ✓✓    | ✓   |
| Jobs, Files, Resources  | ✓    | ✓     | ✓   |
| User Data               | ✓    | ✓     | ✓   |
| Workflow Actions        | ✓    | ✓     | ✓   |
| Resource Monitoring     | ✓    | ✓     | ✓   |
| Slurm Schedulers        | ✓    | ✓     | ✓   |

## YAML Format

**Best for:** Most workflows, especially those using multi-line commands.

**File Extension:** `.yaml` or `.yml`

**Example:**

```yaml
name: data_processing_workflow
user: datauser
description: Multi-stage data processing pipeline

# File definitions
files:
  - name: raw_data
    path: /data/input/raw_data.csv
  - name: processed_data
    path: /data/output/processed_data.csv

# Resource requirements
resource_requirements:
  - name: small_job
    num_cpus: 2
    num_gpus: 0
    num_nodes: 1
    memory: 4g
    runtime: PT30M

# Jobs
jobs:
  - name: download_data
    command: wget https://example.com/data.csv -O ${files.output.raw_data}
    resource_requirements: small_job

  - name: process_data
    command: python process.py ${files.input.raw_data} -o ${files.output.processed_data}
    resource_requirements: small_job
    depends_on:
      - download_data

# Workflow actions
actions:
  - trigger_type: on_workflow_start
    action_type: run_commands
    commands:
      - mkdir -p /data/input /data/output
      - echo "Workflow started"
```

**Advantages:**

- Most widely used configuration format
- Excellent for complex workflows with many jobs
- Clean, readable syntax without brackets

**Disadvantages:**

- Indentation-sensitive
- Can be verbose for deeply nested structures

## JSON5 Format

**Best for:** Programmatic workflow generation and JSON compatibility.

**File Extension:** `.json5`

**Example:**

```json5
{
  name: "data_processing_workflow",
  user: "datauser",
  description: "Multi-stage data processing pipeline",

  // File definitions
  files: [
    {name: "raw_data", path: "/data/input/raw_data.csv"},
    {name: "processed_data", path: "/data/output/processed_data.csv"},
  ],

  // Resource requirements
  resource_requirements: [
    {
      name: "small_job",
      num_cpus: 2,
      num_gpus: 0,
      num_nodes: 1,
      memory: "4g",
      runtime: "PT30M",
    },
  ],

  // Jobs
  jobs: [
    {
      name: "download_data",
      command: "wget https://example.com/data.csv -O ${files.output.raw_data}",
      resource_requirements: "small_job",
    },
    {
      name: "process_data",
      command: "python process.py ${files.input.raw_data} -o ${files.output.processed_data}",
      resource_requirements: "small_job",
      depends_on: ["download_data"],
    },
  ],

  // Workflow actions
  actions: [
    {
      trigger_type: "on_workflow_start",
      action_type: "run_commands",
      commands: [
        "mkdir -p /data/input /data/output",
        "echo 'Workflow started'",
      ],
    },
  ],
}
```

**Advantages:**

- JSON-compatible (easy programmatic manipulation)
- Supports comments and trailing commas
- Familiar to JavaScript/JSON users

**Disadvantages:**

- More verbose than YAML
- More brackets and commas than YAML

## KDL Format

**Best for:** Simple to moderate workflows with clean syntax.

**File Extension:** `.kdl`

**Example:**

```kdl
name "data_processing_workflow"
user "datauser"
description "Multi-stage data processing pipeline"

// File definitions
file "raw_data" path="/data/input/raw_data.csv"
file "processed_data" path="/data/output/processed_data.csv"

// Resource requirements
resource_requirements "small_job" {
    num_cpus 2
    num_gpus 0
    num_nodes 1
    memory "4g"
    runtime "PT30M"
}

// Jobs
job "download_data" {
    command "wget https://example.com/data.csv -O ${files.output.raw_data}"
    resource_requirements "small_job"
}

job "process_data" {
    command "python process.py ${files.input.raw_data} -o ${files.output.processed_data}"
    resource_requirements "small_job"
    depends_on_job "download_data"
}

// Workflow actions
action {
    trigger_type "on_workflow_start"
    action_type "run_commands"
    command "mkdir -p /data/input /data/output"
    command "echo 'Workflow started'"
}
```

**Advantages:**

- Clean, minimal syntax
- No indentation requirements
- Supports all core Torc features

**Disadvantages:**

- Less familiar to most users
- Boolean values use special syntax (`#true`, `#false`)

### KDL-Specific Syntax Notes

1. **Boolean values**: Use `#true` and `#false` (not `true` or `false`)
   ```kdl
   resource_monitor {
       enabled #true
       generate_plots #false
   }
   ```

2. **Repeated child nodes**: Use multiple statements
   ```kdl
   action {
       command "echo 'First command'"
       command "echo 'Second command'"
   }
   ```

3. **User data**: Requires child nodes for properties
   ```kdl
   user_data "metadata" {
       is_ephemeral #true
       data "{\"key\": \"value\"}"
   }
   ```

## Execution Configuration

The `execution_config` section controls how jobs are executed and terminated. It supports three
modes:

- **direct**: Torc manages job execution and termination directly via signals
- **slurm**: Jobs are wrapped with `srun` and Slurm manages resource limits/termination
- **auto** (default): Uses `slurm` mode if `SLURM_JOB_ID` is set, otherwise `direct`

### YAML Example

```yaml
execution_config:
  mode: direct # Options: direct, slurm, auto
  limit_resources: true # Enforce memory limits in direct mode (default: true)

  # Direct mode settings
  termination_signal: SIGTERM # Signal before SIGKILL (default: SIGTERM)
  sigterm_lead_seconds: 30 # Seconds before SIGKILL to send signal (default: 30)
  sigkill_headroom_seconds: 60 # Seconds before end_time for SIGKILL (default: 60)
  timeout_exit_code: 152 # Exit code for timeout (default: 152, matches Slurm)
  oom_exit_code: 137 # Exit code for OOM kill (default: 137)

  # Slurm mode settings
  srun_termination_signal: "TERM@120" # srun --signal spec
  enable_cpu_bind: false # Allow Slurm CPU binding (default: false)
```

### KDL Example

```kdl
execution_config {
    mode "slurm"
    srun_termination_signal "TERM@120"
    sigkill_headroom_seconds 180
    enable_cpu_bind #true
}
```

### JSON5 Example

```json5
{
  execution_config: {
    mode: "direct",
    termination_signal: "SIGTERM",
    sigterm_lead_seconds: 30,
    sigkill_headroom_seconds: 60,
  },
}
```

### Configuration Fields

| Field                      | Type   | Default   | Description                                       |
| -------------------------- | ------ | --------- | ------------------------------------------------- |
| `mode`                     | string | `auto`    | Execution mode: `direct`, `slurm`, or `auto`      |
| `limit_resources`          | bool   | `true`    | Enforce memory limits in direct mode only         |
| `termination_signal`       | string | `SIGTERM` | Signal to send before SIGKILL (direct mode)       |
| `sigterm_lead_seconds`     | int    | `30`      | Seconds before SIGKILL to send termination signal |
| `sigkill_headroom_seconds` | int    | `60`      | Seconds before end_time to send SIGKILL           |
| `timeout_exit_code`        | int    | `152`     | Exit code for timed-out jobs                      |
| `oom_exit_code`            | int    | `137`     | Exit code for OOM-killed jobs                     |
| `srun_termination_signal`  | string | (none)    | Slurm signal spec (e.g., `TERM@120`)              |
| `enable_cpu_bind`          | bool   | `false`   | Allow Slurm CPU binding                           |

### When to Use Each Mode

- **direct mode**: Use when running outside Slurm, or when `srun`/`sacct` are unreliable. Torc
  manages termination via SIGTERM/SIGKILL signals.

- **slurm mode**: Use inside Slurm allocations when srun works correctly. Jobs are wrapped with
  `srun` and Slurm manages resource enforcement and termination.

- **auto mode** (default): Automatically selects `slurm` mode if running inside a Slurm allocation
  (detected via `SLURM_JOB_ID` environment variable), otherwise uses `direct` mode.

### Termination Timeline (Direct Mode)

When `sigkill_headroom_seconds=60` and `sigterm_lead_seconds=30`:

```
end_time - 90s: Send SIGTERM (or configured termination_signal)
end_time - 60s: Send SIGKILL to any still-running jobs
end_time:       Job runner exits
```

### Slurm Mode Headroom

In Slurm mode, `sigkill_headroom_seconds` controls the `srun --time` parameter. The job's srun step
time limit is set to `remaining_time - sigkill_headroom_seconds`, giving the job runner time to
detect completion and report results before the allocation expires.

If using `srun_termination_signal` (e.g., `TERM@120`), ensure its time value is less than
`sigkill_headroom_seconds` so the signal is sent before Slurm kills the step.

### Migration from slurm_config (Deprecated)

The `slurm_config` field and top-level `use_srun`, `limit_resources`, `srun_termination_signal`, and
`enable_cpu_bind` fields are **deprecated** and will be removed in a future release. Please migrate
to `execution_config`:

**Before (deprecated):**

```yaml
# Old style - deprecated
slurm_config:
  use_srun: true
  limit_resources: true
  srun_termination_signal: "TERM@120"

# Or flat fields - also deprecated
use_srun: true
limit_resources: true
```

**After (recommended):**

```yaml
# New style - use execution_config
execution_config:
  mode: slurm # Replaces use_srun: true
  srun_termination_signal: "TERM@120"
  sigkill_headroom_seconds: 180 # New: controls srun --time headroom
```

**Migration mapping:**

| Old Field                 | New Field in execution_config                |
| ------------------------- | -------------------------------------------- |
| `use_srun: true`          | `mode: slurm`                                |
| `use_srun: false`         | `mode: direct`                               |
| (not set)                 | `mode: auto` (default)                       |
| `limit_resources: false`  | `mode: direct` with `limit_resources: false` |
| `srun_termination_signal` | `srun_termination_signal`                    |
| `enable_cpu_bind`         | `enable_cpu_bind`                            |

> **Note**: `limit_resources: false` is only supported with `mode: direct`. If you previously used
> `limit_resources: false` with srun, switch to `mode: direct` to get the same behavior (jobs run
> without resource enforcement).

## Common Features Across All Formats

### Variable Substitution

All formats support the same variable substitution syntax:

- `${files.input.NAME}` - Input file path
- `${files.output.NAME}` - Output file path
- `${user_data.input.NAME}` - Input user data
- `${user_data.output.NAME}` - Output user data

### Supported Fields

All formats support:

- **Workflow metadata**: name, user, description
- **Jobs**: name, command, dependencies, resource requirements
- **Files**: name, path, modification time
- **User data**: name, data (JSON), ephemeral flag
- **Resource requirements**: CPUs, GPUs, memory, runtime
- **Slurm schedulers**: account, partition, walltime, etc.
- **Workflow actions**: triggers, action types, commands
- **Resource monitoring**: enabled, granularity, sampling interval

## Examples Directory

The Torc repository includes comprehensive examples in all three formats:

```
examples/
├── yaml/     # All workflows (15 examples)
├── json/     # All workflows (15 examples)
└── kdl/      # Non-parameterized workflows (9 examples)
```

Compare the same workflow in different formats to choose your preference:

- [sample_workflow.yaml](https://github.com/NatLabRockies/torc/blob/main/examples/yaml/sample_workflow.yaml)
- [sample_workflow.json5](https://github.com/NatLabRockies/torc/blob/main/examples/json/sample_workflow.json5)
- [sample_workflow.kdl](https://github.com/NatLabRockies/torc/blob/main/examples/kdl/sample_workflow.kdl)

See the [examples directory](https://github.com/NatLabRockies/torc/tree/main/examples) for the
complete collection.

## Creating Workflows

All formats use the same command:

```bash
torc workflows create examples/yaml/sample_workflow.yaml
torc workflows create examples/json/sample_workflow.json5
torc workflows create examples/kdl/sample_workflow.kdl
```

Or use the quick execution commands:

```bash
# Create and run locally
torc run examples/yaml/sample_workflow.yaml

# Create and submit to scheduler
torc submit examples/yaml/workflow_actions_data_pipeline.yaml
```

## Recommendations

**Start with YAML** if you're unsure.

**Switch to JSON5** if you need to programmatically generate workflows or prefer JSON syntax.

**Try KDL** if you prefer minimal syntax.

All three formats are fully supported and maintained. Choose based on your workflow complexity and
personal preference.
