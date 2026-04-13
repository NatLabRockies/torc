# Remote Workers

Run workflows across multiple machines via SSH without requiring an HPC scheduler.

## Overview

Torc supports three execution modes:

1. **Local** (`torc run`) - Jobs run on the current machine
2. **HPC** (`torc slurm generate` + `torc submit`) - Jobs run on Slurm-allocated nodes
3. **Remote Workers** (`torc remote run`) - Jobs run on SSH-accessible machines

Remote workers are ideal for:

- Ad-hoc clusters of workstations or cloud VMs
- Environments without a scheduler
- Testing distributed workflows before HPC deployment

## Worker File Format

Create a text file listing remote machines:

```text
# Lines starting with # are comments
# Format: [user@]hostname[:port]

# Simple hostname
worker1.example.com

# With username
alice@worker2.example.com

# With custom SSH port
admin@192.168.1.10:2222

# IPv4 address
10.0.0.5

# IPv6 address (must be in brackets for port specification)
[2001:db8::1]
[::1]:2222
```

Each host can only appear once. Duplicate hosts will cause an error.

## Worker Management

Workers are stored in the database and persist across command invocations. This means you only need
to specify workers once, and subsequent commands can reference them by workflow ID.

### Add Workers

```console
torc remote add-workers <workflow-id> <worker>...
```

Add one or more workers directly on the command line:

```console
torc remote add-workers 42 worker1.example.com alice@worker2.example.com admin@192.168.1.10:2222
```

### Add Workers from File

```console
torc remote add-workers-from-file <worker-file> [workflow-id]
```

Example:

```console
torc remote add-workers-from-file workers.txt 42
```

If `workflow-id` is omitted, you'll be prompted to select a workflow interactively.

### List Workers

```console
torc remote list-workers [workflow-id]
```

If `workflow-id` is omitted, you'll be prompted to select a workflow interactively.

### Remove a Worker

```console
torc remote remove-worker <worker> [workflow-id]
```

Example:

```console
torc remote remove-worker worker1.example.com 42
```

If `workflow-id` is omitted, you'll be prompted to select a workflow interactively.

## Commands

### Start Workers

```console
torc remote run [workflow-id] [options]
```

If `workflow-id` is omitted, you'll be prompted to select a workflow interactively.

Workers are fetched from the database. If you want to add workers from a file at the same time:

```console
torc remote run <workflow-id> --workers <worker-file> [options]
```

**Options:**

| Option                 | Default       | Description                                        |
| ---------------------- | ------------- | -------------------------------------------------- |
| `--workers`            | none          | Worker file to add before starting                 |
| `-o, --output-dir`     | `torc_output` | Output directory on remote machines                |
| `--max-parallel-ssh`   | `10`          | Maximum parallel SSH connections                   |
| `-p, --poll-interval`  | `5.0`         | How often workers poll for jobs (seconds)          |
| `--max-parallel-jobs`  | auto          | Maximum parallel jobs per worker                   |
| `--num-cpus`           | auto          | CPUs per worker (auto-detected if not specified)   |
| `--memory-gb`          | auto          | Memory per worker (auto-detected if not specified) |
| `--num-gpus`           | auto          | GPUs per worker (auto-detected if not specified)   |
| `--skip-version-check` | false         | Skip version verification (not recommended)        |

**Example:**

```console
# First time: add workers and start
torc remote run 42 --workers workers.txt \
    --output-dir /data/torc_output \
    --poll-interval 10

# Subsequent runs: workers already in database
torc remote run 42 --output-dir /data/torc_output
```

### Check Status

```console
torc remote status [workflow-id] [options]
```

Shows which workers are still running. Workers are fetched from the database. If `workflow-id` is
omitted, you'll be prompted to select a workflow interactively.

**Options:**

| Option               | Default | Description                      |
| -------------------- | ------- | -------------------------------- |
| `--max-parallel-ssh` | `10`    | Maximum parallel SSH connections |

### Stop Workers

```console
torc remote stop [workflow-id] [options]
```

If `workflow-id` is omitted, you'll be prompted to select a workflow interactively.

**Options:**

| Option               | Default | Description                      |
| -------------------- | ------- | -------------------------------- |
| `--force`            | `false` | Send SIGKILL instead of SIGTERM  |
| `--max-parallel-ssh` | `10`    | Maximum parallel SSH connections |

### Collect Logs

```console
torc remote collect-logs [workflow-id] [options]
```

If `workflow-id` is omitted, you'll be prompted to select a workflow interactively.

**Options:**

| Option                   | Default       | Description                                    |
| ------------------------ | ------------- | ---------------------------------------------- |
| `-l, --local-output-dir` | `remote_logs` | Local directory for collected logs             |
| `--remote-output-dir`    | `torc_output` | Remote output directory                        |
| `--delete`               | `false`       | Delete remote logs after successful collection |
| `--max-parallel-ssh`     | `10`          | Maximum parallel SSH connections               |

