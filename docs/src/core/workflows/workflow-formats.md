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
