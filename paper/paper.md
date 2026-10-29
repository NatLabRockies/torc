---
title: 'Torc: Durable and Resource-Aware Workflow Orchestration from Workstations to Slurm Clusters'
tags:
  - scientific workflows
  - high-performance computing
  - Slurm
  - workflow orchestration
  - research software
authors:
  - name: Daniel Thom
    corresponding: true
    affiliation: 1
  - name: Nathan Keilbart
    affiliation: 2
  - name: Joseph McKinsey
    affiliation: 1
  - name: Pedro Andres Sanchez Perez
    affiliation: 1
  - name: Andrew Lai
    affiliation: 2
  - name: Kapil Duwadi
    affiliation: 3
affiliations:
  - name: National Laboratory of the Rockies
    index: 1
  - name: Lawrence Livermore National Laboratory
    index: 2
  - name: Pacific Northwest National Laboratory
    index: 3
date: 15 September 2026
bibliography: paper.bib
---

# Summary

Computational studies require carefully prepared inputs across complex parameter sweeps,
managing many simulations with varying computing requirements, and combining the results
into a cohesive story all while tracking the provenance of inputs and outputs.
Even with all this planning, calculations are sure to fail and should not require restarting
an entire study.

Torc is an open-source workflow manager for analysts running existing simulation applications and
scripts. Precompiled binaries support local use without a separate database service or a workflow
programming language. On clusters managed by Slurm, Torc runs many jobs within each allocation of
resources, selecting work that fits the available resources and remaining time rather than submitting
every job separately. It retains searchable workflow history, results, and resource measurements.
Recovery tools selectively repeat failed work and calculations affected by changed inputs; optional
automated recovery retries memory- or time-limited jobs with increased resources. A common application
programming interface supports command-line tools, an interactive terminal interface, a graphical web
dashboard, and Python and Julia clients. Researchers can move from local studies to shared
computational campaigns while preserving execution records and avoiding unnecessary repetition.

# Statement of need

Scientific campaigns often begin with shell scripts or scheduler job arrays. These approaches can
suffice for independent calculations, but large parameter studies combine dependent tasks with
different CPU, GPU, and memory requirements. Analysts running long-lived studies on shared
high-performance computing (HPC) systems need to inspect execution history, diagnose failures, and
repeat only failed or invalidated work without wasting allocation time or becoming workflow
engineers.

Meeting these needs requires more than submitting jobs: workflow state must persist beyond
individual runs, and workers must keep available resources supplied with suitable work. Torc
separates persistent workflow coordination from workers that claim jobs according to available
resources and remaining allocation time. Execution, inspection, and recovery share one data model,
allowing analysts to trace outcomes and repeat affected work without reconstructing a campaign from
separate logs and scripts. Local operation requires no separately administered database service,
while a shared server supports longer-lived campaigns and team access.

# State of the field

Scientific workflow tools differ in how users define work, retain its history, and manage resources.
Snakemake and Nextflow offer portable workflow languages [@molder2021snakemake;
@ditommaso2017nextflow], while Parsl embeds parallel workflows in Python [@babuji2019parsl]. Pegasus
combines workflow planning with provenance, data management, and recovery [@deelman2019pegasus]. For
persistent coordination, FireWorks and Balsam use services and workers [@jain2015fireworks;
@salim2018balsam], while AiiDA couples workflow automation with queryable data provenance
[@huber2020aiida]. At the resource level, RADICAL-Pilot uses pilot jobs and Flux uses hierarchical
scheduling to distribute tasks within allocated HPC resources. Both decouple application tasks from
batch submissions, allowing one allocation to serve many tasks [@merzky2018radicalpilot; @ahn2020flux].

Torc's development team assessed existing workflow engines against the needs of analysts running
established applications in large parameter studies. Local deployment needed precompiled binaries
without separately managed services or language-specific runtime environments. Analysts needed easy 
to understand commands and configuration setup without learning a workflow programming language. On 
HPC systems, workers need to continually draw jobs from an external queue to maximize job 
allocations. Durable, queryable campaign history and integrated recovery were essential for managing
repeated runs, while a language-independent API would support multiple interfaces and integration 
with other software.

