# Database Contention Stress Test

This test workflow is designed to stress test the SQLite database with high contention between:

1. **API handler threads** - Processing job completion requests from workers
2. **Background unblocking thread** - Processing completed jobs and unblocking dependent jobs

## Workflow Structure

- **5 stages** with **1000 jobs each** = **5000 total jobs**
- Barriers between each stage
- Simple `echo` commands that complete almost instantly
- ~6000 total dependencies

## Why This Creates Contention

When 1000 jobs complete almost simultaneously:

1. Each job completion triggers an API call to update job status
2. The background thread periodically processes completed jobs and unblocks dependents
3. Both operations need write locks on the SQLite database
4. With 1000 jobs finishing at once, there's significant lock contention

## Running the Test

### Local Execution

```bash
# Start the server
DATABASE_URL=sqlite:db/sqlite/dev.db cargo run --bin torc-server -- run

# In another terminal
export TORC_API_URL="http://localhost:8080/torc-service/v1"

# Create and run the workflow
torc create tests/workflows/database_contention_test/workflow.yaml
torc run <workflow_id>

# Monitor with TUI
torc tui
```

### With Multiple Workers

To maximize contention, run multiple workers:

```bash
# Terminal 1: Server with debug logging
RUST_LOG=debug DATABASE_URL=sqlite:db/sqlite/dev.db cargo run --bin torc-server -- run

# Terminal 2-5: Multiple workers (to increase parallel job completions)
torc run <workflow_id> &
torc run <workflow_id> &
torc run <workflow_id> &
torc run <workflow_id> &
```

## Expected Behavior

### Success Case

- All 5000 jobs complete successfully
- Transitions between stages happen smoothly
- No ERROR-level "database is locked" messages (only DEBUG level during retries)
- Total runtime depends on parallelism but should be under a few minutes

### Failure Case (if contention handling is broken)

- "database is locked" ERROR messages after 45 seconds of retries
- Jobs stuck in "blocked" state even when dependencies are complete
- Workflow hangs between stages

## Metrics to Monitor

1. **Server logs**: Look for "Database locked" messages at DEBUG level
2. **Retry counts**: How many retries are needed per unblocking operation
3. **Stage transition time**: Time from last stage job completing to barrier completing
4. **Total workflow time**: Should scale linearly with stages, not exponentially
