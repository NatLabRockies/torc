/// Integration tests for client-side TLS configuration.
///
/// Tests that `TlsConfig` correctly configures reqwest to handle HTTPS connections
/// with custom CA certificates and insecure mode.
///
/// Prerequisites: `openssl` CLI and `python3` must be available on PATH.
/// Tests are skipped (not failed) if prerequisites are missing.
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use tempfile::TempDir;
use torc::client::apis::configuration::{Configuration, TlsConfig};

// ============================================================================
// Test infrastructure
// ============================================================================

/// Check if a command is available on PATH.
fn command_available(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

fn find_available_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("Failed to bind")
        .local_addr()
        .expect("Failed to get addr")
        .port()
}

/// Generate a test CA and server certificate using the openssl CLI.
///
/// Creates the following files in `dir`:
/// - `ca-key.pem` / `ca-cert.pem` - Self-signed CA with proper extensions
/// - `server-key.pem` / `server-cert.pem` - Server cert signed by the CA
///   with SAN `DNS:localhost,IP:127.0.0.1`
fn generate_test_certs(dir: &Path) {
    // CA config - must include keyUsage with keyCertSign for macOS compatibility
    std::fs::write(
        dir.join("ca.cnf"),
        "[req]\n\
         distinguished_name = req_dn\n\
         x509_extensions = v3_ca\n\
         prompt = no\n\
         \n\
         [req_dn]\n\
         CN = Torc Test CA\n\
         \n\
         [v3_ca]\n\
         basicConstraints = critical,CA:TRUE\n\
         keyUsage = critical,keyCertSign,cRLSign\n\
         subjectKeyIdentifier = hash\n",
    )
    .expect("Failed to write ca.cnf");

    // Server cert extension config
    std::fs::write(
        dir.join("ext.cnf"),
        "subjectAltName=DNS:localhost,IP:127.0.0.1\n\
         extendedKeyUsage=serverAuth\n\
         basicConstraints=CA:FALSE\n",
    )
    .expect("Failed to write ext.cnf");

    // Generate CA key and self-signed certificate with extensions
    let status = Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-keyout",
            "ca-key.pem",
            "-out",
            "ca-cert.pem",
            "-days",
            "1",
            "-nodes",
            "-config",
            "ca.cnf",
        ])
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("Failed to run openssl for CA");
    assert!(status.success(), "CA cert generation failed");

    // Generate server private key
    let status = Command::new("openssl")
        .args(["genrsa", "-out", "server-key.pem", "2048"])
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("Failed to generate server key");
    assert!(status.success(), "Server key generation failed");

    // Generate Certificate Signing Request
    let status = Command::new("openssl")
        .args([
            "req",
            "-new",
            "-key",
            "server-key.pem",
            "-out",
            "server.csr",
            "-subj",
            "/CN=localhost",
        ])
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("Failed to generate CSR");
    assert!(status.success(), "CSR generation failed");

    // Sign server cert with CA, including SAN extension
    let status = Command::new("openssl")
        .args([
            "x509",
            "-req",
            "-in",
            "server.csr",
            "-CA",
            "ca-cert.pem",
            "-CAkey",
            "ca-key.pem",
            "-CAcreateserial",
            "-out",
            "server-cert.pem",
            "-days",
            "1",
            "-extfile",
            "ext.cnf",
        ])
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("Failed to sign server cert");
    assert!(status.success(), "Server cert signing failed");
}

/// Start a minimal HTTPS server using Python's ssl module.
fn start_python_https_server(cert: &Path, key: &Path, port: u16) -> Child {
    let script = format!(
        r#"
import http.server, ssl, os
os.chdir('/')
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header('Content-Type', 'text/plain')
        self.end_headers()
        self.wfile.write(b'OK')
    def log_message(self, format, *args):
        pass
server = http.server.HTTPServer(('127.0.0.1', {port}), Handler)
ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
ctx.load_cert_chain(r'{cert}', r'{key}')
server.socket = ctx.wrap_socket(server.socket, server_side=True)
server.serve_forever()
"#,
        port = port,
        cert = cert.display(),
        key = key.display(),
    );

    Command::new("python3")
        .args(["-u", "-c", &script])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start Python HTTPS server")
}

