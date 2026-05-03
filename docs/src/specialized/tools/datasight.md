# Analyzing Torc Workflows with datasight

[**datasight**](https://github.com/dsgrid/datasight) is an AI-powered data exploration tool that
connects an AI agent to a SQL database (DuckDB, PostgreSQL, SQLite, Flight SQL) and provides a web
UI for asking natural-language questions. The agent writes SQL, runs it, and renders interactive
Plotly visualizations.

A torc server stores its state in **SQLite**, which makes it a natural fit for datasight. This guide
shows how to point datasight at a torc database to answer questions like:

- _Which jobs in workflow 123 exceeded their memory allocation?_
- _Show failures grouped by return code._
- _What's the average exec time per resource group?_
- _Which compute nodes ran the longest jobs?_

> **Read-only tool.** datasight queries the database — it does not mutate it. For changes (rerun,
> recover, update resources) use the regular `torc` CLI commands.

---

## Three Audiences

Setup depends on whether you have direct read access to the torc server's SQLite file.

| Audience                                | Path                                                                                                              |
| --------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| **Admin / shared-server operator**      | Point datasight directly at `dev.db` (or your prod DB path).                                                      |
| **End user — admin will run an export** | Ask the admin to run `torc-server export` to produce a filtered SQLite copy and hand it back. Same IDs preserved. |
| **End user without any admin coop**     | Use `torc workflows export` to get a JSON, import into a local torc-server, then point datasight at _that_ DB.    |

The admin path is one step. The admin-mediated export (Path B) is the right default when you can get
cooperation from whoever runs the server — it preserves the original workflow and job IDs, so log
files and ticket references continue to make sense. Path C is the fallback when no admin help is
available; the JSON round-trip assigns new IDs.

---

## Path A — Admin / Direct DB Access

### 1. Install datasight

```bash
uv tool install datasight
```

Set up an Anthropic API key (or another supported LLM provider — see the datasight README).

### 2. Bootstrap a project from the torc reference files

This repo ships a ready-to-use config under
[`examples/datasight/`](https://github.com/NatLabRockies/torc/tree/main/examples/datasight):

```bash
mkdir ~/torc-analysis
cd ~/torc-analysis
datasight init
cp /path/to/torc/examples/datasight/schema.yaml .
cp /path/to/torc/examples/datasight/schema_description.md .
cp /path/to/torc/examples/datasight/queries.yaml .
```

Edit `.env` to point at the torc SQLite database:

```bash
DATABASE_URL=sqlite:////absolute/path/to/torc/server/db/sqlite/dev.db
LLM_PROVIDER=anthropic
ANTHROPIC_API_KEY=...
```

### 3. Run

```bash
datasight run
```

Open <http://127.0.0.1:8084> and start asking questions. By default datasight binds to localhost;
use `--unix-socket /path/to/datasight.sock` for SSH-forwarded socket access on HPC login nodes.

---

## Path B — Admin-Mediated SQLite Export (recommended for end users)

The admin runs `torc-server export` on the server host to produce a filtered SQLite file containing
only the requested workflows, then sends the file to you. You point datasight at it like a normal
SQLite database.

This preserves all original workflow and job IDs, so the database in your hands is _the same
database_ the production server has — just trimmed to your subset. Log lines like
`workflow_id=42 job_id=917` keep matching, which makes Path A debugging usable for end users.

### 1. Admin runs the export

```bash
# Filter by user (most common)
torc-server export --user alice --output alice.db

# Or by access group
torc-server export --access-group 7 --output proj-energy.db

# Or by specific workflow IDs
torc-server export 42 99 --output requested.db

# Full copy (no filter)
torc-server export --output snapshot.db
```

By default, `access_group`, `workflow_access_group`, and `user_group_membership` are stripped from
filtered exports because those tables span the whole server and would leak entries for other users
and groups. Pass `--preserve-access-groups` only when producing a full copy or when the recipient is
authorized to see the entire access-control state.

The admin should review the output (`sqlite3 alice.db "SELECT id, user, name FROM workflow"`) before
handing it over.

Useful flags:

| Flag                       | Effect                                                                                                      |
| -------------------------- | ----------------------------------------------------------------------------------------------------------- |
| `--overwrite`              | Replace an existing output file.                                                                            |
| `--preserve-access-groups` | Keep ACL tables instead of stripping them. Only safe for full copies or vetted recipients.                  |
| `--no-vacuum`              | Skip the final `VACUUM`; faster, but the file keeps the source's original size even after rows are deleted. |

The export uses SQLite's `VACUUM INTO` for a transactionally consistent snapshot, then deletes
non-matching workflows; cascading foreign keys clean up the per-workflow rows automatically.

### 2. Point datasight at the file

Follow the same setup as Path A, with `DATABASE_URL` pointing at the SQLite file the admin sent you.
No local torc-server is required.

> **Refresh.** The export is a snapshot in time. For new results, ask for a fresh export — there is
> no in-place update.

---

## Path C — User Without Any DB Access

If you can't get an admin to run `torc-server export` for you, fall back to the JSON export/import
flow. This trades ID preservation for not needing any server-side cooperation.

### 1. Get an export with results included

Either run this yourself (if you have CLI access to the shared server) or ask the admin to run it
for you:

```bash
torc workflows export <workflow_id> --include-results --include-events \
  --output workflow_<id>.json
```

The `--include-results` flag is **essential** — without it the `result` table is empty and most of
the useful queries (memory overruns, slow jobs, failure causes) won't work. `--include-events` is
optional but useful for timeline analysis.

See [Exporting and Importing Workflows](../../core/workflows/export-import-workflows.md) for the
full export/import reference.

### 2. Run a personal local torc-server

You only need this for storage; you don't have to actually execute jobs through it. Install torc
locally, then start a server with its own SQLite database:

```bash
torc-server run --host localhost -p 8080
```

In a second shell, point your CLI at it:

```bash
export TORC_API_URL="http://localhost:8080/torc-service/v1"
torc workflows import workflow_<id>.json
```

The import creates a fresh workflow with a new ID in your local DB. Take note of the local DB path
(default `db/sqlite/dev.db`).

### 3. Run datasight against your local DB

Follow the same setup as Path A, with `DATABASE_URL` pointing at your local torc-server's SQLite
file.

> **Note on freshness.** The export is a snapshot. If you want to analyze new results from the
> shared server you need a fresh export. For ongoing monitoring, ask your admin about adding
> read-only access to the production DB or running datasight against it on the server side.

---

## The Reference Files

| File                                                                                                                | Purpose                                                                                                       |
| ------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| [`schema.yaml`](https://github.com/NatLabRockies/torc/blob/main/examples/datasight/schema.yaml)                     | Restricts AI exploration to the analytically useful tables; hides internal scheduling columns.                |
| [`schema_description.md`](https://github.com/NatLabRockies/torc/blob/main/examples/datasight/schema_description.md) | Domain context for the AI: status integer enum, return code conventions, key joins, JSON metadata extraction. |
| [`queries.yaml`](https://github.com/NatLabRockies/torc/blob/main/examples/datasight/queries.yaml)                   | Seeded NL/SQL pairs the AI uses as few-shot examples.                                                         |

The `schema_description.md` is the highest-leverage file — without it, the AI will see raw integer
status codes (0–10) on `job.status` and have no way to decode them, won't know that `137` return
code means OOM, and won't know to use `memory_bytes` instead of the human-readable `memory` string
for math. **Customize it** with anything specific to how your team uses `workflow.metadata` (project
tags, ticket IDs, dataset versions, etc.) — that's where datasight unlocks the most value.

---

## Example Questions to Try

Once datasight is running, try these to verify the integration is working:

- _"How many workflows are in the database, grouped by user?"_
- _"For workflow 123, show jobs whose peak memory exceeded their allocation, sorted by overage."_
- _"Plot exec time distribution for workflow 123, faceted by resource group."_
- _"Which compute nodes had the most failed jobs last week?"_
- _"For my workflows tagged with project='climate-2026' in metadata, summarize total CPU-hours."_

Pin useful results to the dashboard, or export the session as a self-contained HTML page or a
runnable Python script — see the datasight docs for more.

---

## Troubleshooting

**The AI keeps writing `WHERE status = 'failed'`.** Your `schema_description.md` isn't being loaded.
Confirm it sits in the project directory next to `.env` and restart `datasight run`.

**Queries return nothing for `result`-table questions.** Either the workflow hasn't run any jobs
yet, or (Path C) your export was missing `--include-results`.

**`datasight run` complains it can't open the DB.** SQLite requires an absolute path in
`DATABASE_URL` (`sqlite:////absolute/path/...` — note the four slashes).

**Schema introspection is slow.** The torc DB has many internal tables; the included `schema.yaml`
already filters to the analytical subset, which keeps startup fast.
