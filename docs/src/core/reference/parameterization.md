# Job Parameterization

Parameterization allows creating multiple jobs/files from a single specification by expanding
parameter ranges.

## Parameter Formats

### Integer Ranges

```yaml
parameters:
  i: "1:10"        # Expands to [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
  i: "0:100:10"    # Expands to [0, 10, 20, 30, ..., 90, 100] (with step)
```

### Float Ranges

```yaml
parameters:
  lr: "0.0001:0.01:10"  # 10 values from 0.0001 to 0.01 (log scale)
  alpha: "0.0:1.0:0.1"  # [0.0, 0.1, 0.2, ..., 0.9, 1.0]
```

### Lists (Integer)

```yaml
parameters:
  batch_size: [16, 32, 64, 128]
```

### Lists (Float)

```yaml
parameters:
  threshold: [0.1, 0.5, 0.9]
```

### Lists (String)

```yaml
parameters:
  optimizer: [adam, sgd, rmsprop]
  dataset: [train, test, validation]
```

Lists are written as native YAML/JSON sequences. The legacy string-encoded form (e.g. `"[16,32,64]"`
or `"['adam','sgd']"`) is still accepted in YAML/JSON/JSON5 specs, and remains the required form in
KDL specs, where parameter values must be strings.

## Template Substitution

Use parameter values in job/file specifications with `{param_name}` syntax:

### Basic Substitution

```yaml
jobs:
  - name: job_{i}
    command: python train.py --run={i}
    parameters:
      i: "1:5"
```

Expands to:

```yaml
jobs:
  - name: job_1
    command: python train.py --run=1
  - name: job_2
    command: python train.py --run=2
  # ... etc
```

### Format Specifiers

**Zero-padded integers:**

```yaml
jobs:
  - name: job_{i:03d}
    command: echo {i}
    parameters:
      i: "1:100"
```

Expands to: `job_001`, `job_002`, ..., `job_100`

**Float precision:**

```yaml
jobs:
  - name: train_lr{lr:.4f}
    command: python train.py --lr={lr}
    parameters:
      lr: [0.0001, 0.001, 0.01]
```

Expands to: `train_lr0.0001`, `train_lr0.0010`, `train_lr0.0100`

**Multiple decimals:**

```yaml
files:
  - name: result_{threshold:.2f}
    path: /results/threshold_{threshold:.2f}.csv
    parameters:
      threshold: "0.1:1.0:0.1"
```

Expands to: `result_0.10`, `result_0.20`, ..., `result_1.00`

## Multi-Dimensional Parameterization

Use multiple parameters to create Cartesian products:

### Example: Hyperparameter Sweep

```yaml
jobs:
  - name: train_lr{lr:.4f}_bs{batch_size}
    command: |
      python train.py \
        --learning-rate={lr} \
        --batch-size={batch_size}
    parameters:
      lr: [0.0001, 0.001, 0.01]
      batch_size: [16, 32, 64]
```

This expands to 3 × 3 = **9 jobs**:

- `train_lr0.0001_bs16`
- `train_lr0.0001_bs32`
- `train_lr0.0001_bs64`
- `train_lr0.0010_bs16`
- ... (9 total)

### Example: Multi-Dataset Processing

```yaml
jobs:
  - name: process_{dataset}_rep{rep:02d}
    command: python process.py --data={dataset} --replicate={rep}
    parameters:
      dataset: [train, validation, test]
      rep: "1:5"
```

This expands to 3 × 5 = **15 jobs**

## Parameterized Dependencies

Parameters work in dependency specifications:

```yaml
jobs:
  # Generate data for each configuration
  - name: generate_{config}
    command: python generate.py --config={config}
    output_files:
      - data_{config}
    parameters:
      config: [A, B, C]

  # Process each generated dataset
  - name: process_{config}
    command: python process.py --input=data_{config}.pkl
    input_files:
      - data_{config}
    depends_on:
      - generate_{config}
    parameters:
      config: [A, B, C]
```

This creates 6 jobs with proper dependencies:

