//! `torc-server run` must treat SIGTERM (docker stop, Kubernetes, systemd, Slurm)
//! as a graceful shutdown, not die from the default signal disposition.

#![cfg(unix)]

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use tempfile::TempDir;

#[path = "common.rs"]
mod common;

use common::{ensure_test_binaries_built, torc_server_binary_path};

#[test]
fn server_exits_gracefully_on_sigterm() {
    ensure_test_binaries_built();

    let work = TempDir::new().expect("tempdir");
    let db = work.path().join("torc.db");
    let mut child = Command::new(torc_server_binary_path())
        .args(["run", "--host", "127.0.0.1", "--port", "0", "--database"])
        .arg(&db)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn torc-server");

    // The SIGTERM handler is installed before the port is advertised.
    let stdout = BufReader::new(child.stdout.take().unwrap());
    let ready = stdout
        .lines()
        .map_while(Result::ok)
        .any(|l| l.starts_with("TORC_SERVER_PORT="));
    assert!(ready, "torc-server never reported its port");

    unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGTERM) };

    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(s) = child.try_wait().expect("try_wait") {
            break s;
        }
        if Instant::now() > deadline {
            child.kill().ok();
            panic!("torc-server did not exit within 15s of SIGTERM");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let stderr = std::io::read_to_string(child.stderr.take().unwrap()).unwrap_or_default();

    // Killed by the default disposition => no exit code (signal), not success.
    assert!(
        status.success(),
        "expected clean exit after SIGTERM, got {status:?}; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("Received SIGTERM"),
        "expected graceful-shutdown log line; stderr:\n{stderr}"
    );
}