/// Poll until the HTTPS server accepts connections.
fn wait_for_https_ready(port: u16, timeout: Duration) {
    let start = Instant::now();
    let client = reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();

    while start.elapsed() < timeout {
        if client
            .get(format!("https://127.0.0.1:{}", port))
            .timeout(Duration::from_secs(1))
            .send()
            .is_ok()
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!(
        "HTTPS server on port {} not ready within {:?}",
        port, timeout
    );
}

/// A self-contained HTTPS test server with generated certificates.
struct TestHttpsServer {
    _child: Child,
    port: u16,
    _cert_dir: TempDir,
    ca_cert_path: PathBuf,
}

/// Track Python server PIDs for cleanup at process exit.
///
/// Safety: The `atexit` handler only accesses the `HTTPS_PIDS` static Mutex and
/// calls `libc::kill`. Both are safe to use from a C atexit handler because:
/// - The Mutex is a static with a const initializer (no Rust runtime needed)
/// - `libc::kill` is a direct syscall wrapper
/// - We use `lock()` (not `try_lock()`) and handle the poisoned case via `if let Ok`
///
/// This pattern matches `tests/common.rs` and is needed because the `OnceLock` server
/// is never dropped (shared across all tests for the process lifetime).
static HTTPS_PIDS: Mutex<Vec<u32>> = Mutex::new(Vec::new());
static HTTPS_CLEANUP: std::sync::Once = std::sync::Once::new();

#[cfg(unix)]
fn register_https_cleanup() {
    HTTPS_CLEANUP.call_once(|| {
        extern "C" fn cleanup() {
            if let Ok(pids) = HTTPS_PIDS.lock() {
                for &pid in pids.iter() {
                    unsafe {
                        libc::kill(pid as i32, libc::SIGTERM);
                    }
                }
            }
        }
        unsafe {
            libc::atexit(cleanup);
        }
    });
}

#[cfg(not(unix))]
fn register_https_cleanup() {}

impl TestHttpsServer {
    fn try_new() -> Option<Self> {
        if !command_available("openssl") {
            eprintln!("Skipping TLS test: openssl CLI not available");
            return None;
        }
        if !command_available("python3") {
            eprintln!("Skipping TLS test: python3 not available");
            return None;
        }

        let cert_dir = TempDir::new().expect("Failed to create temp dir");
        generate_test_certs(cert_dir.path());

        let port = find_available_port();
        let cert_path = cert_dir.path().join("server-cert.pem");
        let key_path = cert_dir.path().join("server-key.pem");
        let ca_cert_path = cert_dir.path().join("ca-cert.pem");

        let child = start_python_https_server(&cert_path, &key_path, port);

        // Track PID for cleanup at exit
        register_https_cleanup();
        if let Ok(mut pids) = HTTPS_PIDS.lock() {
            pids.push(child.id());
        }

        wait_for_https_ready(port, Duration::from_secs(10));

        Some(Self {
            _child: child,
            port,
            _cert_dir: cert_dir,
            ca_cert_path,
        })
    }

    fn url(&self) -> String {
        format!("https://localhost:{}", self.port)
    }
}

/// Shared test server instance (created once, reused across all tests).
static TEST_SERVER: OnceLock<Option<TestHttpsServer>> = OnceLock::new();

fn get_test_server() -> Option<&'static TestHttpsServer> {
    TEST_SERVER.get_or_init(TestHttpsServer::try_new).as_ref()
}

/// Return the shared test server, or skip the test if prerequisites are missing.
macro_rules! require_server {
    () => {
        match get_test_server() {
            Some(server) => server,
            None => {
                eprintln!("Skipping: HTTPS test prerequisites not met (need openssl + python3)");
                return;
            }
        }
    };
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn test_tls_insecure_connects_to_self_signed() {
    let server = require_server!();

    let tls = TlsConfig {
        ca_cert_path: None,
        insecure: true,
    };
    let client = tls.build_blocking_client().expect("Failed to build client");
    let resp = client
        .get(server.url())
        .send()
        .expect("insecure client should connect to self-signed HTTPS");
    assert_eq!(resp.status(), 200);
}

#[test]
fn test_tls_ca_cert_connects_to_trusted_server() {
    let server = require_server!();

    let tls = TlsConfig {
        ca_cert_path: Some(server.ca_cert_path.clone()),
        insecure: false,
    };
    let client = tls.build_blocking_client().expect("Failed to build client");
    let resp = client
        .get(server.url())
        .send()
        .expect("client with CA cert should connect");
    assert_eq!(resp.status(), 200);
}

#[test]
fn test_tls_default_client_rejects_self_signed() {
    let server = require_server!();

    // A plain client without any TLS config should reject self-signed certs
    let client = reqwest::blocking::Client::new();
    let result = client.get(server.url()).send();
    assert!(
        result.is_err(),
        "Default client should reject self-signed certificate"
    );
}

#[test]
fn test_tls_wrong_ca_cert_rejects_server() {
    let server = require_server!();

    // Generate a different CA that did NOT sign the server cert
    let wrong_dir = TempDir::new().expect("Failed to create temp dir");
    let status = Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-keyout",
            "wrong-key.pem",
            "-out",
            "wrong-ca.pem",
            "-days",
            "1",
            "-nodes",
            "-subj",
            "/CN=Wrong CA",
        ])
        .current_dir(wrong_dir.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("Failed to generate wrong CA");
    assert!(status.success());

    let tls = TlsConfig {
        ca_cert_path: Some(wrong_dir.path().join("wrong-ca.pem")),
        insecure: false,
    };
    let client = tls.build_blocking_client().expect("Failed to build client");
    let result = client.get(server.url()).send();
    assert!(
        result.is_err(),
        "Client with wrong CA should reject the server certificate"
    );
}

#[test]
fn test_tls_configuration_with_tls_builds_working_client() {
    let server = require_server!();

    // Test that Configuration::with_tls() creates a properly configured client
    let tls = TlsConfig {
        ca_cert_path: Some(server.ca_cert_path.clone()),
        insecure: false,
    };
    let config = Configuration::with_tls(tls);

    // The embedded client should be able to connect to our HTTPS server
    let resp = config
        .client
        .get(server.url())
        .send()
        .expect("Configuration::with_tls client should connect");
    assert_eq!(resp.status(), 200);
}

#[test]
fn test_tls_nonexistent_ca_cert_falls_back_to_system_roots() {
    let server = require_server!();

    // A non-existent CA cert path should not crash - the client still builds,
    // but it won't trust our test CA, so the connection should fail.
    let tls = TlsConfig {
        ca_cert_path: Some(PathBuf::from("/nonexistent/ca.pem")),
        insecure: false,
    };
    let client = tls
        .build_blocking_client()
        .expect("Client should build even with bad cert path");
    let result = client.get(server.url()).send();
    assert!(
        result.is_err(),
        "Client with nonexistent CA cert should not trust self-signed server"
    );
}
