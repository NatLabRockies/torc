# API and database changes

The HTTP API contract is owned by Rust and emitted. The checked-in spec and the Rust, Python, and
Julia clients are all downstream artifacts, and CI verifies that they match.

## Contents

- [The generation chain](#the-generation-chain)
- [Changing an endpoint](#changing-an-endpoint)
- [sync_openapi.sh commands](#sync_openapish-commands)
- [Fixing a parity failure](#fixing-a-parity-failure)
- [Database migrations](#database-migrations)
- [sqlx and offline builds](#sqlx-and-offline-builds)

## The generation chain

```text
src/openapi_spec.rs        Rust-owned contract source (edit this)
src/server/live_router.rs  live handlers
src/models.rs              canonical Rust model surface
        |
        | emit
        v
api/openapi.codegen.yaml   emitted spec
        |
        | promote
        v
api/openapi.yaml           checked-in distribution artifact
        |
        | generate
        v
src/client/apis/                          generated Rust request modules
python_client/src/torc/openapi_client/    generated Python client
julia_client/Torc/src/api/                generated Julia client
```

Never hand-edit anything below the first box. `src/models.rs` stays the canonical Rust model layer;
generated Rust API modules are plumbing over it, which is why the repository keeps
`api/openapi-generator-templates/rust/` checked in as a required generation input.

## Changing an endpoint

1. Edit the Rust-owned contract and handler: `src/openapi_spec.rs`, `src/server/live_router.rs`, and
   `src/models.rs` as needed.
2. Emit and verify:

   ```bash
   cd api
   bash sync_openapi.sh emit
   bash sync_openapi.sh check
   ```

3. Promote and regenerate all clients when the contract is final:

   ```bash
   cd api
   bash sync_openapi.sh all --promote
   ```

4. Bump `HTTP_API_VERSION` in `src/api_version.rs` when the contract changed: patch for a fix to an
   existing endpoint, minor for a new endpoint or new optional field or query parameter, major for a
   removal, rename, or semantic change. The client warns on patch/minor drift and blocks on major,
   so skipping this hides a real incompatibility.
5. Test every client:

   ```bash
   cargo nextest run --all-features
   cd python_client && pytest
   julia --project=julia_client/Torc -e "import Pkg; Pkg.test()"
   ```

Server-side checks that reviewers expect on a new endpoint: authorization via the
`authorize_workflow!` / `authorize_resource!` macros before any business logic; correct status codes
(403 unauthorized, 404 missing, 422 validation, 500 server error); and appropriate SQLite indexes
for any new query pattern.

## sync_openapi.sh commands

| Command                                        | Effect                                                |
| ---------------------------------------------- | ----------------------------------------------------- |
| `bash sync_openapi.sh emit`                    | Emit `openapi.codegen.yaml` from Rust only            |
| `bash sync_openapi.sh check`                   | Emit fresh and verify both checked-in specs match     |
| `bash sync_openapi.sh promote`                 | Replace `openapi.yaml` with the Rust-emitted spec     |
| `bash sync_openapi.sh clients`                 | Regenerate clients from the checked-in `openapi.yaml` |
| `bash sync_openapi.sh clients --use-rust-spec` | Regenerate clients from `openapi.codegen.yaml`        |
| `bash sync_openapi.sh all --promote`           | Emit, verify, promote, and regenerate every client    |

Client regeneration runs `openapi-generator` in a container, so `docker` (or `CONTAINER_EXEC`) must
be available. `api/check_client_codegen_parity.sh` pins the generator version and image digest; a
mismatched local generator produces spurious diffs.

## Fixing a parity failure

CI runs two parity gates:

- `api/check_openapi_codegen_parity.sh` compares the checked-in specs against a fresh Rust emit.
- `api/check_client_codegen_parity.sh` compares the checked-in Python and Julia clients against what
  the pinned generator produces from `api/openapi.yaml`.

Reproduce locally before guessing:

```bash
cd api
bash sync_openapi.sh check
bash check_client_codegen_parity.sh
```

Spec drift means the Rust source changed without a promote. Client drift means the spec changed
without regeneration. In both cases the fix is to rerun generation, not to edit the artifact.

## Database migrations

Migrations live in `torc-server/migrations/` and are applied with `sqlx`:

```bash
sqlx migrate add --source torc-server/migrations <migration_name>
# edit the generated .sql file
sqlx migrate run --source torc-server/migrations
sqlx migrate revert --source torc-server/migrations
```

The server runs pending migrations at startup, which is why a fresh standalone run against a new
database works with no setup.

Points to get right:

- **Index new query patterns.** A new column that is filtered or joined needs an index. Weigh the
  memory cost against the benefit rather than indexing reflexively.
- **Foreign key cascades.** Workflow deletion relies on cascades. A new child table must
  participate, or deleting a workflow will leave orphans or fail.
- **Job statuses are integers.** 0 uninitialized, 1 blocked, 2 ready, 3 pending, 4 running, 5
  completed, 6 failed, 7 canceled, 8 terminated, 9 disabled, 10 pending_failed. Write migrations
  against the integers.
- **Concurrency.** SQLite runs in WAL mode with write locks around job claiming. A migration that
  changes claiming-related tables needs the concurrency tests (`test_claim_next_jobs`,
  `test_concurrent_claim_and_complete`) to pass.

## sqlx and offline builds

`sqlx` macros are checked at compile time, so `cargo clippy --all-features` needs either a live
database or the checked-in `.sqlx/` query cache.

```bash
echo "DATABASE_URL=sqlite:torc.db" > .env
cargo install sqlx-cli --no-default-features --features sqlite
sqlx migrate run --source torc-server/migrations
```

After adding or changing a query, refresh the offline cache so builds without a database keep
working, and commit the resulting `.sqlx/` changes with the code. A clippy failure that names a
missing query but no code you touched is usually a stale cache.
