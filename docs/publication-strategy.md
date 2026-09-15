# Torc Publication Strategy

## Recommendation

Use a two-paper publication strategy:

1. Publish a software paper that establishes Torc as a citable, reusable research software
   product.
2. Publish a systems paper that makes and evaluates a narrower technical claim about HPC workflow
   execution.

This division reflects the current state of the repository. Torc is already a substantial,
well-documented software artifact, but the repository does not yet contain the quantitative evidence
needed for a competitive workflow-systems journal article.

The two papers should have clearly distinct contributions:

- **Paper 1:** What Torc is, why researchers need it, how it is designed, and how it is used.
- **Paper 2:** What Torc teaches us about allocation-resident scheduling, adaptive recovery, dynamic
  workflows, and compact durable control planes.

## Current Assessment

Torc is more than a basic directed acyclic graph runner. Its architecture combines:

- An Axum and Tokio REST service with SQLite as authoritative workflow state.
- Local, SSH, and Slurm execution through workers that claim ready jobs.
- CPU, memory, GPU, node-count, and runtime-aware claims.
- Explicit job dependencies and implicit dependencies inferred from files and JSON user data.
- Dynamic graph extension with lineage and replay handling.
- Slurm allocation planning and task packing inside allocations.
- Failure handlers and OOM- and timeout-driven resource correction.
- Offline completion journals and later reconciliation after server outages.
- Intelligent downstream reruns when inputs change.
- RO-Crate and PROV provenance.
- CLI, TUI, dashboard, Python and Julia clients, OpenAPI, and MCP interfaces.

Useful repository anchors include:

- Overall positioning: `README.md`
- Architecture: `docs/src/core/concepts/architecture.md`
- Dependency semantics: `docs/src/core/concepts/dependencies.md`
- Worker and claiming model: `docs/src/core/concepts/job-runners.md`
- Dynamic workflows: `docs/src/core/tutorials/dynamic-jobs.md`
- Recovery: `docs/src/specialized/fault-tolerance/automatic-recovery.md`
- RO-Crate provenance: `docs/src/core/concepts/ro-crate.md`
- Resource-aware claims: `src/server/http_server/jobs_transport.rs`
- Dynamic spawning: `src/server/http_server/jobs_transport.rs`
- Offline journals: `src/client/offline_journal.rs`
- Slurm integration: `src/client/hpc/slurm_interface.rs`

The repository already has useful starting points for evaluation:

- A 100,000-job scale workload in `tests/workflows/scale_test/`.
- A 5,000-job contention workload in `tests/workflows/database_contention_test/`.
- A 3,001-job pipeline workload in `tests/workflows/pipeline_perf_test/`.
- Real Slurm fault and multi-node tests in `slurm-tests/`.
- Integration tests for claiming, resource packing, dynamic jobs, provenance, and recovery.

What is missing is preserved benchmark output, statistical analysis, baseline implementations,
hardware descriptions, and real deployment measurements.

## Novelty

The papers should not claim that Torc invented:

- DAG execution.
- File-derived dependencies.
- Local-to-Slurm portability.
- Persistent workflow state.
- Pilot jobs or task packing inside allocations.
- Retries and resumption.
- Resource requirements.
- Dynamic workflows.
- REST APIs, dashboards, or provenance.

These areas are established in systems such as Pegasus, Snakemake, Nextflow, Parsl, FireWorks,
Balsam, RADICAL-Pilot, Flux, and Airflow.

The defensible novelty is in Torc's integrated design point and several specific mechanisms.

| Candidate | Potential strength | Important qualification |
| --- | --- | --- |
| Transactional dynamic continuation spawning | Strong | Dynamic task generation is established, but Torc's lineage, iteration cap, replay, parent-edge, and atomic-state combination is specific. |
| Atomic heterogeneous resource claiming | Moderate to strong | Combines multidimensional fit, priority ordering, remaining wall time, reservation, and residual backfill in one transaction. |
| Adaptive resource recovery | Moderate to strong | Stronger than generic retries when monitoring, diagnosis, resource correction, graph reset, and new Slurm capacity are evaluated together. |
| Offline result journaling | Moderate to strong | Workers drain running work during server loss and durably reconcile results afterward. |
| Allocation-resident Slurm execution | Moderate | Pilot scheduling is established; Torc must show a utilization, scheduler-load, or deployment advantage. |
| File and JSON artifact dependencies | Moderate | The combination is useful, but many systems already implement dataflow and incremental invalidation. |
| SQLite control plane | Research question | Interesting if experiments define where a single-node, zero-service database is sufficient. |
| OpenAPI, TUI, dashboard, and MCP | Low technical novelty | Valuable software and usability features, but not the center of a systems claim. |

The strongest high-level thesis is:

