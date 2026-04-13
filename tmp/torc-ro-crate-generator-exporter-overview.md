# Changing Torc's Generator/Exporter

Changing Torc’s generator/exporter could be fairly small or fairly invasive, depending on where you want the new format to exist.

## The Short Version

Right now Torc has two layers:

- it **stores RO-Crate entities** in the database as generic records: `entity_id`, `entity_type`, and a JSON `metadata` blob; see [openapi.yaml](/C:/Users/lai25/Documents/torc/api/openapi.yaml#L8610)
- it **exports** those stored records into a final `ro-crate-metadata.json`; see [ro_crate.rs](/C:/Users/lai25/Documents/torc/src/client/commands/ro_crate.rs#L376)

So to change the format, you can do one of these:

1. Change only the **exporter** so Torc keeps storing roughly the same internal records, but writes the data team’s format on export.
2. Change the **generators** too, so the stored records themselves already look like the data team’s provenance model.

## What “exporter-only” would look like

This is the lower-risk path.

Today `handle_export()` builds:

- the `@context`
- the root dataset `./`
- the `hasPart` list
- then appends each stored entity as-is

That logic is in [ro_crate.rs](/C:/Users/lai25/Documents/torc/src/client/commands/ro_crate.rs#L409).

To move toward the example format, you would change `handle_export()` so it:

- adds the `prov` namespace to `@context`
- emits extra top-level entities like `#torc-workflow` and `#torc-run-{run_id}`
- rewrites simple types like `"File"` into arrays like `["File", "prov:Entity"]`
- rewrites fields like `wasGeneratedBy` into `prov:wasGeneratedBy`
- derives extra links like `prov:wasDerivedFrom` by looking at job inputs and outputs
- adds `prov:wasAssociatedWith`, `prov:hadPlan`, `isPartOf`, and similar fields

That would mostly be new logic in [ro_crate.rs](/C:/Users/lai25/Documents/torc/src/client/commands/ro_crate.rs#L376).

The good part is that this likely needs **no schema migration**, because the DB model is already generic.

The catch is that the exporter may need more workflow context than it currently has. The stored RO-Crate entity alone may not tell you enough to infer:

- which input files produced an output
- which script or software agent a job used
- run start/end times for a workflow-level activity

Some of that can be fetched during export from jobs/files/results APIs, but that makes export smarter and more coupled.

## What “change the generators too” would look like

This is the more complete path.

Today the automatic generators create simple metadata in [ro_crate_utils.rs](/C:/Users/lai25/Documents/torc/src/client/ro_crate_utils.rs#L58), [ro_crate_utils.rs](/C:/Users/lai25/Documents/torc/src/client/ro_crate_utils.rs#L115), and [ro_crate_utils.rs](/C:/Users/lai25/Documents/torc/src/client/ro_crate_utils.rs#L183).

Examples of current behavior:

- input file entities are created during initialization; see [workflow_manager.rs](/C:/Users/lai25/Documents/torc/src/client/workflow_manager.rs#L594)
- output file and `CreateAction` entities are created on job completion; see [job_runner.rs](/C:/Users/lai25/Documents/torc/src/client/job_runner.rs#L1000)

To make the stored records match the data team’s model, you would update those builders so they emit richer metadata up front:

- file entities become `["@type": ["File", "prov:Entity"]]`
- output files get `prov:wasGeneratedBy`
- outputs also get `prov:wasDerivedFrom` based on the job’s inputs
- jobs become `["@type": ["CreateAction", "prov:Activity"]]`
- jobs get `prov:used`, `prov:hadPlan`, `prov:wasAssociatedWith`
- software/script entities become `["@type": ["SoftwareApplication", "prov:SoftwareAgent"]]`

You would probably also add creation of workflow/run entities such as:

- `#torc-workflow`
- `#torc-run-{run_id}`

Those do not really belong to a single file or a single job, so you would likely create them either:

- during initialization in [workflow_manager.rs](/C:/Users/lai25/Documents/torc/src/client/workflow_manager.rs#L580), or
- lazily during export in [ro_crate.rs](/C:/Users/lai25/Documents/torc/src/client/commands/ro_crate.rs#L376)

## What I would recommend

I would start with a hybrid:

- keep the DB schema exactly as-is
- enrich the **automatic generators** enough to store the important provenance facts
- keep some purely document-level structure in the **exporter**

Concretely:

- update `build_file_entity`, `build_file_entity_with_provenance`, and `build_create_action_entity` in [ro_crate_utils.rs](/C:/Users/lai25/Documents/torc/src/client/ro_crate_utils.rs#L58)
- update export assembly in [ro_crate.rs](/C:/Users/lai25/Documents/torc/src/client/commands/ro_crate.rs#L376)

That gives you better internal data and still lets export control the final document shape.

## What specific new code would be needed

At a high level, I would expect these additions:

- A helper that converts Torc file/job relationships into `prov:wasDerivedFrom` and `prov:used`.
- A helper that creates workflow-level provenance entities like `#torc-workflow` and `#torc-run-{run_id}`.
- Export-time normalization that:
  - upgrades `@type` from string to array where needed
  - prefixes fields with `prov:` where the data team expects that
  - inserts `localEvidenceGraph` and any other required root-dataset fields from the example

## What might be tricky

The hardest parts are not Rust-specific. They are provenance-model questions:

- Should every Torc job become a `prov:Activity`?
- Should derivation be direct from all job inputs, or only file inputs?
- How should retries map to run/attempt IDs?
- Where do `startTime` and `endTime` come from for workflow-level run entities?

Those decisions matter more than the mechanics.

## Testing impact

You would extend tests like [test_auto_ro_crate.rs](/C:/Users/lai25/Documents/torc/tests/test_auto_ro_crate.rs#L184) so they assert the new fields, especially:

- `prov:wasGeneratedBy`
- `prov:wasDerivedFrom`
- activity/entity type arrays
- workflow/run entities appearing in exported output

If you want, the next useful step is for me to sketch an actual implementation plan by file, from easiest change to hardest.