- `generate_A` → `process_A`
- `generate_B` → `process_B`
- `generate_C` → `process_C`

## Parameterized Files and User Data

**Files:**

```yaml
files:
  - name: model_{run_id:03d}
    path: /models/run_{run_id:03d}.pt
    parameters:
      run_id: "1:100"
```

**User Data:**

```yaml
user_data:
  - name: config_{experiment}
    data:
      experiment: "{experiment}"
      learning_rate: 0.001
      output_dir: /results/{experiment}
    parameters:
      experiment: [baseline, ablation, full]
```

Parameter tokens (`{name}` / `{name:fmt}`) are substituted into the user_data `name` and into every
string value found recursively inside `data` -- object values, array elements, and nested
substructures. Non-string values (numbers, bools, null) pass through unchanged. The same
`use_parameters` opt-in for inheriting workflow-level parameters applies to user_data as well. See
`examples/yaml/parameterized_user_data.yaml` for a runnable example.

## Workflow Variables

Workflow-level `variables` are constants that get substituted into every string field of the spec
before parameter expansion. Use them to remove repetition of fixed strings -- paths, account codes,
image tags, project IDs -- that appear across many jobs, files, schedulers, or env entries.

Variables are not the same as `parameters`:

- `variables` are constants. Each `{name}` reference is replaced once with the variable's value. The
  number of jobs/files does not change.
- `parameters` are sweep dimensions. They expand into multiple instances via Cartesian (or zip)
  product.

The two mechanisms compose freely: a single command string can mix `{variable}` references and
`{parameter}` references. Variables resolve first; parameters drive expansion afterwards.

### Basic Usage

```yaml
name: variables_demo
variables:
  data_root: /scratch/proj42
  results_root: /shared/proj42/results
  project: proj42
  account: my_hpc_account

env:
  PROJECT: "{project}"

jobs:
  - name: prepare_inputs
    command: "python prepare.py --in {data_root}/raw --out {data_root}/clean"

  # Variables compose with parameters: {data_root} is a constant,
  # {i} drives expansion into 4 jobs.
  - name: "train_{i:02d}"
    command: "python train.py --shard {i} --in {data_root}/clean"
    parameters:
      i: "1:4"

slurm_schedulers:
  - name: shared_sched
    account: "{account}"
    partition: short
    walltime: "01:00:00"
    nodes: 1
```

### Validation Rules

- **Variable names must be valid identifiers** (`[A-Za-z_][A-Za-z0-9_]*`).
- **No collisions.** A variable name must not match any parameter name (at the workflow level or in
  any job/file/user_data `parameters` map). Spec loading fails with an error pointing at the
  offending name.
- **No undefined references.** A `{name}` token whose name is neither a variable nor a parameter is
  rejected as a typo. Tokens with non-identifier inner text (e.g. `find ... {} \;` or JSON-like
  fragments) are ignored. Shell-style `${...}` expansion (used by `${TORC_JOB_ID}`,
  `${files.input.X}`, etc.) is left alone -- the workflow variables system only consumes bare
  `{name}` tokens.
- **Variable values must be plain literal strings.** Any `{name}` template reference inside a
  variable's value is rejected, whether it points at another variable, a parameter, or a typo. This
  keeps semantics simple and deterministic. Compose at the use site instead: `command: "{base}/sub"`
  rather than `inputs: "{base}/sub"`. Shell-style `${...}` expansion (e.g. `${HOME}`,
  `${TORC_JOB_ID}`) is allowed in variable values -- it is preserved verbatim and expanded at
  runtime, not by the spec loader. Note that you can still reference a variable from a parameter
  range: `i: "1:{n_max}"` works because that's a parameter value, not a variable value.
- Variables apply everywhere a string appears in the spec, including descriptions, env values,
  scheduler fields, action arguments, file paths, commands, and parameter range values. They do not
  apply to identifier fields (`parameters` keys, `use_parameters` entries).

### KDL Syntax

