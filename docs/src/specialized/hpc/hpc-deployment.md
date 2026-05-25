# HPC Deployment Reference

Configuration guide for deploying Torc on High-Performance Computing systems.

## Overview

Running Torc on HPC systems requires special configuration to ensure:

- Compute nodes can reach the torc-server running on a login node
- The database lives on storage the **server host** can lock correctly (compute nodes never open the
  database file directly — they reach it through the server over HTTP)
- Network paths use the correct hostnames for the HPC interconnect

## Server Configuration on Login Nodes

### Hostname Requirements

On most HPC systems, login nodes have multiple network interfaces:

- **External hostname**: Used for SSH access from outside (e.g., `kl3.hpc.nrel.gov`)
- **Internal hostname**: Used by compute nodes via the high-speed interconnect (e.g.,
  `kl3.hsn.cm.kestrel.hpc.nrel.gov`)

When running `torc-server` on a login node, you must use the **internal hostname** so compute nodes
can connect.

### NLR Kestrel Example

On NLR's Kestrel system, login nodes use the High-Speed Network (HSN) for internal communication:

| Login Node | External Hostname  | Internal Hostname (for `--host` flag) |
| ---------- | ------------------ | ------------------------------------- |
| kl1        | `kl1.hpc.nrel.gov` | `kl1.hsn.cm.kestrel.hpc.nrel.gov`     |
| kl2        | `kl2.hpc.nrel.gov` | `kl2.hsn.cm.kestrel.hpc.nrel.gov`     |
| kl3        | `kl3.hpc.nrel.gov` | `kl3.hsn.cm.kestrel.hpc.nrel.gov`     |

**Starting the server:**

```bash
# On login node kl3, use the internal hostname
torc-server run \
    --database /scratch/$USER/torc.db \
    --host kl3.hsn.cm.kestrel.hpc.nrel.gov \
    --port 8085
```

**Connecting clients:**

```bash
# Set the API URL using the internal hostname
export TORC_API_URL="http://kl3.hsn.cm.kestrel.hpc.nrel.gov:8085/torc-service/v1"

# Now torc commands will use this URL
torc workflows list
```

### Finding the Internal Hostname

If you're unsure of your system's internal hostname, try these approaches:

```bash
# Check all network interfaces
hostname -A

# Look for hostnames in the hosts file
grep $(hostname -s) /etc/hosts

# Check Slurm configuration for the control machine
scontrol show config | grep ControlMachine
```

Consult your HPC system's documentation or support team for the correct internal hostname format.

## Database Storage Requirements

Only the `torc-server` process opens the SQLite database — compute nodes reach it through the server
over HTTP. So the database does **not** need to be on a filesystem shared with the compute nodes; it
only needs to be on storage the **server host** can open and lock correctly.

### Avoid parallel and networked filesystems for the live database

SQLite coordinates concurrent access using POSIX byte-range (`fcntl`) advisory locks. Parallel and
distributed filesystems implement these locks poorly or not at all:

- **Lustre** — `flock`/`fcntl` locking works only when the filesystem is mounted with the `flock`
  option, and even then SQLite throughput is poor and lock semantics can be unreliable. **Lustre is
  not a good place for the live database.**
- **GPFS / NFS** — advisory locking is frequently misconfigured, partial, or high-latency. NFS in
  particular is a classic source of `database is locked` errors and, in the worst case, corruption.

A stalled shared filesystem can also hang the server's request handlers for tens of seconds, since
the in-flight SQLite call blocks on I/O.

### Recommended storage

| Storage                                | Suitability for the live DB                                                    |
| -------------------------------------- | ------------------------------------------------------------------------------ |
| Node-local disk / `/tmp` on the server | **Best.** Correct locking, lowest latency. Pair with snapshots for durability. |
| In-memory (`:memory:`)                 | **Best for high throughput.** RAM-backed; snapshot to disk for persistence.    |
| Scratch (Lustre/GPFS, e.g. `/scratch`) | Avoid for the live DB (locking + stalls). Fine as a _snapshot/backup_ target.  |
| Project (`/projects/`)                 | Avoid for the live DB. Good for archiving completed databases.                 |
| Home (`~`, often NFS)                  | Avoid — slow and locking-prone.                                                |

