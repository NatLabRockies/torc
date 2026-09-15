# Torc: Durable and Resource-Aware Workflow Orchestration from Workstations to Slurm Clusters

> Working JOSS manuscript and brainstorming document. Author metadata, citations, research-use
> details, and the final AI-use disclosure remain to be added.

## Paper Brief

### Intended Reader

The primary reader is a computational scientist who:

- Starts workflows on a workstation or single compute node.
- Needs to move simulation campaigns or data pipelines to a Slurm cluster.
- Wants durable state, dependency handling, monitoring, and recovery without operating a complex
  workflow service.
- May prefer declarative workflow files and a CLI, but also needs Python and HTTP interfaces for
  automation.

### Lead Problem

Scientific workflows often begin as shell scripts, ad hoc job arrays, or local process pools. As
workloads grow, researchers must add dependency management, heterogeneous resource requests,
persistent state, failure recovery, and cluster integration. Existing workflow systems solve many
of these problems, but adopting them can require a new programming model, a collection of services,
or separate tools for local execution, Slurm submission, monitoring, and recovery.

The paper should not argue that existing systems are generally heavyweight or difficult. Instead,
it should make the narrower and supportable observation that there is room for a compact,
integrated path between local command execution and durable Slurm workflow operation.

### Working Thesis

Torc gives computational scientists one operational model for declarative command workflows across
workstations and Slurm clusters. It combines a compact Rust and SQLite control plane with resource-
aware workers, durable workflow state, monitoring, recovery, provenance, and language-neutral APIs.

### One-Sentence Pitch

Torc helps computational scientists move command-oriented workflows from local execution to Slurm
clusters without replacing the workflow description or assembling separate systems for state,
monitoring, resource management, and recovery.

### Differentiation

The state-of-the-field discussion should combine two related points:

1. **Compact deployment:** Torc uses distributable Rust binaries and SQLite rather than requiring a
   separately administered database or a larger service stack for its core operation.
2. **Integrated workflow path:** The same workflow and API model spans local runs, remote and Slurm
   workers, durable state, monitoring, selective reruns, resource correction, and provenance.

Neither point should be stated as universal superiority. The paper should explain the design space
Torc occupies and the workloads for which that design is useful.

### Authoring and Interaction Model

The paper should foreground declarative workflow specifications and the unified CLI, then show that
the same functionality is available through several interfaces:

- YAML, JSON, JSON5, and KDL workflow specifications.
- A unified CLI for creation, execution, inspection, monitoring, and recovery.
- A Python client for programmatic workflow construction and adaptive orchestration.
- An OpenAPI-described REST interface and generated language clients.
- A terminal UI and web dashboard for interactive operation.
- An MCP interface for tool-based workflow inspection and operation.

This breadth supports the integrated-path argument, but the manuscript should not become a feature
catalog. Each interface should be connected to a concrete user need.

### Research-Impact Evidence

The paper should use at least two deployed applications and ideally cover all three selected
workload classes:

- Simulation campaigns or parameter sweeps.
- Multi-stage data pipelines.
- Heterogeneous CPU and GPU workflows.

For each application, collect publishable, anonymized information:

- Scientific domain and objective.
- Workflow shape and dependency pattern.
- Typical and maximum jobs per workflow.
- Number of workflow runs or campaigns.
- CPU, GPU, memory, and runtime range.
- Local, remote, or Slurm deployment mode.
- Which Torc capabilities were important.
- Operational outcome, such as replacing scripts, preserving state, diagnosing failures, or
  selectively rerunning work.
- Whether workflow definitions or sanitized examples can be released.

The examples should establish actual research use rather than serve as performance claims.

## Candidate Titles

1. **Torc: Durable and Resource-Aware Workflow Orchestration from Workstations to Slurm Clusters**
2. **Torc: A Compact Workflow Manager for Local and Slurm-Based Scientific Computing**
3. **Torc: An Integrated Workflow Path from Local Experiments to HPC Campaigns**
4. **Torc: Persistent Scientific Workflow Orchestration with a Compact Control Plane**

The first title is the strongest current choice because it names the software, identifies two
important properties, and communicates the local-to-HPC scope without claiming novelty.

## Candidate Taglines

- One workflow model from a workstation to a Slurm allocation.
- Durable scientific campaigns without a heavyweight control plane.
- Declarative workflows, resource-aware execution, and recovery across local and HPC environments.

