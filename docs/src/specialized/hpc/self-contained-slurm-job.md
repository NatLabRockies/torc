# Self-Contained Slurm Jobs

If you don't have access to a persistent torc server on your cluster, you can still run torc
workflows inside a single Slurm job. The job script starts an ephemeral torc-server, runs the
workflow, and shuts the server down when the script exits. The SQLite database persists on disk, so
you can inspect the results afterwards from the login node.

This is the simplest way to use torc on HPC when no one has set up a shared deployment.

## When to Use This Pattern

Self-contained jobs fit when:

- You only need **one compute node** for the whole workflow.
- You don't have a running torc-server and don't want to deploy one.
- You want a single sbatch script you can hand to a colleague.

They do **not** fit when jobs must run across multiple nodes in a single allocation. The standalone
server binds to `127.0.0.1`, so nothing outside the script's node can reach it. For multi-node
allocations, use the [persistent-server pattern](#multi-node-alternative) below.

## Run a Workflow Spec

Pass any spec file to `torc run` and add `-s`/`--standalone`:

```bash
#!/bin/bash
#SBATCH --account=my-account
#SBATCH --time=01:00:00
#SBATCH --nodes=1

torc -s run workflow.yaml
```

torc will:

1. Start a local torc-server bound to `127.0.0.1` on a free port.
2. Create the workflow, initialize it, and run all jobs on the allocated node.
3. Shut the server down when `run` returns.

By default the database is written to `./torc_output/torc.db`. Override with `--db` if you want it
somewhere persistent (e.g., a project filesystem):

```bash
torc -s --db /projects/my-proj/torc/runs.db run workflow.yaml
```

## Run Ad-Hoc Commands

If you have a list of shell commands rather than a spec file, use `torc exec`:

```bash
#!/bin/bash
#SBATCH --account=my-account
#SBATCH --time=01:00:00
#SBATCH --nodes=1

torc -s exec -C commands.txt -j 4
```

`commands.txt` is one command per line (blank lines and `#` comments are skipped). `-j 4` caps
concurrent jobs at 4. Torc still records per-job CPU/memory so you can tune resources after the run.
See [Run Inline Commands](../../core/how-to/run-inline-commands.md) for the full parameterization
and monitoring surface.

## Inspect Results After the Job

The database file lives on disk, so after the Slurm job exits you can read it back from any host
that can open the file — typically the login node:

```console
torc -s --db ./torc_output/torc.db workflows list
torc -s --db ./torc_output/torc.db results list <workflow-id>
torc tui --standalone --database ./torc_output/torc.db
```

Each `-s` invocation starts its own short-lived server against the same database, so multiple
inspections don't conflict.

## Multi-Node Alternative

For allocations that span multiple compute nodes, the standalone server doesn't work — the job
runners on other nodes can't reach `127.0.0.1` on the script's node. Start a regular torc-server
bound to a routable hostname and have `srun` launch a runner on each node:

```bash
#!/bin/bash
#SBATCH --account=my-account
#SBATCH --time=01:00:00
#SBATCH --nodes=10

torc-server run --host $(hostname) --port 8080 &
server_pid=$!
trap "kill $server_pid" EXIT

export TORC_API_URL="http://$(hostname):8080/torc-service/v1"
sleep 2   # let the server bind before clients connect

srun torc run workflow.yaml
```

On many HPC systems `$(hostname)` returns the default interface rather than the high-speed
interconnect one that compute nodes route through. See [HPC Deployment](./hpc-deployment.md) for
hostname selection on specific systems.

## See Also

- [Run Inline Commands (`torc exec`)](../../core/how-to/run-inline-commands.md) — full `exec` flag
  reference, parameterization, plots
- [HPC Deployment](./hpc-deployment.md) — persistent server deployments, hostname selection
- [Submitting Slurm Workflows](./submit-slurm-workflow.md) — `slurm generate` + `submit`, for
  workflows whose jobs each launch their own Slurm allocation
