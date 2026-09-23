# Connectivity and outages

Every Torc component reaches the server over HTTP. Most "Torc is broken" reports are a URL, a bind
address, or a firewall.

## Contents

- [Diagnose the connection](#diagnose-the-connection)
- [URL rules](#url-rules)
- [Bind address](#bind-address)
- [Authentication and TLS failures](#authentication-and-tls-failures)
- [Version mismatches](#version-mismatches)
- [Server outages and offline drain](#server-outages-and-offline-drain)

## Diagnose the connection

```bash
torc ping                                        # uses the configured URL
torc --url http://host:8080/torc-service/v1 ping # test an explicit URL
torc config show | grep api_url                   # what the client will actually use
```

`torc ping` prints `Server is running` and exits 0 on success, non-zero otherwise. Failure means the
URL, the network path, or the server itself; it says nothing about workflows.

When the CLI works but a worker does not, test from the worker's own context, not yours:

```bash
ssh worker1 "curl -sf http://server:8080/torc-service/v1/ping" && echo reachable
```

## URL rules

The URL must include the API base path `/torc-service/v1`. A bare `http://host:8080` will not work.

Precedence: `--url` flag, then `TORC_API_URL`, then `client.api_url` from a config file, then the
built-in default `http://localhost:8080/torc-service/v1`.

`localhost` is only correct when the server runs on the same machine as the component using it.
Common failures that follow from ignoring this:

- Remote workers pointed at `localhost` connect to themselves and die immediately.
- A standalone server (`-s`) binds `127.0.0.1` on an auto-assigned port, so it can never serve other
  machines.
- On HPC, compute nodes may need a different hostname than the login node uses; verify from inside
  an allocation before blaming payloads.

## Bind address

`torc-server run --host <value>` controls which interfaces accept connections.

| Value        | Reachable from        | Use for                        |
| ------------ | --------------------- | ------------------------------ |
| `127.0.0.1`  | The same machine only | Local development              |
| `0.0.0.0`    | Every interface       | Remote workers, compute nodes  |
| `<ip>`       | That interface        | Multi-homed hosts              |
| `<hostname>` | That name's interface | HPC with non-default hostnames |

A server bound to `127.0.0.1` cannot serve remote workers no matter what URL they use. Other useful
flags: `--port`, `--threads`, `--database`, plus auth and TLS options.

## Authentication and TLS failures

| Symptom                                  | Cause and fix                                                                                                                                                                                                                                                                   |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 401 Unauthorized                         | Server has `require_auth`; set `TORC_PASSWORD` or `--password`                                                                                                                                                                                                                  |
| 403 Forbidden                            | Access control on and the workflow is not yours; check access groups                                                                                                                                                                                                            |
| Certificate verification failure         | Pass the internal CA with `--tls-ca-cert` / `TORC_TLS_CA_CERT`                                                                                                                                                                                                                  |
| Works with `--tls-insecure` only         | Certificate or CA trust is genuinely wrong; fix the CA, do not ship insecure                                                                                                                                                                                                    |
| Auth works interactively, fails in a job | The job's execution environment has no credentials. Provide them from outside the spec: export `TORC_PASSWORD` in the shell or batch environment that launches the runner, or read it from a user-only-readable file in an `invocation_script`. **Never put a secret in `env`** |

Workflow and job `env` maps are stored in the database and returned by every API read of the job:
`torc jobs get`, any `-f json` output, `torc workflows export`, the dashboard, and the MCP tools.
Anyone who can read the workflow can read the values, and they land in every job's process
environment. Secrets do not belong there.

The username comes from `TORC_USERNAME`, falling back to `USER`/`USERNAME`, and determines workflow
ownership. A mismatch is why `torc workflows list` can look empty while the workflow exists: it
filters by user. Use `-a/--all-users` to confirm.

After changing the server's htpasswd file, `torc admin reload-auth` reloads it without a restart.

## Version mismatches

The HTTP API has its own semver, separate from the binary version. The client compares them and
warns on patch or minor drift; a major difference is blocking.

The runner's startup log line records `version`, `client_api_version`, `server_version`, and
`server_api_version`, which is the fastest way to see a mismatch on a compute node.

`--skip-version-check` suppresses the check, and `torc remote run --skip-version-check` does the
same per worker. Both are diagnosis aids. Fix the installation: mismatched versions produce
confusing, partly-working behavior rather than clean failures.

## Server outages and offline drain

When a runner cannot reach the server for longer than the workflow's
`compute_node_wait_for_healthy_database_minutes`, it drains instead of killing work: it stops
claiming new jobs, lets running jobs finish, and journals their results to a local SQLite file under
`<output-dir>/offline_journal/offline_results_wf<id>_r<run>_<label>.db`.

If the server returns while jobs are still running, the runner flushes the journal and resumes. If
not, it exits once the running jobs finish, leaving the journal behind. The completed work is real
but the server does not know about it yet.

```bash
torc workflows reconcile <workflow_id> <run_id>
torc workflows reconcile <workflow_id> <run_id> --base-dir /scratch/run42
```

`reconcile` searches recursively for every journal belonging to that workflow and run and uploads
the completions in batches. Point `--base-dir` at the shared output root when several compute nodes
wrote journals. Run it only when the server is healthy; it reports and stops otherwise.

Behavior is controlled by `[client.offline]`: `enabled` (default true) and
`drain_ping_interval_secs` (default 120). With `enabled = false` the runner kills running jobs and
exits instead of draining.

Symptoms that point here: jobs that clearly ran but have no results, a runner log showing drain
messages, or `offline_journal/*.db` files present after a run.
