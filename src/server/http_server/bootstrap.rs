use super::Server;
use crate::server::htpasswd::HtpasswdFile;
use crate::server::live_router::{LiveAuthState, LiveRouterState};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder as HyperServerBuilder;
use log::{error, info};
use parking_lot::RwLock;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use tokio::net::TcpListener;

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "ios")))]
use openssl::ssl::{Ssl, SslAcceptor, SslFiletype, SslMethod};

pub(super) async fn sync_admin_group(
    pool: &SqlitePool,
    admin_users: &[String],
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO access_group (name, description, is_system)
        VALUES ('admin', 'System administrators', 1)
        ON CONFLICT (name) DO UPDATE SET is_system = 1
        "#,
    )
    .execute(pool)
    .await?;

    let admin_group_id: i64 =
        sqlx::query_scalar("SELECT id FROM access_group WHERE name = 'admin'")
            .fetch_one(pool)
            .await?;

    let current_members: Vec<String> =
        sqlx::query_scalar("SELECT user_name FROM user_group_membership WHERE group_id = $1")
            .bind(admin_group_id)
            .fetch_all(pool)
            .await?;

    for user in admin_users {
        if !current_members.contains(user) {
            info!("Adding user '{}' to admin group", user);
            sqlx::query(
                r#"
                INSERT INTO user_group_membership (user_name, group_id, role)
                VALUES ($1, $2, 'admin')
                ON CONFLICT (user_name, group_id) DO UPDATE SET role = 'admin'
                "#,
            )
            .bind(user)
            .bind(admin_group_id)
            .execute(pool)
            .await?;
        }
    }

    for member in &current_members {
        if !admin_users.contains(member) {
            info!(
                "Removing user '{}' from admin group (not in config)",
                member
            );
            sqlx::query("DELETE FROM user_group_membership WHERE user_name = $1 AND group_id = $2")
                .bind(member)
                .bind(admin_group_id)
                .execute(pool)
                .await?;
        }
    }

    Ok(())
}

/// Default base path for snapshots when `TORC_SERVER_SNAPSHOT_PATH` is unset.
const DEFAULT_SNAPSHOT_PATH: &str = "torc-server-snapshot.db";
/// Default number of snapshots to keep when `TORC_SERVER_SNAPSHOT_KEEP` is unset.
const DEFAULT_SNAPSHOT_KEEP: usize = 5;

#[derive(Clone)]
struct SnapshotConfig {
    /// Path to the canonical (newest) snapshot. If configured as relative,
    /// `from_env()` resolves it against the startup CWD when possible; if
    /// `current_dir()` itself fails (rare) it remains relative. Older
    /// snapshots are kept alongside as `<base>.1`, `<base>.2`, … up to
    /// `keep - 1`.
    base: std::path::PathBuf,
    /// Total snapshots to retain (canonical + rotated). Always >= 1.
    keep: usize,
}

impl SnapshotConfig {
    fn from_env() -> Self {
        let base_raw = std::env::var("TORC_SERVER_SNAPSHOT_PATH")
            .unwrap_or_else(|_| DEFAULT_SNAPSHOT_PATH.to_string());
        let base = std::path::PathBuf::from(&base_raw);
        // Resolve relative paths once, at startup, against the launch CWD so
        // the SQLite worker thread doesn't resolve them against something else.
        let base = if base.is_absolute() {
            base
        } else {
            std::env::current_dir()
                .map(|c| c.join(&base))
                .unwrap_or(base)
        };

        let keep = std::env::var("TORC_SERVER_SNAPSHOT_KEEP")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .map(|n| n.max(1))
            .unwrap_or(DEFAULT_SNAPSHOT_KEEP);

        Self { base, keep }
    }

    fn tmp_path(&self) -> std::path::PathBuf {
        let mut p = self.base.clone().into_os_string();
        p.push(".tmp");
        p.into()
    }

    /// Path for the `n`th rotated snapshot (n >= 1). `.1` is the
    /// most recently rotated (i.e., the previous canonical).
    fn rotated_path(&self, n: usize) -> std::path::PathBuf {
        let mut p = self.base.clone().into_os_string();
        p.push(format!(".{}", n));
        p.into()
    }
}

