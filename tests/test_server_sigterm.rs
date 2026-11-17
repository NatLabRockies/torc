//! `torc-server run` must treat SIGTERM (docker stop, Kubernetes, systemd, Slurm)
//! and SIGINT as a graceful shutdown, not die from the default signal disposition.

#![cfg(unix)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tempfile::TempDir;

#[path = "common.rs"]
mod common;

use common::{ensure_test_binaries_built, torc_server_binary_path};

struct Server {
    child: Child,
    port: u16,
    stderr: Option<JoinHandle<String>>,
    _work: TempDir,
}

impl Server {
    fn start() -> Self {
        ensure_test_binaries_built();
        let work = TempDir::new().expect("tempdir");
        let mut child = Command::new(torc_server_binary_path())
            .args(["run", "--host", "127.0.0.1", "--port", "0", "--database"])
            .arg(work.path().join("torc.db"))
            // The assertions read INFO-level log lines; don't inherit RUST_LOG or a torc.toml.
            .env("RUST_LOG", "info")
            .current_dir(work.path())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn torc-server");

        // Drain stderr concurrently so a chatty server never blocks on a full pipe.
        let mut err = child.stderr.take().unwrap();
        let stderr = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = err.read_to_string(&mut s);
            s
        });

        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let port = lines
            .by_ref()
            .map_while(Result::ok)
            .find_map(|l| {
                l.strip_prefix("TORC_SERVER_PORT=")
                    .map(|p| p.parse().unwrap())
            })
            .expect("torc-server never reported its port");
        // Keep stdout open and drained so later writes can't hit EPIPE.
        std::thread::spawn(move || lines.for_each(drop));

        Server {
            child,
            port,
            stderr: Some(stderr),
            _work: work,
        }
    }

    fn signal(&self, sig: libc::c_int) {
        unsafe { libc::kill(self.child.id() as libc::pid_t, sig) };
    }

    fn wait_exit(&mut self, within: Duration) -> Option<ExitStatus> {
        let deadline = Instant::now() + within;
        while Instant::now() < deadline {
            if let Some(s) = self.child.try_wait().expect("try_wait") {
                return Some(s);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        None
    }

    /// Wait for exit (killing on timeout) and return the status plus stderr.
    fn finish(&mut self, within: Duration) -> (ExitStatus, String) {
        let status = self.wait_exit(within);
        if status.is_none() {
            self.child.kill().ok();
            self.child.wait().ok();
        }
        let stderr = self.stderr.take().unwrap().join().unwrap();
        let status = status.unwrap_or_else(|| {
            panic!("torc-server did not exit within {within:?}; stderr:\n{stderr}")
        });
        (status, stderr)
    }
}

fn assert_graceful(sig: libc::c_int, name: &str) {
    let mut server = Server::start();
    // Sent right after the port line, while the server is still finishing setup:
    // the handler must already be installed.
    server.signal(sig);
    let (status, stderr) = server.finish(Duration::from_secs(15));
    assert!(
        status.success(),
        "expected clean exit after {name}, got {status:?}; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains(&format!("Received {name}")),
        "expected graceful-shutdown log line; stderr:\n{stderr}"
    );
}

#[test]
fn server_exits_gracefully_on_sigterm() {
    assert_graceful(libc::SIGTERM, "SIGTERM");
}

#[test]
fn server_exits_gracefully_on_sigint() {
    assert_graceful(libc::SIGINT, "SIGINT");
}

/// An idle keep-alive connection (job runners, torc-dash) must not hold the drain
/// open until the 30-second timeout.
#[test]
fn idle_keep_alive_connection_does_not_stall_shutdown() {
    let mut server = Server::start();
    let mut conn = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    conn.write_all(b"GET /torc-service/v1/ping HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();
    let mut buf = [0u8; 1024];
    let n = conn.read(&mut buf).unwrap();
    assert!(buf[..n].starts_with(b"HTTP/1.1"), "no HTTP response");

    server.signal(libc::SIGTERM);
    let (status, stderr) = server.finish(Duration::from_secs(10));
    assert!(status.success(), "got {status:?}; stderr:\n{stderr}");
    // The server closed our idle connection rather than waiting on it.
    assert_eq!(conn.read(&mut buf).unwrap_or(0), 0);
}

/// A second signal during the drain forces shutdown instead of being swallowed.
#[test]
fn second_signal_aborts_drain() {
    let mut server = Server::start();
    // A request with incomplete headers keeps the connection busy during the drain.
    let mut conn = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    conn.write_all(b"GET /torc-service/v1/ping HTTP/1.1\r\nHost: localhost\r\n")
        .unwrap();
    std::thread::sleep(Duration::from_millis(500));

    server.signal(libc::SIGTERM);
    assert!(
        server.wait_exit(Duration::from_secs(2)).is_none(),
        "server should still be draining the busy connection"
    );
    server.signal(libc::SIGTERM);
    let (status, stderr) = server.finish(Duration::from_secs(5));
    assert!(status.success(), "got {status:?}; stderr:\n{stderr}");
    assert!(
        stderr.contains("forcing shutdown"),
        "expected forced-shutdown log line; stderr:\n{stderr}"
    );
    drop(conn);
}
