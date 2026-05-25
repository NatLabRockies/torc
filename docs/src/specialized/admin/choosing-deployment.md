# Choosing a Server Deployment

Torc is a client-server system: every workflow talks to a `torc-server` over HTTP, and the server
owns the SQLite database. How you run that server depends on three things:

1. Whether a **shared server** already exists on your system.
2. How many jobs the workflow has and how long they run.
3. Whether jobs span **one node** or **multiple nodes**.

This page helps you pick a deployment and points to the detailed guide for each. If you are new and
just want to get a small workflow running, start with
[Use the shared server](#1-use-the-shared-server-default).

## Quick decision

| Your situation                                                                     | Recommended deployment                                                                                                             |
| ---------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| A shared server exists and your workflow is small/moderate (≲ a few thousand jobs) | [Use the shared server](#1-use-the-shared-server-default)                                                                          |
| No shared server, all jobs fit on **one node**                                     | [Standalone ephemeral server](#2a-single-node-standalone-ephemeral-server)                                                         |
| No shared server, jobs span **multiple nodes**                                     | [Login-node server](#2b-multi-node-login-node-server) or [dedicated server allocation](#2c-multi-node-dedicated-server-allocation) |
| Very large workflow (≳10⁵ jobs), short jobs, or a slow shared filesystem           | [In-memory server with snapshots](#3-very-large-or-high-throughput-workflows)                                                      |

## What the server actually needs

A key fact that drives every choice below: **only the server process opens the database file.** Job
runners on compute nodes never touch the SQLite file directly — they make HTTP requests to the
server. So the database only has to live somewhere the _server host_ can open it with correct file
locking. It does **not** need to be on a filesystem shared with the compute nodes.

This is why putting the database on node-local storage (and avoiding parallel filesystems like
Lustre) is both safe and recommended. See
[Database storage requirements](../hpc/hpc-deployment.md#database-storage-requirements) for the
details and the locking caveats.

## 1. Use the shared server (default)

If your HPC system or team already runs a persistent `torc-server`, this is the simplest path and
the right default for the common case — a workflow with up to a few thousand jobs whose individual
runs take more than a few seconds. You do not run or configure a server at all; you just point the
client at it:

```bash
export TORC_API_URL="http://<server-host>:<port>/torc-service/v1"
torc create workflow.yaml
torc submit <workflow_id>
```

If the shared server requires authentication, also set credentials — see
[Authentication](./authentication.md).

Move to one of the options below if no shared server exists, or if your workflow is large enough or
fast enough that a single shared SQLite database would become a bottleneck (see
[scenario 3](#3-very-large-or-high-throughput-workflows)).

## 2. No shared server

When nobody has deployed a server, you run your own. Which pattern depends on node topology.

### 2a. Single-node: standalone ephemeral server

If every job fits on one compute node, the standalone client starts an ephemeral server for you,
runs the workflow, and shuts the server down on exit. This is the simplest self-contained option:

```bash
#!/bin/bash
#SBATCH --nodes=1
#SBATCH --time=01:00:00

torc -s run workflow.yaml          # spec file
# or:  torc -s exec -C commands.txt -j 4   # ad-hoc command list
```

The database persists to `./torc_output/torc.db` (override with `--db`) so you can inspect results
afterward. Full details: [Self-Contained Slurm Jobs](../hpc/self-contained-slurm-job.md).

The standalone server binds to `127.0.0.1`, so this pattern **cannot** be used for jobs that run on
other nodes. For that, use one of the multi-node patterns below.

### 2b. Multi-node: login-node server

Run a persistent `torc-server` on a login node, in a `tmux`/`screen` session, for the duration of
the workflow. Jobs submitted to Slurm reach it over the cluster interconnect:

```bash
torc-server run \
    --database /tmp/torc-$USER.db \
    --host <internal-hostname> \
    --port 8085
```

The critical detail is the `--host` value: it must be the **internal/routable hostname** compute
nodes can reach, not the external SSH name. See [HPC Deployment](../hpc/hpc-deployment.md) for
hostname selection, port choice, and `tmux` guidance.

### 2c. Multi-node: dedicated server allocation

If login-node policy forbids long-running processes, or you want the server isolated from a busy
login node, run the server **inside its own small Slurm allocation** while jobs draw their own,
independent allocations. Grab a few CPUs from a shared/standby partition for the workflow's
duration:

```bash
#!/bin/bash
#SBATCH --job-name=torc-server
#SBATCH --partition=shared      # a partition that allows small, long-lived jobs
#SBATCH --nodes=1
#SBATCH --ntasks=1
#SBATCH --cpus-per-task=4
#SBATCH --time=24:00:00         # cover the whole workflow

# Use node-local disk on the server node.
torc-server run \
    --database /tmp/torc.db \
    --host 0.0.0.0 \
    --port 8085 \
    --threads 4
```

Submit this first, find the node it landed on, and point clients (and the jobs they submit) at it:

```bash
SERVER_NODE=$(squeue --me --name torc-server -h -o %N)
export TORC_API_URL="http://${SERVER_NODE}:8085/torc-service/v1"
```

The job allocations are completely separate from the server allocation, so jobs schedule and scale
independently. Because only the server opens the database, you can — and should — keep it on the
server node's local disk rather than a shared parallel filesystem. See
[Running the server in a dedicated allocation](../hpc/hpc-deployment.md#running-the-server-in-a-dedicated-slurm-allocation).

## 3. Very large or high-throughput workflows

For workflows with very many jobs (10⁵–10⁶), very short jobs (seconds), or on systems whose shared
filesystem stalls intermittently, an on-disk SQLite database becomes the bottleneck. Run the server
**in-memory** and snapshot to disk for persistence. The canonical example is **one million jobs that
each run for a few seconds**.

Assemble these pieces:

1. **In-memory server, bound for multi-node access.** Start the server with an in-memory database on
   all interfaces (the `torc -s --in-memory` standalone shortcut binds to `127.0.0.1` and is
   single-node only, so for multi-node use `torc-server` directly):

   ```bash
   torc-server run -d ":memory:" --host 0.0.0.0 --port 8085 --threads 8
   ```

   The in-memory database lives in RAM. Size the server node's memory for your job count — roughly
   on the order of a gigabyte per million jobs, plus indexes; measure on a smaller run first.

2. **Periodic snapshots for durability.** The in-memory database is lost if the process dies, so
   snapshot it to fast local storage. Send `SIGUSR1` (e.g. from cron or a sidecar), tuning retention
   via `TORC_SERVER_SNAPSHOT_PATH` / `TORC_SERVER_SNAPSHOT_KEEP`:

   ```bash
   export TORC_SERVER_SNAPSHOT_PATH=/tmp/$USER/torc-snapshots/torc.db
   kill -USR1 $(pgrep -f 'torc-server run')
   ```

   With the standalone runner you can instead snapshot on a timer:
   `torc -s --in-memory --snapshot-interval-seconds 600 run workflow.yaml`. Each snapshot briefly
   serializes against writes (seconds for a very large database), so prefer larger intervals at
   scale. See
   [In-Memory Database with Snapshots](./server-deployment.md#in-memory-database-with-snapshots-advanced).

3. **Use the barrier pattern to keep dependencies tractable.** A fan-out/fan-in workflow expressed
   as all-to-all dependencies generates `N × M` edges — a million jobs feeding a million jobs is a
   trillion dependencies. A barrier collapses that to roughly `N + M`. This is essential at this
   scale; see [Multi-Stage Workflows with Barriers](../../core/tutorials/multi-stage-barrier.md).

4. **Expect a startup burst.** When thousands of allocations start at once they all contact the
   server simultaneously. Torc staggers runner startup automatically; raise `--threads` on the
   server to widen the connection pool. See
   [Large-Scale Deployments](../hpc/hpc-deployment.md#large-scale-deployments).

Multi-node jobs still require Slurm allocations as in [scenario 2](#2-no-shared-server); the only
change here is the in-memory database and snapshotting.

## See Also

- [Server Deployment](./server-deployment.md) — full `torc-server` flag and service reference
- [HPC Deployment](../hpc/hpc-deployment.md) — hostnames, ports, database storage, dedicated
  allocations
- [Self-Contained Slurm Jobs](../hpc/self-contained-slurm-job.md) — the standalone single-node and
  multi-node patterns
- [Network Connectivity](../../core/concepts/network-connectivity.md) — how clients reach the server
  in each topology
