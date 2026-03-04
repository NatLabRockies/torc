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

### OpenAPI Spec (api/openapi.yaml)

3. **Endpoint Documentation**: All new/modified endpoints are documented
   - Path, method, parameters, request body
   - Response schemas for 200, 403, 404, 422, 500
   - Validate with: `npx @redocly/cli lint api/openapi.yaml`

4. **Schema Syntax** (OpenAPI 3.1):
   - Use `type: [integer, "null"]` not `nullable: true`
   - Use direct `$ref:` without `schema:` wrapper
   - Examples use actual objects: `error: {}` not `error: "{}"`

### Client CLI (src/client/commands/)

5. **Workflow ID Prompting**: Commands use `Option<i64>` for workflow_id
   - Call `select_workflow_interactively()` when None
   - Follow pattern in `jobs.rs` or `ro_crate.rs`

6. **Pagination**: List commands that fetch from server use pagination
   - Check for `paginate_*` functions or pagination iterators
   - Single API calls with no limit will truncate at 10,000 items

7. **JSON Output**: Commands support `-f json` output format
   - Use `print_if_json()` or `print_wrapped_if_json()`
   - Ensure structured data is serializable

8. **Logging**: Check that log messages that include database record IDs use a format like
   `info!("Created workflow workflow_id={}", workflow_id);` so that parsing scripts pick them up.

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
