---
title: 'Torc: Durable and Resource-Aware Workflow Orchestration from Workstations to Slurm Clusters'
tags:
  - scientific workflows
  - high-performance computing
  - Slurm
  - workflow orchestration
  - research software
authors:
  # Replace this placeholder with the final authors, affiliations, and ORCIDs.
  - name: Author list to be finalized
    corresponding: true
    affiliation: 1
affiliations:
  - name: Affiliation to be finalized
    index: 1
date: 15 September 2026
bibliography: paper.bib
---

# Summary

Torc is an open-source workflow orchestration system for command-oriented computational pipelines.
It supports workflows ranging from independent parameter sweeps to dependency-rich, multi-stage
campaigns with heterogeneous CPU, memory, GPU, node-count, and runtime requirements. Researchers can
describe workflows declaratively, run them locally, and execute the same workflow through workers
inside Slurm allocations. A REST service and SQLite database maintain durable workflow state, while
a unified command-line interface supports creation, execution, monitoring, diagnosis, selective
reruns, and recovery. Python and generated API clients provide programmatic access, while terminal
and web interfaces support interactive operation. Torc is intended for computational scientists who
need a practical path from workstation-scale experiments to persistent high-performance computing
campaigns.

# Statement of need

Computational studies commonly begin with shell scripts, local process pools, or scheduler job
arrays. These approaches can be sufficient for independent tasks, but scientific campaigns often
grow to include dependencies, heterogeneous resource requirements, long execution histories, and
failures that must be diagnosed and selectively rerun. Moving such a campaign to a high-performance
computing system can force researchers to change how they describe work, manage state, submit jobs,
inspect progress, and recover from failures.

Torc addresses this transition with a common workflow and control model across local and Slurm
execution. Workflow definitions describe commands, dependencies, files, user data, resource
requirements, schedulers, and failure behavior. Torc persists this information and execution state
behind an HTTP API. Workers claim ready jobs according to available resources, execute them as local
processes or Slurm job steps, and return results and resource observations. This separation allows
the same workflow to be inspected and operated through a command-line interface, Python client,
terminal interface, web dashboard, or direct API calls.

The compact control plane is important to Torc's intended use. Its server uses SQLite and does not
require researchers to administer a separate database service for core workflow state. Torc is not
intended to replace every workflow language, distributed runtime, or high-performance computing
scheduler. It targets command-oriented scientific campaigns that benefit from durable coordination,
resource-aware execution, and a consistent path between workstations and Slurm clusters.

# State of the field

Scientific workflow systems span several overlapping design spaces. Snakemake and Nextflow provide
mature dataflow-oriented workflow languages and broad execution portability
[@molder2021snakemake; @ditommaso2017nextflow]. Pegasus provides planning, provenance, data
management, and recovery for large scientific workflows [@deelman2019pegasus]. Parsl provides
Python-native parallel scripting and high-performance computing executors [@babuji2019parsl].
FireWorks and Balsam provide persistent services and worker-based execution for high-throughput
scientific campaigns [@jain2015fireworks; @salim2018balsam]. Pilot systems and hierarchical
schedulers, including RADICAL-Pilot and Flux, execute many tasks within larger resource allocations
[@merzky2018radicalpilot; @ahn2020flux].

Torc was built rather than added to one of these systems to provide a compact, command-oriented path
from local execution to Slurm without adopting a system-specific programming language or operating
a separate database service. Torc does not claim to originate directed acyclic graph execution,
data-derived dependencies, persistent workflow state, or task execution inside allocations. Its
software contribution is the integration of declarative specifications, a unified command-line
interface, durable SQLite-backed state, resource-aware local and Slurm workers, operational
monitoring, recovery, and provenance behind one API and execution model. This design favors
straightforward deployment and consistent campaign operation over the language ecosystems,
portable standards, or specialized distributed runtimes offered by other systems.

# Software design

Torc uses a client-server architecture. An asynchronous Rust server exposes a versioned REST API and
stores workflows, jobs, dependencies, files, user data, resource requirements, results, events, and
scheduler records in SQLite. Clients and workers communicate with the server over HTTP, allowing the
database to remain on server-local storage while workers execute on other nodes. This design trades
the horizontal write scalability of a distributed database for a control plane that is simple to
deploy, back up, and inspect.

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
generation records workflow, job, input, output, and software provenance
[@soilandreyes2022rocrate]. Workflows can also extend their graphs at runtime through transactional
job spawning for adaptive or iterative algorithms.

Users interact with these capabilities through declarative workflow files and a unified `torc`
command-line interface. The same API supports generated clients, a Python orchestration layer, a
terminal interface, a web dashboard, and an MCP server. This layered design allows interactive and
programmatic clients to share workflow semantics rather than independently implementing workflow
state transitions.

# Research impact statement

<!-- Replace this section with specific evidence from at least two research deployments. Include
the scientific purpose, workflow shape, approximate scale, execution environment, and realized
impact. JOSS requires demonstrated research use rather than aspirational applications. -->

Torc has been used for computational simulation campaigns, multi-stage data-processing pipelines,
and workflows with heterogeneous CPU and GPU requirements. These deployments motivate its emphasis
on durable state, resource-aware Slurm execution, monitoring, and selective recovery. The final
manuscript will describe the deployments for which publication permission and verifiable evidence
are available. Descriptive impact claims will be distinguished from performance claims requiring
released measurements and methods.

# Availability

Torc is distributed under the BSD 3-Clause license. Source code, documentation, examples,
integration tests, and release artifacts are available at
<https://github.com/NatLabRockies/torc>. The project uses automated tests and generated-client
consistency checks across its Rust server and clients. The version reviewed with this paper will be
archived with a persistent DOI, and citation metadata will be provided in the repository.

# AI usage disclosure

OpenAI GPT-5.6, accessed through OpenCode, was used for literature discovery, manuscript
brainstorming, drafting, and formatting this paper for JOSS. Before submission, the authors will
review and edit all AI-assisted text and verify its technical claims, references, and comparisons
against the software, primary literature, and released evidence. The authors made the software's
core design decisions and retain responsibility for the final manuscript.

# Acknowledgements

<!-- Add institutional acknowledgements, funding sources, sponsor involvement, and contributor
credit before submission. -->

Acknowledgements and funding information will be added before submission.

# References
