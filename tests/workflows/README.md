# Test Workflows

This directory contains workflow specifications for testing Torc features. These are not intended
for end users - they are for development and testing purposes.

## Automated Slurm Tests

Slurm-focused integration tests have been moved to `slurm-tests/` at the repository root. Run them
with:

```bash
./slurm-tests/run_all.sh --account myproject --host <login-node-fqdn> [--partition debug] [--timeout 45]
```

This covers: single-node basic, multi-node parallel, multi-node MPI step, OOM detection, resource
monitoring, failure recovery, timeout detection, cancel workflow, and sync-status tests.

---

## Watcher Test Workflows

The following directories contain complete test scenarios for `torc watch` functionality. These are
manual tests only — each recovery cycle requires multiple Slurm allocations and would take 30+
minutes, so they are not part of the automated suite.

### oom_auto_recovery_test/

Tests automatic OOM recovery in `torc watch --recover`.

**Scenario:**

- 10 work jobs that request 10GB memory but try to allocate 30GB
- Jobs fail with OOM
- Watcher detects OOM and increases memory (10GB -> 15GB -> 22GB -> 33GB)
- Eventually jobs get enough memory and succeed

**Usage:**

```bash
cd tests/workflows/oom_auto_recovery_test
# Edit workflow.yaml to set your Slurm account
torc slurm generate --account <account> workflow.yaml
torc submit workflow.yaml
torc watch <workflow_id> --recover --max-retries 5
```

See `oom_auto_recovery_test/README.md` for detailed instructions.

### timeout_auto_recovery_test/

Tests automatic timeout recovery in `torc watch --recover`.

**Scenario:**

- 2 jobs with 5 minute runtime specified
- `job_fast` completes in 1 minute (succeeds)
- `job_slow` runs for 10 minutes (exceeds walltime, gets killed)
- Watcher detects timeout and increases runtime (5min -> 7.5min -> 11.25min)
- Eventually job gets enough time and succeeds

**Usage:**

```bash
cd tests/workflows/timeout_auto_recovery_test
# Edit workflow.yaml to set your Slurm account
torc slurm generate --account <account> workflow.yaml
torc submit workflow.yaml
torc watch <workflow_id> --recover --max-retries 3
```

See `timeout_auto_recovery_test/README.md` for detailed instructions.

---

## Workflows

### resource_regroup_test/

Tests the `analyze_resource_usage` and `regroup_job_resources` MCP tools.

**Scenario:**

- 6 data-processing jobs in 2 RR groups with varying actual memory usage
- Job names and commands are opaque — the AI must analyze results to discover usage patterns
- AI identifies natural clusters and proposes better resource groupings

**Usage:**

```bash
torc run tests/workflows/resource_regroup_test/workflow.yaml
# Then use MCP tools: analyze_resource_usage → regroup_job_resources
```

See `resource_regroup_test/README.md` for detailed instructions.

---

### scale_test/

Stress test for database scaling with **100,000 jobs** arranged in 4 rounds of 25,000 jobs each,
separated by barrier jobs. All jobs are no-ops (`echo`), so the bottleneck is purely database
throughput.

**Purpose:** Verify that the server can handle 100K jobs efficiently — creation, dependency
resolution, job claiming, and completion processing under heavy concurrent access.

**Usage:**

```bash
# Quick start (single process, 16 parallel jobs)
torc run tests/workflows/scale_test/workflow.yaml --num-parallel-processes 16

# Or with multiple independent runners for maximum contention
torc create tests/workflows/scale_test/workflow.yaml
for i in $(seq 1 16); do
  torc run <workflow_id> &
done
```

See `scale_test/README.md` for detailed instructions including multi-node setup.

---

### database_contention_test/

Stress test for SQLite database with high contention. 5 stages x 1000 jobs each = 5000 total jobs,
with barriers between stages.

See `database_contention_test/README.md` for detailed instructions.

---

### tls_test/

Tests client-side TLS features (`--tls-ca-cert` and `--tls-insecure`) using a Python HTTPS reverse
proxy in front of a plain HTTP torc-server.

**Scenario:**

- Generates a CA + server certificate with macOS-compatible extensions
- Starts an HTTPS reverse proxy (Python) on port 8443 → torc-server on port 8080
- Runs 6 tests: insecure mode, CA cert, rejection of untrusted certs, env vars

**Usage:**

```bash
# Terminal 1: start torc-server
torc-server run

# Terminal 2: run the test
cd tests/workflows/tls_test
bash run_test.sh
```

See `tls_test/README.md` for detailed instructions.

---

### access_control_test/

Tests authentication and workflow access control via access groups.

**Scenario:**

- 4 users (alice, bob, carol, dave) with overlapping team memberships
- 3 teams: ml_team (alice, bob), data_team (bob, carol), infra_team (carol, dave)
- 3 team workflows (one per group) + 4 private workflows (one per user)
- ~35 assertions verify visibility rules, auth rejection, and direct ID access

**Usage:**

```bash
bash tests/workflows/access_control_test/run_test.sh
```

See `access_control_test/README.md` for detailed instructions.
