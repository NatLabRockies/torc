# Torc RO-Crate Provenance Change Rationale

## Decision

Torc now treats the data team's PROV-shaped RO-Crate format as the canonical export and generation
model. I chose the breaking-change path because the assignment explicitly allowed it and because a
translation layer would have kept two provenance models alive at once.

That would have increased long-term cost in three ways:

- every generator change would need a matching mapper change
- every export/import path would need dual-format tests
- provenance bugs would become harder to diagnose because the stored model and exported model would
  differ

Using the target model directly keeps Torc's stored entities, auto-generated metadata, and exported
`ro-crate-metadata.json` aligned.

## Core Modifications

### 1. File provenance now uses the PROV-facing shape

Generated file entities now use:

- `@type: ["File", "prov:Entity"]`
- `prov:wasGeneratedBy`
- `prov:wasAttributedTo`
- `prov:wasDerivedFrom`

I removed `torc:run_id` because it was Torc-specific bookkeeping, not a provenance relationship in
the requested model.

### 2. Job provenance is modeled as PROV activities

Generated job entities now use:

- `@type: ["CreateAction", "prov:Activity"]`
- `prov:hadPlan`
- `isPartOf`
- `prov:used`
- `prov:wasAssociatedWith`

This makes job execution records describe both the workflow plan they follow and the inputs they
consume, instead of only pointing at outputs.

### 3. Workflow-level provenance entities were added

Torc now creates:

- `#torc-workflow`
- `#torc-run-{run_id}`

These entities are necessary because the requested model refers to a workflow plan and a workflow
run explicitly. Without them, `prov:hadPlan` and run attribution would point to synthetic IDs that
did not exist as entities.

### 4. Software entities were aligned with the target model

Torc software records now use:

- `@type: ["SoftwareApplication", "prov:SoftwareAgent"]`

That keeps Torc's own binaries compatible with both RO-Crate consumers and the data team's PROV
interpretation.

### 5. Export now preserves the richer stored metadata

The exporter no longer flattens stored metadata back to Torc's older shape. It now:

- preserves stored `@type` arrays
- keeps stored `@id` values when present
- synthesizes `#torc-workflow` and `#torc-run-{run_id}` if older records do not already have them
- adds `localEvidenceGraph`
- emits a `prov` namespace in `@context`

This was important because switching the generators alone would not have been enough. The exported
crate had to look like the data team's example even when some metadata was entered manually or came
from older workflows.

### 6. Workflow export/import remapping still works

The import/export ID remapping logic was updated so job provenance references continue to remap when
entity IDs change. The key case here was switching from `wasGeneratedBy` to
`prov:wasGeneratedBy`.

## Assumptions

These choices were made explicitly:

- file lineage is derived from a job's declared `input_file_ids`
- run attribution should be represented by `#torc-run-{run_id}`
- the current Torc `run_id` is the right identifier to use for workflow-run provenance
- workflow/run provenance entities should be created eagerly during input-file initialization and
  again during output generation so they stay present and current
- software provenance should keep using Torc's existing binary discovery logic instead of adding a
  larger agent-model redesign

## Why I Did Not Add a Mapping Layer

I did not keep the old storage model and export through a conversion layer because that would have
preserved internal semantics the data team explicitly does not want. A mapper would be useful only
if Torc still needed to support both formats as first-class outputs. That was not the assignment's
bias.

## Why I Did Not Change the Database Schema

The database already stores RO-Crate metadata as JSON strings plus a few indexing fields
(`workflow_id`, `file_id`, `entity_id`, `entity_type`). That was already flexible enough for the
new model.

Changing the schema would not have improved provenance quality. It would only have increased risk
and migration cost for no practical gain.

## Validation Status

Validated directly:

- RO-Crate generator unit tests for file entities and CreateAction entities
- workflow export/import unit tests for job-ID remapping
- WSL build for the client/default-feature path

Partially blocked in this worktree:

- full server-feature integration validation
- end-to-end RO-Crate integration tests that require the feature-gated server binary path

Those failures were not caused by the RO-Crate logic itself. This workspace already has unrelated
server-feature build issues and test-harness assumptions about feature-gated binaries.

## Known Follow-Ups

If this needs to be production-hardened further, the next useful follow-ups are:

- run the full server-feature test suite in a clean tree or CI environment
- decide whether workflow plan typing should remain `SoftwareApplication + prov:Plan` or move to a
  more domain-specific plan entity later
- decide whether script-level agents should be auto-generated beyond Torc's own binaries
