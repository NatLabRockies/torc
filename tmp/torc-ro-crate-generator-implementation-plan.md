# Torc RO-Crate Generator Implementation Plan

This plan focuses on changing Torc's automatic RO-Crate generators so the stored provenance data matches the data team's desired format as the new canonical model, even when that is a breaking change.

## Goal

Modify Torc so that automatic RO-Crate generation:

- emits richer PROV-style metadata for files, jobs, software, and workflow runs
- stores that metadata in existing `ro_crate_entity` records
- preserves current workflow behavior outside RO-Crate generation
- treats the data team's format as the new canonical provenance model
- updates exporter, tests, and docs to follow that model directly

This plan does not require a schema migration unless current tables do not expose enough information for the desired provenance graph. If a migration is the cleanest way to follow the target model, it should be preferred over compatibility workarounds.

## Current State

The main automatic generation paths are:

- input file entity creation during workflow initialization in [workflow_manager.rs](/C:/Users/lai25/Documents/torc/src/client/workflow_manager.rs#L594)
- output file and job provenance creation during job completion in [job_runner.rs](/C:/Users/lai25/Documents/torc/src/client/job_runner.rs#L1000)
- metadata builders in [ro_crate_utils.rs](/C:/Users/lai25/Documents/torc/src/client/ro_crate_utils.rs#L58)

The stored model is generic enough already:

- `workflow_id`
- `file_id`
- `entity_id`
- `entity_type`
- `metadata` JSON string

See [openapi.yaml](/C:/Users/lai25/Documents/torc/api/openapi.yaml#L8610).

## Target State

Automatic entities should move toward the example format in [modified-torc-prov.json](/C:/Users/lai25/Documents/torc/tmp/modified-torc-prov.json#L1).

At minimum:

- file entities use `@type: ["File", "prov:Entity"]`
- output files use `prov:wasGeneratedBy`
- output files include `prov:wasDerivedFrom` based on file inputs
- job entities use `@type: ["CreateAction", "prov:Activity"]`
- job entities include `prov:used`, `prov:hadPlan`, `prov:wasAssociatedWith`
- software entities use `@type: ["SoftwareApplication", "prov:SoftwareAgent"]`
- workflow-level provenance entities exist for the workflow plan and run activity

## Breaking-Change Posture

The assignment explicitly allows breaking changes as long as Torc follows the desired model. Because of that, this plan should prefer:

- replacing the old generated metadata shape rather than supporting both shapes
- changing exporter behavior to emit the new document shape directly
- updating tests and docs to the new model rather than preserving old assertions
- changing import/export behavior where needed instead of carrying translation shims inside Torc

The main thing that should still be preserved is operational workflow behavior outside provenance generation.

## Non-Goals

- changing the `ro_crate_entity` table shape
- supporting both the old generated model and the new generated model indefinitely
- fully redesigning manual `torc ro-crate create` semantics
- implementing every possible PROV relation on day one
- preserving backward compatibility for old generated RO-Crate output

## Phase 0: Provenance Decisions

Before coding, lock down these decisions:

1. Whether Torc should store unprefixed fields, `prov:` fields, or both.
2. Whether all derivation should be based only on file inputs, or also user data.
3. Whether `attempt_id` should continue to equal `run_id`, or whether retries need separate provenance identifiers.
4. Whether workflow run entities should be created eagerly during execution or synthesized later during export.
5. Whether script/program entities are expected to be created automatically or only when manually registered.

Recommended default for a breaking-change implementation:

- use `prov:` fields directly in generated metadata as the canonical output
- derive from file inputs only for the first pass
- preserve current `#job-{job_id}-attempt-{attempt_id}` IDs for now
- create workflow-level entities automatically
- keep script-specific entities manual unless there is already reliable source data for them

One additional decision should be made early:

6. Whether `entity_type` in `ro_crate_entity` should continue to mirror a single coarse type like `"File"` for filtering, even though generated metadata will use `@type` arrays.

## Phase 1: Introduce Metadata Builders for the New Shape

Primary file: [ro_crate_utils.rs](/C:/Users/lai25/Documents/torc/src/client/ro_crate_utils.rs)

### Changes

- add a small internal helper for building RO-Crate/PROV `@type` arrays
- add a helper for building `prov` references such as `{ "@id": "..." }`
- update file metadata builders to emit PROV-style fields
- update software metadata builder to emit `prov:SoftwareAgent`
- update job metadata builder to emit `prov:Activity`

### Specific edits

- change `build_file_entity()` to emit:
  - `@type: ["File", "prov:Entity"]`
  - `contentSize`, `dateModified`, `encodingFormat`, `sha256`
  - no `torc:run_id` if the target format should be pure PROV-facing metadata
- change `build_file_entity_with_provenance()` to emit:
  - `prov:wasGeneratedBy`
  - optionally `prov:wasAttributedTo` if a workflow run entity is introduced
- change `build_create_action_entity()` to emit:
  - `@type: ["CreateAction", "prov:Activity"]`
  - `prov:hadPlan`
  - `prov:used`
  - `prov:wasAssociatedWith`
  - `result`
- change `build_software_entity()` to emit:
  - `@type: ["SoftwareApplication", "prov:SoftwareAgent"]`

### Deliverable

Existing builders remain, but produce the new metadata structure.

## Phase 2: Add Workflow-Level Entities

Primary files:

- [ro_crate_utils.rs](/C:/Users/lai25/Documents/torc/src/client/ro_crate_utils.rs)
- [workflow_manager.rs](/C:/Users/lai25/Documents/torc/src/client/workflow_manager.rs#L580)

### Changes

Add builders and creation functions for:

- workflow plan entity, for example `#torc-workflow`
- workflow run activity entity, for example `#torc-run-{run_id}`

### Suggested metadata

Workflow plan entity:

- `@id: "#torc-workflow"`
- `@type: ["SoftwareApplication", "prov:Plan"]`
- `name: workflow.name` or `"torc"`

Workflow run entity:

- `@id: "#torc-run-{run_id}"`
- `@type: ["CreateAction", "prov:Activity"]`
- `prov:hadPlan: { "@id": "#torc-workflow" }`
- `prov:wasAssociatedWith` references to Torc software entities
- `startTime`
- `endTime` if known at generation time

### Execution point

Recommended first implementation:

- create the workflow plan entity during initialization
- create the workflow run entity during initialization with `startTime`
- update or replace the workflow run entity when the workflow completes so `endTime` is filled in

### Deliverable

Automatic generation includes workflow-level provenance anchors, so file and job entities can point to them.

## Phase 3: Enrich Input File Generation

Primary files:

- [workflow_manager.rs](/C:/Users/lai25/Documents/torc/src/client/workflow_manager.rs#L615)
- [ro_crate_utils.rs](/C:/Users/lai25/Documents/torc/src/client/ro_crate_utils.rs#L446)

### Changes

- continue treating `st_mtime.is_some()` as the signal that a file already exists and is an input
- ensure input file entities are created with new `@type` array format
- decide whether input files should also carry `prov:wasAttributedTo` to the workflow run

### Notes

This phase should be mostly mechanical after Phase 1.

### Deliverable

Input file entities are in the new shape and remain upsert-safe across reinitialization.

## Phase 4: Enrich Output File Generation with Derivation

Primary files:

- [job_runner.rs](/C:/Users/lai25/Documents/torc/src/client/job_runner.rs#L1000)
- [ro_crate_utils.rs](/C:/Users/lai25/Documents/torc/src/client/ro_crate_utils.rs#L332)

### Changes

When a job completes:

- fetch the job's input files
- compute `prov:wasDerivedFrom` for each output file from those input file paths
- set `prov:wasGeneratedBy`
- optionally set `prov:wasAttributedTo` to the workflow run entity

### New helper likely needed

Add a helper that takes:

- job model
- input file models
- output file model
- run information

and returns enriched output file metadata.

### Notes

This is the first phase that likely needs extra API lookups inside job completion handling if the needed input file paths are not already in memory.

### Deliverable

Output files carry direct derivation provenance rather than only producer linkage.

## Phase 5: Enrich Job Activity Generation

Primary files:

- [job_runner.rs](/C:/Users/lai25/Documents/torc/src/client/job_runner.rs#L1000)
- [ro_crate_utils.rs](/C:/Users/lai25/Documents/torc/src/client/ro_crate_utils.rs#L183)

### Changes

When creating a job `CreateAction` entity:

- include `prov:hadPlan` referencing the workflow plan
- include `isPartOf` referencing the workflow run
- include `prov:used` based on input files
- include `object` if the data team format expects both `object` and `prov:used`
- include `result` based on output files
- include `prov:wasAssociatedWith` referencing software/script agent if known

### Open question

Torc may not reliably know the script identity for every job from structured metadata alone. If the script entity is not known:

- either omit `instrument` / `prov:wasAssociatedWith`
- or associate the job with the Torc software/run entity only

Recommended first pass:

- always associate with the workflow run or Torc software entity
- leave job-script association optional

### Deliverable

Job activity entities become the center of the provenance graph rather than thin result wrappers.

## Phase 6: Software Entity Alignment

Primary files:

- [ro_crate_utils.rs](/C:/Users/lai25/Documents/torc/src/client/ro_crate_utils.rs#L531)
- server-side torc-server entity code if needed

### Changes

- update automatically created Torc software entities to use the new `@type` shape
- decide whether to keep `url`, `version`, `contentSize`, and `sha256` exactly as-is
- ensure workflow run entities can reference them cleanly via `prov:wasAssociatedWith`

### Notes

The client already creates `torc` entities and the server creates `torc-server` entities. They should be aligned so they look like the same provenance family.

## Phase 7: Export/Import Rewrite for the New Canonical Shape

Primary files:

- [ro_crate.rs](/C:/Users/lai25/Documents/torc/src/client/commands/ro_crate.rs#L376)
- workflow import/export code if needed

### Changes

Even though this plan targets generators, Phase 7 should make export/import follow the new canonical model:

- export serializes the new metadata without losing fields
- `@id` / `@type` overriding logic in export is rewritten for the new model
- workflow import/export remapping logic still works with new `prov:` fields in `metadata`

### Potential issue

Current export logic overwrites `@type` from `entity.entity_type` with a scalar string. That conflicts with the desired model and should be removed or redesigned.

This means Phase 7 requires an exporter change:

- if metadata already contains canonical `@type`, preserve it
- use `entity_type` only as an internal coarse classification if it still has value

### Deliverable

Generated metadata survives export/import in the new canonical shape.

## Phase 8: Tests

Primary files:

- [test_auto_ro_crate.rs](/C:/Users/lai25/Documents/torc/tests/test_auto_ro_crate.rs#L184)
- unit tests in [ro_crate_utils.rs](/C:/Users/lai25/Documents/torc/src/client/ro_crate_utils.rs#L628)
- export-related tests if present

### Add or update unit tests

- file builder emits `@type` arrays
- output file builder emits `prov:wasGeneratedBy`
- output file builder emits `prov:wasDerivedFrom`
- job builder emits `prov:used`, `prov:hadPlan`, `result`
- software builder emits `prov:SoftwareAgent`

### Add or update integration tests

- initialization creates enriched input file entities
- job completion creates enriched output and activity entities
- second run replaces file entities without duplicating them
- export preserves array `@type` and `prov:` fields

### Add one golden test

Recommended:

- create a small workflow like the diamond example
- export the RO-Crate
- compare the normalized JSON structure against a checked-in expected fixture

## Phase 9: Documentation

Primary files:

- [ro-crate.md](/C:/Users/lai25/Documents/torc/docs/src/core/concepts/ro-crate.md)
- [ro-crate-metadata.md](/C:/Users/lai25/Documents/torc/docs/src/core/how-to/ro-crate-metadata.md)
- workflow spec docs if behavior wording changes

### Changes

- update examples to show `prov:` fields
- explain that Torc now emits the data team's PROV-enriched RO-Crate metadata by default
- note the breaking change in entity shape and export structure

## Suggested Implementation Order

1. Phase 0: lock decisions and canonical target shape.
2. Phase 1: update builders in `ro_crate_utils.rs`.
3. Phase 2: add workflow-level entity creation.
4. Phase 3: update input file generation.
5. Phase 4: add output derivation metadata.
6. Phase 5: enrich job activity metadata.
7. Phase 6: align software entities.
8. Phase 7: rewrite exporter assumptions that encode the old model.
9. Phase 8: update tests to assert only the new shape.
10. Phase 9: update docs and explicitly document the breaking change.

## Risks

- exporter currently assumes scalar `@type`
- some desired provenance fields may require extra runtime lookups
- retry/run semantics may not cleanly match the example format
- job-to-script provenance may be incomplete without more structured job metadata
- imported historical RO-Crate data may need one-time normalization if old and new shapes must coexist in the same export path

## Recommended First Patch Set

If the work should be split into the smallest useful PR-sized chunk, start with:

1. update metadata builders in `ro_crate_utils.rs`
2. add workflow-level entities
3. add output `prov:wasDerivedFrom`
4. patch exporter so it preserves the canonical new shape
5. update `test_auto_ro_crate.rs`

That would establish the new model quickly and make later document polish incremental rather than architectural.