```kdl
variables {
    data_root "/scratch/proj42"
    project "proj42"
    account "my_hpc_account"
}

job "train_{i:02d}" {
    command "python train.py --shard {i} --in {data_root}/clean"
    parameters {
        i "1:4"
    }
}
```

### JSON5 Syntax

```json5
{
  variables: {
    data_root: "/scratch/proj42",
    project: "proj42",
  },
  jobs: [
    {
      name: "train_{i:02d}",
      command: "python train.py --shard {i} --in {data_root}/clean",
      parameters: { i: "1:4" },
    },
  ],
}
```

### When to Reach for Variables vs. Shared Parameters

Use `variables` for plain constants (single value, no expansion) -- they DRY up the spec without
changing job counts and don't require any opt-in field on each job or file.

Use shared `parameters` with `use_parameters` (next section) when the same _sweep dimension_ drives
expansion across multiple jobs and files. Shared parameters are still the right tool for
hyperparameter sweeps.

## Shared (Workflow-Level) Parameters

Define parameters once at the workflow level and reuse them across multiple jobs and files using
`use_parameters`:

### Basic Usage

```yaml
name: hyperparameter_sweep
parameters:
  lr: [0.0001, 0.001, 0.01]
  batch_size: [16, 32, 64]
  optimizer: [adam, sgd]

jobs:
  # Training jobs - inherit parameters via use_parameters
  - name: train_lr{lr:.4f}_bs{batch_size}_opt{optimizer}
    command: python train.py --lr={lr} --batch-size={batch_size} --optimizer={optimizer}
    use_parameters:
      - lr
      - batch_size
      - optimizer

  # Aggregate results - also uses shared parameters
  - name: aggregate_results
    command: python aggregate.py
    depends_on:
      - train_lr{lr:.4f}_bs{batch_size}_opt{optimizer}
    use_parameters:
      - lr
      - batch_size
      - optimizer

files:
  - name: model_lr{lr:.4f}_bs{batch_size}_opt{optimizer}
    path: /models/model_lr{lr:.4f}_bs{batch_size}_opt{optimizer}.pt
    use_parameters:
      - lr
      - batch_size
      - optimizer
```

### Benefits

