# Tutorial 3: User Data Dependencies

This tutorial teaches you how to pass structured data (JSON) between jobs using Torc's **user_data**
feature—an alternative to file-based dependencies that stores data directly in the database.

## Learning Objectives

By the end of this tutorial, you will:

- Understand what user_data is and when to use it instead of files
- Learn how to define user_data entries and reference them in jobs
- Know how to update user_data from within a job
- See how user_data creates implicit dependencies (like files)

## Prerequisites

- Completed [Tutorial 2: Diamond Workflow](./diamond.md)
- Torc server running
- `jq` command-line tool installed (for JSON parsing)

## What is User Data?

**User data** is Torc's mechanism for passing small, structured data between jobs without creating
actual files. The data is stored in the Torc database and can be:

- JSON objects (configurations, parameters)
- Arrays
- Simple values (strings, numbers)

Like files, user_data creates **implicit dependencies**: a job that reads user_data will be blocked
until the job that writes it completes.

### User Data vs Files

| Feature  | User Data                | Files                    |
| -------- | ------------------------ | ------------------------ |
| Storage  | Torc database            | Filesystem               |
| Size     | Small (KB)               | Any size                 |
| Format   | JSON                     | Any format               |
| Access   | Via `torc user-data` CLI | Direct file I/O          |
| Best for | Config, params, metadata | Datasets, binaries, logs |

## Step 1: Create the Workflow Specification

Save as `user_data_workflow.yaml`:

```yaml
name: config_pipeline
description: Jobs that pass configuration via user_data

jobs:
  - name: generate_config
    command: |
      echo '{"learning_rate": 0.001, "batch_size": 32, "epochs": 10}' > /tmp/config.json
      torc user-data update ${user_data.output.ml_config} \
        --data "$(cat /tmp/config.json)"
    resource_requirements: minimal

  - name: train_model
    command: |
      echo "Training with config:"
      torc user-data get ${user_data.input.ml_config} | jq '.data'
      # In a real workflow: python train.py --config="${user_data.input.ml_config}"
    resource_requirements: gpu_large

  - name: evaluate_model
    command: |
      echo "Evaluating with config:"
      torc user-data get ${user_data.input.ml_config} | jq '.data'
      # In a real workflow: python evaluate.py --config="${user_data.input.ml_config}"
    resource_requirements: gpu_small

user_data:
  - name: ml_config
    data: null  # Will be populated by generate_config job

resource_requirements:
  - name: minimal
    num_cpus: 1
    memory: 1g
    runtime: PT5M

  - name: gpu_small
    num_cpus: 4
    num_gpus: 1
    memory: 16g
    runtime: PT1H

  - name: gpu_large
    num_cpus: 8
    num_gpus: 2
    memory: 32g
    runtime: PT4H
```

### Understanding the Specification

Key elements:

- **`user_data:` section** - Defines data entries, similar to `files:`
- **`data: null`** - Initial value; will be populated by a job
- **`${user_data.output.ml_config}`** - Job will write to this user_data (creates it)
- **`${user_data.input.ml_config}`** - Job reads from this user_data (creates dependency)

The dependency flow:

1. `generate_config` outputs `ml_config` → runs first
2. `train_model` and `evaluate_model` input `ml_config` → blocked until step 1 completes
3. After `generate_config` finishes, both become ready and can run in parallel

## Step 2: Create and Initialize the Workflow

```bash
# Create the workflow
WORKFLOW_ID=$(torc create user_data_workflow.yaml -f json | jq -r '.id')
echo "Created workflow: $WORKFLOW_ID"

# Initialize jobs
torc workflows init $WORKFLOW_ID
```

## Step 3: Check Initial State

Before running, examine the user_data:

```bash
# Check user_data - should be null
torc user-data list $WORKFLOW_ID
```

Output:

```
╭────┬───────────┬──────┬─────────────╮
│ ID │ Name      │ Data │ Workflow ID │
├────┼───────────┼──────┼─────────────┤
│ 1  │ ml_config │ null │ 1           │
╰────┴───────────┴──────┴─────────────╯
```

Check job statuses:

```bash
torc jobs list $WORKFLOW_ID
```

You should see:

- `generate_config`: **ready** (no input dependencies)
- `train_model`: **blocked** (waiting for `ml_config`)
- `evaluate_model`: **blocked** (waiting for `ml_config`)

## Step 4: Run the Workflow

```bash
torc run $WORKFLOW_ID
```

## Step 5: Observe the Data Flow

After `generate_config` completes, check the updated user_data:

```bash
torc user-data list $WORKFLOW_ID -f json | jq '.[] | {name, data}'
```

Output:

```json
{
  "name": "ml_config",
  "data": {
    "learning_rate": 0.001,
    "batch_size": 32,
    "epochs": 10
  }
}
```

The data is now stored in the database. At this point:

- `train_model` and `evaluate_model` unblock
- Both can read the configuration and run in parallel

## Step 6: Verify Completion

After the workflow completes:

```bash
torc results list $WORKFLOW_ID
```

All three jobs should show return code 0.

## How User Data Dependencies Work

The mechanism is identical to file dependencies:

| Syntax                     | Meaning              | Effect                         |
| -------------------------- | -------------------- | ------------------------------ |
| `${user_data.input.name}`  | Job reads this data  | Creates dependency on producer |
| `${user_data.output.name}` | Job writes this data | Satisfies dependencies         |

Torc substitutes these variables with the actual user_data ID at runtime, and the `torc user-data`
CLI commands use that ID to read/write the data.

## Accessing User Data in Your Code

From within a job, you can:

**Read user_data:**

```bash
# Get the full record
torc user-data get $USER_DATA_ID

# Get just the data field
torc user-data get $USER_DATA_ID | jq '.data'

# Save to a file for your application
torc user-data get $USER_DATA_ID | jq '.data' > config.json
```

**Write user_data:**

```bash
# Update with JSON data
torc user-data update $USER_DATA_ID --data '{"key": "value"}'

# Update from a file
torc user-data update $USER_DATA_ID --data "$(cat results.json)"
```

## What You Learned

In this tutorial, you learned:

- ✅ What user_data is: structured data stored in the Torc database
- ✅ When to use it: configurations, parameters, metadata (not large files)
- ✅ How to define user_data entries with the `user_data:` section
- ✅ How `${user_data.input.*}` and `${user_data.output.*}` create dependencies
- ✅ How to read and write user_data from within jobs

## Common Patterns

### Dynamic Configuration Generation

```yaml
jobs:
  - name: analyze_data
    command: |
      # Analyze data and determine optimal parameters
      OPTIMAL_LR=$(python analyze.py --find-optimal-lr)
      torc user-data update ${user_data.output.optimal_params} \
        --data "{\"learning_rate\": $OPTIMAL_LR}"
```

### Collecting Results from Multiple Jobs

```yaml
jobs:
  - name: worker_{i}
    command: |
      RESULT=$(python process.py --id {i})
      torc user-data update ${user_data.output.result_{i}} --data "$RESULT"
    parameters:
      i: "1:10"

  - name: aggregate
    command: |
      # Collect all results
      for i in $(seq 1 10); do
        torc user-data get ${user_data.input.result_$i} >> all_results.json
      done
      python aggregate.py all_results.json
```

## Next Steps

- [Tutorial 4: Simple Parameterization](./simple-params.md) - Create parameter sweeps
- [Tutorial 5: Advanced Parameterization](./advanced-params.md) - Multi-dimensional grid searches
