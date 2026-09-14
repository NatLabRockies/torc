# Testing

## Contents

- [Running tests](#running-tests)
- [Fixtures](#fixtures)
- [Serialization](#serialization)
- [CLI-level tests](#cli-level-tests)
- [Test conventions](#test-conventions)
- [Slurm and remote coverage](#slurm-and-remote-coverage)
- [Python and Julia clients](#python-and-julia-clients)

## Running tests

```bash
cargo nextest run --all-features                       # full suite
cargo nextest run -E 'test(test_get_ready_jobs)'        # one test
cargo nextest run -E 'binary_id(torc::test_workflows)'  # one binary
RUST_LOG=debug cargo nextest run -E 'test(test_name)'   # with logs
```

`--all-features` matters: server, dashboard, MCP, and Slurm-runner code is feature-gated and will
not compile or run otherwise.

Integration tests build the `torc` and `torc-server` binaries on first use through a `#[once]`
fixture, so the first test in a run is slow. `.config/nextest.toml` raises the slow timeout to 120
seconds with `terminate-after = 3` for exactly this reason. A first-run timeout usually means the
build was slower than expected, not a hung test; run `cargo build --all-features` first to separate
the two.

Unit tests live beside their code in `#[cfg(test)] mod tests`; integration tests live in `tests/`
with shared helpers in `tests/common.rs`.

## Fixtures

Tests use `rstest`. Server fixtures are `#[once]`, so one server process is shared across the tests
in a binary:

```rust
mod common;

use common::{ServerProcess, create_test_workflow, run_cli_with_json, start_server};
use rstest::rstest;

#[rstest]
fn test_workflow_creation(start_server: &ServerProcess) {
    let config = &start_server.config;
    // exercise the API or CLI against this server
}
```

Available fixtures in `tests/common.rs`:

| Fixture                            | Provides                                               |
| ---------------------------------- | ------------------------------------------------------ |
| `start_server`                     | Server on a temporary SQLite database, no auth         |
| `start_server_with_required_auth`  | Server with `require_auth` and a seeded htpasswd file  |
| `start_server_with_access_control` | Server with access control and team/group users seeded |

Because `#[once]` fixtures live in statics, `Drop` does not run for them; server processes are
tracked and terminated at process exit. Do not rely on `Drop` for cleanup of anything a `#[once]`
fixture owns.

Shared state has a consequence for test design: tests in the same binary see each other's workflows.
Create your own workflow per test with the `create_test_*` helpers and assert on your own IDs rather
than on global counts.

Parameterized tests use `#[case]`:

```rust
#[rstest]
#[case(0, "immediate")]
#[case(3600, "one_hour")]
fn test_timeout_handling(#[case] timeout_secs: u64, #[case] description: &str) {
    // runs once per case
}
```

## Serialization

Tests that mutate shared state (ports, PATH, mock executables, a shared server's connection pool)
are serialized two ways:

1. `#[serial]` from `serial_test` in the test source.
2. Test groups in `.config/nextest.toml`, because nextest runs each binary in its own process and
   `#[serial]` alone does not coordinate across processes.

Existing groups: `serial-slurm`, `serial-entities`, `serial-workflows`, `serial-auth`,
`serial-jobs`, `serial-ro-crate`, `serial-compute` (all `max-threads = 1`), and `workflow-actions`
(`max-threads = 4`).

Adding a test binary that shares state means adding it to the right group's `filter`. Skipping that
step produces flaky failures that look unrelated to your change.

Race-condition tests that spawn many threads use `threads-required = 'num-test-threads'` so nextest
runs them alone. Copy that pattern for a genuinely timing-dependent test; a busy machine can
serialize threads and hide the race you are trying to catch.

## CLI-level tests

`tests/common.rs` provides helpers that invoke the built binary, which is how CLI behavior (argument
parsing, output format, exit status) is covered end to end:

| Helper                           | Use                                                   |
| -------------------------------- | ----------------------------------------------------- |
| `run_cli_with_json`              | Runs with `--format json` and parses the result       |
| `run_cli_command`                | Runs and returns stdout                               |
| `run_cli_command_with_auth`      | Runs with basic auth against an auth-enabled fixture  |
| `run_cli_command_with_auth_full` | Returns the full `Output` including status and stderr |

Use the `_full` variant when the assertion is about exit status or stderr, not stdout.

## Test conventions

1. Use `#[serial]` for integration tests that share resources, and register the binary in a nextest
   group.
2. Prefer `expect("descriptive message")` over `unwrap()`; clippy is configured to notice
   `unwrap_used` patterns.
3. Test error paths, not only the happy path. Torc's exit-code and prompt behavior is exactly where
   regressions hide.
4. One behavior per test, with a name that says what it asserts.
5. Clean up resources explicitly, remembering that `#[once]` fixtures do not run `Drop`.

## Slurm and remote coverage

Slurm tests use mock executables on `PATH`, which is why they are serialized: two tests manipulating
`PATH` concurrently corrupt each other.

Real Slurm behavior is only covered by the pre-release suite on a cluster:

```bash
./slurm-tests/run_all.sh --account <SLURM_ACCOUNT> --host <LOGIN_NODE>
./slurm-tests/run_all.sh --account <acct> --host <host> --partition gpu
./slurm-tests/run_all.sh --account <acct> --host <host> --test oom_detection
```

It starts a temporary server, runs each test as a child workflow under Slurm, and writes
`slurm-tests/output/run_<timestamp>/results.json`.

Remote-worker behavior is covered by an SSH loopback test that skips unless opted in:

```bash
TORC_TEST_SSH_LOOPBACK=1 cargo nextest run --all-features -E 'test(loopback_remote_shell_lifecycle)'
```

CI sets up the loopback SSH server and this variable on both Ubuntu and Windows runners.

## Python and Julia clients

```bash
cd python_client && pytest
julia --project=julia_client/Torc -e "import Pkg; Pkg.test()"
```

CI runs the Python client tests against a live `torc-server`. Both clients are generated, so a
failure there after an API change usually means the clients were not regenerated; see
`api-and-database.md`.
