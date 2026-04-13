# AD-324 Linux Follow-Up Prompt

You are picking up work on the `torc` repository after an RO-Crate provenance model change was
implemented and pushed on branch `AD-324-ro-create-mods-for-naerm-data-team`.

## Context

The goal of the branch is to make Torc use the data team's PROV-shaped RO-Crate format as the
canonical generator/exporter model, even though that is a breaking change.

The branch already contains the implementation and documentation updates. Your job is not to
re-design it from scratch. Your job is to validate it cleanly on Linux, fix any remaining issues,
and leave the branch in a review-ready state.

## What Has Already Been Changed

Key model changes already implemented:

- file entities now use `@type: ["File", "prov:Entity"]`
- job entities now use `@type: ["CreateAction", "prov:Activity"]`
- software entities now use
  `@type: ["SoftwareApplication", "prov:SoftwareAgent"]`
- output provenance uses `prov:wasGeneratedBy`
- generated file metadata no longer uses `torc:run_id`
- output files now also use `prov:wasAttributedTo`
- output files now also use `prov:wasDerivedFrom` based on declared file inputs
- workflow-level provenance entities are created:
  - `#torc-workflow`
  - `#torc-run-{run_id}`
- exporter now:
  - preserves stored `@id` and `@type` where present
  - adds `localEvidenceGraph`
  - includes the `prov` namespace in `@context`
  - synthesizes workflow/run entities if older metadata does not already include them
- import/export remapping logic was updated for `prov:wasGeneratedBy`

Primary files touched:

- `src/client/ro_crate_utils.rs`
- `src/client/workflow_manager.rs`
- `src/client/job_runner.rs`
- `src/client/commands/ro_crate.rs`
- `src/client/commands/workflow_export.rs`
- `src/server/api/ro_crate.rs`
- `src/server/http_server.rs`
- `tests/test_auto_ro_crate.rs`
- `tests/test_workflow_export.rs`
- `docs/src/core/concepts/ro-crate.md`
- `docs/src/core/how-to/ro-crate-metadata.md`
- `tmp/torc-ro-crate-change-rationale.md`

## Important Local History

The prior implementation environment was a dirty Windows-mounted worktree plus WSL. That caused
several misleading failures unrelated to the RO-Crate logic itself.

Known environment-specific blockers from the prior session:

- Windows git hooks could not run `cargo` from PATH
- integration tests initially failed because WSL did not have `sqlx-cli`
- integration tests then failed because the harness expects feature-gated binaries like
  `target/debug/torc-slurm-job-runner`
- full server-feature builds in that dirty worktree hit unrelated pre-existing compile problems

Because of that, only targeted unit validation was completed there.

## Validation Already Completed

These passed:

- `cargo test test_build_file_entity_basic --lib`
- `cargo test test_build_create_action_entity --lib`
- `cargo test test_build_file_entity_with_provenance --lib`
- `cargo test remap_ro_crate_job_ids --lib`

## What You Need To Do

Work on Linux in a clean or at least sane environment after checking out the branch.

### 1. Inspect the branch state

Run at least:

```bash
git status
git log --oneline -n 5
git show --stat HEAD
```

### 2. Run the real project checks

The repo instructions say changes should pass:

```bash
cargo fmt -- --check
cargo clippy --all --all-targets --all-features -- -D warnings
dprint check
```

If these fail, fix only issues relevant to this branch unless you discover a genuine blocker that
must be addressed to make the branch buildable.

### 3. Run the important RO-Crate tests

At minimum, run:

```bash
cargo test test_build_file_entity_basic --lib
cargo test test_build_create_action_entity --lib
cargo test test_build_file_entity_with_provenance --lib
cargo test remap_ro_crate_job_ids --lib
cargo test --test test_auto_ro_crate -- --nocapture
cargo test --test test_workflow_export -- --nocapture
```

If the integration tests require setup such as `sqlx-cli`, install whatever is needed in the Linux
environment rather than working around it.

### 4. Pay special attention to these risk areas

- exporter behavior in `src/client/commands/ro_crate.rs`
- workflow/run synthetic entities in export output
- provenance relationships:
  - `prov:wasGeneratedBy`
  - `prov:wasAttributedTo`
  - `prov:wasDerivedFrom`
  - `prov:hadPlan`
  - `prov:used`
  - `prov:wasAssociatedWith`
- server-side creation of workflow provenance entities in
  `src/server/api/ro_crate.rs`
- auto-generated RO-Crate entities across multiple runs
- import/export remapping for job IDs in RO-Crate metadata

### 5. Confirm the docs still match behavior

Review:

- `docs/src/core/concepts/ro-crate.md`
- `docs/src/core/how-to/ro-crate-metadata.md`
- `tmp/torc-ro-crate-change-rationale.md`

If implementation changes are needed, update the docs too.

### 6. If you make fixes, keep the existing design direction

Do not revert the model back to old Torc-specific fields unless you find a hard requirement that
the new model cannot satisfy.

The intended direction is:

- one canonical stored/exported model
- data team format favored over the older Torc-specific shape
- breaking change acceptable

## Design Intent Behind the Existing Implementation

The branch deliberately chose not to add a mapping layer. The reasoning was:

- the assignment explicitly allowed a breaking change
- a mapper would create two provenance models to maintain
- direct generation/export in the target model is simpler and more defensible

The branch also deliberately did not change the RO-Crate database schema, because metadata is
already stored flexibly as JSON plus identifiers.

## One Specific Implementation Detail To Note

The new server-side workflow provenance helper in `src/server/api/ro_crate.rs` was converted away
from new `sqlx` compile-time macros and toward runtime `sqlx::query(...).bind(...)` calls. That was
done to avoid requiring `.sqlx` cache regeneration just for the new RO-Crate queries.

Do not casually revert that unless you also intentionally regenerate and commit the proper SQLx
cache changes.

## Desired End State

When you are done, I want:

1. a clean summary of what passed
2. a clean summary of what failed, if anything
3. fixes committed if needed
4. confidence that the branch is actually reviewable on Linux

If you encounter failures, distinguish clearly between:

- problems caused by this RO-Crate branch
- unrelated repository/environment problems

## Useful Output Style

Please report back with:

- `Passed`
- `Failed`
- `Fixed`
- `Open Risks`

Keep the answer concrete and reference exact files when relevant.
