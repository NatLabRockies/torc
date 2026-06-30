//! Loopback integration test for the remote-worker shell abstraction.
//!
//! This drives the [`RemoteShell`] command builders against a *real* SSH shell
//! by connecting to `localhost`, so it exercises the commands that are
//! generated for the host's actual operating system. On a Windows host it
//! validates the PowerShell `-EncodedCommand` path (including the case where the
//! OpenSSH default shell is `cmd.exe`); on a POSIX host it validates the bash
//! path.
//!
//! It is **opt-in**: it only runs when `TORC_TEST_SSH_LOOPBACK=1` is set, since
//! it requires a working `ssh localhost` with key-based authentication
//! (`BatchMode=yes`). CI configures that before setting the variable. Without
//! the variable the test is a no-op so normal `cargo nextest run` is unaffected.

use std::thread::sleep;
use std::time::Duration;

use torc::client::remote::{
    RemoteShell, WorkerEntry, detect_remote_shell, ssh_execute, ssh_execute_capture,
};

/// Whether the loopback test is enabled for this run.
fn loopback_enabled() -> bool {
    std::env::var("TORC_TEST_SSH_LOOPBACK").as_deref() == Ok("1")
}

/// The shell family we expect for the host running the test.
fn expected_shell() -> RemoteShell {
    if cfg!(windows) {
        RemoteShell::Windows
    } else {
        RemoteShell::Posix
    }
}

/// A long-running stub process used as a stand-in worker, per shell family.
///
/// Returns `(program, args)` for [`RemoteShell::start_detached`]. Both run for
/// ~30 seconds, long enough to observe a live PID and then kill it.
fn stub_worker(shell: RemoteShell) -> (&'static str, Vec<String>) {
    match shell {
        // `ping -n N 127.0.0.1` runs for roughly N-1 seconds on Windows.
        RemoteShell::Windows => ("ping", vec!["-n".into(), "30".into(), "127.0.0.1".into()]),
        RemoteShell::Posix => ("sleep", vec!["30".into()]),
    }
}

#[test]
fn loopback_remote_shell_lifecycle() {
    if !loopback_enabled() {
        eprintln!(
            "skipping loopback_remote_shell_lifecycle: set TORC_TEST_SSH_LOOPBACK=1 \
             (and ensure `ssh localhost` works with key-based auth) to enable"
        );
        return;
    }

    let worker = WorkerEntry::new("localhost");

    // 1. Shell detection should identify this host's family. This also confirms
    //    SSH connectivity to localhost works.
    let shell = detect_remote_shell(&worker)
        .unwrap_or_else(|e| panic!("detect_remote_shell(localhost) failed: {e}"));
    assert_eq!(
        shell,
        expected_shell(),
        "detected shell family does not match the host OS"
    );

    // Use a per-process directory so concurrent/repeated runs do not collide.
    // Forward slashes are accepted by both POSIX and Windows file APIs.
    let output_dir = format!("torc_loopback_test_{}", std::process::id());
    let pid_file = format!("{output_dir}/worker.pid");
    let log_file = format!("{output_dir}/worker.log");

    // Best-effort cleanup of any leftovers from a previous aborted run.
    let _ = ssh_execute(&worker, &shell.remove_dir(&output_dir), Some(30));

    // 2. Create the output directory and confirm it exists.
    let out = ssh_execute_capture(&worker, &shell.mkdir_p(&output_dir))
        .unwrap_or_else(|e| panic!("mkdir_p failed: {e}"));
    eprintln!("mkdir_p output: {out:?}");
    assert_eq!(
        ssh_execute_capture(&worker, &shell.dir_exists(&output_dir))
            .expect("dir_exists failed")
            .trim(),
        "exists",
    );

    // 3. Launch the detached stub "worker" and record its PID.
    let (program, args) = stub_worker(shell);
    let start = ssh_execute(
        &worker,
        &shell.start_detached(program, &args, &log_file, &pid_file),
        Some(60),
    )
    .unwrap_or_else(|e| panic!("start_detached failed: {e}"));
    assert!(
        start.status.success(),
        "start_detached exited non-zero: {}",
        String::from_utf8_lossy(&start.stderr)
    );

    // Give the process a moment to spawn and write the PID file.
    sleep(Duration::from_secs(2));

    // 4. Read and parse the PID.
    let pid_raw = ssh_execute_capture(&worker, &shell.read_file(&pid_file))
        .unwrap_or_else(|e| panic!("reading PID file failed: {e}"));
    let pid: u32 = pid_raw
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("PID file did not contain a number: {pid_raw:?}"));
    eprintln!("stub worker started with PID {pid}");

    // 5. The PID should be alive.
    assert_eq!(
        ssh_execute_capture(&worker, &shell.is_process_alive(pid))
            .expect("is_process_alive failed")
            .trim(),
        "running",
        "stub worker PID {pid} was not reported as running",
    );

    // 6. `tail` should run without error (content is not asserted; the stub may
    //    not have written to the stderr log).
    ssh_execute_capture(&worker, &shell.tail(&log_file, 5)).expect("tail failed");

    // 7. Stop the process and confirm it is reported as killed, then dead.
    assert_eq!(
        ssh_execute_capture(&worker, &shell.kill_process(pid, false))
            .expect("kill_process failed")
            .trim(),
        "killed",
        "kill_process did not report a successful kill for PID {pid}",
    );
    sleep(Duration::from_secs(1));
    assert_eq!(
        ssh_execute_capture(&worker, &shell.is_process_alive(pid))
            .expect("is_process_alive (post-kill) failed")
            .trim(),
        "stopped",
        "stub worker PID {pid} still running after kill",
    );

    // 8. Archive the output directory, then clean up the tarball.
    let tarball = shell.temp_tarball_path(&format!("{output_dir}.tar.gz"));
    let tar = ssh_execute(
        &worker,
        &shell.create_tarball(&tarball, &output_dir),
        Some(120),
    )
    .unwrap_or_else(|e| panic!("create_tarball failed: {e}"));
    assert!(
        tar.status.success(),
        "create_tarball exited non-zero: {}",
        String::from_utf8_lossy(&tar.stderr)
    );
    let _ = ssh_execute(&worker, &shell.remove_file(&tarball), Some(30));

    // 9. Remove the output directory and confirm it is gone.
    let rm = ssh_execute(&worker, &shell.remove_dir(&output_dir), Some(60))
        .unwrap_or_else(|e| panic!("remove_dir failed: {e}"));
    assert!(
        rm.status.success(),
        "remove_dir exited non-zero: {}",
        String::from_utf8_lossy(&rm.stderr)
    );
    assert_eq!(
        ssh_execute_capture(&worker, &shell.dir_exists(&output_dir))
            .expect("dir_exists (post-remove) failed")
            .trim(),
        "missing",
        "output directory still present after remove_dir",
    );
}