Meeting these requirements led the team to separate persistent workflow state from resource-aware
workers that claim jobs within allocations. The team judged that combining durable multi-user state
with this execution model would require changes beyond an executor plugin in the evaluated
Snakemake and Nextflow designs. FireWorks offered a closer architectural fit, but its MongoDB
dependency conflicted with the goal of avoiding separately administered supporting services
[@jain2015fireworks].

This is a fit-for-purpose rationale, not a claim that other systems cannot execute commands locally
or on HPC resources. FireWorks and Balsam are particularly close architectural neighbors in their
persistent, worker-based coordination. Torc's contribution is the integration of that operating
model with standalone local use, SQLite-backed history, resource-aware execution, and common
inspection and recovery tools. The design prioritizes these capabilities within one software package
rather than requiring analysts to assemble a workflow engine, a database service, and separate
operational tooling.

# Software design

Torc uses a server to coordinate work and workers to execute it (\autoref{fig:architecture}). The Rust
server maintains the authoritative record of workflows, job dependencies, resource requests, and
execution outcomes. Since the server has centralized database access and workers request jobs through
its REST API, it can coordinate writes in transactions and batch job-completion updates. This makes
lightweight SQLite practical without a separate database service, although its single-writer
constraint still limits write throughput. Each worker reports its available resources and remaining
runtime when requesting work, allowing the server to select ready jobs that fit. Workers execute
those jobs, report outcomes, and request additional work as capacity becomes available. The server
records these outcomes and releases jobs whose dependencies are satisfied.

The CLI can start a local server for a standalone command or connect to a long-running service for
shared history and distributed execution. Standalone runs retain an on-disk database by default
which can reside on the server's local filesystem rather than the cluster's parallel filesystem.
On Unix, in-memory operation can improve performance by reducing filesystem access, but risks losing
changes since the last successful disk snapshot if the server or its hosting allocation is terminated
unexpectedly or lost. Torc can save snapshots on demand or, in standalone mode, at configured intervals
and before normal shutdown. Retained records can be queried by owner, project labels, or user-defined
metadata. Database backups preserve these records, while external input files, outputs, and logs must
be preserved separately.

![Torc separates user-facing tools, SQLite-backed workflow coordination, and pull-based execution
across local, remote, and Slurm resources. Workers claim jobs and report outcomes over HTTP. Solid
lines show control and state interactions; dashed lines show scientific artifact
access.\label{fig:architecture}](architecture.png){
width=100% }

Regardless of deployment mode, users describe the work to be coordinated in a workflow file containing
commands, parameter ranges, dependencies, and resource requirements. Parameter expansion creates jobs
for individual simulations, while explicit dependencies or input-output relationships connect
preparation, simulation, and aggregation steps. Together, these definitions establish the dependency
graph used to coordinate execution. The graph can also grow during execution: adaptive workflows add
jobs transactionally, with lineage tracking and iteration limits. Existing scientific executables
remain the unit of work, without requiring their implementation in a workflow-specific programming
interface.

Submitting the workflow registers its expanded jobs and dependencies with the server, and
initialization identifies jobs that are ready to run. Once a worker comes online on a local or remote
host, it requests ready jobs according to its available CPUs, memory, GPUs, node count, and remaining
runtime. The server selects jobs and marks them pending in one transaction, preventing competing
workers from claiming the same ready job. Priority ordering and backfill let ready CPU-only jobs use
spare cores and memory alongside GPU jobs. Workers track the resources reserved by running jobs and
release that capacity as jobs finish, including in direct execution mode. Within a Slurm allocation,
workers repeat this process for the remaining walltime, avoiding a separate batch submission for each
application job. As the scheduler handles allocation and placement while Torc selects dependency-ready
work, this allows workers in multiple active allocations to draw jobs concurrently from the same
workflow.
Moving a workflow to a cluster preserves its dependency model but still requires compatible
application environments, access to required files, server connectivity, and site-specific scheduler
configuration.

As jobs finish, the server records their outcomes and updates dependency status, releasing blocked
jobs once their prerequisites are satisfied. During temporary API outages, workers can journal
completions locally and reconcile them with the server later. The resulting history of execution
attempts helps users diagnose failures, while CPU, memory, and runtime measurements can guide
subsequent resource requests. When monitoring and automatic recovery are enabled, client-side tools
can identify likely memory or timeout failures, adjust resource requests, and retry selected jobs.
For application-specific failures, users can define exit-code-based retry rules in the workflow file
and optionally specify a recovery script to run before retrying a job.

