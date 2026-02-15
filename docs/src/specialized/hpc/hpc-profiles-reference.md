# HPC Profiles Reference

Complete reference for HPC profile system and CLI commands.

## Overview

HPC profiles contain pre-configured knowledge about High-Performance Computing systems, enabling
automatic Slurm scheduler generation based on job resource requirements.

## CLI Commands

### `torc hpc list`

List all available HPC profiles.

```bash
torc hpc list [OPTIONS]
```

**Options:**

| Option                  | Description                      |
| ----------------------- | -------------------------------- |
| `-f, --format <FORMAT>` | Output format: `table` or `json` |

**Output columns:**

- **Name**: Profile identifier used in commands
- **Display Name**: Human-readable name
- **Partitions**: Number of configured partitions
- **Detected**: Whether current system matches this profile

---

### `torc hpc detect`

Detect the current HPC system.

```bash
torc hpc detect [OPTIONS]
```

**Options:**

| Option                  | Description                      |
| ----------------------- | -------------------------------- |
| `-f, --format <FORMAT>` | Output format: `table` or `json` |

Returns the detected profile name, or indicates no match.

---

### `torc hpc show`

Display detailed information about an HPC profile.

```bash
torc hpc show <PROFILE> [OPTIONS]
```

**Arguments:**

| Argument    | Description                    |
| ----------- | ------------------------------ |
| `<PROFILE>` | Profile name (e.g., `kestrel`) |

**Options:**

| Option                  | Description                      |
| ----------------------- | -------------------------------- |
| `-f, --format <FORMAT>` | Output format: `table` or `json` |

---

### `torc hpc partitions`

List partitions for an HPC profile.

```bash
torc hpc partitions <PROFILE> [OPTIONS]
```

**Arguments:**

| Argument    | Description                    |
| ----------- | ------------------------------ |
| `<PROFILE>` | Profile name (e.g., `kestrel`) |

**Options:**

| Option                  | Description                      |
| ----------------------- | -------------------------------- |
| `-f, --format <FORMAT>` | Output format: `table` or `json` |

**Output columns:**

- **Name**: Partition name
- **CPUs/Node**: CPU cores per node
- **Mem/Node**: Memory per node
- **Max Walltime**: Maximum job duration
- **GPUs**: GPU count and type (if applicable)
- **Shared**: Whether partition supports shared jobs
- **Notes**: Special requirements or features

---

### `torc hpc match`

Find partitions matching resource requirements.

```bash
torc hpc match <PROFILE> [OPTIONS]
```

**Arguments:**

| Argument    | Description                    |
| ----------- | ------------------------------ |
| `<PROFILE>` | Profile name (e.g., `kestrel`) |

**Options:**

| Option                  | Description                               |
| ----------------------- | ----------------------------------------- |
| `--cpus <N>`            | Required CPU cores                        |
| `--memory <SIZE>`       | Required memory (e.g., `64g`, `512m`)     |
| `--walltime <DURATION>` | Required walltime (e.g., `2h`, `4:00:00`) |
| `--gpus <N>`            | Required GPUs                             |
| `-f, --format <FORMAT>` | Output format: `table` or `json`          |

**Memory format:** `<number><unit>` where unit is `k`, `m`, `g`, or `t` (case-insensitive).

**Walltime formats:**

- `HH:MM:SS` (e.g., `04:00:00`)
- `<N>h` (e.g., `4h`)
- `<N>m` (e.g., `30m`)
- `<N>s` (e.g., `3600s`)

---

### `torc hpc generate`

Generate an HPC profile configuration from the current Slurm cluster.

```bash
torc hpc generate [OPTIONS]
```

**Options:**

| Option                  | Description                                          |
| ----------------------- | ---------------------------------------------------- |
| `--name <NAME>`         | Profile name (defaults to cluster name or hostname)  |
| `--display-name <NAME>` | Human-readable display name                          |
| `-o, --output <FILE>`   | Output file path (prints to stdout if not specified) |
| `--skip-stdby`          | Skip standby partitions (names ending in `-stdby`)   |

**How it works:**

1. Queries `sinfo` to get partition names, CPUs, memory, time limits, and GRES
2. Queries `scontrol show partition` for each partition to get additional details
3. Parses GRES strings to extract GPU count and type
4. Generates hostname-based detection pattern from current hostname
5. Outputs TOML configuration ready to add to your config file

**Example:**

