# Long-Poll Claim Design

This document explains Torc's long-poll claim path for short-job workflows and the related runner
changes that reduce end-of-workflow idle time.

## Problem Statement

Short jobs expose two latency problems in Torc's original claim loop:

1. A runner with free capacity can ask the server for work, receive an empty response, and then
   sleep locally until the next poll interval even if another job becomes ready immediately after
   the claim returns.
2. Near the end of a workflow, this "missed wakeup" behavior can leave CPUs idle for a full poll
   interval between dependent jobs.

This is especially visible in HPC workflows with:

- large numbers of short jobs
- fan-out or pipeline stages that repeatedly unblock downstream work
- many compute nodes polling the same workflow

The goal is to improve throughput without turning claim traffic into a tight empty-poll loop.

## Design Goals

- Reduce idle time between short dependent jobs.
- Avoid repeated empty claim requests when no work is ready.
- Preserve atomic server-side claim behavior.
- Keep the generated OpenAPI client unchanged.
- Limit behavioral changes to the job runner paths that actually use long-poll claims.

## Solution Overview

The solution has three parts:

1. The runner can issue claim requests with `wait_seconds`.
2. The server treats an empty claim as a workflow-scoped long-poll wait instead of forcing the
   runner to sleep locally.
3. Runner HTTP clients are rebuilt with a longer blocking request timeout so long-poll claims are
   not cut off by reqwest's default timeout.

At a high level:

1. Runner sends `claim_jobs_based_on_resources(..., wait_seconds=N)` or
   `claim_next_jobs(..., wait_seconds=N)`.
2. Server tries the normal atomic claim path immediately.
3. If no jobs are claimable and `wait_seconds > 0`, the server waits for workflow readiness.
4. When jobs transition to `Ready`, waiting claim requests wake and retry the claim path.
5. If no work appears before the timeout, the server returns the original empty response.

This changes claim behavior from "poll, sleep, poll again" to "ask once, then wait for readiness or
timeout."

## Server Design

The server supports long-poll for both claim endpoints:

- `claim_jobs_based_on_resources`
- `claim_next_jobs`

The transport layer clamps `wait_seconds` to a bounded range before using it. The current cap is 60
seconds.

The long-poll wait is keyed by `workflow_id`. A request that receives an empty claim result can
register as a waiter for that workflow. The waiter is woken when the server observes a workflow
state change that may make new jobs claimable, such as:

- job initialization making jobs ready
- dependency-unblock processing after job completion
- workflow cancellation

After wakeup, the request retries the normal claim path. Claim ownership is still decided by the
existing atomic server-side claim logic, so long-poll does not weaken correctness under contention.

## Runner Design

The runner computes `wait_seconds` from `job_completion_poll_interval` and attaches it to claim
requests when long-poll is allowed for that execution path.

This is used in:

- single-node resource-based claims
- `max_parallel_jobs` queue-based claims

The multi-node resource-based path does not currently long-poll once per node inside the same loop
iteration. That would serialize waits across nodes and weaken throughput for that mode.

The runner also uses SIGCHLD-triggered wakeups on Unix so local job completions can wake the main
loop before the next sleep timeout. That improves slot refill latency for jobs already running on
the current node. Long-poll solves a different problem: waiting for downstream jobs to become ready
on the server.

## HTTP Client Timeout Design

Long-poll introduces an HTTP client constraint:

- server wait cap: 60 seconds
- reqwest blocking client default timeout: 30 seconds

Without a client override, a long-poll claim can time out just before the server returns. This was
observed while trying to claim the last job in a workflow, where the runner often spends the full
wait window on an otherwise idle request.

### Why not patch generated client code?

The OpenAPI-generated client is the wrong layer for this change:

- generated files are overwritten during regeneration
- the longer timeout is only needed for runner long-poll requests
- changing generated defaults affects every Torc API consumer, including paths that do not use
  long-poll

Instead, Torc rebuilds the blocking HTTP client in handwritten runner setup code with:

- `timeout = 120s`
- `tcp_keepalive = 10s`
- existing TLS settings
- existing cookie header, if configured

This keeps long-poll behavior scoped to:

- `torc run`
- `torc-slurm-job-runner`

Other API consumers continue using the normal generated client defaults.

## Keepalive Rationale

Some HPC deployments place an HTTPS route or reverse proxy in front of `torc-server` even though the
application itself only binds HTTP. Long-poll requests can otherwise sit idle long enough to
interact badly with intermediary timeout behavior.

TCP keepalive does not guarantee proxy compatibility, but it is a low-risk way to reduce the chance
of silently dead connections while a runner is waiting for work.

## Alternatives Considered

### Shorten the local poll interval

Rejected as the primary fix.

It reduces latency, but it increases empty claim traffic and still leaves a full poll-window race: a
job can become ready immediately after the runner receives an empty claim.

### Lower `wait_seconds` to stay under the default 30 second client timeout

Rejected as a complete solution.

This avoids the timeout race, but it gives up much of the long-poll benefit and increases the rate
of empty claim requests under idle periods.

### Patch the generated OpenAPI client defaults

Rejected.

This fixes the symptom, but it makes a runtime policy decision in generated code and broadens the
change to unrelated clients.

### Server push or streaming

Not pursued.

Streaming or a dedicated push channel would be more complex operationally and would require a larger
protocol change than bounded request/response long-polling.

## Operational Considerations

- If an HPC environment uses OpenShift routes, ingress controllers, or other HTTPS intermediaries,
  their timeout behavior must tolerate the chosen `wait_seconds`.
- The server should have metrics for active long-poll waiters and wakeups so burst behavior is
  visible.
- Long-poll reduces repeated empty polls, but wakeups can still create bursts of productive claim
  contention when many runners wait on the same workflow.

## Summary

Torc uses bounded long-poll claim requests to improve short-job throughput without busy polling. The
server keeps claim ownership atomic, the runner wakes promptly on local completion, and the runner's
blocking HTTP client is tuned in handwritten code so long-poll requests survive long enough to be
useful.
