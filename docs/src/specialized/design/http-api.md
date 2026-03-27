# HTTP API Design

This document describes the design principles and conventions of Torc's HTTP API.

## Design Philosophy

The API follows REST conventions where appropriate, with pragmatic deviations for workflow
orchestration operations that don't map cleanly to CRUD semantics.

**Core principles:**

- **Resource-oriented**: Primary entities (workflows, jobs, files) have standard CRUD endpoints
- **Predictable URLs**: Consistent naming and structure across all resources
- **JSON everywhere**: All request and response bodies use `application/json`
- **Explicit over implicit**: Required fields are marked required; optional fields have sensible
  defaults

## Base URL and Versioning

The API is served under a versioned base path:

```
/torc-service/v1
```

**Versioning strategy:**

- The version in the URL path (`v1`) represents the major API version
- The detailed version (e.g., `0.12.0`) is in the OpenAPI spec and server responses
- Breaking changes increment the major version; non-breaking changes increment minor/patch
- The version is single-sourced in `src/api_version.rs` and propagates to all artifacts

## URL Structure

### Resource Collections

```
GET    /resources              # List all (with pagination)
POST   /resources              # Create new
```

### Individual Resources

```
GET    /resources/{id}         # Get by ID
PUT    /resources/{id}         # Update (full replacement)
PATCH  /resources/{id}         # Partial update (where supported)
DELETE /resources/{id}         # Delete
```

### Nested Resources

Resources that belong to a parent use nested URLs:

```
GET    /workflows/{id}/jobs              # Jobs in workflow
GET    /workflows/{id}/files             # Files in workflow
GET    /access_groups/{id}/members       # Members in group
```

### Action Endpoints (RPC-Style)

Operations that don't map to CRUD use verb-based paths under the resource:

```
POST   /workflows/{id}/initialize_jobs           # Build dependency graph
POST   /workflows/{id}/claim_next_jobs           # Atomically claim ready jobs
POST   /workflows/{id}/cancel                    # Cancel workflow execution
POST   /workflows/{id}/reset_status              # Reset workflow state
POST   /workflows/{id}/process_changed_job_inputs # Detect and handle input changes
POST   /jobs/{id}/complete                       # Mark job completed
POST   /jobs/{id}/manage_status_change           # Transition job status
```

**When to use action endpoints:**

- Operations with side effects beyond simple CRUD
- Operations requiring atomicity (like `claim_next_jobs`)
- State machine transitions
- Batch operations

## HTTP Methods

| Method | Semantics                         | Idempotent | Request Body |
| ------ | --------------------------------- | ---------- | ------------ |
| GET    | Read resource(s)                  | Yes        | No           |
| POST   | Create resource or trigger action | No         | Yes          |
| PUT    | Replace resource entirely         | Yes        | Yes          |
| PATCH  | Partial update                    | No         | Yes          |
| DELETE | Remove resource                   | Yes        | No           |

**Notes:**

- `PUT` expects the complete resource representation
- `PATCH` accepts partial updates (only fields to change)
- `DELETE` on non-existent resources returns 404 (not 204)

## Request Format

All request bodies use JSON with `Content-Type: application/json`.

### Creating Resources

```json
POST /workflows
{
  "name": "my-workflow",
  "user": "dthom",
  "description": "Example workflow"
}
```

### Bulk Operations

Some endpoints accept arrays for batch creation:

```json
POST /bulk_jobs
{
  "jobs": [
    {"name": "job1", "workflow_id": 1, "command": "echo hello"},
    {"name": "job2", "workflow_id": 1, "command": "echo world"}
  ]
}
```

## Response Format

### Success Responses

Single resource:

```json
{
  "id": 1,
  "name": "my-workflow",
  "user": "dthom",
  "status": "ready"
}
```

List response (with pagination metadata):

```json
{
  "items": [...],
  "offset": 0,
  "count": 10,
  "total_count": 42,
  "max_limit": 10000,
  "has_more": true
}
```

### Error Responses

All errors use the `ErrorResponse` schema:

```json
{
  "error": {
    "error": "NotFound",
    "message": "Workflow 999 not found"
  }
}
```

Or with additional context:

```json
{
  "error": {
    "error": "ValidationError",
    "message": "Invalid job status transition"
  },
  "errorMessage": "Cannot transition from 'completed' to 'ready'",
  "code": 422
}
```

## HTTP Status Codes

| Code | Meaning               | When Used                                                      |
| ---- | --------------------- | -------------------------------------------------------------- |
| 200  | OK                    | Successful GET, PUT, PATCH, DELETE, or POST action             |
| 201  | Created               | Resource created (some POST endpoints)                         |
| 400  | Bad Request           | Malformed JSON, missing required fields                        |
| 403  | Forbidden             | User lacks permission for this resource                        |
| 404  | Not Found             | Resource doesn't exist                                         |
| 422  | Unprocessable Entity  | Valid JSON but invalid semantics (e.g., bad status transition) |
| 500  | Internal Server Error | Unexpected server failure                                      |

## Pagination

All list endpoints support offset-based pagination:

