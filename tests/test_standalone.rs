//! Integration tests for `torc --standalone` / `torc -s`.
//!
//! These tests exercise the ephemeral-server path that `main.rs` takes when the
//! user passes `-s`: spawning a `torc-server` subprocess bound to 127.0.0.1, routing
//! the client against it, and tearing it down on exit. Unlike most other tests in
//! this crate we do NOT rely on the shared `start_server` fixture — the whole point
//! here is to verify that `torc -s ...` brings up and shuts down its own server.
//!
//! These tests run the CLI end-to-end as a subprocess, so they require the
//! debug-built `torc` and `torc-server` binaries. `ensure_test_binaries_built()`
//! takes care of that once per test binary.

use std::process::Command;
use std::time::Duration;

use tempfile::TempDir;

#[path = "common.rs"]
mod common;

use common::{
    ensure_test_binaries_built, run_torc_standalone, run_torc_standalone_ok, torc_binary_path,
    torc_server_binary_path,
};

#[test]
fn standalone_exec_creates_and_runs_workflow() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc_output").join("torc.db");

    // Single-command exec is the canonical `torc -s` smoke test: spawn the server,
    // synthesize a one-job workflow, run it locally, then tear the server down.
    let out = run_torc_standalone_ok(work.path(), &db, &["exec", "-c", "echo hello-standalone"]);

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Started standalone torc-server"),
        "stderr should log server startup; got:\n{}",
        stderr
    );
    assert!(
        stdout.contains("Created workflow"),
        "stdout should announce workflow creation; got:\n{}",
        stdout
    );
    assert!(db.exists(), "database at {:?} was not created", db);
}

#[test]
fn standalone_persists_workflow_across_invocations() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");

    // First invocation creates and runs the workflow.
    let first = run_torc_standalone_ok(work.path(), &db, &["exec", "-c", "echo persist-me"]);
    assert!(
        String::from_utf8_lossy(&first.stdout).contains("Created workflow"),
        "first invocation should create a workflow"
    );

    // Second invocation — brand new server, same DB — must be able to see the workflow.
    // Use `-f json` so we can machine-parse the response.
    let second = run_torc_standalone_ok(work.path(), &db, &["-f", "json", "workflows", "list"]);
    let stdout = String::from_utf8_lossy(&second.stdout).to_string();
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("workflows list JSON parse failed: {}\n---\n{}", e, stdout));
    // `list_workflows` returns `{"workflows": [...]}`.
    let items = parsed
        .get("workflows")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("expected workflows[] in list response: {}", stdout));
    assert!(
        !items.is_empty(),
        "expected ≥1 workflow in standalone DB after exec run; got {}",
        stdout
    );
}

#[test]
fn standalone_invalid_server_bin_fails_cleanly() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");

    // Use a bogus server binary path. We can't call run_torc_standalone() because
    // it wires up the real binary — build the command manually here instead.
    let out = Command::new(torc_binary_path())
        .current_dir(work.path())
        .args([
            "-s",
            "--torc-server-bin",
            "/nonexistent/torc-server-bogus",
            "--db",
            db.to_str().unwrap(),
            "exec",
            "-c",
            "echo no-server",
        ])
        .env_remove("TORC_API_URL")
        .env("RUST_LOG", "warn")
        .output()
        .expect("failed to spawn torc");

    assert!(
        !out.status.success(),
        "bogus --torc-server-bin should fail; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Error starting standalone torc-server")
            || stderr.contains("failed to spawn"),
        "stderr should explain the failure; got:\n{}",
        stderr,
    );
}

#[test]
fn standalone_creates_missing_db_parent_directory() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    // Intentionally nested path — the CLI is responsible for mkdir -p'ing this.
    let db = work.path().join("nested").join("subdir").join("torc.db");
    assert!(!db.parent().unwrap().exists());

    run_torc_standalone_ok(work.path(), &db, &["exec", "-c", "echo nested-ok"]);

    assert!(
        db.parent().unwrap().exists(),
        "standalone should have created parent dir for --db path"
    );
    assert!(db.exists(), "db file should exist at {:?}", db);
}

#[test]
fn standalone_default_db_is_torc_output_torc_db() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");

    // Omit --db to exercise the default (./torc_output/torc.db, relative to CWD).
    let server_bin = torc_server_binary_path();
    let out = Command::new(torc_binary_path())
        .current_dir(work.path())
        .args([
            "-s",
            "--torc-server-bin",
            server_bin.to_str().unwrap(),
            "exec",
            "-c",
            "echo default-db",
        ])
        .env_remove("TORC_API_URL")
        .env("RUST_LOG", "warn")
        .output()
        .expect("failed to spawn torc");

    assert!(
        out.status.success(),
        "default-db exec should succeed. stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let default_db = work.path().join("torc_output").join("torc.db");
    assert!(
        default_db.exists(),
        "expected default DB at {:?} after `torc -s exec`",
        default_db
    );
}