These are brainstorming phrases, not proposed scientific claims.

## Draft Manuscript

### Summary

Torc is an open-source workflow orchestration system for command-oriented computational pipelines.
It supports workflows ranging from independent parameter sweeps to dependency-rich, multi-stage
campaigns with heterogeneous CPU, memory, GPU, node-count, and runtime requirements. Researchers can
describe workflows declaratively, run them locally, and execute the same workflow through workers
inside Slurm allocations. A REST service and SQLite database maintain durable workflow state while a
unified command-line interface supports creation, execution, monitoring, diagnosis, selective
reruns, and recovery. Python and generated API clients provide programmatic access, while terminal
and web interfaces support interactive operation. Torc is intended for computational scientists who
need a practical path from workstation-scale experiments to persistent HPC campaigns.

### Statement of Need

Computational studies commonly begin with shell scripts, local process pools, or scheduler job
arrays. These approaches can be sufficient for independent tasks, but scientific campaigns often
grow to include dependencies, heterogeneous resource requirements, long execution histories, and
failures that must be diagnosed and selectively rerun. Moving such a campaign to an HPC system can
force researchers to change how they describe work, manage state, submit jobs, inspect progress, and
recover from failures.

Torc addresses this transition with a common workflow and control model across local and Slurm
execution. Workflow definitions describe commands, dependencies, files, user data, resource
requirements, schedulers, and failure behavior. Torc persists this information and execution state
behind an HTTP API. Workers claim ready jobs according to available resources, execute them as local
processes or Slurm job steps, and return results and resource observations. This separation allows
the same workflow to be inspected and operated through a CLI, Python client, terminal UI, web
dashboard, or direct API calls.

The compact control plane is important to Torc's intended use. Its server uses SQLite and does not
require researchers to administer a separate database service for the core workflow state. Torc is
not intended to replace every workflow language, distributed runtime, or HPC scheduler. It targets
command-oriented scientific campaigns that benefit from durable coordination, resource-aware
execution, and a consistent path between workstations and Slurm clusters.

### State of the Field

Scientific workflow systems span several overlapping design spaces. Systems such as Snakemake and
Nextflow provide mature dataflow-oriented workflow languages and broad execution portability.
Pegasus provides planning, provenance, data management, and recovery for large scientific
workflows. Parsl provides Python-native parallel scripting and HPC executors. FireWorks and Balsam
provide persistent services and worker-based execution for high-throughput scientific campaigns.
Pilot systems and hierarchical schedulers, including RADICAL-Pilot and Flux, execute many tasks
within larger resource allocations.

Torc does not claim to originate DAG execution, data-derived dependencies, persistent workflow
state, or task execution inside allocations. Its software contribution is a compact integration of
these ideas for command-oriented scientific users. Declarative specifications, a unified CLI,
durable SQLite-backed state, resource-aware local and Slurm workers, operational monitoring,
recovery, and provenance share one API and execution model. This design favors straightforward
deployment and consistent campaign operation over the language ecosystems, portable standards, or
specialized distributed runtimes offered by other systems.

### Software Design

Torc uses a client-server architecture. An asynchronous Rust server exposes a versioned REST API and
stores workflows, jobs, dependencies, files, user data, resource requirements, results, events, and
scheduler records in SQLite. Clients and workers communicate with the server over HTTP, allowing the
database to remain on server-local storage while workers execute on other nodes.

Dependencies may be declared directly between jobs or inferred from producer and consumer
relationships over files and JSON user data. During workflow initialization, Torc resolves these
relationships into a common dependency graph. Workers atomically claim ready jobs and transition
them to a pending state, preventing concurrent workers from allocating the same job. Resource-aware
claims consider available CPUs, memory, GPUs, node count, and remaining runtime. Slurm workers run
inside allocations and can launch multiple resource-constrained job steps, enabling fine-grained
workflow execution without submitting every task independently to the scheduler.

Torc persists results and resource observations for later inspection. It supports failure handlers,
selective workflow reinitialization, resource correction after memory or runtime failures, and
offline completion journals when a worker temporarily loses access to the server. Optional RO-Crate
generation records workflow, job, input, output, and software provenance. Workflows can also extend
their graphs at runtime through transactional job spawning for adaptive or iterative algorithms.

Users interact with these capabilities through declarative workflow files and a unified `torc`
command-line interface. The same API supports generated clients, a Python orchestration layer, a
terminal UI, a web dashboard, and an MCP server. This layered design allows interactive and
programmatic clients to share workflow semantics rather than independently implementing workflow
state transitions.

