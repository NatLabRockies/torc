# PR 262 Comment Response Report

> PR: `#262`\
> Title: `AD-324: Switch RO-Crate provenance export to a PROV-shaped model`\
> Branch: `AD-324-ro-create-mods-for-naerm-data-team`\
> Base: `main`\
> URL: <https://github.com/NatLabRockies/torc/pull/262>

## Scope of the PR

Compared with `main`, the branch is primarily a RO-Crate provenance refactor:

- switch generated/exported RO-Crate metadata to a PROV-shaped model
- add synthetic workflow/run provenance entities
- update export/import remapping and tests
- update RO-Crate docs

There are also two unrelated changes in the branch history:

- access-group tutorial/test wording changed from "Data Team" to "Analytics Team"
- `/tmp` was added to `.gitignore`

## What comments exist

- Issue comments on the PR: `0`
- Review threads: `13`
- Review threads marked resolved by GitHub: `0`
- Explicit reply comments in threads: `3`
- Explicit replies from the PR author: `0`

The only written thread responses are from `daniel-thom`:

1. On the server-side hashing comment: "This should be fixed."
2. On the same thread later: "Actually, this is entirely invalid. The server does not have access to
   files. The client provides this information."
3. On the `find_entity_by_entity_id` performance comment: "Tracked by #201 already."

## Response assessment

### 1. Server computes file metadata/hash in async API

- Comment: server-side `std::fs::metadata` and `compute_file_sha256` in `src/server/api/ro_crate.rs`
- Thread response: yes, reviewer follow-up said the code is invalid because the server should not
  read workflow files
- Current branch state: not addressed
- Evidence: `src/server/api/ro_crate.rs` still calls `std::fs::metadata(&file.path)` and
  `compute_file_sha256(&file.path)` in `create_entities_for_input_files`

Assessment: This thread escalated from a performance concern to a correctness/architecture concern.
The current branch still contains the criticized logic.

### 2. Client-side `find_entity_by_entity_id` is O(n) over listed entities

- Comment: `find_entity_by_entity_id` lists entities and scans the page
- Thread response: yes, deferred with "Tracked by #201 already."
- Current branch state: unchanged

Assessment: This received a process response, not a code response. The branch still has the
inefficiency, but there is at least an explicit disposition for it.

### 3. Workflow run `startTime` drifts because updates use `Utc::now()`

- Comment: repeated calls to `create_workflow_provenance_entities` will overwrite the run start time
- Thread response: none
- Current branch state: not addressed
- Evidence: `src/client/ro_crate_utils.rs` still builds the run entity with `Utc::now()` and then
  upserts it

Assessment: Still open. This is a real semantic issue because the code updates workflow provenance
more than once.

### 4. Exported RO-Crate dropped the `torc` namespace from `@context`

- Comment: exported metadata still contains `torc:` keys, but export context only declares `prov`
- Thread response: none
- Current branch state: not addressed
- Evidence: `src/client/commands/ro_crate.rs` exports only the RO-Crate context plus `prov`; stored
  metadata still includes `torc:git_hash` elsewhere

Assessment: Still open. This is the clearest unresolved export-format issue in the PR.

### 5. `attempt_id` appears to be using workflow `run_id`

- Comment: "`job attempt_id != workflow run_id`"
- Thread response: none
- Current branch state: not addressed
- Evidence: `src/client/job_runner.rs` still sets `let attempt_id = self.run_id;`

Assessment: Still open. No follow-up code or thread explanation was added.

### 6. Workflow-level upserts are triggered on each job completion

- Comment: asks why workflow-specific methods are called whenever individual jobs complete
- Thread response: none
- Current branch state: not addressed
- Evidence: `src/client/job_runner.rs` still calls `create_workflow_provenance_entities` and
  `create_software_entities` inside per-job output handling

Assessment: Still open. The likely intended rationale is "ensure referenced entities exist before
writing file/job provenance", but that explanation was not written in the thread and the repeated
upsert behavior contributes to the `startTime` drift issue.

### 7. Server `create_workflow_provenance_entities` appears unused

- Comment: "This function is never called."
- Thread response: none
- Current branch state: not addressed
- Evidence: repository search only finds calls to the client helper, not the server API method

Assessment: Still open. This looks like dead code in the current branch.

### 8. JSON export mode returns too early to include synthesized export metadata

- Comment: returning early for `format == "json"` means the later synthesis logic is skipped
- Thread response: none
- Current branch state: not addressed
- Evidence: `src/client/commands/ro_crate.rs` still returns at the raw-entities path before
  workflow/run synthesis and `@context` assembly

Assessment: Still open. The branch currently has two different "JSON" outputs with different
semantics.

### 9. `localEvidenceGraph` references `provenance-graph.html` with no corresponding file

- Comment: "What is this file?"
- Thread response: none
- Current branch state: not addressed
- Evidence: `src/client/commands/ro_crate.rs` still emits
  `"localEvidenceGraph": { "@id": "provenance-graph.html" }`

Assessment: Still open. The thread is essentially asking for justification or implementation, and
neither appears in the branch.

### 10. Exported run timing spans all runs and dead time

- Comment: using all results may produce confusing run timing
- Thread response: none
- Current branch state: not addressed
- Evidence: `src/client/commands/ro_crate.rs` still uses `.with_all_runs(true)` and computes min/max
  across the result set

Assessment: Still open. The reviewer concern directly matches the current implementation.

### 11. `paginate_results(...).unwrap_or_default()` failure behavior is unclear

- Comment: "What happens on failure?"
- Thread response: none
- Current branch state: partially implicit, but not answered
- Evidence: `src/client/commands/ro_crate.rs` still swallows errors and falls back to no timing data

Assessment: The code path is inferable, but the review question was never answered explicitly. In
practice, export continues silently without run timing.

### 12. Access-groups tutorial change looks unrelated

- Comment: "Why was this file changed?"
- Thread response: none
- Current branch state: unchanged
- Evidence: commit history includes `a67836f8 removing references to data team`

Assessment: This appears intentional but unrelated to the PR’s core provenance work. The branch
contains no written justification in the PR thread.

### 13. Unused `_run_id` parameter should probably be removed

- Comment: asks whether `_run_id` should be removed
- Thread response: none
- Current branch state: not addressed
- Evidence: `src/client/ro_crate_utils.rs` still keeps `_run_id: i64`

Assessment: Still open, though low severity. The underscore suppresses warnings but does not answer
the API cleanliness concern.

## Overall conclusion

The PR has very little actual response activity in GitHub:

- no issue-comment discussion
- no author replies in review threads
- only three written replies total, all from the reviewer side

From a code-status perspective, nearly every substantive thread remains open in the current branch.
The only comment that has a clear recorded disposition is the `find_entity_by_entity_id` performance
concern, which was deferred to issue `#201` rather than fixed here.

The highest-signal unresolved comments are:

1. server-side file access/hash generation in `src/server/api/ro_crate.rs`
2. missing `torc` namespace in exported `@context`
3. workflow run `startTime` drift from repeated upserts
4. misuse of `run_id` as `attempt_id`
5. export-only additions (`localEvidenceGraph`, timing synthesis) that are still underexplained or
   semantically questionable
