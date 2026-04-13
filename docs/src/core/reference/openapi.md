# OpenAPI Specification

The Torc server implements a HTTP API under `/torc-service/v1`.

The checked-in OpenAPI artifact is `api/openapi.yaml`, but the contract is now emitted from
`src/openapi_spec.rs`, live handlers in `src/server/live_router.rs`, and `src/models.rs`. Refresh
the emitted artifact with:

```bash
cd api
bash sync_openapi.sh all --promote
```

For day-to-day development:

```bash
cd api

# Emit Rust-owned spec only
bash sync_openapi.sh emit

# Verify checked-in specs match the emitted contract
bash sync_openapi.sh check

# Regenerate Rust, Python, and Julia clients from the checked-in contract
bash sync_openapi.sh clients

# Or regenerate all three client surfaces from the emitted Rust spec before promotion
bash sync_openapi.sh emit
bash sync_openapi.sh check
bash sync_openapi.sh clients --use-rust-spec
```

## Core Endpoints

### Workflows

**Create Workflow**

```bash
# curl
curl -X POST http://localhost:8080/torc-service/v1/workflows \
  -H "Content-Type: application/json" \
  -d '{
    "name": "test_workflow",
    "user": "alice",
    "description": "Test workflow"
  }' | jq '.'

# nushell
http post http://localhost:8080/torc-service/v1/workflows {
  name: "test_workflow"
  user: "alice"
  description: "Test workflow"
}
```

Response:

```json
{
  "id": 1,
  "name": "test_workflow",
  "user": "alice",
  "description": "Test workflow",
  "timestamp": 1699000000.0
}
```

**List Workflows**

```bash
# curl with jq
curl http://localhost:8080/torc-service/v1/workflows?offset=0&limit=10 | jq '.workflows'

# nushell (native JSON parsing)
http get http://localhost:8080/torc-service/v1/workflows?offset=0&limit=10 | get workflows
```

**Get Workflow**

```bash
# curl
curl http://localhost:8080/torc-service/v1/workflows/1 | jq '.'

# nushell
http get http://localhost:8080/torc-service/v1/workflows/1
```

**Initialize Jobs**

```bash
# curl
curl -X POST http://localhost:8080/torc-service/v1/workflows/1/initialize_jobs \
  -H "Content-Type: application/json" \
  -d '{"reinitialize": false, "ignore_missing_data": false}' | jq '.'

# nushell
http post http://localhost:8080/torc-service/v1/workflows/1/initialize_jobs {
  reinitialize: false
  ignore_missing_data: false
}
```

### Jobs

**Create Job**

```bash
# curl
curl -X POST http://localhost:8080/torc-service/v1/jobs \
  -H "Content-Type: application/json" \
  -d '{
    "workflow_id": 1,
    "name": "job1",
    "command": "echo hello",
    "resource_requirements_id": 1,
    "input_file_ids": [],
    "output_file_ids": [],
    "depends_on_job_ids": []
  }' | jq '.'
```

**List Jobs**

```bash
# curl - filter by status
curl "http://localhost:8080/torc-service/v1/jobs?workflow_id=1&status=ready" \
  | jq '.jobs[] | {name, status, id}'

# nushell - filter and format
http get "http://localhost:8080/torc-service/v1/jobs?workflow_id=1"
  | get jobs
  | where status == "ready"
  | select name status id
```

**Update Job Status**

```bash
# curl
curl -X POST http://localhost:8080/torc-service/v1/jobs/1/manage_status_change \
  -H "Content-Type: application/json" \
  -d '{"target_status": "running"}' | jq '.'
```

### Files

**Create File**

```bash
# curl
curl -X POST http://localhost:8080/torc-service/v1/files \
  -H "Content-Type: application/json" \
  -d '{
    "workflow_id": 1,
    "name": "input_data",
    "path": "/data/input.csv"
  }' | jq '.'
```

**List Files**

```bash
curl "http://localhost:8080/torc-service/v1/files?workflow_id=1" | jq '.files'
```

### User Data

**Create User Data**

```bash
curl -X POST http://localhost:8080/torc-service/v1/user_data \
  -H "Content-Type: application/json" \
  -d '{
    "workflow_id": 1,
    "name": "config",
    "data": {"learning_rate": 0.001, "batch_size": 32}
  }' | jq '.'
```

**Update User Data**

```bash
curl -X PUT http://localhost:8080/torc-service/v1/user_data/1 \
  -H "Content-Type: application/json" \
  -d '{
    "workflow_id": 1,
    "name": "config",
    "data": {"learning_rate": 0.01, "batch_size": 64}
  }' | jq '.'
```

### Resource Requirements

**Create Resource Requirements**

```bash
curl -X POST http://localhost:8080/torc-service/v1/resource_requirements \
  -H "Content-Type: application/json" \
  -d '{
    "workflow_id": 1,
    "name": "gpu_large",
    "num_cpus": 16,
    "num_gpus": 4,
    "num_nodes": 1,
    "memory": "128g",
    "runtime": "PT8H"
  }' | jq '.'
```

**Memory Format**: String with suffix: `1m` (MB), `2g` (GB), `512k` (KB)

**Runtime Format**: ISO 8601 duration: `PT30M` (30 minutes), `PT2H` (2 hours), `P1DT12H` (1.5 days)

### Compute Nodes

**Create Compute Node**

```bash
curl -X POST http://localhost:8080/torc-service/v1/compute_nodes \
  -H "Content-Type: application/json" \
  -d '{
    "workflow_id": 1,
    "hostname": "compute-01",
    "num_cpus": 32,
    "memory": "256g",
    "num_gpus": 8,
    "is_active": true
  }' | jq '.'
```

**List Active Compute Nodes**

```bash
curl "http://localhost:8080/torc-service/v1/compute_nodes?workflow_id=1&is_active=true" \
  | jq '.compute_nodes[] | {hostname, num_cpus, num_gpus}'
```

### Results

**Create Result**

```bash
curl -X POST http://localhost:8080/torc-service/v1/results \
  -H "Content-Type: application/json" \
  -d '{
    "workflow_id": 1,
    "job_id": 1,
    "exit_code": 0,
    "stdout": "Job completed successfully",
    "stderr": ""
  }' | jq '.'
```

### Events

**List Events**

```bash
curl "http://localhost:8080/torc-service/v1/events?workflow_id=1&limit=20" \
  | jq '.events[] | {timestamp, data}'
```

## Advanced Endpoints

**Prepare Next Jobs for Submission** (Job Runner)

```bash
curl -X POST "http://localhost:8080/torc-service/v1/workflows/1/claim_next_jobs?num_jobs=5" \
  -H "Content-Type: application/json" \
  -d '{}' | jq '.jobs'
```

**Process Changed Job Inputs** (Reinitialization)

```bash
curl -X POST "http://localhost:8080/torc-service/v1/workflows/1/process_changed_job_inputs?dry_run=true" \
  -H "Content-Type: application/json" \
  -d '{}' | jq '.reinitialized_jobs'
```
