# Working with HPC Profiles

HPC (High-Performance Computing) profiles provide pre-configured knowledge about specific HPC
systems, including their partitions, resource limits, and optimal settings. Torc uses this
information to automatically match job requirements to appropriate partitions.

## Overview

HPC profiles contain:

- **Partition definitions**: Available queues with their resource limits (CPUs, memory, walltime,
  GPUs)
- **Detection rules**: How to identify when you're on a specific HPC system
- **Default settings**: Account names and other system-specific defaults

Built-in profiles are available for systems like NLR's Kestrel. You can also define custom profiles
for private clusters.

## Listing Available Profiles

View all known HPC profiles:

```bash
torc hpc list
```

Example output:

```
Known HPC profiles:

╭─────────┬──────────────┬────────────┬──────────╮
│ Name    │ Display Name │ Partitions │ Detected │
├─────────┼──────────────┼────────────┼──────────┤
│ kestrel │ NLR Kestrel  │ 15         │ ✓        │
╰─────────┴──────────────┴────────────┴──────────╯
```

The "Detected" column shows if Torc recognizes you're currently on that system.

## Detecting the Current System

Torc can automatically detect which HPC system you're on:

```bash
torc hpc detect
```

Detection works through environment variables. For example, NLR Kestrel is detected when
`NREL_CLUSTER=kestrel` is set.

## Viewing Profile Details

See detailed information about a specific profile:

```bash
torc hpc show kestrel
```

This displays:

- Profile name and description
- Detection method
- Default account (if configured)
- Number of partitions

## Viewing Available Partitions

List all partitions for a profile:

```bash
torc hpc partitions kestrel
```

Example output:

```
Partitions for kestrel:

╭──────────┬─────────────┬───────────┬─────────────────┬─────────────────╮
│ Name     │ CPUs/Node   │ Mem/Node  │ Max Walltime    │ GPUs            │
├──────────┼─────────────┼───────────┼─────────────────┼─────────────────┤
│ debug    │ 104         │ 240 GB    │ 1h              │ -               │
│ short    │ 104         │ 240 GB    │ 4h              │ -               │
│ standard │ 104         │ 240 GB    │ 48h             │ -               │
│ gpu-h100 │ 2           │ 240 GB    │ 48h             │ 4 (H100)        │
│ ...      │ ...         │ ...       │ ...             │ ...             │
╰──────────┴─────────────┴───────────┴─────────────────┴─────────────────╯
```

## Finding Matching Partitions

Find partitions that can satisfy specific resource requirements:

```bash
torc hpc match kestrel --cpus 32 --memory 64g --walltime 2h
```

Options:

- `--cpus <N>`: Required CPU cores
- `--memory <SIZE>`: Required memory (e.g., `64g`, `512m`)
- `--walltime <DURATION>`: Required walltime (e.g., `2h`, `4:00:00`)
- `--gpus <N>`: Required GPUs (optional)

This is useful for understanding which partitions your jobs will be assigned to.

## Custom HPC Profiles

If your HPC system doesn't have a built-in profile, you have two options:

> **Request Built-in Support** (Recommended)
>
> If your HPC is widely used, please [open an issue](https://github.com/NatLabRockies/torc/issues)
> requesting built-in support. Include:
>
> - Your HPC system name and organization
> - Partition names with resource limits (CPUs, memory, walltime, GPUs)
> - Detection method (environment variable or hostname pattern)
>
> Built-in profiles benefit everyone using that system and are maintained by the Torc team.

If you need to use your HPC immediately or have a private cluster, you can define a custom profile
in your configuration file. See the [Custom HPC Profile Tutorial](./custom-hpc-profile.md) for a
complete walkthrough.

### Quick Example

Define custom profiles in your configuration file:

```toml
# ~/.config/torc/config.toml

[client.hpc.custom_profiles.mycluster]
display_name = "My Research Cluster"
description = "Internal research HPC system"
detect_env_var = "MY_CLUSTER=research"
default_account = "default_project"

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

See [Configuration Reference](../../core/reference/configuration.md) for full configuration options.

## Using Profiles with Slurm Workflows

HPC profiles are used by Slurm-related commands to automatically generate scheduler configurations.
See [Working with Slurm](./slurm.md) for details on:

- `torc submit-slurm` - Submit workflows with auto-generated schedulers
- `torc workflows create-slurm` - Create workflows with auto-generated schedulers

## See Also

- [Working with Slurm](./slurm.md)
- [Custom HPC Profile Tutorial](./custom-hpc-profile.md)
- [HPC Profiles Reference](./hpc-profiles-reference.md)
- [Configuration Reference](../../core/reference/configuration.md)
- [Resource Requirements Reference](../../core/reference/resources.md)
