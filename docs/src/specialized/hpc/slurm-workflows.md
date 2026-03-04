# Slurm Overview

This document explains how Torc simplifies running workflows on Slurm-based HPC systems. The key
insight is that **you don't need to understand Slurm schedulers or workflow actions** to run
workflows on HPC systems—Torc handles this automatically.

## The Simple Approach

Running a workflow on Slurm requires just two things:

1. **Define your jobs with resource requirements**
2. **Submit with `submit-slurm`**

That's it. Torc will analyze your workflow, generate appropriate Slurm configurations, and submit
everything for execution.

> **⚠️ Important:** The `submit-slurm` command uses heuristics to auto-generate Slurm schedulers and
> workflow actions. For complex workflows with unusual dependency patterns, the generated
> configuration may not be optimal and could result in suboptimal allocation timing. **Always
> preview the configuration first** using `torc slurm generate` (see
> [Previewing Generated Configuration](#previewing-generated-configuration)) before submitting
> production workflows.

### Example Workflow

Here's a complete workflow specification that runs on Slurm:

```yaml
name: data_analysis_pipeline
description: Analyze experimental data with preprocessing, training, and evaluation

resource_requirements:
  - name: light
    num_cpus: 4
    memory: 8g
    runtime: PT30M

  - name: compute
    num_cpus: 32
    memory: 64g
    runtime: PT2H

  - name: gpu
    num_cpus: 16
    num_gpus: 2
    memory: 128g
    runtime: PT4H

jobs:
  - name: preprocess
    command: python preprocess.py --input data/ --output processed/
    resource_requirements: light

  - name: train_model
    command: python train.py --data processed/ --output model/
    resource_requirements: gpu
    depends_on: [preprocess]

  - name: evaluate
    command: python evaluate.py --model model/ --output results/
    resource_requirements: compute
    depends_on: [train_model]

  - name: generate_report
    command: python report.py --results results/
    resource_requirements: light
    depends_on: [evaluate]
```

### Submitting the Workflow

```bash
torc submit-slurm --account myproject workflow.yaml
```

Torc will:

1. Detect which HPC system you're on (e.g., NLR Kestrel)
2. Match each job's requirements to appropriate partitions
3. Generate Slurm scheduler configurations
4. Create workflow actions that stage resource allocation based on dependencies
5. Submit the workflow for execution

## How It Works

When you use `submit-slurm`, Torc performs intelligent analysis of your workflow:

### 1. Per-Job Scheduler Generation

Each job gets its own Slurm scheduler configuration based on its resource requirements. This means:

- Jobs are matched to the most appropriate partition
- Memory, CPU, and GPU requirements are correctly specified
- Walltime is set to the partition's maximum (explained below)

### 2. Staged Resource Allocation

Torc analyzes job dependencies and creates **staged workflow actions**:

- **Jobs without dependencies** trigger `on_workflow_start` — resources are allocated immediately
- **Jobs with dependencies** trigger `on_jobs_ready` — resources are allocated only when the job
  becomes ready to run

This prevents wasting allocation time on resources that aren't needed yet. For example, in the
workflow above:

- `preprocess` resources are allocated at workflow start
- `train_model` resources are allocated when `preprocess` completes
- `evaluate` resources are allocated when `train_model` completes
- `generate_report` resources are allocated when `evaluate` completes

### 3. Walltime Calculation

By default, Torc sets the walltime to **1.5× your longest job's runtime** (capped at the partition's
maximum). This provides headroom for jobs that run slightly longer than expected.

You can customize this behavior:

- `--walltime-strategy max-job-runtime` (default): Uses longest job runtime × multiplier
- `--walltime-strategy max-partition-time`: Uses the partition's maximum walltime
- `--walltime-multiplier 2.0`: Change the safety multiplier (default: 1.5)

See [Walltime Strategy Options](#walltime-strategy-options) for details.

### 4. HPC Profile Knowledge

Torc includes built-in knowledge of HPC systems like NLR Kestrel, including:

- Available partitions and their resource limits
- GPU configurations
- Memory and CPU specifications
- Special requirements (e.g., minimum node counts for high-bandwidth partitions)

> **Using an unsupported HPC?** Please
> [request built-in support](https://github.com/NatLabRockies/torc/issues) so everyone benefits. You
> can also [create a custom profile](./custom-hpc-profile.md) for immediate use.

## Resource Requirements Specification

Resource requirements are the key to the simplified workflow. Define them once and reference them
from jobs:

```yaml
resource_requirements:
  - name: small
    num_cpus: 4
    num_gpus: 0
    num_nodes: 1
    memory: 8g
    runtime: PT1H

  - name: gpu_training
    num_cpus: 32
    num_gpus: 4
    num_nodes: 1
    memory: 256g
    runtime: PT8H
```

### Fields

| Field       | Description               | Example             |
| ----------- | ------------------------- | ------------------- |
| `name`      | Reference name for jobs   | `"compute"`         |
| `num_cpus`  | CPU cores required        | `32`                |
| `num_gpus`  | GPUs required (0 if none) | `2`                 |
| `num_nodes` | Nodes required            | `1`                 |
| `memory`    | Memory with unit suffix   | `"64g"`, `"512m"`   |
| `runtime`   | ISO8601 duration          | `"PT2H"`, `"PT30M"` |

### Runtime Format

Use ISO8601 duration format:

- `PT30M` — 30 minutes
- `PT2H` — 2 hours
- `PT1H30M` — 1 hour 30 minutes
- `P1D` — 1 day
- `P2DT4H` — 2 days 4 hours

## Job Dependencies

Define dependencies explicitly or implicitly through file/data relationships:

### Explicit Dependencies

```yaml
jobs:
  - name: step1
    command: ./step1.sh
    resource_requirements: small

  - name: step2
    command: ./step2.sh
    resource_requirements: small
    depends_on: [step1]

  - name: step3
    command: ./step3.sh
    resource_requirements: small
    depends_on: [step1, step2]  # Waits for both
```

### Implicit Dependencies (via Files)

```yaml
files:
  - name: raw_data
    path: /data/raw.csv
  - name: processed_data
    path: /data/processed.csv

jobs:
  - name: process
    command: python process.py
    input_files: [raw_data]
    output_files: [processed_data]
    resource_requirements: compute

  - name: analyze
    command: python analyze.py
    input_files: [processed_data]  # Creates implicit dependency on 'process'
    resource_requirements: compute
```

## Previewing Generated Configuration

> **Recommended Practice:** Always preview the generated configuration before submitting to Slurm,
> especially for complex workflows. This allows you to verify that schedulers and actions are
> appropriate for your workflow structure.

### Viewing the Execution Plan

Before generating schedulers, visualize how your workflow will execute in stages:

```bash
torc workflows execution-plan workflow.yaml
```

This shows the execution stages, which jobs run at each stage, and (if schedulers are defined) when
Slurm allocations are requested. See
[Visualizing Workflow Structure](../../core/workflows/visualizing-workflows.md) for detailed
examples.

### Generating Slurm Configuration

Preview what Torc will generate:

```bash
torc slurm generate --account myproject --profile kestrel workflow.yaml
```

This outputs the complete workflow with generated schedulers and actions:

#### Scheduler Grouping Options

By default, Torc creates **one scheduler per unique `resource_requirements` name**. This means if
you have three jobs with three different resource requirement definitions (e.g., `cpu`, `memory`,
`mixed`), you get three schedulers—even if all three would fit on the same partition.

The `--group-by` option controls how jobs are grouped into schedulers:

```bash
# Default: one scheduler per resource_requirements name
torc slurm generate --account myproject workflow.yaml
torc slurm generate --account myproject --group-by resource-requirements workflow.yaml
# Result: 3 schedulers (cpu_scheduler, memory_scheduler, mixed_scheduler)

# Group by partition: one scheduler per partition
torc slurm generate --account myproject --group-by partition workflow.yaml
# Result: 1 scheduler (short_scheduler) if all jobs fit on the "short" partition
```

**When to use `--group-by partition`:**

- Your workflow has many small resource requirement definitions that all fit on the same partition
- You want to minimize Slurm queue overhead by reducing the number of allocations
- Jobs have similar characteristics and can share nodes efficiently

**When to use `--group-by resource-requirements` (default):**

- Jobs have significantly different resource profiles that benefit from separate allocations
- You want fine-grained control over which jobs share resources
- You're debugging and want clear separation between job types

When grouping by partition, the scheduler uses the **maximum** resource values from all grouped
requirements (max memory, max CPUs, max runtime, etc.) to ensure all jobs can run.

#### Walltime Strategy Options

The `--walltime-strategy` option controls how Torc calculates the walltime for generated schedulers:

```bash
# Default: use max job runtime with a safety multiplier (1.5x)
torc slurm generate --account myproject workflow.yaml
torc slurm generate --account myproject --walltime-strategy max-job-runtime workflow.yaml

# Use the partition's maximum allowed walltime
torc slurm generate --account myproject --walltime-strategy max-partition-time workflow.yaml
```

**Walltime strategies:**

| Strategy             | Description                                                                               |
| -------------------- | ----------------------------------------------------------------------------------------- |
| `max-job-runtime`    | Uses the longest job's runtime × multiplier (default: 1.5x). Capped at partition max.     |
| `max-partition-time` | Uses the partition's maximum walltime. More conservative but may impact queue scheduling. |

**Customizing the multiplier:**

The `--walltime-multiplier` option (default: 1.5) provides a safety margin when using
`max-job-runtime`:

```bash
# Use 2x the max job runtime for extra buffer
torc slurm generate --account myproject --walltime-multiplier 2.0 workflow.yaml

# Use exact job runtime (no buffer - use with caution)
torc slurm generate --account myproject --walltime-multiplier 1.0 workflow.yaml
```

**When to use `max-job-runtime` (default):**

- You want better queue scheduling (shorter walltime requests often get prioritized)
- Your job runtime estimates are reasonably accurate
- You prefer the Torc runner to exit early rather than holding idle allocations

**When to use `max-partition-time`:**

- Your job runtimes are highly variable or unpredictable
- You consistently underestimate job runtimes
- Queue priority is not a concern

```yaml
name: data_analysis_pipeline
# ... original content ...

jobs:
  - name: preprocess
    command: python preprocess.py --input data/ --output processed/
    resource_requirements: light
    scheduler: preprocess_scheduler

  # ... more jobs ...

slurm_schedulers:
  - name: preprocess_scheduler
    account: myproject
    mem: 8g
    nodes: 1
    walltime: "04:00:00"

  - name: train_model_scheduler
    account: myproject
    mem: 128g
    nodes: 1
    gres: "gpu:2"
    walltime: "04:00:00"

  # ... more schedulers ...

actions:
  - trigger_type: on_workflow_start
    action_type: schedule_nodes
    scheduler: preprocess_scheduler
    scheduler_type: slurm
    num_allocations: 1

  - trigger_type: on_jobs_ready
    action_type: schedule_nodes
    jobs: [train_model]
    scheduler: train_model_scheduler
    scheduler_type: slurm
    num_allocations: 1

  # ... more actions ...
```

Save the output to inspect or modify before submission:

```bash
torc slurm generate --account myproject workflow.yaml -o workflow_with_schedulers.yaml
```

## Torc Server Considerations

The Torc server must be accessible to compute nodes. Options include:

1. **Shared server** (Recommended): A team member allocates a dedicated server in the HPC
   environment
2. **Login node**: Suitable for small workflows with few, long-running jobs

For large workflows with many short jobs, a dedicated server prevents overloading login nodes.

## Best Practices

### 1. Focus on Resource Requirements

Spend time accurately defining resource requirements. Torc handles the rest:

```yaml
resource_requirements:
  # Be specific about what each job type needs
  - name: io_heavy
    num_cpus: 4
    memory: 32g      # High memory for data loading
    runtime: PT1H

  - name: compute_heavy
    num_cpus: 64
    memory: 16g      # Less memory, more CPU
    runtime: PT4H
```

### 2. Use Meaningful Names

Name resource requirements by their purpose, not by partition:

```yaml
# Good - describes the workload
resource_requirements:
  - name: data_preprocessing
  - name: model_training
  - name: inference

# Avoid - ties you to specific infrastructure
resource_requirements:
  - name: short_partition
  - name: gpu_h100
```

### 3. Group Similar Jobs

Jobs with similar requirements can share resource requirement definitions:

```yaml
resource_requirements:
  - name: quick_task
    num_cpus: 2
    memory: 4g
    runtime: PT15M

jobs:
  - name: validate_input
    command: ./validate.sh
    resource_requirements: quick_task

  - name: check_output
    command: ./check.sh
    resource_requirements: quick_task
    depends_on: [main_process]
```

### 4. Test Locally First

Validate your workflow logic locally before submitting to HPC:

```bash
# Run locally (without Slurm)
torc run workflow.yaml

# Then submit to HPC
torc submit-slurm --account myproject workflow.yaml
```

## Limitations and Caveats

The auto-generation in `submit-slurm` uses heuristics that work well for common workflow patterns
but may not be optimal for all cases:

### When Auto-Generation Works Well

- **Linear pipelines**: A → B → C → D
- **Fan-out patterns**: One job unblocks many (e.g., preprocess → 100 work jobs)
- **Fan-in patterns**: Many jobs unblock one (e.g., 100 work jobs → postprocess)
- **Simple DAGs**: Clear dependency structures with distinct resource tiers

### When to Use Manual Configuration

Consider using `torc slurm generate` to preview and manually adjust, or define schedulers manually,
when:

- **Complex dependency graphs**: Multiple interleaved dependency patterns
- **Shared schedulers**: You want multiple jobs to share the same Slurm allocation
- **Custom timing**: Specific requirements for when allocations should be requested
- **Resource optimization**: Fine-tuning to minimize allocation waste
- **Multi-node jobs**: Jobs requiring coordination across multiple nodes

### What Could Go Wrong

Without previewing, auto-generation might:

1. **Request allocations too early**: Wasting queue time waiting for dependencies
2. **Request allocations too late**: Adding latency to job startup
3. **Create suboptimal scheduler groupings**: Not sharing allocations when beneficial
4. **Miss optimization opportunities**: Not recognizing patterns that could share resources

**Best Practice**: For production workflows, always run `torc slurm generate` first, review the
output, and submit the reviewed configuration with `torc submit`.

## Advanced: Manual Scheduler Configuration

For advanced users who need fine-grained control, you can define schedulers and actions manually.
See [Advanced Slurm Configuration](./slurm.md) for details.

Common reasons for manual configuration:

- Non-standard partition requirements
- Custom Slurm directives (e.g., `--constraint`)
- Multi-node jobs with specific topology requirements
- Reusing allocations across multiple jobs for efficiency

## Troubleshooting

### "No partition found for job"

Your resource requirements exceed what's available. Check:

- Memory doesn't exceed partition limits
- Runtime doesn't exceed partition walltime
- GPU count is available on GPU partitions

Use `torc hpc partitions <profile>` to see available resources.

### Jobs Not Starting

Ensure the Torc server is accessible from compute nodes:

```bash
# From a compute node
curl $TORC_API_URL/health
```

### Wrong Partition Selected

Use `torc hpc match` to see which partitions match your requirements:

```bash
torc hpc match kestrel --cpus 32 --memory 64g --walltime 02:00:00 --gpus 2
```

## See Also

- [Visualizing Workflow Structure](../../core/workflows/visualizing-workflows.md) — Execution plans
  and DAG visualization
- [HPC Profiles](./hpc-profiles.md) — Detailed HPC profile usage
- [Advanced Slurm Configuration](./slurm.md) — Manual Slurm scheduler setup
- [Resource Requirements Reference](../../core/reference/resources.md) — Complete specification
- [Workflow Actions](../design/workflow-actions.md) — Understanding actions