/// Listen for SIGUSR1 and snapshot the database via SQLite's `VACUUM INTO`.
/// Works for both on-disk and `:memory:` databases and is the persistence
/// mechanism for in-memory deployments (e.g. HPC login/compute nodes where
/// Lustre is unreliable).
///
/// Snapshots are written to a `.tmp` sibling first and then atomically renamed
/// into place, so a failed or interrupted snapshot never corrupts a prior one.
/// Older snapshots are rotated to `<base>.1`, `<base>.2`, … so the canonical
/// path always points at the newest snapshot.
///
/// Configured via env vars: `TORC_SERVER_SNAPSHOT_PATH` (default
/// `./torc-server-snapshot.db`) and `TORC_SERVER_SNAPSHOT_KEEP` (default 5,
/// minimum 1).
/// Replace a signal's kernel-default disposition (terminate the process) with a
/// tokio-managed stream. Call *before* the server advertises readiness on
/// stdout, so a parent that signals right after `TORC_SERVER_PORT=` can't kill
/// the server through the default disposition.
#[cfg(unix)]
fn register_signal(
    kind: tokio::signal::unix::SignalKind,
    name: &str,
) -> Option<tokio::signal::unix::Signal> {
    match tokio::signal::unix::signal(kind) {
        Ok(s) => Some(s),
        Err(e) => {
            error!("Failed to install {} handler: {}", name, e);
            None
        }
    }
}

#[cfg(unix)]
async fn snapshot_on_sigusr1(pool: SqlitePool, mut sig: tokio::signal::unix::Signal) {
    let cfg = SnapshotConfig::from_env();
    info!(
        "SIGUSR1 handler installed: send SIGUSR1 to snapshot the database to {} (keeping {} total)",
        cfg.base.display(),
        cfg.keep
    );
    while sig.recv().await.is_some() {
        snapshot_once(&pool, &cfg).await;
    }
}

#[cfg(unix)]
async fn snapshot_once(pool: &SqlitePool, cfg: &SnapshotConfig) {
    let tmp = cfg.tmp_path();
    info!(
        "Received SIGUSR1, snapshotting database to {}",
        cfg.base.display()
    );
    if let Some(parent) = cfg.base.parent()
        && !parent.as_os_str().is_empty()
        && let Err(e) = tokio::fs::create_dir_all(parent).await
    {
        error!(
            "Failed to create snapshot directory {}: {}",
            parent.display(),
            e
        );
        return;
    }
    let _ = tokio::fs::remove_file(&tmp).await;
    // Inline the path (single-quote-escaped) since parameter binding for
    // VACUUM INTO has been unreliable across SQLite versions.
    let escaped = tmp.to_string_lossy().replace('\'', "''");
    let sql = format!("VACUUM INTO '{}'", escaped);
    if let Err(e) = sqlx::query(sqlx::AssertSqlSafe(&*sql)).execute(pool).await {
        error!("Failed to snapshot database: {}", e);
        let _ = tokio::fs::remove_file(&tmp).await;
        return;
    }
    if let Err(e) = rotate_and_promote(cfg, &tmp).await {
        error!("Failed to rotate snapshots: {}", e);
        let _ = tokio::fs::remove_file(&tmp).await;
        return;
    }
    info!("Database snapshot written to {}", cfg.base.display());
    // Emit a machine-readable line on stdout so a parent process (e.g. `torc
    // --standalone --in-memory`) can synchronize on snapshot completion. This
    // is a stable contract — do not change without updating the parent-side
    // reader in `src/main.rs`. Flush explicitly because stdout is block-
    // buffered when piped, and the parent is waiting on this exact line.
    // Run in spawn_blocking so a slow/backpressured stdout pipe can't park a
    // Tokio worker thread.
    let line = format!("TORC_SNAPSHOT_DONE={}", cfg.base.display());
    let join = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        use std::io::Write;
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        writeln!(handle, "{}", line)?;
        handle.flush()
    })
    .await;
    match join {
        Ok(Ok(())) => {}
        Ok(Err(e)) => error!("Failed to write snapshot completion line: {}", e),
        Err(e) => error!("snapshot stdout-notify task panicked: {}", e),
    }
}