### Research Impact

<!-- Replace this section with two or more concrete, anonymized deployments. -->

Torc has been used for computational simulation campaigns, multi-stage data-processing pipelines,
and workflows with heterogeneous CPU and GPU requirements. These deployments motivate its emphasis
on durable state, resource-aware Slurm execution, monitoring, and selective recovery.

For each selected deployment, the final manuscript should identify the scientific use, workflow
shape, approximate scale, resource heterogeneity, execution environment, and concrete benefit
provided by Torc. Claims should remain descriptive unless the underlying measurements and methods
are released.

### Availability and Reproducibility

Torc is distributed under the BSD 3-Clause license. Source code, documentation, examples,
integration tests, and release artifacts are publicly available in the project repository. The
project uses automated tests and generated-client consistency checks across its Rust server and
clients. The paper release will be archived with a persistent DOI, and citation metadata will be
provided in the repository.

<!-- Add repository URL, archived release DOI, exact paper version, and installation command. -->

### AI Usage Disclosure

<!-- Add the disclosure required by the target venue. This manuscript began with AI-assisted
brainstorming and drafting and must not state otherwise. Authors remain responsible for verifying
all claims, references, and final text. -->

## Working Contribution Statement

The first paper's contribution is the software and its demonstrated research utility, not a new
scheduling algorithm. A concise contribution statement could be:

> Torc provides computational scientists with an open-source, compact, and durable orchestration
> system that unifies declarative workflow authoring, local and Slurm execution, resource-aware job
> management, operational recovery, monitoring, and provenance behind a common API.

The adjective "compact" must be grounded in architecture and deployment requirements. It should not
be presented as a measured performance result unless supporting measurements are added.

## Word-Budget Sketch

Target approximately 1,500 to 1,700 words before references:

- Summary: 150 to 200 words.
- Statement of Need: 300 to 400 words.
- State of the Field: 250 to 350 words.
- Software Design: 400 to 500 words.
- Research Impact: 250 to 350 words.
- Availability, acknowledgments, and disclosures: 100 to 150 words.

The manuscript should link to project documentation rather than enumerate every feature.

## Evidence Checklist

- [ ] Identify at least two deployed applications.
- [ ] Obtain permission to publish anonymized descriptions and measurements.
- [ ] Record application domains and scientific objectives.
- [ ] Record typical and maximum workflow sizes.
- [ ] Record dependency structures and resource classes.
- [ ] Record local, remote, and Slurm execution modes used in practice.
- [ ] Identify concrete research outcomes or operational improvements.
- [ ] Select one small reproducible workflow for reviewers.
- [ ] Confirm the public repository and issue tracker meet JOSS requirements.
- [ ] Add `CITATION.cff` and archive the reviewed release.
- [ ] Confirm author list, affiliations, ORCIDs, CRediT roles, and funding.
- [ ] Build and verify the final bibliography from primary sources.
- [ ] Check every comparison against the cited software version.
- [ ] Add the required AI-use disclosure.

## Candidate References

The bibliography should include primary references for the systems discussed in the State of the
Field section. Initial candidates are:

- Pegasus WMS.
- Snakemake.
- Nextflow.
- Parsl.
- FireWorks.
- Balsam.
- RADICAL-Pilot or RADICAL-Cybertools.
- Flux Framework.
- Common Workflow Language, where workflow portability standards are discussed.
- RO-Crate, if provenance receives more than a brief mention.

The final reference set should remain concise enough for a JOSS paper. Dask, Ray, and Airflow are
useful broader context but may not need citations unless the manuscript directly contrasts Torc with
their programming or deployment models.

## Open Questions

- Which two or three deployed applications best demonstrate breadth without exposing sensitive
  details?
- Can any sanitized workflow specifications be released with the paper?
- What evidence supports the compact-deployment claim: required services, installation size, startup
  steps, or a reproducible reviewer exercise?
- Should remote SSH workers be prominent in the paper or treated as a secondary execution mode?
- Is automatic recovery mature and widely used enough to feature in the short manuscript?
- Should dynamic workflows be mentioned in the software-design section or reserved mostly for the
  systems paper?
- Which interfaces have real users and should receive scarce manuscript space?
- What research outputs, reports, or publications were produced by the deployed workflows?
