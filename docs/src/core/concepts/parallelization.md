# Parallelization Strategies

Torc provides flexible parallelization strategies to accommodate different workflow patterns and
resource allocation scenarios. Understanding these strategies helps you optimize job execution for
your specific use case.

## Overview

Torc supports two primary approaches to parallel job execution:

1. **Resource-aware allocation** - Define per-job resource requirements and let runners
   intelligently select jobs that fit available resources
2. **Queue-depth parallelism** - Control the number of concurrent jobs without resource tracking

The choice between these approaches depends on your workflow characteristics and execution
environment.

## Use Case 1: Resource-Aware Job Allocation

This strategy is ideal for heterogeneous workflows where jobs have varying resource requirements
(CPU, memory, GPU, runtime). The server intelligently allocates jobs based on available compute node
resources.

### How It Works

When you define resource requirements for each job:

```yaml
resource_requirements:
  - name: small
    num_cpus: 2
    num_gpus: 0
    memory: 4g
    runtime: PT30M

  - name: large
    num_cpus: 16
    num_gpus: 2
    memory: 128g
    runtime: PT8H

jobs:
  - name: preprocessing
    command: ./preprocess.sh
    resource_requirements: small

  - name: model_training
    command: python train.py
    resource_requirements: large
```

The job runner pulls jobs from the server by detecting its available resources automatically.

```bash
torc run $WORKFLOW_ID
```

The server's `GET /workflows/{id}/claim_jobs_based_on_resources` endpoint:

1. Receives the runner's resource capacity
2. Queries the ready queue for jobs that fit within those resources
3. Returns a set of jobs that can run concurrently without over-subscription
4. Updates job status from `ready` to `pending` atomically

### Priority-Based Job Selection

When more than one ready job fits the runner's available resources, Torc orders the candidates by
job `priority` before claiming work.

- Higher priority values are claimed first
- The default priority is `0`
- Resource fit still applies first: a high-priority job that does not fit is skipped
- Priority works with both resource-aware allocation and queue-depth parallelism

Example:

```yaml
jobs:
  - name: preprocess
    command: ./preprocess.sh
    resource_requirements: small
    priority: 10

  - name: train_model
    command: python train.py
    resource_requirements: large
    priority: 100
```

In that workflow, `train_model` is preferred whenever it is ready and fits the requesting runner's
resources. If it does not fit, Torc continues scanning lower-priority ready jobs that do fit.

### Job Allocation Ambiguity: Two Approaches

When you have multiple compute nodes or schedulers with different capabilities, there are two ways
to handle job allocation:

#### Approach 1: Priority Only (Flexible but Potentially Ambiguous)

**How it works:**

- Jobs do NOT specify a particular scheduler/compute node
- The server uses job `priority` to decide which ready jobs to consider first
- Any runner with sufficient resources can claim any ready job

**Tradeoffs:**

✅ **Advantages:**

- Maximum flexibility - any runner can execute any compatible job
- Better resource utilization - if GPU runner is idle, it can pick up CPU-only jobs
- Simpler workflow specifications - no need to explicitly map jobs to schedulers
- Fault tolerance - if one runner fails, others can pick up its jobs

❌ **Disadvantages:**

- Ambiguity - no guarantee GPU jobs go to GPU runners
- Potential inefficiency - high-memory jobs might land on low-memory nodes if timing is unlucky
- Less predictable job placement

**When to use:**

- Homogeneous or mostly-homogeneous compute resources
- Workflows where job placement flexibility is valuable
- When you want runners to opportunistically pick up work
- Development and testing environments

#### Approach 2: Scheduler ID (Deterministic but Less Flexible)

**How it works:**

- Define scheduler configurations in your workflow spec
- Assign each job a specific `scheduler_id`
- Runners provide their `scheduler_config_id` when requesting jobs
- Server only returns jobs matching that scheduler ID

**Example workflow specification:**

```yaml
slurm_schedulers:
  - name: gpu_cluster
    partition: gpu
    account: myproject

  - name: highmem_cluster
    partition: highmem
    account: myproject

jobs:
  - name: model_training
    command: python train.py
    resource_requirements: large
    slurm_scheduler: gpu_cluster     # Binds to specific scheduler

  - name: large_analysis
    command: ./analyze.sh
    resource_requirements: highmem
    slurm_scheduler: highmem_cluster
```

**Example runner invocation:**

