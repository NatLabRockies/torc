# Environment Variables

When Torc executes jobs, it automatically sets several environment variables that provide context
about the job and enable communication with the Torc server. These variables are available to all
job commands during execution.

## Variables Set During Job Execution

### TORC_WORKFLOW_ID

The unique identifier of the workflow that contains this job.

- **Type**: Integer (provided as string)
- **Example**: `"42"`
- **Use case**: Jobs can use this to query workflow information or to organize output files by
  workflow

```bash
# Example: Create a workflow-specific output directory
mkdir -p "/data/results/workflow_${TORC_WORKFLOW_ID}"
echo "Processing data..." > "/data/results/workflow_${TORC_WORKFLOW_ID}/output.txt"
```

### TORC_JOB_ID

The unique identifier of the currently executing job.

- **Type**: Integer (provided as string)
- **Example**: `"123"`
- **Use case**: Jobs can use this for logging, creating job-specific output files, or querying job
  metadata

```bash
# Example: Log job-specific information
echo "Job ${TORC_JOB_ID} started at $(date)" >> "/var/log/torc/job_${TORC_JOB_ID}.log"
```

### TORC_API_URL

The URL of the Torc API server that the job runner is communicating with.

- **Type**: String (URL)
- **Example**: `"http://localhost:8080/torc-service/v1"`
- **Use case**: Jobs can make API calls to the Torc server to query data, create files, update user
  data, or perform other operations

```bash
# Example: Query workflow information from within a job
curl -s "${TORC_API_URL}/workflows/${TORC_WORKFLOW_ID}" | jq '.name'

# Example: Create a file entry in Torc
curl -X POST "${TORC_API_URL}/files" \
  -H "Content-Type: application/json" \
  -d "{
    \"workflow_id\": ${TORC_WORKFLOW_ID},
    \"name\": \"result_${TORC_JOB_ID}\",
    \"path\": \"/data/results/output.txt\"
  }"
```

### TORC_JOB_NAME

The name of the currently executing job as defined in the workflow specification.

- **Type**: String
- **Example**: `"train_model"`
- **Use case**: Jobs can use this for logging or creating human-readable output file names

```bash
# Example: Log with job name
echo "[${TORC_JOB_NAME}] Processing started at $(date)"
```

### TORC_OUTPUT_DIR

The output directory where job logs and artifacts are stored.

- **Type**: String (path)
- **Example**: `"/path/to/output"`
- **Use case**: Jobs can write additional output files to this directory alongside the standard
  stdout/stderr logs

```bash
# Example: Write job artifacts to output directory
cp results.json "${TORC_OUTPUT_DIR}/job_${TORC_JOB_ID}_results.json"
```

### TORC_ATTEMPT_ID

The current attempt number for this job execution. Starts at 1 and increments with each retry when
using failure handlers.

- **Type**: Integer (provided as string)
- **Example**: `"1"` (first attempt), `"2"` (first retry), etc.
- **Use case**: Jobs can adjust behavior based on retry attempt, or include attempt information in
  logs

```bash
# Example: Log attempt information
echo "Running attempt ${TORC_ATTEMPT_ID} of job ${TORC_JOB_NAME}"

# Example: Adjust behavior on retry
if [ "${TORC_ATTEMPT_ID}" -gt 1 ]; then
  echo "This is a retry - using more conservative settings"
  BATCH_SIZE=16
else
  BATCH_SIZE=64
fi
```

## Variables Set During Recovery Script Execution

When a job fails and has a failure handler configured, Torc may run a recovery script before
retrying the job. Recovery scripts receive all the standard job environment variables plus
additional context about the failure.

### TORC_RETURN_CODE

The exit code from the failed job that triggered the recovery script. Only available in recovery
scripts, not during normal job execution.

- **Type**: Integer (provided as string)
- **Example**: `"137"` (OOM killed), `"1"` (general error)
- **Use case**: Recovery scripts can inspect the exit code to determine appropriate recovery actions

```bash
# Example: Recovery script that handles different exit codes
#!/bin/bash
echo "Job ${TORC_JOB_NAME} failed with exit code ${TORC_RETURN_CODE}"

case ${TORC_RETURN_CODE} in
  137)
    echo "Out of memory - reducing batch size for retry"
    # Modify config for next attempt
    ;;
  139)
    echo "Segmentation fault - checking for corrupted data"
    # Clean up corrupted files
    ;;
  *)
    echo "Unknown error - attempting general recovery"
    ;;
esac

exit 0  # Exit 0 to proceed with retry, non-zero to abort
```

## Complete Example

Here's a complete example of a job that uses the environment variables:

```yaml
name: "Environment Variables Demo"
user: "demo"

jobs:
  - name: "example_job"
    command: |
      #!/bin/bash
      set -e

      echo "=== Job Environment ==="
      echo "Workflow ID: ${TORC_WORKFLOW_ID}"
      echo "Job ID: ${TORC_JOB_ID}"
      echo "Job Name: ${TORC_JOB_NAME}"
      echo "Attempt: ${TORC_ATTEMPT_ID}"
      echo "Output Dir: ${TORC_OUTPUT_DIR}"
      echo "API URL: ${TORC_API_URL}"

      # Create job-specific output directory
      OUTPUT_DIR="/tmp/workflow_${TORC_WORKFLOW_ID}/job_${TORC_JOB_ID}"
      mkdir -p "${OUTPUT_DIR}"

      # Do some work
      echo "Processing data..." > "${OUTPUT_DIR}/status.txt"
      date >> "${OUTPUT_DIR}/status.txt"
      echo "Job completed successfully!"
```

## Summary Table

| Variable           | Type    | Available In           | Description                         |
| ------------------ | ------- | ---------------------- | ----------------------------------- |
| `TORC_WORKFLOW_ID` | Integer | Jobs, Recovery Scripts | Workflow identifier                 |
| `TORC_JOB_ID`      | Integer | Jobs, Recovery Scripts | Job identifier                      |
| `TORC_JOB_NAME`    | String  | Jobs, Recovery Scripts | Job name from workflow spec         |
| `TORC_API_URL`     | URL     | Jobs, Recovery Scripts | Torc server API endpoint            |
| `TORC_OUTPUT_DIR`  | Path    | Jobs, Recovery Scripts | Output directory for logs/artifacts |
| `TORC_ATTEMPT_ID`  | Integer | Jobs, Recovery Scripts | Current attempt number (1, 2, 3...) |
| `TORC_RETURN_CODE` | Integer | Recovery Scripts only  | Exit code that triggered recovery   |

## Notes

- All environment variables are set as strings, even numeric values like workflow and job IDs
- The `TORC_API_URL` includes the full base path to the API (e.g., `/torc-service/v1`)
- Jobs inherit all other environment variables from the job runner process
- These variables are available in both local and Slurm-scheduled job executions
- `TORC_ATTEMPT_ID` starts at 1 for the first execution and increments with each retry
- `TORC_RETURN_CODE` is only available in recovery scripts, not during normal job execution

## Client Configuration Variables

### TORC_USERNAME

Override the username for Torc API operations (workflow ownership, authentication).

- **Type**: String
- **Default**: `$USER` (Unix) or `$USERNAME` (Windows)
- **Use case**: Service accounts or when system username differs from Torc identity

**Note**: Does not affect Slurm job submission, which always uses the real system user.

### TORC_API_URL

The URL of the Torc API server for CLI operations.

- **Type**: String (URL)
- **Default**: `http://localhost:8080/torc-service/v1`
- **Use case**: Connect to a remote Torc server or custom port

```bash
export TORC_API_URL=http://myserver:8080/torc-service/v1
torc workflows list
```

**Note**: Can also be set via the `--url` CLI flag.

### TORC_PASSWORD

Password for HTTP Basic authentication with the Torc server.

- **Type**: String
- **Default**: None (no authentication)
- **Use case**: Authenticate with a password-protected Torc server

```bash
export TORC_PASSWORD=mysecretpassword
torc workflows list
```

**Note**: Username comes from `TORC_USERNAME` or system username. See
[Authentication](../../specialized/admin/authentication.md) for server setup.

### TORC_TLS_CA_CERT

Path to a custom CA certificate for TLS verification.

- **Type**: String (file path)
- **Default**: System CA certificates
- **Use case**: Connect to servers using self-signed or internal CA certificates

```bash
export TORC_TLS_CA_CERT=/path/to/ca-cert.pem
torc workflows list
```

### TORC_TLS_INSECURE

Skip TLS certificate verification (use with caution).

- **Type**: Boolean (`true` or `1` to enable)
- **Default**: `false` (TLS verification enabled)
- **Use case**: Development/testing with self-signed certificates

```bash
export TORC_TLS_INSECURE=true
torc workflows list
```

**Warning**: Disabling TLS verification exposes connections to man-in-the-middle attacks. Only use
in trusted development environments.

### TORC_COOKIE_HEADER

Cookie header value for cookie-based authentication.

- **Type**: String
- **Default**: None
- **Use case**: Authenticate via session cookies (e.g., when behind an authentication proxy)

```bash
export TORC_COOKIE_HEADER="session=abc123; token=xyz789"
torc workflows list
```

### RUST_LOG

Control log verbosity for the Torc CLI.

- **Type**: String (log level)
- **Default**: `warn`
- **Values**: `error`, `warn`, `info`, `debug`, `trace`
- **Use case**: Debug CLI issues or see detailed API interactions

```bash
# Show info-level logs
export RUST_LOG=info
torc workflows list

# Show debug logs for torc modules only
export RUST_LOG=torc=debug
torc run workflow.yaml

# Show SQL queries (server debugging)
RUST_LOG=sqlx=debug torc-server run
```
