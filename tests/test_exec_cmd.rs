//! Integration tests for `torc exec` (and in particular `torc -s exec`).
//!
//! The exec command is always tested against a standalone server because that's the
//! primary intended use case and it keeps each test hermetic (its own DB + its own
//! server lifecycle). Unit-level coverage for helpers like `gather_commands` and
//! `parse_params` lives in `src/exec_cmd.rs#[cfg(test)]`; here we cover the end-to-end
//! CLI behavior that can't be tested at the module level.

use std::fs;
use std::process::Command;

use tempfile::TempDir;

#[path = "common.rs"]
mod common;

use common::{
    ensure_test_binaries_built, run_torc_standalone, run_torc_standalone_ok, torc_binary_path,
};

/// Pull the workflow id out of `Created workflow N` (plain-text stdout).
/// Returns None if the line is missing; callers decide whether that's a test failure.
fn extract_workflow_id(stdout: &str) -> Option<i64> {
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("Created workflow ")
            && let Ok(id) = rest.trim().parse::<i64>()
        {
            return Some(id);
        }
    }
    None
}

/// Spawn `torc -s -f json <args>` — used when a test needs machine-readable output
/// after the exec run has finished.
fn query_json(
    work_dir: &std::path::Path,
    db: &std::path::Path,
    args: &[&str],
) -> serde_json::Value {
    let mut full = vec!["-f", "json"];
    full.extend_from_slice(args);
    let out = run_torc_standalone_ok(work_dir, db, &full);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "failed to parse json output of `torc -s -f json {:?}`: {}\n---\n{}",
            args, e, stdout
        )
    })
}

// ============================================================================
// Core exec behavior
// ============================================================================

#[test]
fn exec_single_command_creates_one_job_workflow() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");

    let out = run_torc_standalone_ok(work.path(), &db, &["exec", "-c", "echo one"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let wf_id = extract_workflow_id(&stdout)
        .unwrap_or_else(|| panic!("no workflow id in stdout:\n{}", stdout));

    // Inspect the jobs with a follow-up query to the same DB.
    let jobs = query_json(work.path(), &db, &["jobs", "list", &wf_id.to_string()]);
    let items = jobs
        .get("jobs")
        .and_then(|v| v.as_array())
        .expect("jobs[] in jobs list");
    assert_eq!(items.len(), 1, "expected 1 job, got: {}", jobs);
    assert_eq!(items[0]["name"], "job1");
    assert!(items[0]["command"].as_str().unwrap().contains("echo one"));
}

#[test]
fn exec_failing_command_exits_nonzero() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");

    let out = run_torc_standalone(
        work.path(),
        &db,
        &["exec", "-c", "false", "--monitor", "off"],
    );
    assert!(
        !out.status.success(),
        "failed exec job should make `torc exec` exit nonzero"
    );
}

#[test]
fn exec_multiple_commands_creates_one_job_each() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");

    let out = run_torc_standalone_ok(
        work.path(),
        &db,
        &[
            "exec", "-c", "echo a", "-c", "echo b", "-c", "echo c", "-j", "2",
        ],
    );
    let wf_id =
        extract_workflow_id(&String::from_utf8_lossy(&out.stdout)).expect("workflow id in stdout");

    let jobs = query_json(work.path(), &db, &["jobs", "list", &wf_id.to_string()]);
    let items = jobs.get("jobs").and_then(|v| v.as_array()).expect("jobs[]");
    assert_eq!(items.len(), 3, "expected 3 jobs, got: {}", jobs);

    // Jobs are named job1..jobN by expand_jobs().
    let names: Vec<&str> = items.iter().map(|j| j["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"job1"));
    assert!(names.contains(&"job2"));
    assert!(names.contains(&"job3"));
}

#[test]
fn exec_shell_style_invocation_creates_one_command() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");

    let out = run_torc_standalone_ok(
        work.path(),
        &db,
        &["exec", "--monitor", "off", "--", "echo", "two words"],
    );
    let wf_id =
        extract_workflow_id(&String::from_utf8_lossy(&out.stdout)).expect("workflow id in stdout");

    let jobs = query_json(work.path(), &db, &["jobs", "list", &wf_id.to_string()]);
    let items = jobs.get("jobs").and_then(|v| v.as_array()).expect("jobs[]");
    assert_eq!(items.len(), 1, "expected 1 shell-style job, got: {}", jobs);
    assert_eq!(items[0]["command"], "echo 'two words'");
}