**Best practice:** run the live database on node-local disk (or in memory), and snapshot/back up to
scratch or project storage. The default `torc.db` in the current directory is fine on a login node
whose home/scratch is fast and POSIX-correct, but when in doubt prefer `/tmp`:

```bash
# Live DB on node-local disk; back up to durable shared storage periodically
torc-server run \
    --database /tmp/torc-$USER.db \
    --host $(hostname -s).hsn.cm.kestrel.hpc.nrel.gov \
    --port 8085
```

For RAM-backed operation with snapshots, see
[In-Memory Database with Snapshots](../admin/server-deployment.md#in-memory-database-with-snapshots-advanced).

### Database Backup

For long-running workflows, periodically backup the database:

```bash
# SQLite backup (safe while server is running)
sqlite3 /tmp/torc-$USER.db ".backup /projects/$USER/torc_backup.db"
```

You can also snapshot a running server with `SIGUSR1` (works for on-disk and in-memory databases) —
see [Persisting State with SIGUSR1](../admin/server-deployment.md#persisting-state-with-sigusr1).

## Port Selection

Login nodes are shared resources. To avoid conflicts:

1. **Use a non-default port**: Choose a port in the range 8000-9999
2. **Check for conflicts**: `lsof -i :8085`
3. **Consider using your UID**: `--port $((8000 + UID % 1000))`

```bash
# Use a unique port based on your user ID
MY_PORT=$((8000 + $(id -u) % 1000))
torc-server run \
    --database /scratch/$USER/torc.db \
    --host kl3.hsn.cm.kestrel.hpc.nrel.gov \
    --port $MY_PORT
```

## Running in tmux/screen

Always run `torc-server` in a terminal multiplexer to prevent loss on disconnect:

```bash
# Start a tmux session
tmux new -s torc

# Start the server
torc-server run \
    --database /scratch/$USER/torc.db \
    --host kl3.hsn.cm.kestrel.hpc.nrel.gov \
    --port 8085

# Detach with Ctrl+b, then d
# Reattach later with: tmux attach -t torc
```

## Running the Server in a Dedicated Slurm Allocation

When login-node policy forbids long-running processes, or you want the server isolated from a busy,
oversubscribed login node, run `torc-server` inside its **own** small Slurm allocation while your
jobs draw their own, independent allocations. Request a few CPUs from a shared/standby partition for
the full duration of the workflow:

```bash
#!/bin/bash
#SBATCH --job-name=torc-server
#SBATCH --partition=shared      # a partition that allows small, long-lived jobs
#SBATCH --account=my-account
#SBATCH --nodes=1
#SBATCH --ntasks=1
#SBATCH --cpus-per-task=4
#SBATCH --time=24:00:00         # long enough to outlast the workflow

# Live DB on the server node's local disk (correct locking, low latency).
torc-server run \
    --database /tmp/torc.db \
    --host 0.0.0.0 \
    --port 8085 \
    --threads 4
```

Submit it, wait for it to start, then discover the node it landed on and point clients (and the jobs
they submit) at it:

```bash
sbatch torc-server.sbatch
# Once it is RUNNING:
SERVER_NODE=$(squeue --me --name torc-server -h -o %N)
export TORC_API_URL="http://${SERVER_NODE}:8085/torc-service/v1"
torc workflows list
```

Notes:

- **Bind to `0.0.0.0`** so the server accepts connections on whichever interface compute nodes use.
  If the bare node name isn't routable between nodes on your cluster, use the HSN name (see
  [Finding the Internal Hostname](#finding-the-internal-hostname)).
- **The server allocation is independent of the job allocations.** Jobs schedule and scale on their
  own; the server just needs to stay up. Size `--time` to cover the whole workflow — if the server
  allocation expires mid-run, runners lose the server. Pair with periodic
  [snapshots](../admin/server-deployment.md#persisting-state-with-sigusr1) so you can restart and
  resume if that happens.
- **Keep the database on node-local disk** (`/tmp`), not a parallel filesystem — see
  [Database Storage Requirements](#database-storage-requirements).

## Complete Configuration Example

### Server Configuration File

Create `~/.config/torc/config.toml`:

```toml
[server]
# Use internal hostname for compute node access
host = "kl3.hsn.cm.kestrel.hpc.nrel.gov"
port = 8085
database = "/scratch/myuser/torc/workflows.db"
threads = 4
completion_check_interval_secs = 30.0
log_level = "info"

[server.logging]
log_dir = "/scratch/myuser/torc/logs"
```

### Client Configuration File

Create `~/.config/torc/config.toml` (or add to existing):

```toml
[client]
# Match the server's internal hostname and port
api_url = "http://kl3.hsn.cm.kestrel.hpc.nrel.gov:8085/torc-service/v1"
format = "table"

[client.run]
output_dir = "/scratch/myuser/torc/torc_output"
```

### Environment Variables

Alternatively, set environment variables in your shell profile:

```bash
# Add to ~/.bashrc or ~/.bash_profile
export TORC_API_URL="http://kl3.hsn.cm.kestrel.hpc.nrel.gov:8085/torc-service/v1"
export TORC_CLIENT__RUN__OUTPUT_DIR="/scratch/$USER/torc/torc_output"
```

## Slurm Job Runner Configuration

When submitting workflows to Slurm, the job runners on compute nodes need to reach the server. The
`TORC_API_URL` is automatically passed to Slurm jobs.

Verify connectivity from a compute node:

```bash
# Submit an interactive job
salloc -N 1 -t 00:10:00

# Test connectivity to the server
curl -s "$TORC_API_URL/workflows" | head

# Exit the allocation
exit
```

## Large-Scale Deployments

### Startup Jitter (Thundering Herd Mitigation)

When many Slurm allocations start simultaneously — for example, 1000 single-node jobs scheduled at
once — all `torc-slurm-job-runner` processes may contact the server at the same instant. This
"thundering herd" can overwhelm the server with concurrent requests, causing connection timeouts and
SQLite lock contention.

Torc mitigates this automatically. When `torc slurm schedule-nodes` generates sbatch scripts, it
calculates a startup delay window based on the total number of runners that will start:

| Total runners | Max startup delay |
| ------------- | ----------------- |
| 1             | 0 s (disabled)    |
| 2–10          | 2–10 s            |
| 11–100        | 10–60 s           |
| 100+          | 60 s              |

Each runner picks a deterministic delay within this window (hashed from its hostname, Slurm job ID,
node ID, and task PID), then sleeps before making its first API call. This spreads the initial burst
of requests across the delay window.

The delay is passed to `torc-slurm-job-runner` via the `--startup-delay-seconds` flag in the
generated sbatch script. You can override it manually if needed:

```bash
# In a custom sbatch script: set a 120-second jitter window
torc-slurm-job-runner $URL $WORKFLOW_ID $OUTPUT --startup-delay-seconds 120
```

When `start_one_worker_per_node` is enabled, the total runner count includes all nodes across all
allocations (e.g., 10 allocations × 4 nodes = 40 runners), so the delay window scales appropriately.

To disable staggered startup, set `staggered_start: false` in `execution_config`:

```yaml
execution_config:
  staggered_start: false
```

### Server Tuning for Large Workflows

For workflows with many concurrent compute nodes, consider increasing the server thread count to
expand the database connection pool:

```bash
# Default is 1 thread (3 connections). For 100+ nodes, increase:
torc-server run --threads 8 --database /scratch/$USER/torc.db --host $HOST --port $PORT
```

The connection pool size is `max(threads, 2) + 2`, so `--threads 8` gives 10 connections.

## Troubleshooting

### "Connection refused" from compute nodes

1. Verify the server is using the internal hostname:
   ```bash
   torc-server run --host <internal-hostname> --port 8085
   ```

2. Check the server is listening on all interfaces:
   ```bash
   netstat -tlnp | grep 8085
   ```

3. Verify no firewall blocks the port:
   ```bash
   # From a compute node
   nc -zv <internal-hostname> 8085
   ```

### Database locked errors

SQLite may report locking issues on network filesystems:

1. Ensure only one `torc-server` instance is running
2. Use a local scratch filesystem rather than NFS home directories
3. Consider increasing `completion_check_interval_secs` to reduce database contention

### Server stops when SSH disconnects

Always use tmux or screen (see above). If the server dies unexpectedly:

```bash
# Check if the server is still running
pgrep -f torc-server

# Check server logs
tail -100 /scratch/$USER/torc/logs/torc-server*.log
```

## See Also

- [Configuration Reference](../../core/reference/configuration.md)
- [HPC Profiles Reference](./hpc-profiles-reference.md)
- [Advanced Slurm Configuration](./slurm.md)
- [Server Deployment](../admin/server-deployment.md)
