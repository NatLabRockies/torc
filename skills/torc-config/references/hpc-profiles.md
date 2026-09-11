# HPC profiles

A profile tells Torc a cluster's partitions and their limits so `torc slurm generate` can map
resource requirements to real Slurm options. Two profiles ship built in (`kestrel`, `dane`), and a
dynamic `slurm` profile queries the live cluster.

## Contents

- [Try dynamic detection first](#try-dynamic-detection-first)
- [Override a built-in profile](#override-a-built-in-profile)
- [Generate a profile from the cluster](#generate-a-profile-from-the-cluster)
- [Custom profile schema](#custom-profile-schema)
- [Verify a profile](#verify-a-profile)

## Try dynamic detection first

```bash
torc hpc list                  # built-in profiles and which one is detected
torc hpc detect                # what matches this system
torc hpc partitions slurm      # partitions discovered from the live cluster
```

Torc falls back to dynamic Slurm detection when no profile matches, and `--profile slurm` forces it.
If `torc hpc partitions slurm` shows correct partitions, no custom profile is needed. Only write one
when detection is wrong, the cluster is private, or specific partitions must be excluded or
annotated.

## Override a built-in profile

The common case is not a new profile but a default account:

```toml
[client.hpc]
default_account = "my_project"          # applies to every profile

[client.hpc.profile_overrides.kestrel]
default_account = "my_kestrel_account"  # per-profile
```

With this set, `torc slurm generate` no longer needs `--account`. An explicit `--account`, or the
spec's `slurm_defaults`, still wins.

## Generate a profile from the cluster

```bash
torc hpc generate
torc hpc generate --name mycluster --display-name "My Research Cluster"
torc hpc generate --skip-stdby -o mycluster-profile.toml
```

This queries `sinfo` and `scontrol` for partition names, CPUs, memory, time limits, GPU GRES, node
sharing, and a hostname detection pattern, then prints a config snippet (or writes it with `-o`).
Paste it into a config file.

Review two things in the generated output before trusting it: set `requires_explicit_request = true`
on partitions that should never be auto-selected (debug, standby, reservations), and add
`description` values so later readers know what each partition is for.

`--skip-stdby` omits `-stdby` partitions, which are usually preemptible and a poor default target.

## Custom profile schema

```toml
[client.hpc.custom_profiles.mycluster]
display_name = "My Research Cluster"
description = "Internal research HPC system"
detect_env_var = "MY_CLUSTER=research"      # NAME=value
detect_hostname = "^mc-login\\d+$"           # regex
default_account = "default_project"
charge_factor_cpu = 1.0
charge_factor_gpu = 10.0

[[client.hpc.custom_profiles.mycluster.partitions]]
name = "compute"
description = "General purpose nodes"
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
gpu_type = "a100"
gpu_memory_gb = 80
shared = false
requires_explicit_request = false
```

Profile fields: `display_name` is required; `description`, `detect_env_var`, `detect_hostname`,
`default_account`, `charge_factor_cpu` (default 1.0), `charge_factor_gpu` (default 10.0), and
`partitions` are optional.

Partition fields: `name`, `cpus_per_node`, `memory_mb`, and `max_walltime_secs` are required;
`description`, `gpus_per_node`, `gpu_type`, `gpu_memory_gb`, `shared`, and
`requires_explicit_request` are optional.

Details that change generated output:

- `memory_mb` is per node in MB, and `max_walltime_secs` is in seconds, unlike the spec's `memory`
  strings and ISO8601 `runtime`.
- Understating `memory_mb` or `cpus_per_node` makes matching fail for jobs that would actually fit;
  overstating it produces allocations Slurm rejects.
- `shared` controls whether the partition supports sharing a node between jobs.
- `requires_explicit_request = true` keeps a partition out of automatic matching until named.
- Detection uses `detect_env_var` or `detect_hostname`. Omit both if you always pass `--profile`.

Precedence: built-in profiles, then `profile_overrides` on top of them, then `custom_profiles`. A
custom profile named after a built-in replaces it.

## Verify a profile

```bash
torc config validate
torc hpc list
torc hpc show mycluster
torc hpc partitions mycluster
torc hpc match --cpus 32 --memory 64g --walltime 2:00:00 mycluster
torc slurm generate --profile mycluster --account acct workflow.yaml --dry-run
```

`hpc match` is the direct test of whether a partition will be selected for a given requirement.
Confirm it before submitting: a wrong profile produces plausible-looking schedulers that Slurm then
rejects, or allocations too small for the jobs assigned to them.