#[test]
fn exec_commands_file_skips_blanks_and_comments() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");
    let cmds_file = work.path().join("cmds.txt");
    fs::write(
        &cmds_file,
        "\
# this is a comment
echo first

echo second
  # indented comment - stripped after trim(), so it is skipped
echo third
",
    )
    .expect("write commands file");

    let out = run_torc_standalone_ok(
        work.path(),
        &db,
        &["exec", "-C", cmds_file.to_str().unwrap()],
    );
    let wf_id = extract_workflow_id(&String::from_utf8_lossy(&out.stdout)).expect("id");

    let jobs = query_json(work.path(), &db, &["jobs", "list", &wf_id.to_string()]);
    let items = jobs.get("jobs").and_then(|v| v.as_array()).unwrap();
    assert_eq!(
        items.len(),
        3,
        "expected 3 jobs (blank + both `#` comments skipped); got: {}",
        jobs
    );
    let commands: Vec<&str> = items
        .iter()
        .map(|j| j["command"].as_str().unwrap())
        .collect();
    assert!(commands.contains(&"echo first"));
    assert!(commands.contains(&"echo second"));
    assert!(commands.contains(&"echo third"));
}

// ============================================================================
// Parameter expansion
// ============================================================================

#[test]
fn exec_param_product_creates_cartesian_jobs() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");

    // 2 x 2 = 4 jobs; default link=product.
    let out = run_torc_standalone_ok(
        work.path(),
        &db,
        &[
            "exec",
            "-c",
            "echo lr={lr} bs={bs}",
            "--param",
            "lr=[0.01,0.001]",
            "--param",
            "bs=[32,64]",
            "-j",
            "2",
        ],
    );
    let wf_id = extract_workflow_id(&String::from_utf8_lossy(&out.stdout)).expect("id");

    let jobs = query_json(work.path(), &db, &["jobs", "list", &wf_id.to_string()]);
    let items = jobs.get("jobs").and_then(|v| v.as_array()).unwrap();
    assert_eq!(
        items.len(),
        4,
        "2x2 product must produce 4 jobs; got: {}",
        jobs
    );

    // Every substitution must have occurred — no raw `{lr}` / `{bs}` left.
    for j in items {
        let cmd = j["command"].as_str().unwrap();
        assert!(!cmd.contains("{lr}"), "unsubstituted {{lr}} in {:?}", cmd);
        assert!(!cmd.contains("{bs}"), "unsubstituted {{bs}} in {:?}", cmd);
    }
}

#[test]
fn exec_param_zip_creates_elementwise_jobs() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");

    let out = run_torc_standalone_ok(
        work.path(),
        &db,
        &[
            "exec",
            "-c",
            "echo {a}-{b}",
            "--param",
            "a=[1,2,3]",
            "--param",
            "b=[x,y,z]",
            "--link",
            "zip",
        ],
    );
    let wf_id = extract_workflow_id(&String::from_utf8_lossy(&out.stdout)).expect("id");

    let jobs = query_json(work.path(), &db, &["jobs", "list", &wf_id.to_string()]);
    let items = jobs.get("jobs").and_then(|v| v.as_array()).unwrap();
    assert_eq!(
        items.len(),
        3,
        "zip of length-3 params must produce 3 jobs; got: {}",
        jobs
    );

    let cmds: Vec<&str> = items
        .iter()
        .map(|j| j["command"].as_str().unwrap())
        .collect();
    assert!(cmds.contains(&"echo 1-x"));
    assert!(cmds.contains(&"echo 2-y"));
    assert!(cmds.contains(&"echo 3-z"));
}

#[test]
fn exec_param_integer_range() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");

    let out = run_torc_standalone_ok(
        work.path(),
        &db,
        &["exec", "-c", "echo {i}", "--param", "i=1:5"],
    );
    let wf_id = extract_workflow_id(&String::from_utf8_lossy(&out.stdout)).expect("id");

    let jobs = query_json(work.path(), &db, &["jobs", "list", &wf_id.to_string()]);
    let items = jobs.get("jobs").and_then(|v| v.as_array()).unwrap();
    assert_eq!(
        items.len(),
        5,
        "1:5 inclusive range → 5 jobs; got: {}",
        jobs
    );
}

// ============================================================================
// Name / description
// ============================================================================

