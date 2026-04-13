# Database Index Analysis and Recommendations

## Overview

This document analyzes the Torc database query patterns and recommends indexes to improve
performance, particularly for workflows with thousands of jobs.

## Query Pattern Analysis

### Most Common Query Patterns

1. **Filtering by workflow_id** - Used in virtually every list operation:
   - `SELECT * FROM job WHERE workflow_id = ?`
   - `SELECT * FROM result WHERE workflow_id = ?`
   - `SELECT * FROM event WHERE workflow_id = ?`
   - `SELECT * FROM compute_node WHERE workflow_id = ?`

2. **Filtering by workflow_id and status** - Common for finding jobs in specific states:
   - `SELECT * FROM job WHERE workflow_id = ? AND status = ?`
   - Used in workflow initialization, status checks, and job runner queries

3. **Ready job queries** - Critical for job allocation:
   ```sql
   SELECT ... FROM job
   JOIN resource_requirements rr ON job.resource_requirements_id = rr.id
   WHERE job.workflow_id = ? AND job.status = ?
   ORDER BY rr.num_gpus DESC, rr.runtime_s DESC, rr.memory_bytes DESC
   ```

4. **Dependency lookups** - Used for finding blocked/blocking jobs:
   - `SELECT job_id FROM job_depends_on WHERE depends_on_job_id = ?`
   - `SELECT depends_on_job_id FROM job_depends_on WHERE job_id = ?`

5. **File relationship lookups** - Used for dependency resolution:
   - `SELECT job_id FROM job_input_file WHERE file_id = ?`
   - `SELECT job_id FROM job_output_file WHERE file_id = ?`

6. **Result queries** - Filtered by workflow, job, and run:
   - `SELECT * FROM result WHERE workflow_id = ? AND job_id = ?`
   - `SELECT * FROM result WHERE workflow_id = ? AND run_id = ?`

## Implemented Indexes

### Deferred Unblocking Indexes (2024-11)

**Migration**: `20251123000000_add_unblocking_processed`

#### `job(workflow_id, status, unblocking_processed)` WHERE unblocking_processed = 0

**Impact**: Critical for background unblocking task **Rationale**: Enables efficient finding of
completed jobs that need unblocking processing. **Queries affected**:

- Background task finding workflows with pending completions
- Batch processing of job completions

#### `job(workflow_id)` WHERE status IN (6,7,8) AND unblocking_processed = 0

**Impact**: High for background unblocking task **Rationale**: Allows quick filtering to active
workflows needing unblock processing. **Queries affected**:

- Finding workflows with unprocessed completions

## Recommended Indexes

### Priority 1: Critical Performance Impact

These indexes address the most common and expensive queries:

#### 1. `job(workflow_id)`

**Impact**: Very High **Rationale**: Every job list operation filters by workflow_id. Without this
index, a table scan is required for each query. **Queries affected**:

- `list_jobs` API endpoint
- Job initialization queries
- Workflow status checks
- Job dependency resolution

#### 2. `job(workflow_id, status)`

**Impact**: Very High **Rationale**: Composite index for the extremely common pattern of finding
jobs in a specific status for a workflow. **Queries affected**:

- Finding ready/running/completed jobs
- Workflow initialization (finding uninitialized jobs)
- Status transition queries
- `is_workflow_uninitialized` endpoint

#### 3. `result(workflow_id)`

**Impact**: High **Rationale**: Result listing is frequently used and can involve thousands of rows
per workflow. **Queries affected**:

- `list_results` API endpoint
- Workflow completion checks
- Result aggregation queries

#### 4. `event(workflow_id)`

**Impact**: Medium-High **Rationale**: Event listing filtered by workflow is common, especially for
debugging and monitoring. **Queries affected**:

- `list_events` API endpoint
- Event timeline queries

#### 5. `compute_node(workflow_id)`

**Impact**: Medium-High **Rationale**: Tracking active compute nodes per workflow. **Queries
affected**:

- `list_compute_nodes` API endpoint
- Active node checks
- Resource availability queries

### Priority 2: Dependency and Relationship Lookups

These indexes support reverse lookups for dependencies and file relationships:

#### 6. `job_depends_on(depends_on_job_id)`

**Impact**: High **Rationale**: Enables efficient lookup of jobs that depend on a specific job
(reverse dependency lookup). **Queries affected**:

- Finding downstream jobs when a job completes
- Dependency graph traversal
- Impact analysis for job failures

Note: `job_depends_on(job_id)` is already indexed via PRIMARY KEY (job_id, depends_on_job_id)

#### 7. `job_input_file(file_id)`

**Impact**: Medium **Rationale**: Enables finding which jobs consume a specific file (reverse
lookup). **Queries affected**:

- File dependency resolution
- `list_jobs` with `needs_file_id` parameter
- Change detection queries

Note: `job_input_file(job_id)` is already indexed via PRIMARY KEY (job_id, file_id)

#### 8. `job_output_file(file_id)`

**Impact**: Medium **Rationale**: Enables finding which jobs produce a specific file (reverse
lookup). **Queries affected**:

- File producer lookups
- Output file tracking

Note: `job_output_file(job_id)` is already indexed via PRIMARY KEY (job_id, file_id)

#### 9. `job_input_user_data(user_data_id)`

**Impact**: Low-Medium **Rationale**: Similar to file lookups but for user_data dependencies.

#### 10. `job_output_user_data(user_data_id)`

**Impact**: Low-Medium **Rationale**: Similar to file lookups but for user_data outputs.

