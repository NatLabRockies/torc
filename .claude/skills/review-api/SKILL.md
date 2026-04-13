---
name: review-api
description: Review new or modified API features for completeness and consistency
user-invocable: true
---

# API Feature Review

Review new or modified API features for completeness and consistency.

## Checklist

### Server-Side (src/server/)

1. **Authorization**: All API endpoints use `authorize_workflow!` or `authorize_resource!` macros
   - Check `src/server/http_server.rs` for the endpoint handler
   - Verify the macro is called before any business logic

2. **Error Responses**: Proper HTTP status codes are returned
   - 403 for unauthorized access
   - 404 for not found resources
   - 422 for validation errors
   - 500 for server errors

3. **SQLite indexes**: Ensure changes to SQLite tables are paired with appropriate indexes. If the
   branch changes any SQLite tables, check what indexes exist for those tables. Report to the
   developer whether you would recommend any changes. Be careful recommending indexes that would
   consume lots of memory with minimal user value.

### OpenAPI Spec (api/openapi.yaml)

4. **Endpoint Documentation**: All new/modified endpoints are documented
   - Path, method, parameters, request body
   - Response schemas for 200, 403, 404, 422, 500
   - Validate with: `npx @redocly/cli lint api/openapi.yaml`

5. **Schema Syntax** (OpenAPI 3.1):
   - Use `type: [integer, "null"]` not `nullable: true`
   - Use direct `$ref:` without `schema:` wrapper
   - Examples use actual objects: `error: {}` not `error: "{}"`

### Client CLI (src/client/commands/)

1. **Workflow ID Prompting**: Commands use `Option<i64>` for workflow_id
   - Call `select_workflow_interactively()` when None
   - Follow pattern in `jobs.rs` or `ro_crate.rs`

2. **Pagination**: List commands that fetch from server use pagination
   - Check for `paginate_*` functions or pagination iterators
   - Single API calls with no limit will truncate at 10,000 items

3. **JSON Output**: Commands support `-f json` output format
   - Use `print_if_json()` or `print_wrapped_if_json()`
   - Ensure structured data is serializable

4. **Logging**: Check that log messages that include database record IDs use a format like
   `info!("Created workflow workflow_id={}", workflow_id);` so that parsing scripts pick them up.

### Performance

1. Review all changes for performance.
2. Look for excessive invocations of CLI commands as subprocesses.
3. Look for problematic API calls from client to the server. Batching is preferred where possible.
4. Check SQL queries on the server for unnecessary joins.

### Code Quality

1. Check the changes for duplication and flag instances.
2. Check functions for being too long and complicated.

## How to Review

1. Identify all files changed in the feature
2. For each server endpoint: check authorization and error responses
3. For each CLI command: check prompting, pagination, JSON support
4. Verify OpenAPI spec matches implementation
5. Run linters: `cargo clippy`, `npx @redocly/cli lint api/openapi.yaml`

## Example Usage

```
/review-api

# Or with context:
Review the RO-Crate feature for API completeness
```