/// Rotate `<base>.{n-1}` → `<base>.{n}` for n down to 1, drop anything beyond
/// `keep - 1`, then move the freshly-written `tmp` file into the canonical
/// path. Each step is best-effort — a missing source is fine, since rotation
/// runs on every snapshot but earlier slots may not exist yet.
#[cfg(unix)]
async fn rotate_and_promote(cfg: &SnapshotConfig, tmp: &std::path::Path) -> std::io::Result<()> {
    let mut demoted_canonical = false;
    if cfg.keep > 1 {
        // Drop the oldest if it would push us over the limit. With `keep`
        // total slots, we retain `.1 ..= .{keep - 1}` plus the canonical.
        let oldest = cfg.rotated_path(cfg.keep - 1);
        let _ = tokio::fs::remove_file(&oldest).await;
        // Shift `.{n-1}` → `.{n}` from oldest to newest so we never clobber.
        for n in (2..cfg.keep).rev() {
            let from = cfg.rotated_path(n - 1);
            let to = cfg.rotated_path(n);
            match tokio::fs::rename(&from, &to).await {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
        // Demote the previous canonical to `.1`.
        let demoted = cfg.rotated_path(1);
        match tokio::fs::rename(&cfg.base, &demoted).await {
            Ok(()) => demoted_canonical = true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    // Final promotion. If this fails, roll back the canonical demotion so
    // the canonical path keeps pointing at a valid snapshot rather than
    // disappearing on a transient FS error (out of space, permissions, etc.).
    if let Err(e) = tokio::fs::rename(tmp, &cfg.base).await {
        if demoted_canonical {
            let demoted = cfg.rotated_path(1);
            if let Err(re) = tokio::fs::rename(&demoted, &cfg.base).await {
                error!(
                    "snapshot promotion failed and rollback also failed; \
                     canonical snapshot may be missing — recover from {}: {}",
                    demoted.display(),
                    re
                );
            }
        }
        return Err(e);
    }
    Ok(())
}

/// Shutdown signals: SIGTERM (sent by `docker stop`, Kubernetes, systemd, and
/// Slurm) and SIGINT/Ctrl+C. On Unix the handlers are installed eagerly by
/// [`ShutdownSignals::install`], so call it before advertising readiness. The
/// first signal starts a graceful drain; a second one aborts the drain.
struct ShutdownSignals {
    #[cfg(unix)]
    term: Option<tokio::signal::unix::Signal>,
    #[cfg(unix)]
    int: Option<tokio::signal::unix::Signal>,
}

impl ShutdownSignals {
    fn install() -> Self {
        #[cfg(unix)]
        {
            use tokio::signal::unix::SignalKind;
            Self {
                term: register_signal(SignalKind::terminate(), "SIGTERM"),
                int: register_signal(SignalKind::interrupt(), "SIGINT"),
            }
        }
        #[cfg(not(unix))]
        Self {}
    }

    /// Wait for the next shutdown signal and return its name.
    async fn recv(&mut self) -> &'static str {
        #[cfg(unix)]
        {
            async fn next(sig: &mut Option<tokio::signal::unix::Signal>) {
                match sig {
                    Some(s) => {
                        s.recv().await;
                    }
                    None => std::future::pending().await,
                }
            }
            tokio::select! {
                _ = next(&mut self.term) => "SIGTERM",
                _ = next(&mut self.int) => "SIGINT",
            }
        }
        #[cfg(not(unix))]
        {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C handler");
            "Ctrl+C"
        }
    }
}

/// Resolves on stdin EOF when `enabled`, otherwise never. Used by
/// `torc --standalone` to tie the server's lifetime to the parent process —
/// when the parent dies for any reason (including std::process::exit, which
/// bypasses destructors), the kernel closes the pipe write end, stdin sees EOF
/// here, and the server shuts down.
async fn stdin_eof(enabled: bool) {
    if !enabled {
        return std::future::pending().await;
    }
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    std::thread::spawn(move || {
        use std::io::Read;
        let mut buf = [0u8; 1024];
        let mut lock = std::io::stdin().lock();
        while let Ok(n) = lock.read(&mut buf) {
            if n == 0 {
                break;
            }
        }
        let _ = tx.send(());
    });
    let _ = rx.await;
}

/// Tell every connection to finish its in-flight request and close (idle
/// keep-alive connections close at once), then wait up to 30 seconds for them.
/// A second shutdown signal aborts the wait.
async fn drain_connections(
    graceful: hyper_util::server::graceful::GracefulShutdown,
    mut tasks: tokio::task::JoinSet<Result<(), ()>>,
    signals: &mut ShutdownSignals,
    kind: &str,
) {
    if tasks.is_empty() {
        return;
    }
    info!(
        "Waiting up to 30 seconds for {} active {} connections to finish \
         (signal again to force shutdown)...",
        tasks.len(),
        kind
    );
    let drain = async {
        tokio::join!(graceful.shutdown(), async {
            while tasks.join_next().await.is_some() {}
        })
    };
    tokio::select! {
        _ = drain => return,
        _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
            info!("Timeout waiting for {} connections", kind);
        }
        sig = signals.recv() => {
            info!("Received {} during drain, forcing shutdown", sig);
        }
    }
    info!("Aborting {} remaining {} connections", tasks.len(), kind);
    tasks.abort_all();
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn create_server(
    addr: &str,
    https: bool,
    pool: SqlitePool,
    htpasswd: Option<HtpasswdFile>,
    require_auth: bool,
    credential_cache_ttl_secs: u64,
    enforce_access_control: bool,
    admin_users: Vec<String>,
    #[allow(unused_variables)] tls_cert: Option<String>,
    #[allow(unused_variables)] tls_key: Option<String>,
    auth_file_path: Option<String>,
    shutdown_on_stdin_eof: bool,
    admin_sql: crate::server::live_state::AdminSqlConfig,
    compute_node_stale_after_secs: u64,
) -> u16 {
    let addr = tokio::net::lookup_host(addr)
        .await
        .expect("Failed to resolve bind address")
        .next()
        .expect("No addresses resolved for bind address");

    let tcp_listener = TcpListener::bind(&addr)
        .await
        .expect("Failed to bind to address");
    let actual_addr = tcp_listener
        .local_addr()
        .expect("Failed to get local address");
    let actual_port = actual_addr.port();

    // Register the SIGUSR1 handler *before* advertising readiness so a parent
    // that races between `TORC_SERVER_PORT=` and snapshot-loop spawn can't
    // accidentally kill the server (default SIGUSR1 disposition is terminate).
    #[cfg(unix)]
    let sigusr1 = register_signal(tokio::signal::unix::SignalKind::user_defined1(), "SIGUSR1");
    // Same reasoning for SIGTERM/SIGINT: an early signal drains instead of
    // killing the process.
    let mut signals = ShutdownSignals::install();
    let stdin_eof = stdin_eof(shutdown_on_stdin_eof);
    tokio::pin!(stdin_eof);

    println!("TORC_SERVER_PORT={}", actual_port);

    if let Err(e) = sync_admin_group(&pool, &admin_users).await {
        error!("Failed to sync admin group: {}", e);
    } else if !admin_users.is_empty() {
        info!(
            "Admin group synchronized with {} configured users",
            admin_users.len()
        );
    }

    let shared_htpasswd: crate::server::auth::SharedHtpasswd = Arc::new(RwLock::new(htpasswd));
    let credential_cache = if shared_htpasswd.read().is_some() && credential_cache_ttl_secs > 0 {
        Some(crate::server::credential_cache::CredentialCache::new(
            std::time::Duration::from_secs(credential_cache_ttl_secs),
        ))
    } else {
        None
    };
    let shared_credential_cache: crate::server::auth::SharedCredentialCache =
        Arc::new(RwLock::new(credential_cache));

    let server = Server::new(
        pool.clone(),
        enforce_access_control,
        shared_htpasswd.clone(),
        auth_file_path,
        shared_credential_cache.clone(),
        admin_sql,
        compute_node_stale_after_secs,
    );

    let server_clone = server.clone();
    tokio::spawn(async move {
        super::unblock_processing::background_unblock_task(server_clone).await;
    });

    #[cfg(unix)]
    if let Some(sig) = sigusr1 {
        let snapshot_pool = pool.clone();
        tokio::spawn(async move {
            snapshot_on_sigusr1(snapshot_pool, sig).await;
        });
    }

    #[cfg(feature = "openapi-codegen")]
    let app = crate::server::live_router::app_router(LiveRouterState {
        openapi_state: server.openapi_app_state(),
        server: server.clone(),
        auth: LiveAuthState {
            htpasswd: shared_htpasswd.clone(),
            require_auth,
            credential_cache: shared_credential_cache.clone(),
        },
    });

    if https {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "ios"))]
        {
            unimplemented!("SSL is not implemented for the examples on MacOS, Windows or iOS");
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "ios")))]
        {
            let key_path = tls_key.as_deref().expect(
                "--tls-key is required when --https is enabled. \
                 Provide the path to your TLS private key file (PEM format).",
            );
            let cert_path = tls_cert.as_deref().expect(
                "--tls-cert is required when --https is enabled. \
                 Provide the path to your TLS certificate chain file (PEM format).",
            );

            let mut ssl = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls())
                .expect("Failed to create SSL Acceptor");

            ssl.set_private_key_file(key_path, SslFiletype::PEM)
                .expect("Failed to set private key");
            ssl.set_certificate_chain_file(cert_path)
                .expect("Failed to set certificate chain");
            ssl.check_private_key()
                .expect("Failed to check private key");

            let tls_acceptor = ssl.build();

            info!("Starting a server (with https) on port {}", actual_port);
            let graceful = hyper_util::server::graceful::GracefulShutdown::new();

            let mut connection_tasks = tokio::task::JoinSet::new();
            let mut consecutive_accept_errors: u32 = 0;

            loop {
                while connection_tasks.try_join_next().is_some() {}

                tokio::select! {
                    result = tcp_listener.accept() => {
                        match result {
                            Ok((tcp, _)) => {
                                consecutive_accept_errors = 0;
                                let ssl = Ssl::new(tls_acceptor.context()).unwrap();
                                let _addr = tcp.peer_addr().expect("Unable to get remote address");
                                let app = app.clone();
                                let watcher = graceful.watcher();
                                connection_tasks.spawn(async move {
                                    let mut tls = tokio_openssl::SslStream::new(ssl, tcp).map_err(|_| ())?;
                                    std::pin::Pin::new(&mut tls).accept().await.map_err(|_| ())?;
                                    let hyper_service =
                                        hyper_util::service::TowerToHyperService::new(app.clone());
                                    let io = TokioIo::new(tls);

                                    let conn = HyperServerBuilder::new(TokioExecutor::new())
                                        .serve_connection(io, hyper_service)
                                        .into_owned();
                                    watcher.watch(conn).await.map_err(|_| ())
                                });
                            }
                            Err(e) => {
                                consecutive_accept_errors += 1;
                                error!("TLS accept error (consecutive: {}): {}", consecutive_accept_errors, e);
                                let delay = std::cmp::min(consecutive_accept_errors * 10, 1000);
                                tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;
                            }
                        }
                    }
                    sig = signals.recv() => {
                        info!("Received {}, gracefully shutting down...", sig);
                        break;
                    }
                    _ = &mut stdin_eof => {
                        info!("Parent process exited (stdin EOF), gracefully shutting down...");
                        break;
                    }
                }
            }

            drain_connections(graceful, connection_tasks, &mut signals, "TLS").await;

            actual_port
        }
    } else {
        info!(
            "Starting a server (over http, so no TLS) on port {}",
            actual_port
        );
        let graceful = hyper_util::server::graceful::GracefulShutdown::new();

        let mut connection_tasks = tokio::task::JoinSet::new();
        let mut consecutive_accept_errors: u32 = 0;

        loop {
            while connection_tasks.try_join_next().is_some() {}

            tokio::select! {
                result = tcp_listener.accept() => {
                    match result {
                        Ok((tcp, _addr)) => {
                            consecutive_accept_errors = 0;
                            let app = app.clone();
                            let watcher = graceful.watcher();
                            connection_tasks.spawn(async move {
                                let hyper_service =
                                    hyper_util::service::TowerToHyperService::new(app.clone());
                                let io = TokioIo::new(tcp);

                                let conn = HyperServerBuilder::new(TokioExecutor::new())
                                    .serve_connection(io, hyper_service)
                                    .into_owned();
                                watcher.watch(conn).await.map_err(|_| ())
                            });
                        }
                        Err(e) => {
                            consecutive_accept_errors += 1;
                            error!("HTTP accept error (consecutive: {}): {}", consecutive_accept_errors, e);
                            let delay = std::cmp::min(consecutive_accept_errors * 10, 1000);
                            tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;
                        }
                    }
                }
                sig = signals.recv() => {
                    info!("Received {}, gracefully shutting down...", sig);
                    break;
                }
                _ = &mut stdin_eof => {
                    info!("Parent process exited (stdin EOF), gracefully shutting down...");
                    break;
                }
            }
        }

        drain_connections(graceful, connection_tasks, &mut signals, "HTTP").await;

        actual_port
    }
}
