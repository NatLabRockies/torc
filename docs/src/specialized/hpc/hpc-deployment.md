# HPC Deployment Reference

Configuration guide for deploying Torc on High-Performance Computing systems.

## Overview

Running Torc on HPC systems requires special configuration to ensure:

- Compute nodes can reach the torc-server running on a login node
- The database is stored on a filesystem accessible to all nodes
- Network paths use the correct hostnames for the HPC interconnect

## Server Configuration on Login Nodes

### Hostname Requirements

On most HPC systems, login nodes have multiple network interfaces:

- **External hostname**: Used for SSH access from outside (e.g., `kl3.hpc.nrel.gov`)
- **Internal hostname**: Used by compute nodes via the high-speed interconnect (e.g.,
  `kl3.hsn.cm.kestrel.hpc.nrel.gov`)

When running `torc-server` on a login node, you must use the **internal hostname** so compute nodes
can connect.

### NREL Kestrel Example

On NREL's Kestrel system, login nodes use the High-Speed Network (HSN) for internal communication:

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

## Database Placement

The SQLite database must be on a filesystem accessible to both:

- The login node running `torc-server`
- All compute nodes running jobs

### Recommended Locations

| Filesystem                  | Pros                        | Cons                       |
| --------------------------- | --------------------------- | -------------------------- |
| Scratch (`/scratch/$USER/`) | Fast, shared, high capacity | May be purged periodically |
| Project (`/projects/`)      | Persistent, shared          | May have quotas            |
| Home (`~`)                  | Persistent                  | Often slow, limited space  |

**Best practice:** Use scratch for active workflows, backup completed workflows to project storage.

```bash
# Create a dedicated directory
mkdir -p /scratch/$USER/torc

# Start server with scratch database
torc-server run \
    --database /scratch/$USER/torc/workflows.db \
    --host $(hostname -s).hsn.cm.kestrel.hpc.nrel.gov \
    --port 8085
```

### Database Backup

For long-running workflows, periodically backup the database:

```bash
# SQLite backup (safe while server is running)
sqlite3 /scratch/$USER/torc.db ".backup /projects/$USER/torc_backup.db"
```

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
output_dir = "/scratch/myuser/torc/output"
```

### Environment Variables

Alternatively, set environment variables in your shell profile:

```bash
# Add to ~/.bashrc or ~/.bash_profile
export TORC_API_URL="http://kl3.hsn.cm.kestrel.hpc.nrel.gov:8085/torc-service/v1"
export TORC_CLIENT__RUN__OUTPUT_DIR="/scratch/$USER/torc/output"
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
- [Working with Slurm](./slurm.md)
- [Server Deployment](../admin/server-deployment.md)