```bash
# Generate profile from current cluster
torc hpc generate

# Output:
# [client.hpc.custom_profiles.mycluster]
# display_name = "Mycluster"
# detect_hostname = ".*\\.mycluster\\.edu"
#
# [[client.hpc.custom_profiles.mycluster.partitions]]
# name = "compute"
# cpus_per_node = 64
# memory_mb = 256000
# max_walltime_secs = 172800
# ...
```

**Fields extracted automatically:**

- Partition name, CPUs per node, memory (MB), max walltime (seconds)
- GPU count and type from GRES (e.g., `gpu:a100:4`)
- Shared node support from OverSubscribe setting

**Fields that may need manual adjustment:**

- `requires_explicit_request`: Defaults to `false`; set to `true` for partitions that shouldn't be
  auto-selected
- `description`: Not available from Slurm; add human-readable descriptions
- `gpu_memory_gb`: Not available from Slurm; add if known

---

### `torc slurm generate`

Generate Slurm schedulers for a workflow based on job resource requirements.

```bash
torc slurm generate [OPTIONS] --account <ACCOUNT> <WORKFLOW_FILE>
```

**Arguments:**

| Argument          | Description                                                |
| ----------------- | ---------------------------------------------------------- |
| `<WORKFLOW_FILE>` | Path to workflow specification file (YAML, JSON, or JSON5) |

**Options:**

| Option                | Description                                          |
| --------------------- | ---------------------------------------------------- |
| `--account <ACCOUNT>` | Slurm account to use (required)                      |
| `--profile <PROFILE>` | HPC profile to use (auto-detected if not specified)  |
| `-o, --output <FILE>` | Output file path (prints to stdout if not specified) |
| `--no-actions`        | Don't add workflow actions for scheduling nodes      |
| `--force`             | Overwrite existing schedulers in the workflow        |

**Generated artifacts:**

1. **Slurm schedulers**: One for each unique resource requirement
2. **Job scheduler assignments**: Each job linked to appropriate scheduler
3. **Workflow actions**: `on_workflow_start`/`schedule_nodes` actions (unless `--no-actions`)

**Scheduler naming:** `<resource_requirement_name>_scheduler`

---

## Built-in Profiles

### NLR Kestrel

**Profile name:** `kestrel`

**Detection:** Environment variable `NREL_CLUSTER=kestrel`

**Partitions:**

| Partition  | CPUs | Memory  | Max Walltime | GPUs    | Notes                               |
| ---------- | ---- | ------- | ------------ | ------- | ----------------------------------- |
| `debug`    | 104  | 240 GB  | 1h           | -       | Quick testing                       |
| `short`    | 104  | 240 GB  | 4h           | -       | Short jobs                          |
| `standard` | 104  | 240 GB  | 48h          | -       | General workloads                   |
| `long`     | 104  | 240 GB  | 240h         | -       | Extended jobs                       |
| `medmem`   | 104  | 480 GB  | 48h          | -       | Medium memory                       |
| `bigmem`   | 104  | 2048 GB | 48h          | -       | High memory                         |
| `shared`   | 104  | 240 GB  | 48h          | -       | Shared node access                  |
| `hbw`      | 104  | 240 GB  | 48h          | -       | High-bandwidth memory, min 10 nodes |
| `nvme`     | 104  | 240 GB  | 48h          | -       | NVMe local storage                  |
| `gpu-h100` | 2    | 240 GB  | 48h          | 4x H100 | GPU compute                         |

**Node specifications:**

- **Standard nodes**: 104 cores (2x Intel Xeon Sapphire Rapids), 240 GB RAM
- **GPU nodes**: 4x NVIDIA H100 80GB HBM3, 128 cores, 2 TB RAM

---

## Configuration

### Custom Profiles

