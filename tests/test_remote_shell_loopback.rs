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

/// Poll the remote PID file until it contains a parseable PID, up to a bounded
/// timeout. Returns the PID, or panics if it never appears.
///
/// `read_file` exits non-zero (Err) while the file is absent, and can briefly
/// return empty content after the file is created but before the PID is written;
/// both are treated as "not ready yet" and retried.
fn poll_for_pid(worker: &WorkerEntry, shell: RemoteShell, pid_file: &str) -> u32 {
    let deadline = 30;
    let mut waited = 0;
    let mut last = String::from("<never read>");
    while waited < deadline {
        if let Ok(raw) = ssh_execute_capture(worker, &shell.read_file(pid_file)) {
            if let Ok(pid) = raw.trim().parse::<u32>() {
                return pid;
            }
            last = raw;
        }
        sleep(Duration::from_millis(500));
        waited += 1;
    }
    panic!("PID file {pid_file} did not contain a number within {deadline} polls (last: {last:?})");
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

    // 4. Poll for the PID file rather than sleeping a fixed interval: the
    //    detached spawn (especially Win32_Process.Create on a busy Windows CI
    //    runner) can take a variable amount of time to write the PID.
    let pid = poll_for_pid(&worker, shell, &pid_file);
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
