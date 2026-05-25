# Job State Transitions

Jobs progress through a defined lifecycle:

```mermaid
stateDiagram-v2
    [*] --> uninitialized
    uninitialized --> ready: initialize_jobs
    uninitialized --> blocked: has dependencies

    blocked --> ready: dependencies met
    ready --> pending: runner claims
    pending --> running: execution starts

    running --> completed: exit 0
    running --> failed: exit != 0 (handler match + max retries)
    running --> pending_failed: exit != 0 (no handler match)
    running --> ready: exit != 0 (failure handler retry)
    running --> canceled: user cancels
    running --> terminated: system terminates

    pending_failed --> failed: AI classifies as permanent
    pending_failed --> ready: AI classifies as transient
    pending_failed --> uninitialized: reset-status

    completed --> [*]
    failed --> [*]
    canceled --> [*]
    terminated --> [*]

    classDef waiting fill:#6c757d,color:#fff
    classDef ready fill:#17a2b8,color:#fff
    classDef active fill:#ffc107,color:#000
    classDef success fill:#28a745,color:#fff
    classDef error fill:#dc3545,color:#fff
    classDef stopped fill:#6f42c1,color:#fff
    classDef classification fill:#fd7e14,color:#fff

    class uninitialized,blocked waiting
    class ready ready
    class pending,running active
    class completed success
    class failed error
    class canceled,terminated stopped
    class pending_failed classification
```

## State Descriptions

- **uninitialized** (0) - Job created but dependencies not evaluated
- **blocked** (1) - Waiting for dependencies to complete
- **ready** (2) - All dependencies satisfied, ready for execution
- **pending** (3) - Job claimed by runner
- **running** (4) - Currently executing
- **completed** (5) - Finished successfully (exit code 0)
- **failed** (6) - Finished with error (exit code != 0)
- **canceled** (7) - Explicitly canceled by user or torc. Never executed.
- **terminated** (8) - Explicitly terminated by system, such as at wall-time timeout
- **pending_failed** (10) - Job failed without a matching failure handler. Awaiting AI-assisted
  classification to determine if the error is transient (retry) or permanent (fail). See
  [AI-Assisted Recovery](../../specialized/fault-tolerance/ai-assisted-recovery.md).