### Priority 3: Resource-Based Job Allocation

These indexes optimize the resource-based job allocation query:

#### 11. `resource_requirements(num_gpus, runtime_s, memory_bytes)`

**Impact**: Medium **Rationale**: Supports filtering candidate jobs for
`claim_jobs_based_on_resources` by resource requirements before job priority is applied. **Queries
affected**:

- `claim_jobs_based_on_resources`
- Resource-based job scheduling

### Priority 4: User and Workflow Queries

#### 12. `workflow(user)`

**Impact**: Low-Medium **Rationale**: Enables efficient filtering of workflows by user. **Queries
affected**:

- `list_workflows` with user filter
- User-specific workflow queries

#### 13. `workflow(user, is_archived)`

**Impact**: Low **Rationale**: Common pattern for listing active workflows for a user. **Queries
affected**:

- `list_workflows` excluding archived workflows

## Existing Indexes

The following columns are already indexed via primary keys or unique constraints:

- All `id` columns (PRIMARY KEY)
- `result(job_id, run_id)` - UNIQUE constraint provides index
- `job_depends_on(job_id, depends_on_job_id)` - PRIMARY KEY provides index on job_id prefix
- `job_input_file(job_id, file_id)` - PRIMARY KEY provides index on job_id prefix
- `job_output_file(job_id, file_id)` - PRIMARY KEY provides index on job_id prefix
- Similar patterns for `job_input_user_data` and `job_output_user_data`

## Index Size Estimates

For a workflow with 10,000 jobs:

- `job(workflow_id)`: ~40-80 KB (4-8 bytes per entry)
- `job(workflow_id, status)`: ~60-120 KB (6-12 bytes per entry)
- `result(workflow_id)`: ~40-80 KB per run
- `job_depends_on(depends_on_job_id)`: Varies based on dependency graph density
- `resource_requirements(num_gpus, runtime_s, memory_bytes)`: ~100-200 KB for unique resource specs

Total estimated overhead: ~500 KB - 1 MB per 10,000-job workflow (negligible)

## Performance Impact Estimates

Based on common workload patterns:

### Before Indexes

- List 10,000 jobs for a workflow: ~100-500ms (table scan)
- Find ready jobs (status filter): ~100-500ms (table scan)
- List results for workflow: ~50-200ms (table scan)
- Reverse dependency lookup: ~50-200ms (table scan)

### After Indexes

- List 10,000 jobs for a workflow: ~5-20ms (index scan)
- Find ready jobs (status filter): ~2-10ms (composite index)
- List results for workflow: ~5-15ms (index scan)
- Reverse dependency lookup: ~1-5ms (index seek)

**Expected improvement**: 10-50x faster for common queries

## Implementation Strategy

### Phase 1: Critical Indexes (Immediate)

1. `job(workflow_id)`
2. `job(workflow_id, status)`
3. `result(workflow_id)`

These three indexes address the most common bottlenecks with minimal overhead.

### Phase 2: Relationship Indexes (Short-term)

4. `event(workflow_id)`
5. `compute_node(workflow_id)`
6. `job_depends_on(depends_on_job_id)`
7. `job_input_file(file_id)`
8. `job_output_file(file_id)`

These improve dependency resolution and reverse lookups.

### Phase 3: Optimization Indexes (Medium-term)

9. `resource_requirements(num_gpus, runtime_s, memory_bytes)`
10. `workflow(user)`
11. Additional user_data indexes if usage patterns warrant

### Phase 4: Monitor and Refine

- Use SQLite's `EXPLAIN QUERY PLAN` to verify index usage
- Monitor query performance with logging
- Add additional indexes based on actual usage patterns
- Consider removing unused indexes

## SQLite-Specific Considerations

### Index Selection

SQLite's query planner is quite good at choosing indexes, but:

- Only one index per table is used in most queries
- Composite indexes can satisfy prefix queries: `(workflow_id, status)` can be used for
  `WHERE workflow_id = ?`
- Covering indexes are beneficial but rare in our schema

### Write Performance

- Each index adds overhead to INSERT, UPDATE, and DELETE operations
- For Torc, this is acceptable because:
  - Job creation is done in batches at workflow creation time
  - Most operations are reads (job runner polling, status queries)
  - The write amplification is minimal (~10-15% overhead estimated)

### WAL Mode Benefits

- Since Torc uses SQLite in WAL mode, readers don't block writers
- Indexes improve read performance without significantly affecting write concurrency

## Maintenance

### Monitoring

- Enable `PRAGMA optimize` in periodic maintenance
- SQLite auto-analyzes tables after significant changes
- Use `ANALYZE` command manually if query plans seem suboptimal

### Vacuum

- Periodic `VACUUM` to reclaim space and rebuild indexes
- Not critical for active databases but useful during maintenance windows

## Alternative Approaches

If index overhead becomes a concern (unlikely):

1. **Partitioning by workflow_id**: Not supported natively in SQLite
2. **Separate databases per workflow**: Would lose referential integrity and complicate API
3. **Materialized views**: Could cache computed results, but SQLite doesn't support natively
4. **Application-level caching**: Could reduce database load but adds complexity

## Conclusion

The recommended indexes provide substantial performance improvements with minimal storage and write
overhead. The critical indexes (Phase 1) should be implemented immediately, as they address the most
common bottlenecks in workflows with thousands of jobs.

The total storage overhead is estimated at less than 1 MB per 10,000-job workflow, while query
performance can improve by 10-50x for common operations.