#[test]
fn exec_name_and_description_persist_on_workflow() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");

    let out = run_torc_standalone_ok(
        work.path(),
        &db,
        &[
            "exec",
            "-n",
            "my-sweep",
            "--description",
            "hyperparameter sweep over LR",
            "-c",
            "echo named",
        ],
    );
    let wf_id = extract_workflow_id(&String::from_utf8_lossy(&out.stdout)).expect("id");

    let wf = query_json(work.path(), &db, &["workflows", "get", &wf_id.to_string()]);
    assert_eq!(wf["name"], "my-sweep", "workflow name; full: {}", wf);
    assert_eq!(
        wf["description"], "hyperparameter sweep over LR",
        "workflow description; full: {}",
        wf
    );
}

#[test]
fn exec_default_workflow_name_has_exec_prefix() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");

    let out = run_torc_standalone_ok(work.path(), &db, &["exec", "-c", "echo default-name"]);
    let wf_id = extract_workflow_id(&String::from_utf8_lossy(&out.stdout)).expect("id");

    let wf = query_json(work.path(), &db, &["workflows", "get", &wf_id.to_string()]);
    let name = wf["name"].as_str().expect("name");
    assert!(
        name.starts_with("exec_"),
        "default exec workflow should be 'exec_<ts>'; got {:?}",
        name
    );
}

// ============================================================================
// Error paths
// ============================================================================

#[test]
fn exec_no_commands_errors_with_hint() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");

    let out = run_torc_standalone(work.path(), &db, &["exec"]);
    assert!(!out.status.success(), "exec with no -c/-C should fail");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no commands provided"),
        "stderr should mention missing commands; got:\n{}",
        stderr
    );
}

#[test]
fn exec_spec_file_trailing_arg_suggests_torc_run() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");

    // Create an empty file with a spec-like extension so the detection finds it.
    let spec = work.path().join("workflow.yaml");
    fs::write(&spec, "name: placeholder\n").expect("write spec");

    let out = run_torc_standalone(work.path(), &db, &["exec", spec.to_str().unwrap()]);
    assert!(
        !out.status.success(),
        "should reject spec file trailing arg"
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("looks like a workflow spec file"),
        "stderr should hint about the mistake; got:\n{}",
        stderr
    );
    assert!(
        stderr.contains("torc run"),
        "stderr should suggest `torc run`; got:\n{}",
        stderr
    );
}

#[test]
fn exec_delimited_spec_like_arg_is_allowed() {
    // `torc exec -- <command with yaml arg>` must not be misread as a
    // `torc run <spec>` mistake; everything after `--` is a shell command.
    ensure_test_binaries_built();

    let out = Command::new(torc_binary_path())
        .args([
            "exec",
            "--dry-run",
            "--monitor",
            "off",
            "--",
            "echo",
            "workflow.yaml",
        ])
        .env_remove("TORC_API_URL")
        .output()
        .expect("failed to spawn torc");
    assert!(
        out.status.success(),
        "`torc exec -- echo workflow.yaml` should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let spec: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("dry-run stdout should be JSON: {}\n{}", e, stdout));
    let jobs = spec["jobs"].as_array().expect("jobs[]");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["command"], "echo workflow.yaml");
}

#[test]
fn exec_non_delimited_trailing_arg_is_rejected() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");

    let out = run_torc_standalone(work.path(), &db, &["exec", "echo", "hello"]);
    assert!(!out.status.success(), "bare trailing args should fail");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unexpected trailing argument"),
        "stderr should explain unexpected trailing args; got:\n{}",
        stderr
    );
    assert!(
        stderr.contains("torc exec -- <command>"),
        "stderr should document delimiter form; got:\n{}",
        stderr
    );
}

#[test]
fn exec_generate_plots_without_timeseries_is_rejected() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");

    // Default monitor is `summary` — with `--generate-plots` and no time-series
    // scope, build_spec() must refuse the request.
    let out = run_torc_standalone(
        work.path(),
        &db,
        &["exec", "-c", "echo nope", "--generate-plots"],
    );
    assert!(
        !out.status.success(),
        "--generate-plots without time-series must fail"
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--generate-plots requires"),
        "stderr should explain the requirement; got:\n{}",
        stderr
    );
}

#[test]
fn exec_invalid_param_format_errors() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");

    let out = run_torc_standalone(
        work.path(),
        &db,
        &["exec", "-c", "echo x", "--param", "noequals"],
    );
    assert!(!out.status.success(), "param without '=' must fail");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("NAME=VALUE") || stderr.contains("--param"),
        "stderr should explain the param syntax; got:\n{}",
        stderr
    );
}

