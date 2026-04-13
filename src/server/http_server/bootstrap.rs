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

#[allow(clippy::too_many_arguments)]
pub(super) async fn create_server(
    addr: &str,
    https: bool,
    pool: SqlitePool,
    htpasswd: Option<HtpasswdFile>,
    require_auth: bool,
    credential_cache_ttl_secs: u64,
    enforce_access_control: bool,
    completion_check_interval_secs: f64,
    admin_users: Vec<String>,
    #[allow(unused_variables)] tls_cert: Option<String>,
    #[allow(unused_variables)] tls_key: Option<String>,
    auth_file_path: Option<String>,
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
    );

    let server_clone = server.clone();
    tokio::spawn(async move {
        super::unblock_processing::background_unblock_task(
            server_clone,
            completion_check_interval_secs,
        )
        .await;
    });

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
            let shutdown = async {
                tokio::signal::ctrl_c()
                    .await
                    .expect("Failed to install Ctrl+C handler");
                info!("Received shutdown signal, gracefully shutting down TLS server...");
            };
            tokio::pin!(shutdown);

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
                                connection_tasks.spawn(async move {
                                    let mut tls = tokio_openssl::SslStream::new(ssl, tcp).map_err(|_| ())?;
                                    std::pin::Pin::new(&mut tls).accept().await.map_err(|_| ())?;
                                    let hyper_service =
                                        hyper_util::service::TowerToHyperService::new(app.clone());
                                    let io = TokioIo::new(tls);

                                    HyperServerBuilder::new(TokioExecutor::new())
                                        .serve_connection(io, hyper_service)
                                        .await
                                        .map_err(|_| ())
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
                    _ = &mut shutdown => {
                        break;
                    }
                }
            }

            if !connection_tasks.is_empty() {
                info!(
                    "Waiting up to 30 seconds for {} active TLS connections to finish...",
                    connection_tasks.len()
                );
                let drain = async { while connection_tasks.join_next().await.is_some() {} };
                if tokio::time::timeout(std::time::Duration::from_secs(30), drain)
                    .await
                    .is_err()
                {
                    info!(
                        "Timeout waiting for TLS connections, aborting {} remaining",
                        connection_tasks.len()
                    );
                    connection_tasks.abort_all();
                }
            }

            actual_port
        }
    } else {
        info!(
            "Starting a server (over http, so no TLS) on port {}",
            actual_port
        );
        let shutdown = async {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C handler");
            info!("Received shutdown signal, gracefully shutting down...");
        };
        tokio::pin!(shutdown);

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
                            connection_tasks.spawn(async move {
                                let hyper_service =
                                    hyper_util::service::TowerToHyperService::new(app.clone());
                                let io = TokioIo::new(tcp);

                                HyperServerBuilder::new(TokioExecutor::new())
                                    .serve_connection(io, hyper_service)
                                    .await
                                    .map_err(|_| ())
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
                _ = &mut shutdown => {
                    break;
                }
            }
        }

        if !connection_tasks.is_empty() {
            info!(
                "Waiting up to 30 seconds for {} active HTTP connections to finish...",
                connection_tasks.len()
            );
            let drain = async { while connection_tasks.join_next().await.is_some() {} };
            if tokio::time::timeout(std::time::Duration::from_secs(30), drain)
                .await
                .is_err()
            {
                info!(
                    "Timeout waiting for HTTP connections, aborting {} remaining",
                    connection_tasks.len()
                );
                connection_tasks.abort_all();
            }
        }

        actual_port
    }
}