> **Don't see your HPC?** Please
> [request built-in support](https://github.com/NatLabRockies/torc/issues) so everyone benefits. See
> the [Custom HPC Profile Tutorial](./custom-hpc-profile.md) for creating a profile while you wait.

Define custom profiles in your Torc configuration file:

```toml
# ~/.config/torc/config.toml

[client.hpc.custom_profiles.mycluster]
display_name = "My Cluster"
description = "Description of the cluster"
detect_env_var = "CLUSTER_NAME=mycluster"
detect_hostname = ".*\\.mycluster\\.org"
default_account = "myproject"

[[client.hpc.custom_profiles.mycluster.partitions]]
name = "compute"
cpus_per_node = 64
memory_mb = 256000
max_walltime_secs = 172800
shared = false

[[client.hpc.custom_profiles.mycluster.partitions]]
name = "gpu"
cpus_per_node = 32
memory_mb = 128000
max_walltime_secs = 86400
gpus_per_node = 4
gpu_type = "A100"
shared = false
```

### Profile Override

Override settings for built-in profiles:

```toml
[client.hpc.profile_overrides.kestrel]
default_account = "my_default_account"
```

### Configuration Options

**`[client.hpc]` Section:**

| Option              | Type  | Description                             |
| ------------------- | ----- | --------------------------------------- |
| `profile_overrides` | table | Override settings for built-in profiles |
| `custom_profiles`   | table | Define custom HPC profiles              |

**Profile override options:**

| Option            | Type   | Description                            |
| ----------------- | ------ | -------------------------------------- |
| `default_account` | string | Default Slurm account for this profile |

**Custom profile options:**

| Option            | Type   | Required | Description                                       |
| ----------------- | ------ | -------- | ------------------------------------------------- |
| `display_name`    | string | No       | Human-readable name                               |
| `description`     | string | No       | Profile description                               |
| `detect_env_var`  | string | No       | Environment variable for detection (`NAME=value`) |
| `detect_hostname` | string | No       | Regex pattern for hostname detection              |
| `default_account` | string | No       | Default Slurm account                             |
| `partitions`      | array  | Yes      | List of partition configurations                  |

**Partition options:**

| Option                      | Type   | Required | Description                            |
| --------------------------- | ------ | -------- | -------------------------------------- |
| `name`                      | string | Yes      | Partition name                         |
| `cpus_per_node`             | int    | Yes      | CPU cores per node                     |
| `memory_mb`                 | int    | Yes      | Memory per node in MB                  |
| `max_walltime_secs`         | int    | Yes      | Maximum walltime in seconds            |
| `gpus_per_node`             | int    | No       | GPUs per node                          |
| `gpu_type`                  | string | No       | GPU model (e.g., "H100")               |
| `shared`                    | bool   | No       | Whether partition supports shared jobs |
| `min_nodes`                 | int    | No       | Minimum required nodes                 |
| `requires_explicit_request` | bool   | No       | Must be explicitly requested           |

---

## Resource Matching Algorithm

When generating schedulers, Torc uses this algorithm to match resource requirements to partitions:

1. **Filter by resources**: Partitions must satisfy:
   - CPUs >= required CPUs
   - Memory >= required memory
   - GPUs >= required GPUs (if specified)
   - Max walltime >= required runtime

2. **Exclude debug partitions**: Unless no other partition matches

3. **Prefer best fit**:
   - Partitions that exactly match resource needs
   - Non-shared partitions over shared
   - Shorter max walltime over longer

4. **Handle special requirements**:
   - GPU jobs only match GPU partitions
   - Respect `requires_explicit_request` flag
   - Honor `min_nodes` constraints

---

## Generated Scheduler Format

Example generated Slurm scheduler:

```yaml
slurm_schedulers:
  - name: medium_scheduler
    account: myproject
    nodes: 1
    mem: 64g
    walltime: 04:00:00
    gres: null
    partition: null  # Let Slurm choose based on resources
```

Corresponding workflow action:

```yaml
actions:
  - trigger_type: on_workflow_start
    action_type: schedule_nodes
    scheduler: medium_scheduler
    scheduler_type: slurm
    num_allocations: 1
```

---

## Runtime Format Parsing

Resource requirements use ISO 8601 duration format for runtime:

| Format   | Example   | Meaning            |
| -------- | --------- | ------------------ |
| `PTnH`   | `PT4H`    | 4 hours            |
| `PTnM`   | `PT30M`   | 30 minutes         |
| `PTnS`   | `PT3600S` | 3600 seconds       |
| `PTnHnM` | `PT2H30M` | 2 hours 30 minutes |
| `PnDTnH` | `P1DT12H` | 1 day 12 hours     |

Generated walltime uses `HH:MM:SS` format (e.g., `04:00:00`).

---

## See Also

- [Working with HPC Profiles](./hpc-profiles.md)
- [Custom HPC Profile Tutorial](./custom-hpc-profile.md)
- [Working with Slurm](./slurm.md)
- [Resource Requirements](../../core/reference/resources.md)
- [Configuration Reference](../../core/reference/configuration.md)