#[test]
fn standalone_no_op_for_local_command_prints_notice() {
    ensure_test_binaries_built();

    // PlotResources is one of the commands that doesn't need a server. `main.rs`
    // treats `--standalone` as a no-op for these and prints a warning instead of
    // launching a server.
    let work = TempDir::new().expect("tempdir");
    let fake_metrics_db = work.path().join("no-such.db");

    let out = Command::new(torc_binary_path())
        .current_dir(work.path())
        .args([
            "-s",
            "--torc-server-bin",
            "/definitely/not/a/real/path",
            "plot-resources",
            fake_metrics_db.to_str().unwrap(),
        ])
        .env_remove("TORC_API_URL")
        .env("RUST_LOG", "warn")
        .output()
        .expect("failed to spawn torc");

    // The command will fail (no metrics db), but the `--standalone` handling must
    // not have attempted to launch the bogus server binary. Specifically, we
    // should see the "has no effect" notice on stderr and *not* see a spawn error.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--standalone has no effect"),
        "expected '--standalone has no effect' notice; stderr:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("Error starting standalone torc-server"),
        "should not have attempted to launch the bogus server; stderr:\n{}",
        stderr
    );
}

#[cfg(unix)]
#[test]
fn standalone_server_shuts_down_after_command_exits() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");

    run_torc_standalone_ok(work.path(), &db, &["exec", "-c", "echo shutdown-test"]);

    // Give the OS a moment to reap the child.
    std::thread::sleep(Duration::from_millis(500));

    // The unique db path in the command line is our fingerprint for the server
    // subprocess — find any `torc-server` process still holding it open.
    let ps = Command::new("ps")
        .args(["-Ao", "args="])
        .output()
        .expect("ps failed");
    let listing = String::from_utf8_lossy(&ps.stdout);
    let db_str = db.to_string_lossy();
    let lingering: Vec<&str> = listing
        .lines()
        .filter(|l| l.contains(&*db_str) && l.contains("torc-server"))
        .collect();
    assert!(
        lingering.is_empty(),
        "expected no torc-server subprocess after `torc -s exec` exits; found: {:#?}",
        lingering
    );
}

#[cfg(unix)]
#[test]
fn standalone_server_shuts_down_when_client_exits_via_process_exit() {
    // Failure paths in the CLI frequently call std::process::exit, which bypasses
    // destructors. Without the parent-death pipe, the standalone subprocess would
    // be orphaned. This test fails the client *after* the server has started
    // (`exec` with no `-c` commands rejects via process::exit(2)) and verifies the
    // server still shuts down.
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("process_exit.db");

    let out = run_torc_standalone(work.path(), &db, &["exec"]);
    assert!(
        !out.status.success(),
        "`torc -s exec` with no commands should fail. stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("Started standalone torc-server"),
        "the server must have been started before the failure for this test to be meaningful"
    );

    // Give the server a moment to see stdin EOF and drain connections.
    std::thread::sleep(Duration::from_secs(2));

    let ps = Command::new("ps")
        .args(["-Ao", "args="])
        .output()
        .expect("ps failed");
    let listing = String::from_utf8_lossy(&ps.stdout);
    let db_str = db.to_string_lossy();
    let lingering: Vec<&str> = listing
        .lines()
        .filter(|l| l.contains(&*db_str) && l.contains("torc-server"))
        .collect();
    assert!(
        lingering.is_empty(),
        "torc-server subprocess leaked after client exited via process::exit; found: {:#?}",
        lingering
    );
}

#[test]
fn non_standalone_does_not_start_server() {
    // Sanity check: without -s, the client should *not* print the standalone
    // startup line. Guards against accidentally wiring standalone as the default.
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");

    // Point at an obviously-unreachable URL so the command fails fast rather
    // than waiting for a network timeout against some other process.
    let out = Command::new(torc_binary_path())
        .current_dir(work.path())
        .args([
            "--url",
            "http://127.0.0.1:1/torc-service/v1",
            "workflows",
            "list",
        ])
        .env_remove("TORC_API_URL")
        .env("RUST_LOG", "warn")
        .output()
        .expect("failed to spawn torc");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("Started standalone torc-server"),
        "non-standalone invocation must not start a server; stderr:\n{}",
        stderr
    );
    // The command itself is expected to fail (unreachable URL); we only care that
    // the standalone code path was not triggered.
}
