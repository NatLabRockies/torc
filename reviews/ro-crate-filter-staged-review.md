# RO-Crate Filter Staged Review

## Scope

Review covers the currently staged changes only.

Agent assignments:

- Server/API contract/spec: `api/openapi.codegen.yaml`, `api/openapi.yaml`, `src/server/api/ro_crate.rs`, `src/server/api_contract.rs`, `src/server/http_server.rs`, `src/server/http_server/ro_crate_transport.rs`, `src/server/live_router.rs`
- Client/generated clients: `src/client/apis/ro_crate_api.rs`, `src/client/apis/ro_crate_entities_api.rs`, `src/client/commands/pagination/ro_crate_entities.rs`, `src/client/commands/workflows.rs`, `src/client/ro_crate_utils.rs`, `python_client/src/torc/openapi_client/api/ro_crate_entities_api.py`, `julia_client/Torc/src/api/apis/api_RoCrateEntitiesApi.jl`, `julia_client/julia_client/docs/RoCrateEntitiesApi.md`
- Tests/behavioral coverage: `tests/test_ro_crate.rs`, `tests/test_auto_ro_crate.rs`, `tests/test_workflow_export.rs`

## Synthesis

No high-severity correctness defects were identified in the staged diff. The reviewers converged on the main server/client path being internally consistent: the new `file_id` and `entity_id` filters are threaded through the HTTP layer, the server query/bind order looks correct, and the client call sites that should preserve old behavior now pass explicit `None` values.

The main concerns are around staged-diff completeness, implicit uniqueness assumptions, and test strength.

## Findings

### Medium

1. OpenAPI source-of-truth risk

Files:

- [api/openapi.yaml](/home/lai25/torc/api/openapi.yaml)
- [api/openapi.codegen.yaml](/home/lai25/torc/api/openapi.codegen.yaml)
- [src/openapi_spec.rs](/home/lai25/torc/src/openapi_spec.rs)

The staged diff updates the checked-in OpenAPI YAML to add `file_id` and `entity_id`, but there is no staged change in the Rust-owned spec validation/emission source. This may be fine if the YAML was produced from local Rust changes already present elsewhere, but as staged it creates a drift risk and should be verified with the project’s OpenAPI sync/check flow before merge.

2. New Rust `find_*` helpers return the first match without enforcing uniqueness

File:

- [src/client/apis/ro_crate_api.rs](/home/lai25/torc/src/client/apis/ro_crate_api.rs)

`find_ro_crate_entity_by_file_id` and `find_ro_crate_entity_by_entity_id` fetch `limit = 1` and return the first item. That matches prior `.find(...)` behavior, but it codifies a silent "first match wins" policy. If the server ever allows duplicate matches for either key, callers will get an arbitrary row with no signal.

3. Duplicate-file-link test is too weak to prove the intended failure mode

File:

- [tests/test_ro_crate.rs](/home/lai25/torc/tests/test_ro_crate.rs)

`test_ro_crate_rejects_duplicate_workflow_file_link` only asserts `result.is_err()`. Any server failure would satisfy the test, so it does not specifically prove that the duplicate rejection comes from the intended uniqueness behavior or that the API surfaces the right class of error.

4. Transport trait signature change may break out-of-tree implementors

File:

- [src/server/api_contract.rs](/home/lai25/torc/src/server/api_contract.rs)

`TransportApiCore::list_ro_crate_entities` now takes two additional parameters. In-repo implementations were updated, but any external implementor of that trait will need a coordinated update.

### Low

1. Coverage gaps around filter semantics

File:

- [tests/test_ro_crate.rs](/home/lai25/torc/tests/test_ro_crate.rs)

The new filter test covers positive lookups only. It does not cover:

- empty/unknown `file_id` or `entity_id`
- combined `file_id` + `entity_id` filtering
- multiple-match behavior
- pagination metadata such as `has_more`, nonzero `offset`, or truncated `limit`

2. Per-workflow uniqueness semantics are not directly tested

File:

- [tests/test_ro_crate.rs](/home/lai25/torc/tests/test_ro_crate.rs)

There is no companion test proving that the same `file_id` may still appear in a different workflow. The staged tests therefore do not directly pin the intended scope of uniqueness as "per workflow" rather than global.

3. Cross-language convenience API parity is incomplete

Files:

- [src/client/apis/ro_crate_api.rs](/home/lai25/torc/src/client/apis/ro_crate_api.rs)
- [python_client/src/torc/openapi_client/api/ro_crate_entities_api.py](/home/lai25/torc/python_client/src/torc/openapi_client/api/ro_crate_entities_api.py)
- [julia_client/Torc/src/api/apis/api_RoCrateEntitiesApi.jl](/home/lai25/torc/julia_client/Torc/src/api/apis/api_RoCrateEntitiesApi.jl)

Rust adds `find_*` convenience helpers, while Python and Julia expose only the raw filtered list API. Endpoint parity is present; helper-level parity is not.

4. Count query lost SQLx macro-time checking

File:

- [src/server/api/ro_crate.rs](/home/lai25/torc/src/server/api/ro_crate.rs)

The count path moved from `query!` to dynamic `sqlx::query(...)` so it can share the same optional filters. That is reasonable, but it removes compile-time SQL validation for that query.

## Clean Checks

The reviewers found these parts of the staged change to be sound:

- Server filter plumbing is consistent from router to transport to query execution.
- Query parameter binding order matches the generated SQL placeholders.
- Existing call sites that should preserve old behavior now pass explicit `None` values for the new filters.
- Python and Julia staged client changes appear structurally consistent with the new endpoint parameters.

## Suggested Follow-Up Before Merge

1. Verify the OpenAPI source-of-truth flow on the staged tree so the YAML changes are not drifting from Rust-owned spec generation.
2. Strengthen `test_ro_crate_rejects_duplicate_workflow_file_link` to assert the specific failure semantics you want.
3. Add at least one negative/empty filter test and one pagination-metadata test for the new filtered list path.
4. Decide whether the silent "first match wins" behavior in the new Rust helpers is an acceptable long-term contract.