**Example with deletion:**

```console
# Collect logs and clean up remote workers
torc remote collect-logs 42 --delete
```

### Delete Logs

```console
torc remote delete-logs [workflow-id] [options]
```

Delete the output directory from all remote workers without collecting logs first. Use
`collect-logs --delete` if you want to save logs before deleting.

If `workflow-id` is omitted, you'll be prompted to select a workflow interactively.

**Options:**

| Option                | Default       | Description                      |
| --------------------- | ------------- | -------------------------------- |
| `--remote-output-dir` | `torc_output` | Remote output directory          |
| `--max-parallel-ssh`  | `10`          | Maximum parallel SSH connections |

## Typical Workflow

1. **Create a workflow:**

   ```console
   torc create my_workflow.yaml
   ```

2. **Add workers:**

   ```console
   # From command line
   torc remote add-workers 42 worker1.example.com worker2.example.com

   # Or from file
   torc remote add-workers-from-file workers.txt 42
   ```

3. **Start workers:**

   ```console
   torc remote run 42
   ```

4. **Monitor status:**

   ```console
   torc remote status 42
   ```

5. **Collect logs when complete:**

   ```console
   torc remote collect-logs 42 -l ./logs
   ```

Or combine steps 2 and 3:

```console
torc remote run 42 --workers workers.txt
```

## How It Works

1. **Version Check**: Verifies all remote machines have the same torc version
2. **Worker Start**: Uses `nohup` to start detached workers that survive SSH disconnection
3. **Job Execution**: Each worker polls the server for available jobs and executes them locally
4. **Completion**: Workers exit when the workflow is complete or canceled

The server coordinates job distribution. Multiple workers can safely poll the same workflow without
double-allocating jobs.

## SSH Configuration

Workers connect using SSH with these options:

- `ConnectTimeout=30` - 30 second connection timeout
- `BatchMode=yes` - No password prompts (requires key-based auth)
- `StrictHostKeyChecking=accept-new` - Accept new host keys automatically

For custom SSH configuration, use `~/.ssh/config` on the local machine:

```ssh-config
Host worker1
    HostName worker1.example.com
    User alice
    Port 2222
    IdentityFile ~/.ssh/worker_key
```

Then reference the alias in your worker file:

```text
worker1
worker2
worker3
```

## Resource Monitoring

If your workflow has resource monitoring enabled, each worker collects utilization data:

```yaml
name: my_workflow
resource_monitor_config:
  enabled: true
  granularity: time_series
  sample_interval_seconds: 10
```

The `collect-logs` command retrieves these databases along with job logs.

## Troubleshooting

### No Workers Configured

```
No workers configured for workflow 42. Use 'torc remote add-workers' or '--workers' flag.
```

Add workers to the workflow using `torc remote add-workers` or the `--workers` flag on `run`.

### Version Mismatch

```
Error: Version check failed on 2 worker(s):
  worker1: Version mismatch: local=0.7.0, worker1=0.6.5
  worker2: Version mismatch: local=0.7.0, worker2=0.6.5
```

Install the same torc version on all machines, or use `--skip-version-check` (not recommended for
production).

### SSH Connection Failed

```
Error: SSH connectivity check failed for 1 worker(s):
  worker1: SSH connection failed to worker1: Permission denied (publickey)
```

Verify SSH key-based authentication works:

```console
ssh worker1.example.com true
```

### Worker Died Immediately

```
[FAILED] worker1: Process died immediately. Last log:
  Error: connection refused...
```

The worker couldn't connect to the server. Check:

1. Server is accessible from the remote machine
2. Firewall allows connections on the server port
3. The `--url` points to the correct server address

### Workers Not Claiming Jobs

If workers start but don't claim jobs:

1. Check the workflow is initialized: `torc status <id>`
2. Check jobs are ready: `torc jobs list <id>`
3. Check resource requirements match available resources

## Comparison with Slurm

| Feature             | Remote Workers           | Slurm              |
| ------------------- | ------------------------ | ------------------ |
| Scheduler required  | No                       | Yes                |
| Resource allocation | Manual (worker file)     | Automatic          |
| Fault tolerance     | Limited                  | Full (job requeue) |
| Walltime limits     | No                       | Yes                |
| Priority/queuing    | No                       | Yes                |
| Best for            | Ad-hoc clusters, testing | Production HPC     |

## Security Considerations

- Workers authenticate to the torc server (if authentication is enabled)
- SSH keys should be properly secured
- Workers run with the permissions of the SSH user on each machine
- The torc server URL is passed to workers and visible in process lists
