var e=`# Simple Retry Example
# Demonstrates using match_all_exit_codes for simple retry-on-any-failure behavior.
#
# This is useful for jobs that are flaky due to transient issues and just need
# to be retried without any special recovery logic.
#
# To run this example:
#   torc run examples/yaml/simple_retry.yaml

name: simple_retry_example
description: Simple retry on any failure using match_all_exit_codes

failure_handlers:
  - name: simple_retry
    rules:
      # Retry any non-zero exit code up to 3 times
      - match_all_exit_codes: true
        max_retries: 3

jobs:
  # A flaky job that randomly fails
  - name: flaky_job
    command: bash examples/scripts/failure_demo_job.sh --fail-rate 0.6 --work-time 0.2
    failure_handler: simple_retry

  # Final job that runs after flaky_job succeeds
  - name: summary
    command: echo "Flaky job eventually succeeded!"
    depends_on:
      - flaky_job
`;export{e as default};