- **DRY (Don't Repeat Yourself)** - Define parameter ranges once, use everywhere
- **Consistency** - Ensures all jobs use the same parameter values
- **Maintainability** - Change parameters in one place, affects all uses
- **Selective inheritance** - Jobs can choose which parameters to use

### Selective Parameter Inheritance

Jobs don't have to use all workflow parameters:

```yaml
parameters:
  lr: [0.0001, 0.001, 0.01]
  batch_size: [16, 32, 64]
  dataset: [train, validation]

jobs:
  # Only uses lr and batch_size (9 jobs)
  - name: train_lr{lr:.4f}_bs{batch_size}
    command: python train.py --lr={lr} --batch-size={batch_size}
    use_parameters:
      - lr
      - batch_size

  # Only uses dataset (2 jobs)
  - name: prepare_{dataset}
    command: python prepare.py --dataset={dataset}
    use_parameters:
      - dataset
```

### Local Parameters Override Shared

Jobs can define local parameters that take precedence over workflow-level parameters:

```yaml
parameters:
  lr: [0.0001, 0.001, 0.01]

jobs:
  # Uses workflow parameter (3 jobs)
  - name: train_lr{lr:.4f}
    command: python train.py --lr={lr}
    use_parameters:
      - lr

  # Uses local override (2 jobs instead of 3)
  - name: special_lr{lr:.4f}
    command: python special.py --lr={lr}
    parameters:
      lr: [0.01, 0.1] # Local override - ignores workflow's lr
```

### KDL Syntax

KDL parameter values must be strings, so lists use the string-encoded form:

```kdl
parameters {
    lr "[0.0001,0.001,0.01]"
    batch_size "[16,32,64]"
}

job "train_lr{lr:.4f}_bs{batch_size}" {
    command "python train.py --lr={lr} --batch-size={batch_size}"
    use_parameters "lr" "batch_size"
}
```

### JSON5 Syntax

```json5
{
  parameters: {
    lr: [0.0001, 0.001, 0.01],
    batch_size: [16, 32, 64]
  },
  jobs: [
    {
      name: "train_lr{lr:.4f}_bs{batch_size}",
      command: "python train.py --lr={lr} --batch-size={batch_size}",
      use_parameters: ["lr", "batch_size"]
    }
  ]
}
```

## Parameter Modes

By default, when multiple parameters are specified, Torc generates the Cartesian product of all
parameter values. You can change this behavior using `parameter_mode`.

### Product Mode (Default)

The default mode generates all possible combinations:

```yaml
jobs:
  - name: job_{a}_{b}
    command: echo {a} {b}
    parameters:
      a: [1, 2, 3]
      b: [x, y, z]
    # parameter_mode: product  # This is the default
```

This creates 3 × 3 = **9 jobs**: `job_1_x`, `job_1_y`, `job_1_z`, `job_2_x`, etc.

### Zip Mode

Use `parameter_mode: zip` to pair parameters element-wise (like Python's `zip()` function). All
parameter lists must have the same length.

```yaml
jobs:
  - name: train_{dataset}_{model}
    command: python train.py --dataset={dataset} --model={model}
    parameters:
      dataset: [cifar10, mnist, imagenet]
      model: [resnet, cnn, transformer]
    parameter_mode: zip
```

This creates **3 jobs** (not 9):

- `train_cifar10_resnet`
- `train_mnist_cnn`
- `train_imagenet_transformer`

**When to use zip mode:**

- Pre-determined parameter pairings (dataset A always uses model X)
- Corresponding input/output file pairs
- Parallel arrays where position matters

**Error handling:** If parameter lists have different lengths in zip mode, Torc will return an
error:

```
All parameters must have the same number of values when using 'zip' mode.
Parameter 'dataset' has 3 values, but 'model' has 2 values.
```

### KDL Syntax

KDL parameter values must be strings, so lists use the string-encoded form:

```kdl
job "train_{dataset}_{model}" {
    command "python train.py --dataset={dataset} --model={model}"
    parameters {
        dataset "['cifar10', 'mnist', 'imagenet']"
        model "['resnet', 'cnn', 'transformer']"
    }
    parameter_mode "zip"
}
```

### JSON5 Syntax

```json5
{
  name: "train_{dataset}_{model}",
  command: "python train.py --dataset={dataset} --model={model}",
  parameters: {
    dataset: ["cifar10", "mnist", "imagenet"],
    model: ["resnet", "cnn", "transformer"]
  },
  parameter_mode: "zip"
}
```

## Table-Based Parameterization (CSV / JSON Files)

The `parameters`/`parameter_mode` mechanism builds combinations from independent axes (Cartesian
product or zip). When you instead have an explicit table of combinations -- a parameter sweep
generated by another tool, or an irregular set that is not a full grid -- point a job, file, or
user_data at a CSV or JSON file with `parameters_file`. Each **row** (CSV) or **object** (JSON)
becomes exactly one generated instance, and its columns/keys are available as substitution tokens.

The file format is selected by extension: `.csv`, `.json` (a JSON array of objects), or `.jsonl` /
`.ndjson` (line-delimited JSON, one object per line). Relative paths are resolved against the
current working directory (the same convention as the `@file` list syntax), not the spec file.

### CSV

The header row supplies the column names. Each cell is inferred as integer → float → string, so
numeric columns can be used with format specifiers like `{lr:.4f}`.

`sweep.csv`:

```csv
model,lr,batch_size,dataset
resnet,0.001,32,cifar10
vit,0.0001,16,imagenet
```

```yaml
jobs:
  - name: train_{model}_{dataset}_bs{batch_size}
    command: echo "training {model} on {dataset} lr={lr} batch_size={batch_size}"
    parameters_file: examples/parameter_tables/sweep.csv
```

This expands to one job per row (`train_resnet_cifar10_bs32`, `train_vit_imagenet_bs16`).

### JSON

The document must be a JSON array of objects. JSON preserves native types, so numbers map directly
to integers/floats without inference. Any nested or non-scalar value (object, array, bool, null) is
stringified into a string. A line-delimited variant (`.jsonl` / `.ndjson`) is also accepted, where
each non-blank line is one object -- handy for large or tool-generated tables.

`sweep.json`:

```json
[
  { "model": "resnet", "lr": 0.001, "batch_size": 32, "dataset": "cifar10" },
  { "model": "vit", "lr": 0.0001, "batch_size": 16, "dataset": "imagenet" }
]
```

```yaml
jobs:
  - name: train_{model}_{dataset}_bs{batch_size}
    command: echo "training {model} on {dataset} lr={lr} batch_size={batch_size}"
    parameters_file: examples/parameter_tables/sweep.json
```

### Shared (Workflow-Level) Table

When several jobs, files, or user_data records should iterate the **same** table, declare it once at
the workflow level and have each spec opt in with `use_parameters_file: true`. This is the
table-based counterpart to [shared parameters](#shared-workflow-level-parameters): every opted-in
spec expands over **all** rows of the table (all columns are available as `{tokens}`), while specs
that don't opt in remain single instances.

```yaml
name: shared_table
# model,lr,batch_size,dataset -> 4 rows
parameters_file: examples/parameter_tables/sweep.csv

jobs:
  - name: train_{model}_{dataset}_bs{batch_size}
    command: python train.py --model {model} --lr {lr} --bs {batch_size}
    use_parameters_file: true # -> one job per row

  - name: eval_{model}_{dataset}_bs{batch_size}
    command: python eval.py --model {model}
    depends_on:
      - train_{model}_{dataset}_bs{batch_size}
    use_parameters_file: true # -> one job per row

  # No opt-in: a single fan-in job over all eval runs.
  - name: aggregate
    command: python aggregate.py
    depends_on_regexes:
      - "eval_.*"
```

Unlike `use_parameters` for inline shared parameters, `use_parameters_file` is a boolean opt-in for
the whole table -- it does not select a subset of columns. (If you need a job to run once per
distinct value of a single column, give it its own smaller `parameters_file`.)

### Rules

- `parameters_file` and `use_parameters_file` are **mutually exclusive** with `parameters`,
  `parameter_mode`, and `use_parameters`. A spec that mixes a table source with any of these is
  rejected.
- A spec may not set both a local `parameters_file` and `use_parameters_file: true`.
- `use_parameters_file: true` requires a workflow-level `parameters_file` to inherit.
- The workflow-level `parameters` and `parameters_file` are mutually exclusive -- a workflow has at
  most one shared parameter source.
- The table must contain at least one row; an empty table is an error.
- Template substitution, unique-name validation, and dependency resolution work exactly as they do
  for inline parameters.

See `examples/yaml/parameterized_from_csv.yaml`, `examples/yaml/parameterized_from_json.yaml`, and
`examples/yaml/parameterized_shared_table.yaml` for runnable examples.

## Best Practices

1. **Use descriptive parameter names** - `lr` not `x`, `batch_size` not `b`
2. **Format numbers consistently** - Use `:03d` for run IDs, `:.4f` for learning rates
3. **Keep parameter counts reasonable** - 3×3×3 = 27 jobs is manageable, 10×10×10 = 1000 may
   overwhelm the system
4. **Match parameter ranges across related jobs** - Use same parameter values for generator and
   consumer jobs
5. **Consider parameter dependencies** - Some parameter combinations may be invalid
6. **Prefer shared parameters for multi-job workflows** - Use `use_parameters` to avoid repeating
   definitions
7. **Use selective inheritance** - Only inherit the parameters each job actually needs
8. **Use zip mode for paired parameters** - When parameters have a 1:1 correspondence, use
   `parameter_mode: zip`
9. **Use `variables` for repeated constants** - Lift fixed strings (paths, account codes, image
   tags) into the workflow-level `variables` map rather than copying them across jobs and files