| Parameter | Type    | Default | Description               |
| --------- | ------- | ------- | ------------------------- |
| `offset`  | integer | 0       | Number of records to skip |
| `limit`   | integer | 10000   | Maximum records to return |

**Constraints:**

- Maximum `limit`: 10,000 records (enforced server-side)
- Response includes `has_more` boolean for client-side iteration
- Response includes `total_count` for progress indication

**Example:**

```
GET /workflows?offset=0&limit=50
GET /workflows?offset=50&limit=50  # Next page
```

## Filtering and Sorting

### Filtering

List endpoints support query parameters for filtering:

```
GET /workflows?user=dthom&is_archived=false
GET /jobs?workflow_id=1&status=ready
GET /compute_nodes?workflow_id=1&is_active=true
```

Common filter parameters:

- `workflow_id`: Filter by parent workflow (required for nested resources)
- `name`: Filter by name (often substring match)
- `user`: Filter by owner
- `status`: Filter by status value

### Sorting

```
GET /workflows?sort_by=created_at&reverse_sort=true
GET /jobs?sort_by=name&reverse_sort=false
```

| Parameter      | Type    | Description              |
| -------------- | ------- | ------------------------ |
| `sort_by`      | string  | Field name to sort by    |
| `reverse_sort` | boolean | If true, sort descending |

## Authentication

The server supports multiple authentication modes:

### HTTP Basic Auth

```
Authorization: Basic base64(username:password)
```

Credentials are validated against an htpasswd file when `--htpasswd-file` is specified.

### Anonymous Access

When authentication is not enforced (`--no-auth` or no htpasswd file), requests are accepted with
the username derived from the `X-Remote-User` header or defaulting to "anonymous".

### Authorization Model

Access control is resource-based:

1. **Workflow ownership**: Users can access workflows they own
2. **Group membership**: Users can access workflows shared with their groups
3. **System administrators**: Full access to all resources

The `enforce_access_control` server flag controls whether authorization is checked.

## Resource Organization

The API is organized into logical resource groups (OpenAPI tags):

| Tag                 | Resources                        | Description                   |
| ------------------- | -------------------------------- | ----------------------------- |
| `workflows`         | Workflows, workflow operations   | Core workflow management      |
| `jobs`              | Jobs, job status, job operations | Job execution and tracking    |
| `files`             | File records                     | Input/output file tracking    |
| `user_data`         | User data records                | Key-value data dependencies   |
| `events`            | Workflow events                  | Audit log and event stream    |
| `compute_nodes`     | Compute node records             | Worker node tracking          |
| `slurm_schedulers`  | Slurm scheduler configs          | Slurm integration             |
| `remote_workers`    | Remote worker registrations      | Distributed execution         |
| `access_control`    | Groups, memberships, permissions | Authorization management      |
| `workflow_actions`  | Scheduled actions                | Automated workflow operations |
| `failure_handlers`  | Failure handler configs          | Error handling rules          |
| `ro_crate_entities` | RO-Crate metadata                | Research object packaging     |
| `system`            | Health, version                  | Server status                 |

## Thread Safety and Concurrency

Certain endpoints are designed for concurrent access from multiple workers:

### `claim_next_jobs`

```
POST /workflows/{id}/claim_next_jobs?limit=5
```

This endpoint uses database-level write locks (`BEGIN IMMEDIATE TRANSACTION`) to ensure that
multiple workers calling simultaneously will not receive the same jobs. Each job is allocated to
exactly one worker.

### `claim_jobs_based_on_resources`

Similar to `claim_next_jobs` but factors in resource requirements (CPU, memory, GPU) and available
capacity on the requesting worker.

## Content Types

| Content-Type        | Usage                                            |
| ------------------- | ------------------------------------------------ |
| `application/json`  | All request and response bodies                  |
| `text/event-stream` | Server-Sent Events (dashboard real-time updates) |

## Data Type Conventions

### IDs

All resource IDs are 64-bit integers (`int64` in OpenAPI).

### Timestamps

Timestamps use Unix epoch format as `float64` (seconds with fractional milliseconds).

### Durations

Runtime durations use ISO 8601 format: `PT30M` (30 minutes), `PT2H` (2 hours).

### Memory Sizes

Memory specifications use string format with units: `"512m"`, `"2g"`, `"100k"`.

### Job Status

Job status is stored and transmitted as integers (0-10):

| Value | Status         |
| ----- | -------------- |
| 0     | uninitialized  |
| 1     | blocked        |
| 2     | ready          |
| 3     | pending        |
| 4     | running        |
| 5     | completed      |
| 6     | failed         |
| 7     | canceled       |
| 8     | terminated     |
| 9     | disabled       |
| 10    | pending_failed |

## API Evolution

When evolving the API:

1. **Additive changes** (new fields, new endpoints) don't require version bumps
2. **Breaking changes** (removed fields, changed semantics) require major version increment
3. **Deprecation** should be communicated via documentation before removal
4. The OpenAPI spec is the authoritative contract; regenerate clients after spec changes

See [API Generation Architecture](./api-generation.md) for the code-first workflow that maintains
the API contract.
