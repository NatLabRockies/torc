# Torc Execution Benchmark Plan

## Purpose

Measure how effectively Torc keeps allocated resources supplied with work as job concurrency
increases and execution time decreases. The study should characterize throughput, dispatch delays,
and operating limits, not just the largest workflow that completes or the point at which a run
crashes.

Pair these measurements with an approved description of the production-cost modeling (PCM)
application as evidence of research use. Benchmarks supplement that application evidence rather
than replace it. The paper should not depend on ExaEpi details unless their disclosure is approved.

## Experiments

Separate three effects rather than vary every parameter at once:

1. **Queue size:** Increase the total number of jobs while holding worker capacity fixed. Measure
   workflow creation, initialization, and execution to determine how a larger stored workflow affects
   performance. Use enough jobs at each size to sustain a measurable steady-state interval.
2. **Execution scale:** Increase nodes and concurrent job slots while keeping the workload per slot
   approximately constant. Maintain a deep queue so lack of ready work does not limit throughput.
3. **Completion bursts:** Compare staggered completions with deliberately synchronized batches at
   the same concurrency. Measure whether simultaneous completion reports delay replacement claims.

Start with independent jobs to isolate orchestration overhead. A later dependency-driven workload
can test how completion bursts affect the release of downstream jobs if the initial results warrant
it.

## Workloads and Scale

Use five-second jobs as the primary short-job workload and approximately 60-second jobs as a
longer-duration baseline. Add one-second jobs near the observed scaling limit rather than running
every duration at every configuration.

A `sleep 5` workload isolates scheduling and reporting overhead, but does not demonstrate useful CPU
utilization. Label it as a synthetic orchestration workload. Include a small CPU-bound workload as a
complementary check, recording actual job durations rather than assuming they are exactly five
seconds.

Progress through approximately 1, 4, 16, 32, and 100 nodes. Treat 100 nodes as the final scale point,
not the starting point. Add intermediate points when throughput begins flattening, and investigate
unexpected failures before increasing scale. Bound each run by an agreed allocation size, job count,
and walltime.

Report concurrent job slots per node as well as node count. For example, 100 nodes with 32 concurrent
five-second jobs per node imply an ideal rate of 640 completions per second, before orchestration
overhead. This produces completion updates and replacement claims, not just 640 API requests per
second.

Jobs launched together will drift apart over successive rounds. Explicitly synchronize completion
batches for the burst experiment and document the mechanism and any synchronization overhead.

## Measurements

- **Throughput:** Completed jobs per second during steady state, plus end-to-end workflow duration.
- **Dispatch delay:** Time between a job finishing and replacement work starting while ready jobs
  remain queued. Report the median and high percentiles, such as p95 and p99.
- **Slot occupancy:** Aggregate job runtime divided by available job-slot time over the same
  measurement window. Do not label occupancy of sleeping jobs as CPU utilization.
- **Correctness:** Expected versus observed job completions, missing results, duplicate executions,
  stuck jobs, and unreconciled completion records. Distinguish configured retries from duplicates.
- **Server behavior:** CPU and memory use, database growth, API latency, lock contention, and request
  errors or timeouts.
- **Setup cost:** Workflow creation and initialization times, measured separately from execution.

Separate startup, steady-state execution, and final queue drain. Measure dispatch intervals within a
worker using a consistent clock where possible; document clock synchronization before comparing
timestamps across hosts. Record how each metric is obtained and keep instrumentation consistent
between runs.

## Controls and Repetition

Record the Torc version and commit, server hardware and placement, worker hardware, network context,
execution mode, workers and job slots per node, resource requests, polling settings, monitoring,
retry configuration, and database location. Record whether the server runs inside the allocation.

Use a fixed server configuration for the initial worker-scaling study. Compare on-disk SQLite with
in-memory SQLite at selected stress points rather than doubling the full experiment matrix. For
in-memory runs, record snapshot settings and include snapshot overhead in the reported results.
Distinguish server-local disk from shared-filesystem storage.

Repeat representative configurations at least three times and report variability alongside median
results. Preserve failed runs and their logs rather than reporting successful attempts alone.

## Interpreting Limits

Distinguish throughput saturation and increasing dispatch delays from correctness failures or lost
state. A throughput plateau can identify a practical operating limit without implying that Torc has
failed. Correlate the plateau with server, database, network, worker, and filesystem measurements
before attributing it to a particular component.

The synthetic workloads alone do not establish performance for GPU applications, mixed-resource
workflows, or other workflow engines. Those claims would require additional experiments.

## Paper and Reproducibility Outputs

Publish workflow specifications, workload scripts, launch commands, configurations, raw measurements,
and analysis code alongside the benchmark results. No measurements have been collected for this plan.

Use a compact figure showing throughput and dispatch delay against concurrent job slots, with node
counts annotated. Include queue-size setup costs and burst behavior in supporting results as needed.
The research impact statement should connect approved PCM use to the measured execution behavior,
state the tested configurations and limits, and avoid implying that synthetic throughput measures
application speedup.