```bash
# GPU runner - only pulls jobs assigned to gpu_cluster
torc-slurm-job-runner $WORKFLOW_ID \
  --scheduler-config-id 1 \
  --num-cpus 32 \
  --num-gpus 8

# High-memory runner - only pulls jobs assigned to highmem_cluster
torc-slurm-job-runner $WORKFLOW_ID \
  --scheduler-config-id 2 \
  --num-cpus 64 \
  --memory-gb 512
```

**Tradeoffs:**

✅ **Advantages:**

- Zero ambiguity - jobs always run on intended schedulers
- Predictable job placement
- Prevents GPU jobs from landing on CPU-only nodes
- Clear workflow specification - explicit job→scheduler mapping
- Better for heterogeneous clusters (GPU vs CPU vs high-memory)

❌ **Disadvantages:**

- Less flexibility - idle runners can't help other queues
- Potential resource underutilization - GPU runner sits idle while CPU queue is full
- More complex workflow specifications
- If a scheduler fails, its jobs remain stuck until that scheduler returns

**When to use:**

- Highly heterogeneous compute resources (GPU clusters, high-memory nodes, specialized hardware)
- Production workflows requiring predictable job placement
- Multi-cluster environments
- When job-resource matching is critical (e.g., GPU-only codes, specific hardware requirements)
- Slurm or HPC scheduler integrations

### Choosing Between Sort Method and Scheduler ID

| Scenario                                        | Recommended Approach | Rationale                          |
| ----------------------------------------------- | -------------------- | ---------------------------------- |
| All jobs can run anywhere                       | Priority only        | Maximum flexibility, simplest spec |
| Some jobs need GPUs, some don't                 | Scheduler ID         | Prevent GPU waste on CPU jobs      |
| Multi-cluster Slurm environment                 | Scheduler ID         | Jobs must target correct clusters  |
| Development/testing                             | Priority only        | Easier to experiment               |
| Production with SLAs                            | Scheduler ID         | Predictable resource usage         |
| Homogeneous compute nodes                       | Priority only        | No benefit to restricting          |
| Specialized hardware (GPUs, high-memory, FPGAs) | Scheduler ID         | Match jobs to capabilities         |

You can also **mix approaches**: Use `scheduler_id` for jobs with strict requirements, leave it NULL
for flexible jobs.

## Use Case 2: Queue-Depth Parallelism

This strategy is ideal for workflows with homogeneous resource requirements where you simply want to
control the level of parallelism.

### How It Works

Instead of tracking resources, you specify a maximum number of concurrent jobs:

```bash
torc run $WORKFLOW_ID \
  --max-parallel-jobs 10 \
  --output-dir ./results
```

or with Slurm:

```bash
torc slurm schedule-nodes $WORKFLOW_ID \
  --scheduler-config-id 1 \
  --num-hpc-jobs 4 \
  --max-parallel-jobs 8
```

**Server behavior:**

The `GET /workflows/{id}/claim_next_jobs` endpoint:

1. Accepts `limit` parameter specifying maximum jobs to return
2. Ignores all resource requirements
3. Returns the next N ready jobs from the queue
4. Updates their status from `ready` to `pending`

**Runner behavior:**

- Maintains a count of running jobs
- When count falls below `max_parallel_jobs`, requests more work
- Does NOT track CPU, memory, GPU, or other resources
- Simply enforces the concurrency limit

### Ignoring Resource Consumption

This is a critical distinction: when using `--max-parallel-jobs`, the runner **completely ignores
current resource consumption**.

**Normal resource-aware mode:**

```
Runner has: 32 CPUs, 128 GB memory
Job A needs: 16 CPUs, 64 GB
Job B needs: 16 CPUs, 64 GB
Job C needs: 16 CPUs, 64 GB

Runner starts Job A and Job B (resources fully allocated)
Job C waits until resources free up
```

**Queue-depth mode with --max-parallel-jobs 3:**

```
Runner has: 32 CPUs, 128 GB memory (IGNORED)
Job A needs: 16 CPUs, 64 GB (IGNORED)
Job B needs: 16 CPUs, 64 GB (IGNORED)
Job C needs: 16 CPUs, 64 GB (IGNORED)

Runner starts Job A, Job B, and Job C simultaneously
Total requested: 48 CPUs, 192 GB (exceeds node capacity!)
System may: swap, OOM, or throttle performance
```

### When to Use Queue-Depth Parallelism

**✅ Use queue-depth parallelism when:**

1. **All jobs have similar resource requirements**
   ```yaml
   # All jobs use ~4 CPUs, ~8GB memory
   jobs:
     - name: process_file_1
       command: ./process.sh file1.txt
     - name: process_file_2
       command: ./process.sh file2.txt
     # ... 100 similar jobs
   ```