Beyond recovering from failures, researchers may need to repeat a successful study with revised
inputs or parameters. After applying definition changes to the stored workflow or modifying tracked
input files, users reinitialize the workflow as a whole. Torc compares file modification times and
job-input hashes with stored values, checks for missing outputs, and follows dependencies to
identify which jobs need to run again. Affected jobs and their downstream dependents are reset for
execution, while unaffected completed jobs are preserved. Reinitialization therefore checks the full
workflow without requiring every job to be rerun.

Torc makes the same workflow records available for interactive use, programmatic control, and
documentation of results. The CLI, interactive terminal interface, and web dashboard use a common
REST API to inspect and manage workflows. An OpenAPI specification supports the generated Python
and Julia clients included in the repository, as well as clients in other languages. The Model
Context Protocol (MCP) server exposes workflow operations to AI assistants for inspection, debugging,
and recovery, without making AI a requirement for execution or deterministic recovery. To document
and share how results were produced, optional RO-Crate exports capture workflow, job, input, output,
and software provenance from the recorded state [@soilandreyes2022rocrate].

# Research impact statement

\textcolor{red}{TODO before submission: Confirm which ExaEpi deployment details may be disclosed and
what evidence can be shared with editors or reviewers. Obtain an approved reference or supporting
record for the Dane PCM work; a research publication may not be available. Remove or qualify claims
that cannot be supported, then remove this note.}

At the National Energy Research Scientific Computing Center at Lawrence Berkeley National
Laboratory, Torc orchestrated ExaEpi parameter sweeps for predicting epidemiological events,
including COVID-19. The campaign ran three million simulations in batches of 100,000 on the
Perlmutter system. Its Slurm workflow combined GPU-backed main simulations with CPU-backed setup and
postprocessing jobs. In the previous workflow, the delay before a worker picked up and started the
next calculation could approach the runtime of a short simulation. Torc reduced this between-job
dispatch delay, reducing campaign walltime rather than accelerating the simulations themselves. This
experience illustrates the importance of keeping workers supplied with ready work when individual
calculations are short.

At Lawrence Livermore National Laboratory's Livermore Computing Dane system, Torc ran power-system
production-cost modeling (PCM) calculations across varied input scenarios. Each calculation modeled
a year of power-system operation; this is the simulated period, not the execution duration. These
CPU-only calculations ran longer than the ExaEpi simulations. Resource monitoring exposed CPU and
memory use for debugging and allowed the team to tune resource requests for subsequent simulations.
Together, these applications illustrate the value of dispatching short heterogeneous tasks and
retaining resource evidence for longer-running analyses.

<!-- Add a documented Kestrel deployment after its scientific use, scale, and outcomes are confirmed. -->

# Availability

Torc is distributed under the BSD 3-Clause license. Source code, documentation, workflow examples,
and precompiled binaries are available at <https://github.com/NatLabRockies/torc>.

\textcolor{red}{TODO before publication: Archive the reviewed release, identify its version here,
and add a citation and link to its persistent DOI. Remove this note once complete.}

# AI usage disclosure

OpenAI GPT-6 Astra Project, accessed through OpenCode, assisted with literature discovery, manuscript
drafting and revision, figure preparation, formatting, and citation checks. Generative AI was also
used in developing Torc. The authors made the software's core design decisions and retain
responsibility for the software and manuscript.

<!-- Developer input required before submission: Identify the AI tools/models and versions used in
Torc's development and documentation, and describe where and how they assisted (for example, code
generation, refactoring, tests, or documentation). Complete the disclosure above with these details.
Also add an explicit confirmation, once true, that human authors reviewed, edited, and validated all
AI-assisted outputs, explaining how software and manuscript accuracy were checked. Remove this
comment after completing the disclosure. -->

# Acknowledgements

<!-- Add institutional acknowledgements, funding sources, sponsor involvement, and contributor
credit before submission. -->

Acknowledgements and funding information will be added before submission.

# References
