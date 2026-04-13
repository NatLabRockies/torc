# Torc Workflow Examples

This directory contains example workflow specifications in three formats: YAML, JSON5, and KDL.

## Directory Structure

```
examples/
├── yaml/     # YAML format examples (.yaml)
├── json/     # JSON5 format examples (.json5)
├── kdl/      # KDL format examples (.kdl)
└── README.md # This file
```

## Format Comparison

### YAML Format (`.yaml`)

- **Best for**: Most workflows, especially those using parameterization
- **Advantages**:
  - Full feature support including job/file parameterization
  - Human-readable with minimal syntax
  - Industry standard for configuration
- **Location**: `examples/yaml/`

### JSON5 Format (`.json5`)

- **Best for**: Workflows that need JSON compatibility with comments
- **Advantages**:
  - All YAML features with JSON-like structure
  - Supports comments and trailing commas
  - Easy programmatic manipulation
- **Location**: `examples/json/`

### KDL Format (`.kdl`)

- **Best for**: Simple to moderate workflows
- **Advantages**:
  - Clean, readable syntax
  - Support for actions, resource monitoring, and all core features
- **Limitations**:
  - No parameter expansion support (use YAML/JSON5 for parameterized workflows)
- **Location**: `examples/kdl/`

## Available Examples

### Simple Workflows

- **`sample_workflow`** - Complete example demonstrating files, user_data, resource requirements,
  jobs, and Slurm schedulers
- **`diamond_workflow`** - Classic diamond dependency pattern (fan-out and fan-in)
- **`three_stage_workflow`** - Multi-stage workflow with barriers

### Resource Monitoring

- **`resource_monitoring_demo`** - Demonstrates CPU and memory monitoring with time-series data
  collection

### Workflow Actions

- **`workflow_actions_simple`** - Basic workflow with on_workflow_start and on_workflow_complete
  actions
- **`workflow_actions_simple_slurm`** - Multi-stage workflow with automated Slurm node scheduling
  per stage
- **`workflow_actions_data_pipeline`** - Data pipeline with automated resource management
- **`workflow_actions_ml_training`** - ML training with dynamic GPU allocation using on_jobs_ready
  actions

### Complex Pipelines

- **`slurm_staged_pipeline`** - Multi-stage Slurm pipeline with automated scheduling and resource
  monitoring

### Parameterized Workflows (YAML/JSON5 only)

These workflows use parameter expansion to generate many jobs from concise specifications:

- **`hundred_jobs_parameterized`** - Generates 100 jobs using parameter ranges
- **`data_pipeline_parameterized`** - Multi-dataset pipeline with parameter sweeps
- **`hyperparameter_sweep`** - ML hyperparameter grid search (3×3×2 = 18 training jobs)
- **`simulation_sweep`** - Parameter sweep for scientific simulations
- **`multi_stage_barrier_pattern`** - Efficient multi-stage workflow using barrier jobs (1000+ jobs
  per stage)

**Note**: Parameterized workflows are only available in YAML and JSON5 formats, as KDL does not
currently support parameter expansion. For these workflows, simplified KDL versions with fewer jobs
may be available for reference.

## Usage

### Creating a workflow from a specification file:

```bash
# All three formats are supported
torc create examples/yaml/sample_workflow.yaml
torc create examples/json/sample_workflow.json5
torc create examples/kdl/sample_workflow.kdl
```

### Quick execution:

```bash
# Create and run locally
torc run examples/yaml/sample_workflow.yaml

# Create and submit to scheduler
torc submit examples/yaml/workflow_actions_data_pipeline.yaml
```

## Choosing a Format

**Use YAML when**:

- You need parameter expansion for job/file generation
- You want the most widely supported format
- You're creating complex workflows with many repeated patterns

**Use JSON5 when**:

- You need JSON compatibility
- You want to programmatically generate or manipulate workflows
- You prefer JSON-like structure with comment support

**Use KDL when**:

- You prefer minimal, clean syntax
- You want a more modern configuration language

All three formats support the same core features (jobs, files, user_data, resource requirements,
Slurm schedulers, actions, and resource monitoring).