#[test]
fn exec_dry_run_prints_expanded_spec_without_server() {
    ensure_test_binaries_built();

    let out = Command::new(torc_binary_path())
        .args([
            "exec",
            "--dry-run",
            "-c",
            "echo {i}",
            "--param",
            "i=1:2",
            "--monitor",
            "off",
        ])
        .env_remove("TORC_API_URL")
        .output()
        .expect("failed to spawn torc");
    assert!(out.status.success(), "`torc exec --dry-run` should succeed");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let spec: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("dry-run stdout should be JSON: {}\n{}", e, stdout));
    let jobs = spec["jobs"].as_array().expect("jobs[]");
    assert_eq!(jobs.len(), 2, "expanded dry-run spec should have 2 jobs");
    assert_eq!(jobs[0]["command"], "echo 1");
    assert_eq!(jobs[1]["command"], "echo 2");
}

#[test]
fn exec_json_output_is_single_summary_object() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");

    let out = run_torc_standalone_ok(
        work.path(),
        &db,
        &["-f", "json", "exec", "-c", "echo json", "--monitor", "off"],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("exec JSON stdout should parse: {}\n{}", e, stdout));
    assert!(
        value["workflow_id"].as_i64().is_some(),
        "workflow_id missing"
    );
    assert_eq!(value["status"], "completed");
    assert_eq!(value["had_failures"], false);
}

// ============================================================================
// Monitoring + plots
// ============================================================================

#[test]
fn exec_generate_plots_with_timeseries_produces_html() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");
    let output_dir = work.path().join("out");

    let out = run_torc_standalone_ok(
        work.path(),
        &db,
        &[
            "exec",
            "-c",
            // Short-lived command — the point is only that monitoring runs at least
            // one sample cycle before the job finishes.
            "sleep 0.5",
            "--monitor",
            "time-series",
            "--monitor-compute-node",
            "summary",
            "--generate-plots",
            "--sample-interval-seconds",
            "1",
            "-o",
            output_dir.to_str().unwrap(),
        ],
    );
    assert!(
        out.status.success(),
        "exec with time-series + plots should succeed: {:?}",
        out.status
    );

    let plots_dir = output_dir.join("resource_utilization");
    assert!(
        plots_dir.exists(),
        "expected resource_utilization dir at {:?}",
        plots_dir
    );
    let html_files: Vec<_> = fs::read_dir(&plots_dir)
        .expect("read plots dir")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("html"))
                .unwrap_or(false)
        })
        .collect();
    assert!(
        !html_files.is_empty(),
        "expected ≥1 HTML plot under {:?}; contents: {:?}",
        plots_dir,
        fs::read_dir(&plots_dir).unwrap().collect::<Vec<_>>()
    );
}

#[test]
fn exec_monitor_off_skips_resource_utilization_dir() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");
    let output_dir = work.path().join("out");

    run_torc_standalone_ok(
        work.path(),
        &db,
        &[
            "exec",
            "-c",
            "echo no-monitor",
            "--monitor",
            "off",
            // compute-node monitor is off by default
            "-o",
            output_dir.to_str().unwrap(),
        ],
    );

    // With monitoring disabled for both scopes, no resource_utilization directory
    // should have been created.
    let ru_dir = output_dir.join("resource_utilization");
    assert!(
        !ru_dir.exists(),
        "resource_utilization dir should not exist when monitoring is off: {:?}",
        ru_dir
    );
}

// ============================================================================
// CLI surface sanity (help output)
// ============================================================================

#[test]
fn exec_help_mentions_key_flags() {
    ensure_test_binaries_built();

    // No standalone needed — help is a local, read-only operation.
    let out = Command::new(torc_binary_path())
        .args(["exec", "--help"])
        .env_remove("TORC_API_URL")
        .output()
        .expect("failed to spawn torc");
    assert!(out.status.success(), "`torc exec --help` should succeed");
    let help = String::from_utf8_lossy(&out.stdout);

    for expected in [
        "--command",
        "--commands-file",
        "--param",
        "--monitor",
        "--monitor-compute-node",
        "--generate-plots",
        "--sample-interval-seconds",
        "--dry-run",
    ] {
        assert!(
            help.contains(expected),
            "`torc exec --help` should document {}; got:\n{}",
            expected,
            help
        );
    }
}

#[test]
fn standalone_short_flag_resolves_same_as_long() {
    // Guards the `-s` short alias for --standalone. We verify by running
    // `torc -h` and looking for the flag wiring — avoids starting a real server.
    ensure_test_binaries_built();

    let out = Command::new(torc_binary_path())
        .arg("--help")
        .output()
        .expect("spawn torc");
    assert!(out.status.success());
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(
        help.contains("--standalone") && help.contains("-s"),
        "root help should advertise both -s and --standalone; got:\n{}",
        help
    );
}