2. **Resource requirements are negligible compared to node capacity**
   - Running 100 lightweight Python scripts on a 64-core machine
   - I/O-bound jobs that don't consume much CPU/memory

3. **Jobs are I/O-bound or sleep frequently**
   - Data download jobs
   - Jobs waiting on external services
   - Polling or monitoring tasks

4. **You want simplicity over precision**
   - Quick prototypes
   - Testing workflows
   - Simple task queues

5. **Jobs self-limit their resource usage**
   - Application has built-in thread pools
   - Container resource limits
   - OS-level cgroups or resource controls

**❌ Avoid queue-depth parallelism when:**

1. **Jobs have heterogeneous resource requirements**
   - Mix of 2-CPU and 32-CPU jobs
   - Some jobs need 4GB, others need 128GB

2. **Resource contention causes failures**
   - Out-of-memory errors
   - CPU thrashing
   - GPU memory exhaustion

3. **You need efficient bin-packing**
   - Maximizing node utilization
   - Complex resource constraints

4. **Jobs are compute-intensive**
   - CPU-bound numerical simulations
   - Large matrix operations
   - Video encoding

### Queue-Depth Parallelism in Practice

**Example 1: Slurm with Queue Depth**

```bash
# Schedule 4 Slurm nodes, each running up to 8 concurrent jobs
torc slurm schedule-nodes $WORKFLOW_ID \
  --scheduler-config-id 1 \
  --num-hpc-jobs 4 \
  --max-parallel-jobs 8
```

This creates 4 Slurm job allocations. Each allocation runs a worker that:

- Pulls up to 8 jobs at a time
- Runs them concurrently
- Requests more when any job completes

Total concurrency: up to 32 jobs (4 nodes × 8 jobs/node)

**Example 2: Local Runner with Queue Depth**

```bash
# Run up to 20 jobs concurrently on local machine
torc-job-runner $WORKFLOW_ID \
  --max-parallel-jobs 20 \
  --output-dir ./output
```

**Example 3: Mixed Approach**

You can even run multiple runners with different strategies:

```bash
# Terminal 1: Resource-aware runner for large jobs
torc run $WORKFLOW_ID \
  --num-cpus 32 \
  --memory-gb 256

# Terminal 2: Queue-depth runner for small jobs
torc run $WORKFLOW_ID \
  --max-parallel-jobs 50
```

The ready queue serves both runners. The resource-aware runner gets large jobs that fit its
capacity, while the queue-depth runner gets small jobs for fast parallel execution.

### Performance Characteristics

**Resource-aware allocation:**

- Query complexity: O(jobs in ready queue)
- Requires computing resource sums
- Slightly slower due to filtering and sorting
- Better resource utilization

**Queue-depth allocation:**

- Query complexity: O(1) with limit
- Simple LIMIT clause, no resource computation
- Faster queries
- Simpler logic

For workflows with thousands of ready jobs, queue-depth allocation has lower overhead.

## Best Practices

1. **Start with resource-aware allocation** for new workflows
   - Better default behavior
   - Prevents resource over-subscription
   - Easier to debug resource issues

2. **Use scheduler_id for production multi-cluster workflows**
   - Explicit job placement
   - Predictable resource usage
   - Better for heterogeneous resources

3. **Use priority for flexible single-cluster workflows**
   - Simpler specifications
   - Lets you prefer urgent or expensive work
   - Good for homogeneous resources

4. **Use queue-depth parallelism for homogeneous task queues**
   - Many similar jobs
   - I/O-bound workloads
   - When simplicity matters more than precision

5. **Monitor resource usage** when switching strategies
   - Check for over-subscription
   - Verify expected parallelism
   - Look for resource contention

6. **Test with small workflows first**
   - Validate job allocation behavior
   - Check resource accounting
   - Ensure jobs run on intended schedulers

## Summary

| Strategy                      | Use When                                | Allocation Method                         | Resource Tracking |
| ----------------------------- | --------------------------------------- | ----------------------------------------- | ----------------- |
| Resource-aware + priority     | Heterogeneous jobs, flexible allocation | Server filters by resources               | Yes               |
| Resource-aware + scheduler_id | Heterogeneous jobs, strict allocation   | Server filters by resources AND scheduler | Yes               |
| Queue-depth                   | Homogeneous jobs, simple parallelism    | Server returns next N jobs                | No                |

Choose the strategy that best matches your workflow characteristics and execution environment. You
can even mix strategies across different runners for maximum flexibility.
