# Creating a Custom HPC Profile

This tutorial walks you through creating a custom HPC profile for a cluster that Torc doesn't have
built-in support for.

## Before You Start

> **Request Built-in Support First!**
>
> If your HPC system is widely used, consider requesting that Torc developers add it as a built-in
> profile. This benefits everyone using that system.
>
> Open an issue at
> [github.com/NatLabRockies/torc/issues](https://github.com/NatLabRockies/torc/issues) with:
>
> - Your HPC system name and organization
> - Partition names and their resource limits (CPUs, memory, walltime, GPUs)
> - How to detect the system (environment variable or hostname pattern)
> - Any special requirements (minimum nodes, exclusive partitions, etc.)
>
> Built-in profiles are maintained by the Torc team and stay up-to-date as systems change.

## When to Create a Custom Profile

Create a custom profile when:

- Your HPC isn't supported and you need to use it immediately
- You have a private or internal cluster
- You want to test profile configurations before submitting upstream

## Quick Start: Auto-Generate from Slurm

If you're on a Slurm cluster, you can automatically generate a profile from the cluster
configuration:

```bash
# Generate profile from current Slurm cluster
torc hpc generate

# Specify a custom name
torc hpc generate --name mycluster --display-name "My Research Cluster"

# Skip standby/preemptible partitions
torc hpc generate --skip-stdby

# Save to a file
torc hpc generate --skip-stdby -o mycluster-profile.toml
```

This queries `sinfo` and `scontrol` to extract:

- Partition names, CPUs, memory, and time limits
- GPU configuration from GRES
- Node sharing settings
- Hostname-based detection pattern

The generated profile can be added directly to your config file. You may want to review and adjust:

- `requires_explicit_request`: Set to `true` for partitions that shouldn't be auto-selected
- `description`: Add human-readable descriptions for each partition

After generation, skip to [Step 4: Verify the Profile](#step-4-verify-the-profile).

## Manual Profile Creation

If automatic generation isn't available or you need more control, follow these steps.

### Step 1: Gather Partition Information

Collect information about your HPC's partitions. On most Slurm systems:

```bash
# List all partitions
sinfo -s

# Get detailed partition info
sinfo -o "%P %c %m %l %G"
```

For this tutorial, let's say your cluster "ResearchCluster" has these partitions:

| Partition | CPUs/Node | Memory  | Max Walltime | GPUs    |
| --------- | --------- | ------- | ------------ | ------- |
| `batch`   | 48        | 192 GB  | 72 hours     | -       |
| `short`   | 48        | 192 GB  | 4 hours      | -       |
| `gpu`     | 32        | 256 GB  | 48 hours     | 4x A100 |
| `himem`   | 48        | 1024 GB | 48 hours     | -       |

### Step 2: Identify Detection Method

Determine how Torc can detect when you're on this system. Common methods:

**Environment variable** (most common):

```bash
echo $CLUSTER_NAME    # e.g., "research"
echo $SLURM_CLUSTER   # e.g., "researchcluster"
```

**Hostname pattern**:

```bash
hostname              # e.g., "login01.research.edu"
```

For this tutorial, we'll use the environment variable `CLUSTER_NAME=research`.

### Step 3: Create the Configuration File

Create or edit your Torc configuration file:

```bash
# Linux
mkdir -p ~/.config/torc
nano ~/.config/torc/config.toml

# macOS
mkdir -p ~/Library/Application\ Support/torc
nano ~/Library/Application\ Support/torc/config.toml
```

Add your custom profile:

```toml
# Custom HPC Profile for ResearchCluster
[client.hpc.custom_profiles.research]
display_name = "Research Cluster"
description = "University Research HPC System"
detect_env_var = "CLUSTER_NAME=research"
default_account = "my_project"

# Batch partition - general purpose
[[client.hpc.custom_profiles.research.partitions]]
name = "batch"
cpus_per_node = 48
memory_mb = 192000        # 192 GB in MB
max_walltime_secs = 259200  # 72 hours in seconds
shared = false

# Short partition - quick jobs
[[client.hpc.custom_profiles.research.partitions]]
name = "short"
cpus_per_node = 48
memory_mb = 192000
max_walltime_secs = 14400   # 4 hours
shared = true               # Allows sharing nodes

# GPU partition
[[client.hpc.custom_profiles.research.partitions]]
name = "gpu"
cpus_per_node = 32
memory_mb = 256000          # 256 GB
max_walltime_secs = 172800  # 48 hours
gpus_per_node = 4
gpu_type = "A100"
shared = false

# High memory partition
[[client.hpc.custom_profiles.research.partitions]]
name = "himem"
cpus_per_node = 48
memory_mb = 1048576         # 1024 GB (1 TB)
max_walltime_secs = 172800  # 48 hours
shared = false
```

## Step 4: Verify the Profile

Check that Torc recognizes your profile:

```bash
# List all profiles
torc hpc list
```

You should see your custom profile:

```
Known HPC profiles:

╭──────────┬──────────────────┬────────────┬──────────╮
│ Name     │ Display Name     │ Partitions │ Detected │
├──────────┼──────────────────┼────────────┼──────────┤
│ kestrel  │ NLR Kestrel      │ 15         │          │
│ research │ Research Cluster │ 4          │ ✓        │
╰──────────┴──────────────────┴────────────┴──────────╯
```

View the partitions:

```bash
torc hpc partitions research
```

```
Partitions for research:

╭─────────┬───────────┬───────────┬─────────────┬──────────╮
│ Name    │ CPUs/Node │ Mem/Node  │ Max Walltime│ GPUs     │
├─────────┼───────────┼───────────┼─────────────┼──────────┤
│ batch   │ 48        │ 192 GB    │ 72h         │ -        │
│ short   │ 48        │ 192 GB    │ 4h          │ -        │
│ gpu     │ 32        │ 256 GB    │ 48h         │ 4 (A100) │
│ himem   │ 48        │ 1024 GB   │ 48h         │ -        │
╰─────────┴───────────┴───────────┴─────────────┴──────────╯
```

## Step 5: Test Partition Matching

Verify that Torc correctly matches resource requirements to partitions:

```bash
# Should match 'short' partition
torc hpc match research --cpus 8 --memory 16g --walltime 2h

# Should match 'gpu' partition
torc hpc match research --cpus 16 --memory 64g --walltime 8h --gpus 2

# Should match 'himem' partition
torc hpc match research --cpus 24 --memory 512g --walltime 24h
```

## Step 6: Test Scheduler Generation

Create a test workflow to verify scheduler generation:

```yaml
# test_workflow.yaml
name: profile_test
description: Test custom HPC profile

resource_requirements:
  - name: standard
    num_cpus: 16
    memory: 64g
    runtime: PT2H

  - name: gpu_compute
    num_cpus: 16
    num_gpus: 2
    memory: 128g
    runtime: PT8H

jobs:
  - name: preprocess
    command: echo "preprocessing"
    resource_requirements: standard

  - name: train
    command: echo "training"
    resource_requirements: gpu_compute
    depends_on: [preprocess]
```

Generate schedulers:

```bash
torc slurm generate --account my_project --profile research test_workflow.yaml
```

You should see the generated workflow with appropriate schedulers for each partition.

## Step 7: Use Your Profile

Now you can submit workflows using your custom profile:

```bash
# Auto-detect the profile (if on the cluster)
torc submit-slurm --account my_project workflow.yaml

# Or explicitly specify the profile
torc submit-slurm --account my_project --hpc-profile research workflow.yaml
```

## Advanced Configuration

### Hostname-Based Detection

If your cluster doesn't set a unique environment variable, use hostname detection:

```toml
[client.hpc.custom_profiles.research]
display_name = "Research Cluster"
detect_hostname = ".*\\.research\\.edu"  # Regex pattern
```

### Minimum Node Requirements

Some partitions require a minimum number of nodes:

```toml
[[client.hpc.custom_profiles.research.partitions]]
name = "large_scale"
cpus_per_node = 128
memory_mb = 512000
max_walltime_secs = 172800
min_nodes = 16  # Must request at least 16 nodes
```

### Explicit Request Partitions

Some partitions shouldn't be auto-selected:

```toml
[[client.hpc.custom_profiles.research.partitions]]
name = "priority"
cpus_per_node = 48
memory_mb = 192000
max_walltime_secs = 86400
requires_explicit_request = true  # Only used when explicitly requested
```

## Troubleshooting

### Profile Not Detected

If `torc hpc detect` doesn't find your profile:

1. Check the environment variable or hostname:
   ```bash
   echo $CLUSTER_NAME
   hostname
   ```

2. Verify the detection pattern in your config matches exactly

3. Test with explicit profile specification:
   ```bash
   torc hpc show research
   ```

### No Partition Found for Job

If `torc slurm generate` can't find a matching partition:

1. Check if any partition satisfies all requirements:
   ```bash
   torc hpc match research --cpus 32 --memory 128g --walltime 8h
   ```

2. Verify memory is specified in MB in the config (not GB)

3. Verify walltime is in seconds (not hours)

### Configuration File Location

Torc looks for config files in these locations:

- **Linux**: `~/.config/torc/config.toml`
- **macOS**: `~/Library/Application Support/torc/config.toml`
- **Windows**: `%APPDATA%\torc\config.toml`

You can also use the `TORC_CONFIG` environment variable to specify a custom path.

## Contributing Your Profile

If your HPC is used by others, please contribute it upstream:

1. Fork the [Torc repository](https://github.com/NatLabRockies/torc)
2. Add your profile to `src/client/hpc_profiles.rs`
3. Add tests for your profile
4. Submit a pull request

Or simply [open an issue](https://github.com/NatLabRockies/torc/issues) with your partition
information and we'll add it for you.

## See Also

- [Working with HPC Profiles](./hpc-profiles.md) - General HPC profile usage
- [HPC Profiles Reference](./hpc-profiles-reference.md) - Complete configuration options
- [Slurm Workflows](./slurm-workflows.md) - Simplified Slurm approach