> Torc demonstrates that a compact, durable workflow control plane can coordinate adaptive,
> heterogeneous scientific campaigns across local and Slurm resources by combining transactional
> job claiming, allocation-resident execution, dynamic graph extension, and resource-aware recovery.

## Paper 1: Software Paper

### Recommended Venue

The Journal of Open Source Software (JOSS) is the best default venue.

Reasons include:

- Torc is substantial, open source, tested, documented, and actively developed.
- It has more than six months of public development history.
- Existing deployed workflows can establish actual research use.
- JOSS creates a recognized software citation.
- JOSS does not charge publication fees.
- Its concise format helps preserve the systems-paper contribution for a later article.

Current JOSS guidance calls for approximately 750 to 1,750 words. Required material includes a
statement of need, state of the field, software design, and research impact.

See the [JOSS submission requirements](https://joss.readthedocs.io/en/latest/submitting.html).

SoftwareX is an alternative if more space is needed for architecture, examples, and software
characterization.

### Proposed Title

> Torc: Durable and Resource-Aware Workflow Orchestration from Workstations to Slurm Clusters

### Core Message

Computational researchers need a workflow manager that supports simple local use and durable
distributed execution without requiring a heavyweight external database or scheduler stack. Torc
provides one workflow and API model across local processes, remote workers, and Slurm allocations.

### Suggested Outline

1. Summary
2. Statement of Need
3. State of the Field
4. Software Architecture
5. Workflow and Dependency Model
6. Local and HPC Execution
7. Fault Recovery and Provenance
8. Research Use and Impact
9. Availability and Reproducibility

### Evidence Needed

The software paper does not need a full comparative benchmark campaign, but it does need credible
research use:

- Two or more deployed scientific applications.
- Anonymized scale and usage summaries.
- At least one reproducible example.
- Installation and basic workflow instructions.
- A description of tests and continuous integration.
- A clear comparison against the closest alternatives.
- A frozen software release and archive DOI.

### Repository Work Before Submission

- Add `CITATION.cff`.
- Archive a paper release with Zenodo or an equivalent service.
- Add a "How to cite" section.
- Confirm paper authors, software contributors, affiliations, and ORCIDs.
- Add funding and institutional acknowledgments.
- Update the stale `README.md` statement targeting a 1.0 release by July 2026.
- Reconcile the Julia client version and disabled CI status.
- Resolve inconsistent or stale documentation, particularly the Python dashboard description.
- Document at least two real research uses.
- Include an AI-use disclosure if required by the venue.

## Paper 2: Systems Journal Article

### Recommended Venue

Future Generation Computer Systems is the strongest initial journal target if a comprehensive
evaluation succeeds. It fits scientific workflows, distributed execution, heterogeneous resources,
HPC and cloud convergence, scheduling, fault recovery, and systems architecture with substantial
experimentation.

Alternatives include:

- **Concurrency and Computation: Practice and Experience:** Best if the result is primarily an
  operational experience and architecture paper.
- **Journal of Parallel and Distributed Computing:** Best if resource scheduling or control-plane
  scalability becomes the primary technical contribution.
- **IEEE Transactions on Parallel and Distributed Systems:** A stretch unless there is a more
  fundamental scheduling or resilience advance.
- **WORKS:** The best specialist audience for an earlier, tightly scoped conference paper.
- **IEEE eScience:** Strong if the scientific case studies are central.

### Proposed Titles

Integrated systems framing:

> Torc: A Compact Control Plane for Adaptive Scientific Workflows on Slurm Clusters

Scheduling and recovery framing:

> Allocation-Resident Scheduling and Resource-Adaptive Recovery for Heterogeneous Scientific
> Workflows

Architecture framing:

> Durable Workflow Orchestration with a SQLite Control Plane: Scalability, Recovery, and Slurm
> Execution

The integrated systems framing is the best current choice.

### Central Claim

The article should evaluate whether a compact, centralized control plane can provide practical
scale, utilization, and recovery for scientific campaign workloads without requiring a heavyweight
distributed workflow service.

### Research Questions

#### RQ1: Control-Plane Scalability

> For what workflow sizes and worker counts can a SQLite-backed server sustain scientific campaign
> orchestration?

Measure:

- Workflow creation and initialization time.
- Job claim throughput.
- Completion throughput.
- API latency percentiles.
- SQLite lock contention and retries.
- Server CPU and memory.
- Database size.
- Worker idle fraction.
- Saturation point.

#### RQ2: Allocation-Resident Scheduling

> When does pulling heterogeneous tasks inside Slurm allocations outperform per-task scheduler
> submission and job arrays?

Measure:

- Number of Slurm submissions.
- Queue wait.
- Task dispatch latency.
- Makespan.
- CPU and GPU utilization.
- Idle core-seconds and GPU-seconds.
- Packing efficiency.
- Resource fragmentation.
- Slurm controller interactions, if available.

#### RQ3: Adaptive Recovery

> Does telemetry-driven resource correction reduce wasted computation and time to successful
> completion after OOM and timeout failures?

Measure:

- Failure-classification precision and recall.
- Retries to completion.
- Wasted node-hours.
- Recovery latency.
- Final over-allocation.
- Makespan.
- Unrecoverable and falsely classified failures.

#### RQ4: Dynamic Workflows

> What correctness and performance costs arise from transactional runtime graph extension?

Measure:

- Spawn transaction latency.
- Throughput as generations and lineages grow.
- Replay behavior after injected worker failures.
- Iteration-cap enforcement.
- Independent-lineage concurrency.
- Dynamic versus pre-expanded graph overhead.

#### RQ5: Scientific Utility

> Do the mechanisms produce measurable operational benefits in deployed scientific campaigns?

Use at least two anonymized applications with meaningfully different characteristics, such as:

- A large parameter sweep or simulation campaign.
- A dependent multi-stage or adaptive workflow.
- A CPU- and GPU-heterogeneous campaign.
- A workflow that experienced OOM, timeout, or control-plane interruption.

## Experimental Design

### Workload Matrix

Use several graph structures:

- Independent bag of tasks.
- Linear chain.
- Diamond.
- Fan-out and fan-in.
- Multi-stage barrier.
- Irregular heterogeneous DAG.
- Dynamic continuation workflow.
- File- and user-data-derived graph.

Use several task-duration regimes:

- No-op jobs for the control-plane upper bound.
- 100 millisecond to 1 second jobs for dispatch sensitivity.
- 1 to 30 second jobs for scheduler overhead.
- Minute-scale representative tasks.
- Real scientific tasks.

Scale across:

- 100, 1,000, 10,000, 100,000, and, if feasible, 1,000,000 jobs.
- 1, 2, 4, 8, 16, 32, and 64 workers.
- Multiple server thread counts.
- On-disk versus in-memory SQLite.
- One node through multi-node Slurm allocations.
- Homogeneous and heterogeneous CPU and GPU requirements.

### Baselines

Do not attempt to benchmark every workflow system. Choose baselines by claim.

For Slurm packing:

- One `sbatch` submission per task.
- Slurm job arrays.
- Torc allocation-resident execution.
- One credible pilot or high-throughput executor such as Parsl HTEX, Flux, RADICAL-Pilot, or Balsam.

For durable workflow management:

- FireWorks or Balsam as the closest architectural comparison.
- Snakemake or Nextflow as a familiar file-centric comparison, with carefully matched semantics.

For recovery:

- No retry.
- Fixed retry with unchanged resources.
- Blanket resource multiplication.
- Torc telemetry-driven correction.
- Attempt-dependent resource escalation where supported by a competing system.

For local execution:

- GNU Parallel or a fixed process pool.
- Torc queue-depth mode.
- Torc resource-aware mode.

### Ablations

- On-disk versus in-memory database.
- Single-row versus batch completion.
- Resource-aware versus queue-depth claiming.
- Backfill enabled versus disabled.
- Monitoring disabled versus aggregate versus time series.
- Provenance disabled versus enabled.
- Static resources versus adaptive resource correction.
- One worker per allocation versus one per node.
- Dynamic spawning versus a statically pre-expanded graph.
- Offline journaling enabled versus immediate failure on server loss.

### Fault Injection

Test:

- Worker death before starting a claimed job.
- Worker death during execution.
- Server restart.
- Temporary server or network outage.
- Lost or duplicated completion request.
- Stale run ID.
- OOM.
- Wall-time expiration.
- Slurm cancellation or preemption.
- SQLite write contention.
- Partial dynamic-spawn replay.

Fault recovery is one of Torc's strongest potential contributions, so this portion of the evaluation
should be treated as a primary experiment rather than a peripheral test.

## Systems Paper Outline

1. Introduction
2. Motivation and Requirements
3. Related Work
4. System Model
5. Core Mechanisms
6. Experimental Methodology
7. Control-Plane Evaluation
8. Scheduling and Utilization Evaluation
9. Recovery and Fault-Tolerance Evaluation
10. Scientific Case Studies
11. Limitations
12. Reproducibility and Artifact Availability
13. Conclusion

The core-mechanisms section should cover:

- Atomic heterogeneous claiming and backfill.
- Allocation-resident execution.
- Dynamic continuation spawning.
- Resource-aware recovery.
- Offline result reconciliation.

## Figures and Tables

### Figures

1. Torc architecture and control and data paths.
2. Job state transitions and recovery paths.
3. Allocation-resident Slurm execution.
4. Dynamic continuation and lineage model.
5. Claim and completion throughput versus workers.
6. End-to-end makespan versus task duration.
7. Packing efficiency or utilization versus workload heterogeneity.
8. Recovery time and wasted node-hours.
9. Case-study DAGs and resource timelines.
10. SQLite saturation or operating-envelope plot.

### Tables

1. Feature and architecture comparison with the closest workflow systems.
2. Experimental hardware and software versions.
3. Workload definitions and graph properties.
4. Fault-injection scenarios.
5. Case-study characteristics.
6. Summary of results and effect sizes.
7. Limitations and appropriate workload boundaries.

## Claims to Avoid

Avoid broad claims that no other system combines Torc's capabilities unless they are supported by a
carefully versioned comparison.

Also avoid:

- "Torc is lightweight" without deployment-size and resource measurements.
- "Torc scales to one million jobs" until this is demonstrated.
- "Torc improves utilization" without node-level utilization measurements.
- "Torc automatically recovers most failures."
- The documentation's approximate OOM and timeout failure distribution, for which no supporting
  dataset was found.
- "Exactly once" execution. The current architecture is better characterized through atomic
  claiming, idempotent completion handling, run IDs, and reconciliation.
- "Optimal packing." The scheduler is a practical greedy heuristic.
- "Single binary" without explaining the feature-gated server, dashboard, MCP, and Slurm runner
  binaries.
- "Portable workflow standard." Torc's workflow format is not CWL.

## Important Technical Risks

Several implementation details should be strengthened or explicitly treated as limitations:

- Claims do not appear to use persisted leases or fencing tokens.
- Dynamic-spawn replay appears primarily name-based rather than based on a canonical request digest.
- Job names are not protected by a database-level unique `(workflow_id, name)` constraint.
- File content is not represented symmetrically with JSON user-data content in the job-input hash.
- Some initialization operations occur after the core transaction has committed.
- Aggregate resource feasibility is not always equivalent to realizable node-level placement.
- SSE events are ephemeral and do not support replay.
- SQLite remains a single-writer coordination point.
- Remote-worker fault tolerance is more limited than Slurm recovery.
- Some important Slurm scenarios are manual rather than continuously tested.

These issues do not prevent publication. Clearly characterizing the operating boundaries would
strengthen the paper.

## Other Considerations

### Reproducibility

Release:

- Raw anonymized measurements.
- Workflow definitions.
- Benchmark drivers.
- Exact Torc and baseline versions.
- Slurm configuration.
- Machine descriptions.
- Analysis and plotting scripts.
- Failed-run and outlier policy.
- Checksums.
- An RO-Crate generated by Torc for the evaluation itself.

Using Torc's own RO-Crate support to package the paper experiments would be a strong demonstration.

### Statistical Methodology

For controlled experiments:

- Use repeated trials.
- Report medians and distributions or confidence intervals.
- Separate queue wait from execution.
- Record cold and warm runs.
- Randomize competing configurations when using a shared production cluster.
- Report negative and failed experiments.
- Use effect sizes rather than relying only on significance tests.

### Case-Study Privacy

Because only anonymized deployed-workflow measurements can be released:

- Define the anonymization protocol before collecting data.
- Remove usernames, paths, project names, account names, and confidential commands.
- Preserve graph structure and resource distributions where permitted.
- Obtain approval from application owners.
- Determine whether institutional review or data-release review is required.
- Avoid exposing scheduler accounting that could identify projects indirectly.

### Authorship

Set authorship expectations early using CRediT roles:

- Conceptualization.
- Software.
- Methodology.
- Validation.
- Investigation.
- Data curation.
- Visualization.
- Writing.
- Supervision.
- Funding acquisition.

The six Cargo authors and the two Python-package authors are currently inconsistent, so software
credit and paper authorship need explicit resolution.

## Suggested Publication Sequence

1. Inventory the available deployed workflows, usage evidence, permissions, and possible anonymized
   metrics.
2. Prepare the JOSS paper, citation metadata, and archived release.
3. Build a reusable benchmark and fault-injection harness.
4. Run the control-plane and local experiments.
5. Run the production Slurm scheduling and recovery campaign.
6. Analyze at least two deployed scientific applications.
7. Present an early focused result to the workflow community, potentially at WORKS or eScience.
8. Submit the expanded systems article to Future Generation Computer Systems, with Concurrency and
   Computation: Practice and Experience as the likely alternative.

The intended relationship between the papers is:

> The JOSS paper establishes Torc as research software. The systems paper uses Torc to test and
> quantify a compact architectural approach to adaptive HPC workflow orchestration.

This distinction gives Torc the best chance of obtaining both a durable software citation and a
substantive workflow-systems publication.
