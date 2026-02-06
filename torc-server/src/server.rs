//! Main library entry point for torc implementation.

#![allow(unused_imports)]
#![allow(dead_code)]

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt, TryFutureExt, TryStreamExt, future};
use hyper::server::conn::Http;
use hyper::service::Service;
use log::{debug, error, info};
use sqlx::Row;
use std::collections::hash_set::Union;
use std::future::Future;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};
use swagger::EmptyContext;
use swagger::auth::Authorization;
use swagger::{Has, XSpanIdString};
use tokio::net::TcpListener;
use torc::models;
use torc::server::api::AccessGroupsApiImpl;
use torc::server::api::ComputeNodesApi;
use torc::server::api::EventsApi;
use torc::server::api::FailureHandlersApi;
use torc::server::api::FailureHandlersApiImpl;
use torc::server::api::FilesApi;
use torc::server::api::JobsApi;
use torc::server::api::RemoteWorkersApi;
use torc::server::api::ResourceRequirementsApi;
use torc::server::api::ResultsApi;
use torc::server::api::SchedulersApi;
use torc::server::api::UserDataApi;
use torc::server::api::WorkflowActionsApi;
use torc::server::api::WorkflowsApi;
use torc::server::api::database_error;
use torc::server::api_types::*;
use torc::server::auth::MakeHtpasswdAuthenticator;
use torc::server::authorization::{AccessCheckResult, AuthorizationService};
use torc::server::event_broadcast::{BroadcastEvent, EventBroadcaster};
use torc::server::htpasswd::HtpasswdFile;
use tracing::instrument;

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "ios")))]
use openssl::ssl::{Ssl, SslAcceptor, SslAcceptorBuilder, SslFiletype, SslMethod};

use sqlx::sqlite::SqlitePool;

const TORC_VERSION: &str = env!("CARGO_PKG_VERSION");
const GIT_HASH: &str = env!("GIT_HASH");
const GIT_DIRTY: &str = env!("GIT_DIRTY");

/// Returns the full version string including git hash (e.g., "0.8.0 (abc1234)")
fn full_version() -> String {
    format!("{} ({}{})", TORC_VERSION, GIT_HASH, GIT_DIRTY)
}

const MAX_RECORD_TRANSFER_COUNT: i64 = 10_000;

/// Process optional offset and limit parameters and return concrete values.
/// Returns (offset, limit) where:
/// - offset defaults to 0 if not provided
/// - limit defaults to 10000 if not provided
/// - Returns an error if limit exceeds 10000
fn process_pagination_params(
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<(i64, i64), ApiError> {
    let processed_offset = offset.unwrap_or(0);
    let processed_limit = limit.unwrap_or(10000);

    if processed_limit > 10000 {
        error!(
            "Limit exceeds maximum allowed value: {} > 10000",
            processed_limit
        );
        return Err(ApiError("Limit cannot exceed 10000".to_string()));
    }

    Ok((processed_offset, processed_limit))
}

/// Sync the admin group with configured admin users
///
/// Creates the "admin" system group if it doesn't exist and ensures
/// all configured admin users are members with admin role.
async fn sync_admin_group(pool: &SqlitePool, admin_users: &[String]) -> Result<(), sqlx::Error> {
    // Create admin group if it doesn't exist
    sqlx::query(
        r#"
        INSERT INTO access_group (name, description, is_system)
        VALUES ('admin', 'System administrators', 1)
        ON CONFLICT (name) DO UPDATE SET is_system = 1
        "#,
    )
    .execute(pool)
    .await?;

    // Get admin group ID
    let admin_group_id: i64 =
        sqlx::query_scalar("SELECT id FROM access_group WHERE name = 'admin'")
            .fetch_one(pool)
            .await?;

    // Get current admin group members
    let current_members: Vec<String> =
        sqlx::query_scalar("SELECT user_name FROM user_group_membership WHERE group_id = $1")
            .bind(admin_group_id)
            .fetch_all(pool)
            .await?;

    // Add missing admin users
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

    // Remove users not in config from admin group
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

/// Builds an SSL implementation for Simple HTTPS from some hard-coded file names
///
/// Returns the actual port the server bound to (useful when port 0 is specified for auto-detection)
#[allow(clippy::too_many_arguments)]
pub async fn create(
    addr: &str,
    https: bool,
    pool: SqlitePool,
    htpasswd: Option<HtpasswdFile>,
    require_auth: bool,
    credential_cache_ttl_secs: u64,
    enforce_access_control: bool,
    completion_check_interval_secs: f64,
    admin_users: Vec<String>,
) -> u16 {
    // Resolve hostname to socket address (supports both hostnames and IP addresses)
    let addr = tokio::net::lookup_host(addr)
        .await
        .expect("Failed to resolve bind address")
        .next()
        .expect("No addresses resolved for bind address");

    // Bind early to get the actual port (especially important when port 0 is used)
    let tcp_listener = TcpListener::bind(&addr)
        .await
        .expect("Failed to bind to address");
    let actual_addr = tcp_listener
        .local_addr()
        .expect("Failed to get local address");
    let actual_port = actual_addr.port();

    // Print the actual port in a machine-parseable format for tools like torc-dash
    // This MUST be printed before any other output so it can be reliably parsed
    println!("TORC_SERVER_PORT={}", actual_port);

    // Sync admin group with configured admin users
    if let Err(e) = sync_admin_group(&pool, &admin_users).await {
        error!("Failed to sync admin group: {}", e);
    } else if !admin_users.is_empty() {
        info!(
            "Admin group synchronized with {} configured users",
            admin_users.len()
        );
    }

    let server = Server::new(pool.clone(), enforce_access_control);

    // Spawn background task for deferred job unblocking
    let server_clone = server.clone();
    tokio::spawn(async move {
        background_unblock_task(server_clone, completion_check_interval_secs).await;
    });

    let service = MakeService::new(server);

    let service = MakeHtpasswdAuthenticator::with_cache_ttl(
        service,
        htpasswd,
        require_auth,
        credential_cache_ttl_secs,
    );

    #[allow(unused_mut)]
    let mut service = torc::server::context::MakeAddContext::<_, EmptyContext>::new(service);

    if https {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "ios"))]
        {
            unimplemented!("SSL is not implemented for the examples on MacOS, Windows or iOS");
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "ios")))]
        {
            let mut ssl = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls())
                .expect("Failed to create SSL Acceptor");

            // Server authentication
            ssl.set_private_key_file("examples/server-key.pem", SslFiletype::PEM)
                .expect("Failed to set private key");
            ssl.set_certificate_chain_file("examples/server-chain.pem")
                .expect("Failed to set certificate chain");
            ssl.check_private_key()
                .expect("Failed to check private key");

            let tls_acceptor = ssl.build();

            info!("Starting a server (with https) on port {}", actual_port);
            loop {
                if let Ok((tcp, _)) = tcp_listener.accept().await {
                    let ssl = Ssl::new(tls_acceptor.context()).unwrap();
                    let addr = tcp.peer_addr().expect("Unable to get remote address");
                    let service = service.call(addr);

                    tokio::spawn(async move {
                        let tls = tokio_openssl::SslStream::new(ssl, tcp).map_err(|_| ())?;
                        let service = service.await.map_err(|_| ())?;

                        Http::new()
                            .serve_connection(tls, service)
                            .await
                            .map_err(|_| ())
                    });
                }
            }

            // Note: The loop above never exits, but we need to return the port for API consistency
            #[allow(unreachable_code)]
            actual_port
        }
    } else {
        info!(
            "Starting a server (over http, so no TLS) on port {}",
            actual_port
        );
        // Using HTTP - convert tokio TcpListener to std for hyper
        let std_listener = tcp_listener
            .into_std()
            .expect("Failed to convert TcpListener");
        hyper::server::Server::from_tcp(std_listener)
            .expect("Failed to create server from TCP listener")
            .serve(service)
            .await
            .unwrap();
        actual_port
    }
}

/// Background task that periodically processes pending job unblocks.
///
/// This task uses an optimization to avoid database queries when no jobs have completed:
/// - The server tracks `last_completion_time` which is updated when any job completes
/// - This task tracks `last_checked_time` and only queries the database if
///   `last_completion_time > last_checked_time`
/// - On first run (or after server restart), `last_completion_time` is initialized to 1
///   to ensure we process any completions that occurred while the server was down
async fn background_unblock_task<C>(server: Server<C>, interval_seconds: f64)
where
    C: Has<XSpanIdString> + Send + Sync,
{
    info!(
        "Starting background job completion checker with interval = {} seconds",
        interval_seconds
    );

    let mut interval = tokio::time::interval(std::time::Duration::from_secs_f64(interval_seconds));
    let mut last_checked_time: u64 = 0;

    loop {
        interval.tick().await;

        // Check if any jobs have completed since our last check
        let completion_time = server.last_completion_time.load(Ordering::Acquire);
        if completion_time <= last_checked_time {
            // No new completions, skip database query
            debug!("No new job completions since last check, skipping unblock processing");
            continue;
        }

        // Update our checkpoint before processing
        last_checked_time = completion_time;

        if let Err(e) = process_pending_unblocks(&server).await {
            error!("Error processing pending unblocks: {}", e);
        }
    }
}

/// Process all pending job unblocks across all workflows
async fn process_pending_unblocks<C>(server: &Server<C>) -> Result<(), ApiError>
where
    C: Has<XSpanIdString> + Send + Sync,
{
    let completed_status = models::JobStatus::Completed.to_int();
    let failed_status = models::JobStatus::Failed.to_int();
    let canceled_status = models::JobStatus::Canceled.to_int();
    let terminated_status = models::JobStatus::Terminated.to_int();

    // Find all workflows with unprocessed completions
    let workflows = match sqlx::query!(
        r#"
        SELECT DISTINCT workflow_id
        FROM job
        WHERE status IN (?, ?, ?, ?)
          AND unblocking_processed = 0
        "#,
        completed_status,
        failed_status,
        canceled_status,
        terminated_status
    )
    .fetch_all(server.pool.as_ref())
    .await
    {
        Ok(workflows) => workflows,
        Err(e) => {
            error!(
                "Database error finding workflows with pending unblocks: {}",
                e
            );
            return Err(ApiError("Database error".to_string()));
        }
    };

    if workflows.is_empty() {
        return Ok(());
    }

    debug!(
        "Processing pending unblocks for {} workflows",
        workflows.len()
    );

    for workflow in workflows {
        if let Err(e) = process_workflow_unblocks(server, workflow.workflow_id).await {
            error!(
                "Error processing unblocks for workflow {}: {}",
                workflow.workflow_id, e
            );
            // Continue processing other workflows even if one fails
        }
    }

    Ok(())
}

/// Check if an error is a SQLite database lock error
fn is_database_lock_error(error: &ApiError) -> bool {
    let error_str = error.0.to_lowercase();
    error_str.contains("database is locked")
        || error_str.contains("database is busy")
        || error_str.contains("sqlite_busy")
}

/// Process all pending unblocks for a specific workflow
async fn process_workflow_unblocks<C>(server: &Server<C>, workflow_id: i64) -> Result<(), ApiError>
where
    C: Has<XSpanIdString> + Send + Sync,
{
    // Retry logic for database lock contention.
    // Note: SQLite's busy_timeout doesn't work with sqlx's BEGIN DEFERRED transactions
    // because SQLITE_BUSY is returned immediately when upgrading to a write lock.
    // We implement our own retry logic to wait up to ~45 seconds (matching busy_timeout).
    const MAX_RETRIES: u32 = 45;
    const RETRY_DELAY_MS: u64 = 1000;

    let mut last_error: Option<ApiError> = None;

    for attempt in 0..MAX_RETRIES {
        match process_workflow_unblocks_inner(server, workflow_id).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                if is_database_lock_error(&e) && attempt < MAX_RETRIES - 1 {
                    debug!(
                        "Database locked for workflow {}, retrying in {}ms (attempt {}/{})",
                        workflow_id,
                        RETRY_DELAY_MS,
                        attempt + 1,
                        MAX_RETRIES
                    );
                    last_error = Some(e);
                    tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                    continue;
                }
                // Non-retryable error or final attempt
                return Err(e);
            }
        }
    }

    // If we exhausted all retries, return the last error
    Err(last_error.unwrap_or_else(|| ApiError("Unknown error in retry loop".to_string())))
}

/// Inner implementation of process_workflow_unblocks (for retry logic)
async fn process_workflow_unblocks_inner<C>(
    server: &Server<C>,
    workflow_id: i64,
) -> Result<(), ApiError>
where
    C: Has<XSpanIdString> + Send + Sync,
{
    let completed_status = models::JobStatus::Completed.to_int();
    let failed_status = models::JobStatus::Failed.to_int();
    let canceled_status = models::JobStatus::Canceled.to_int();
    let terminated_status = models::JobStatus::Terminated.to_int();

    let mut tx = match server.pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            debug!(
                "Failed to begin transaction for workflow {}: {}",
                workflow_id, e
            );
            return Err(ApiError(format!("Database error: {}", e)));
        }
    };

    // Get all unprocessed completions for this workflow
    let completed_jobs = match sqlx::query!(
        r#"
        SELECT j.id, r.return_code
        FROM job j
        JOIN result r ON j.id = r.job_id
        JOIN workflow_status ws ON j.workflow_id = ws.id AND r.run_id = ws.run_id
        WHERE j.workflow_id = ?
          AND j.status IN (?, ?, ?, ?)
          AND j.unblocking_processed = 0
        "#,
        workflow_id,
        completed_status,
        failed_status,
        canceled_status,
        terminated_status
    )
    .fetch_all(&mut *tx)
    .await
    {
        Ok(jobs) => jobs,
        Err(e) => {
            debug!(
                "Database error fetching completed jobs for workflow {}: {}",
                workflow_id, e
            );
            return Err(ApiError(format!("Database error: {}", e)));
        }
    };

    if completed_jobs.is_empty() {
        return Ok(());
    }

    debug!(
        "Processing {} completed jobs for workflow {}",
        completed_jobs.len(),
        workflow_id
    );

    // Check if any job in this batch failed
    let batch_has_failures = completed_jobs.iter().any(|j| j.return_code != 0);

    // Check if we've seen failures before for this workflow (from previous batches)
    let workflow_has_prior_failures = server
        .workflows_with_failures
        .read()
        .map(|set| set.contains(&workflow_id))
        .unwrap_or(true); // Assume failures on lock error

    // If this batch has failures, record it for future batches
    if batch_has_failures && let Ok(mut set) = server.workflows_with_failures.write() {
        set.insert(workflow_id);
    }

    // Combine: workflow has failures if either this batch or prior batches had failures
    let workflow_has_failures = batch_has_failures || workflow_has_prior_failures;

    // Track all jobs that become ready for action triggering
    let mut all_ready_job_ids = Vec::new();

    // Process unblocking for each completed job
    for job in &completed_jobs {
        match Server::<EmptyContext>::unblock_jobs_waiting_for_tx(
            &mut tx,
            workflow_id,
            job.id,
            job.return_code,
            workflow_has_failures,
        )
        .await
        {
            Ok(ready_job_ids) => {
                all_ready_job_ids.extend(ready_job_ids);
            }
            Err(e) => {
                debug!(
                    "Error unblocking jobs for completed job {} in workflow {}: {}",
                    job.id, workflow_id, e
                );
                return Err(e);
            }
        }
    }

    // Mark all as processed
    let job_ids: Vec<i64> = completed_jobs.iter().map(|j| j.id).collect();
    let job_ids_str = job_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");

    // SAFETY: job_ids are i64 from database query results (line 252-269).
    // i64::to_string() only produces numeric strings - SQL injection impossible.
    // Using string formatting because sqlx doesn't support parameterized IN clauses.
    let sql = format!(
        "UPDATE job SET unblocking_processed = 1 WHERE id IN ({})",
        job_ids_str
    );

    if let Err(e) = sqlx::query(&sql).execute(&mut *tx).await {
        debug!(
            "Database error marking jobs as processed for workflow {}: {}",
            workflow_id, e
        );
        return Err(ApiError(format!("Database error: {}", e)));
    }

    // Commit the transaction
    if let Err(e) = tx.commit().await {
        debug!(
            "Failed to commit transaction for workflow {}: {}",
            workflow_id, e
        );
        return Err(ApiError(format!("Database error: {}", e)));
    }

    info!(
        "Jobs unblocked workflow_id={} completed_count={} ready_count={}",
        workflow_id,
        completed_jobs.len(),
        all_ready_job_ids.len()
    );

    // After committing, trigger on_jobs_ready actions for jobs that became ready
    // This must be done AFTER the transaction commit to avoid SQLite database locks
    if !all_ready_job_ids.is_empty() {
        debug!(
            "process_workflow_unblocks: checking on_jobs_ready actions for {} jobs that became ready",
            all_ready_job_ids.len()
        );

        // Trigger on_jobs_ready actions that involve the newly ready jobs
        if let Err(e) = server
            .workflow_actions_api
            .check_and_trigger_actions(
                workflow_id,
                "on_jobs_ready",
                Some(all_ready_job_ids.clone()),
            )
            .await
        {
            error!(
                "Failed to check_and_trigger_actions for on_jobs_ready: {}",
                e
            );
            // Don't fail the entire operation, but log the error
        }
    }

    Ok(())
}

#[derive(Clone)]
pub struct Server<C> {
    marker: PhantomData<C>,
    pool: Arc<SqlitePool>,
    /// Timestamp (Unix millis) of the last job completion. Used by the background
    /// unblock task to skip processing when no new completions have occurred.
    last_completion_time: Arc<AtomicU64>,
    /// Tracks workflows that have had job failures in the current run.
    /// Used to skip expensive cancellation queries when all jobs succeed.
    /// Cleared when a workflow is reset/restarted.
    workflows_with_failures: Arc<std::sync::RwLock<std::collections::HashSet<i64>>>,
    /// Authorization service for access control checks
    authorization_service: AuthorizationService,
    /// Event broadcaster for SSE clients
    event_broadcaster: EventBroadcaster,
    access_groups_api: AccessGroupsApiImpl,
    compute_nodes_api: ComputeNodesApiImpl,
    events_api: EventsApiImpl,
    failure_handlers_api: FailureHandlersApiImpl,
    files_api: FilesApiImpl,
    jobs_api: JobsApiImpl,
    remote_workers_api: RemoteWorkersApiImpl,
    resource_requirements_api: ResourceRequirementsApiImpl,
    results_api: ResultsApiImpl,
    schedulers_api: SchedulersApiImpl,
    user_data_api: UserDataApiImpl,
    workflow_actions_api: WorkflowActionsApiImpl,
    workflows_api: WorkflowsApiImpl,
}

impl<C> Server<C> {
    pub fn new(pool: SqlitePool, enforce_access_control: bool) -> Self {
        let pool_arc = Arc::new(pool);
        let api_context = ApiContext::new(pool_arc.as_ref().clone());
        let authorization_service =
            AuthorizationService::new(pool_arc.clone(), enforce_access_control);

        Server {
            marker: PhantomData,
            pool: pool_arc,
            // Initialize to 1 so the background task runs at least once on startup
            // to process any completions that happened while server was down
            last_completion_time: Arc::new(AtomicU64::new(1)),
            workflows_with_failures: Arc::new(std::sync::RwLock::new(
                std::collections::HashSet::new(),
            )),
            authorization_service,
            event_broadcaster: EventBroadcaster::new(512),
            access_groups_api: AccessGroupsApiImpl::new(api_context.clone()),
            compute_nodes_api: ComputeNodesApiImpl::new(api_context.clone()),
            events_api: EventsApiImpl::new(api_context.clone()),
            failure_handlers_api: FailureHandlersApiImpl::new(api_context.clone()),
            files_api: FilesApiImpl::new(api_context.clone()),
            jobs_api: JobsApiImpl::new(api_context.clone()),
            remote_workers_api: RemoteWorkersApiImpl::new(api_context.clone()),
            resource_requirements_api: ResourceRequirementsApiImpl::new(api_context.clone()),
            results_api: ResultsApiImpl::new(api_context.clone()),
            schedulers_api: SchedulersApiImpl::new(api_context.clone()),
            user_data_api: UserDataApiImpl::new(api_context.clone()),
            workflow_actions_api: WorkflowActionsApiImpl::new(api_context.clone()),
            workflows_api: WorkflowsApiImpl::new(api_context.clone()),
        }
    }

    /// Signal that a job has completed. This wakes up the background unblock task.
    fn signal_job_completion(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(1);
        self.last_completion_time.store(now, Ordering::Release);
    }

    /// Get a reference to the event broadcaster for SSE subscriptions.
    pub fn get_event_broadcaster(&self) -> &EventBroadcaster {
        &self.event_broadcaster
    }

    /// Create an association between a job and a file.
    /// The table name must be job_input_file or job_output_file.
    async fn add_job_file_association(
        &self,
        job_id: i64,
        file_id: i64,
        workflow_id: i64,
        table: &str,
    ) -> Result<(), ApiError> {
        if table != "job_input_file" && table != "job_output_file" {
            error!(
                "Invalid table name provided for job-file association: '{}'. Must be 'job_input_file' or 'job_output_file'",
                table
            );
            return Err(ApiError(
                "Invalid table name. Must be 'job_input_file' or 'job_output_file'".to_string(),
            ));
        }

        let sql = format!(
            r#"
            INSERT INTO {}
            (
                job_id
                ,file_id
                ,workflow_id
            )
            VALUES ($1, $2, $3)
        "#,
            table
        );

        match sqlx::query(&sql)
            .bind(job_id)
            .bind(file_id)
            .bind(workflow_id)
            .execute(self.pool.as_ref())
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                info!("Database error: {}", e);
                Err(ApiError("Database error".to_string()))
            }
        }
    }

    /// Create a depends-on association between two jobs.
    async fn add_depends_on_association(
        &self,
        job_id: i64,
        depends_on_job_id: i64,
        workflow_id: i64,
    ) -> Result<(), ApiError> {
        match sqlx::query!(
            r#"
            INSERT INTO job_depends_on
            (
                job_id
                ,depends_on_job_id
                ,workflow_id
            )
            VALUES ($1, $2, $3)
        "#,
            job_id,
            depends_on_job_id,
            workflow_id,
        )
        .execute(self.pool.as_ref())
        .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Database error: {}", e);
                Err(ApiError("Database error".to_string()))
            }
        }
    }

    /// Create a depends-on association between two jobs based on file dependencies.
    async fn add_depends_on_associations_from_files<'e, E>(
        &self,
        executor: E,
        workflow_id: i64,
    ) -> Result<(), ApiError>
    where
        E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
    {
        match sqlx::query!(
            r#"INSERT OR IGNORE INTO job_depends_on (job_id, depends_on_job_id, workflow_id)
            SELECT
                i.job_id AS job_id
                ,o.job_id AS depends_on_job_id
                ,i.workflow_id AS workflow_id
            FROM job_input_file i
            JOIN job_output_file o ON i.file_id = o.file_id
            WHERE i.workflow_id = $1
            "#,
            workflow_id
        )
        .execute(executor)
        .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Database error: {}", e);
                Err(ApiError("Database error".to_string()))
            }
        }
    }

    /// Add depends-on associations based on user_data dependencies.
    /// If job A outputs user_data X and job B inputs user_data X, then job B is blocked by job A.
    async fn add_depends_on_associations_from_user_data<'e, E>(
        &self,
        executor: E,
        workflow_id: i64,
    ) -> Result<(), ApiError>
    where
        E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
    {
        match sqlx::query!(
            r#"INSERT OR IGNORE INTO job_depends_on (job_id, depends_on_job_id, workflow_id)
            SELECT
                i.job_id AS job_id
                ,o.job_id AS depends_on_job_id
                ,j.workflow_id AS workflow_id
            FROM job_input_user_data i
            JOIN job_output_user_data o ON i.user_data_id = o.user_data_id
            JOIN job j ON i.job_id = j.id
            WHERE j.workflow_id = $1
            "#,
            workflow_id
        )
        .execute(executor)
        .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Database error: {}", e);
                Err(ApiError("Database error".to_string()))
            }
        }
    }

    /// Ensure that all jobs downstream of an uninitialized job are also uninitialized.
    async fn uninitialize_blocked_jobs<'e, E>(
        &self,
        executor: E,
        workflow_id: i64,
    ) -> Result<(), ApiError>
    where
        E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
    {
        let uninitialized_status = models::JobStatus::Uninitialized.to_int();

        // Use a recursive CTE to find all jobs that should be uninitialized
        // based on the transitive closure of job dependencies
        match sqlx::query!(
            r#"
            WITH RECURSIVE jobs_to_uninitialize(job_id) AS (
                -- Base case: find all uninitialized jobs in this workflow
                SELECT id FROM job
                WHERE workflow_id = $1 AND status = $2

                UNION

                -- Recursive case: find jobs blocked by any job that should be uninitialized
                SELECT jbb.job_id
                FROM job_depends_on jbb
                JOIN jobs_to_uninitialize jtu ON jbb.depends_on_job_id = jtu.job_id
                WHERE jbb.workflow_id = $1
            )
            UPDATE job
            SET status = $2
            WHERE workflow_id = $1
            AND id IN (SELECT job_id FROM jobs_to_uninitialize)
            "#,
            workflow_id,
            uninitialized_status
        )
        .execute(executor)
        .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Database error: {}", e);
                Err(ApiError("Database error".to_string()))
            }
        }
    }

    /// Set the status of blocked jobs to blocked.
    /// Only sets a job to blocked if it has at least one incomplete blocking job.
    /// Jobs blocked only by complete jobs will be left alone (to be marked ready later).
    async fn initialize_blocked_jobs_to_blocked<'e, E>(
        &self,
        executor: E,
        workflow_id: i64,
        only_uninitialized: bool,
    ) -> Result<(), ApiError>
    where
        E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
    {
        let uninitialized_status = models::JobStatus::Uninitialized.to_int();
        let blocked_status = models::JobStatus::Blocked.to_int();
        let completed_status = models::JobStatus::Completed.to_int();
        let failed_status = models::JobStatus::Failed.to_int();
        let canceled_status = models::JobStatus::Canceled.to_int();
        let terminated_status = models::JobStatus::Terminated.to_int();

        let sql = if only_uninitialized {
            r#"
            UPDATE job
            SET status = $1
            WHERE workflow_id = $2
            AND status = $3
            AND id IN (
                SELECT DISTINCT jbb.job_id
                FROM job_depends_on jbb
                JOIN job j ON jbb.depends_on_job_id = j.id
                WHERE jbb.workflow_id = $2
                AND j.status NOT IN ($4, $5, $6, $7)
            )
            "#
        } else {
            r#"
            UPDATE job
            SET status = $1
            WHERE workflow_id = $2
            AND id IN (
                SELECT DISTINCT jbb.job_id
                FROM job_depends_on jbb
                JOIN job j ON jbb.depends_on_job_id = j.id
                WHERE jbb.workflow_id = $2
                AND j.status NOT IN ($3, $4, $5, $6)
            )
            "#
        };

        let query = if only_uninitialized {
            sqlx::query(sql)
                .bind(blocked_status)
                .bind(workflow_id)
                .bind(uninitialized_status)
                .bind(completed_status)
                .bind(failed_status)
                .bind(canceled_status)
                .bind(terminated_status)
        } else {
            sqlx::query(sql)
                .bind(blocked_status)
                .bind(workflow_id)
                .bind(completed_status)
                .bind(failed_status)
                .bind(canceled_status)
                .bind(terminated_status)
        };

        match query.execute(executor).await {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Database error: {}", e);
                Err(ApiError("Database error".to_string()))
            }
        }
    }

    /// Get the default resource requirements ID for a workflow
    async fn get_default_resource_requirements_id<Ctx>(
        &self,
        workflow_id: i64,
        context: &Ctx,
    ) -> Result<i64, ApiError>
    where
        Ctx: Has<XSpanIdString> + Send + Sync,
    {
        let result = self
            .resource_requirements_api
            .list_resource_requirements(
                workflow_id,
                None,                        // job_id
                Some("default".to_string()), // name
                None,                        // memory
                None,                        // num_cpus
                None,                        // num_gpus
                None,                        // num_nodes
                None,                        // runtime
                0,                           // offset
                1,                           // limit
                None,                        // sort_by
                None,                        // reverse_sort
                context,
            )
            .await;

        match result {
            Ok(ListResourceRequirementsResponse::SuccessfulResponse(records)) => {
                if let Some(items) = records.items {
                    if items.len() != 1 {
                        return Err(ApiError("Expected exactly 1 default resource requirement, found different number".to_string()));
                    }
                    if let Some(id) = items[0].id {
                        Ok(id)
                    } else {
                        Err(ApiError(
                            "Default resource requirement has no ID".to_string(),
                        ))
                    }
                } else {
                    Err(ApiError(
                        "No default resource requirements found".to_string(),
                    ))
                }
            }
            Ok(ListResourceRequirementsResponse::ForbiddenErrorResponse(_))
            | Ok(ListResourceRequirementsResponse::NotFoundErrorResponse(_))
            | Ok(ListResourceRequirementsResponse::DefaultErrorResponse(_)) => Err(ApiError(
                "Did not find default resource requirements".to_string(),
            )),
            Err(e) => {
                error!(
                    "Database error looking up default resource requirements: {}",
                    e
                );
                Err(ApiError("Database error".to_string()))
            }
        }
    }

    /// Set the status of all unblocked jobs to ready.
    async fn initialize_unblocked_jobs<'e, E>(
        &self,
        executor: E,
        workflow_id: i64,
    ) -> Result<(), ApiError>
    where
        E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
    {
        let disabled = models::JobStatus::Disabled.to_int();
        let blocked = models::JobStatus::Blocked.to_int();
        let completed = models::JobStatus::Completed.to_int();
        let failed = models::JobStatus::Failed.to_int();
        let canceled = models::JobStatus::Canceled.to_int();
        let ready = models::JobStatus::Ready.to_int();
        match sqlx::query!(
            r#"
            UPDATE job
            SET status = $1
            WHERE workflow_id = $2
            AND status NOT IN ($3, $4, $5, $6, $7, $8)
            "#,
            ready,
            workflow_id,
            disabled,
            blocked,
            completed,
            failed,
            canceled,
            ready,
        )
        .execute(executor)
        .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Database error: {}", e);
                Err(ApiError("Database error".to_string()))
            }
        }
    }

    /// Validate that the provided run_id matches the workflow's current run_id.
    ///
    /// # Arguments
    /// * `workflow_id` - The workflow ID to look up
    /// * `provided_run_id` - The run_id to validate
    ///
    /// # Returns
    /// * `Ok(())` if the run_id matches
    /// * `Err(String)` with an error message if validation fails
    async fn validate_run_id(&self, workflow_id: i64, provided_run_id: i64) -> Result<(), String> {
        let workflow_status = match sqlx::query!(
            "SELECT run_id FROM workflow_status WHERE id = ?",
            workflow_id
        )
        .fetch_optional(self.pool.as_ref())
        .await
        {
            Ok(Some(row)) => row,
            Ok(None) => {
                return Err(format!(
                    "Workflow status not found for workflow ID: {}",
                    workflow_id
                ));
            }
            Err(e) => {
                error!("Database error looking up workflow status: {}", e);
                return Err("Database error".to_string());
            }
        };

        if provided_run_id != workflow_status.run_id {
            return Err(format!(
                "Run ID mismatch: provided {} but workflow status has {}",
                provided_run_id, workflow_status.run_id
            ));
        }

        Ok(())
    }

    /// Manage job status change with validation and side effects.
    ///
    /// This function validates a job status change by:
    /// 1. Looking up the current job status from the database
    /// 2. Comparing with the new status and returning early if they match
    /// 3. Validating the run_id matches the workflow status run_id
    /// 4. For terminal statuses (done, canceled, terminated), ensuring a result exists
    ///
    /// # Arguments
    /// * `job` - The JobModel containing the new status and job ID
    /// * `run_id` - The run_id to validate against workflow status
    ///
    /// # Returns
    /// * `Ok(())` if the status change is valid
    /// * `Err(ApiError)` if validation fails
    ///
    /// # Errors
    /// This function will return an error if:
    /// - The job ID is missing from the JobModel
    /// - The job status is missing from the JobModel
    /// - The job is not found in the database
    /// - The workflow status is not found
    /// - The run_id does not match the workflow's run_id
    /// - A terminal status is requested but no result exists for the job and run_id
    async fn manage_job_status_change(
        &self,
        job: &models::JobModel,
        run_id: i64,
    ) -> Result<(), ApiError> {
        let job_id = job
            .id
            .ok_or_else(|| ApiError("Job ID is required".to_string()))?;
        let new_status = job
            .status
            .as_ref()
            .ok_or_else(|| ApiError("Job status is required".to_string()))?;

        debug!(
            "manage_job_status_change: job_id={}, new_status={}, run_id={}",
            job_id, new_status, run_id
        );

        // 1. Look up current job status from database
        let current_job =
            match sqlx::query!("SELECT status, workflow_id FROM job WHERE id = ?", job_id)
                .fetch_optional(self.pool.as_ref())
                .await
            {
                Ok(Some(row)) => row,
                Ok(None) => {
                    error!("Job not found with ID: {}", job_id);
                    return Err(ApiError(format!("Job not found with ID: {}", job_id)));
                }
                Err(e) => {
                    error!("Database error looking up job: {}", e);
                    return Err(ApiError("Database error".to_string()));
                }
            };

        // Parse current status
        let current_status = match models::JobStatus::from_int(current_job.status as i32) {
            Ok(status) => status,
            Err(e) => {
                error!(
                    "Failed to parse current job status '{}': {}",
                    current_job.status, e
                );
                return Err(ApiError("Invalid current job status".to_string()));
            }
        };

        // If new status matches current status, return early and do nothing
        if current_status == *new_status {
            debug!(
                "manage_job_status_change: job_id={} already has status '{}', no change needed",
                job_id, current_status
            );
            return Ok(());
        }

        debug!(
            "manage_job_status_change: job_id={} status change from '{}' to '{}'",
            job_id, current_status, new_status
        );

        // 2. Validate run_id matches workflow status run_id
        if let Err(e) = self.validate_run_id(current_job.workflow_id, run_id).await {
            error!("manage_job_status_change: {}", e);
            return Err(ApiError(e));
        }

        // 3. For terminal statuses, check if result exists
        if new_status.is_complete() {
            let result_record = match sqlx::query!(
                "SELECT return_code FROM result WHERE job_id = ? AND run_id = ?",
                job_id,
                run_id
            )
            .fetch_optional(self.pool.as_ref())
            .await
            {
                Ok(result) => result,
                Err(e) => {
                    error!("Database error checking for result: {}", e);
                    return Err(ApiError("Database error".to_string()));
                }
            };

            if result_record.is_none() {
                error!(
                    "No result found for job ID {} and run_id {}",
                    job_id, run_id
                );
                return Err(ApiError(format!(
                    "No result found for job ID {} and run_id {} when transitioning to terminal status '{}'",
                    job_id, run_id, new_status
                )));
            }
        }

        // 4. Update the job status in the database
        // If transitioning to a complete status, mark as needing unblock processing
        let new_status_int = new_status.to_int();

        if new_status.is_complete() {
            // Set unblocking_processed = 0 so background task will process this
            match sqlx::query!(
                "UPDATE job SET status = ?, unblocking_processed = 0 WHERE id = ?",
                new_status_int,
                job_id
            )
            .execute(self.pool.as_ref())
            .await
            {
                Ok(result) => {
                    if result.rows_affected() == 0 {
                        error!(
                            "No rows affected for job ID {} when updating status",
                            job_id
                        );
                        return Err(ApiError(format!(
                            "Failed to update job status: no rows affected for job ID {}",
                            job_id
                        )));
                    }
                }
                Err(e) => {
                    error!("Database error updating job status: {}", e);
                    return Err(ApiError(format!(
                        "Database error updating job status: {}",
                        e
                    )));
                }
            }
            // Signal that a job completed so the background task knows to check
            self.signal_job_completion();
            debug!(
                "Marked job {} as complete, unblocking will be processed by background task",
                job_id
            );
        } else {
            // For non-complete statuses, just update status
            match sqlx::query!(
                "UPDATE job SET status = ? WHERE id = ?",
                new_status_int,
                job_id
            )
            .execute(self.pool.as_ref())
            .await
            {
                Ok(result) => {
                    if result.rows_affected() == 0 {
                        error!(
                            "No rows affected for job ID {} when updating status",
                            job_id
                        );
                        return Err(ApiError(format!(
                            "Failed to update job status: no rows affected for job ID {}",
                            job_id
                        )));
                    }
                }
                Err(e) => {
                    error!("Database error updating job status: {}", e);
                    return Err(ApiError(format!(
                        "Database error updating job status: {}",
                        e
                    )));
                }
            }
        }

        // 6. If reverting from complete to non-complete status, reset downstream jobs
        if current_status.is_complete() && !new_status.is_complete() {
            debug!(
                "manage_job_status_change: reverting completed job_id={}, resetting downstream jobs",
                job_id
            );
            self.update_jobs_from_completion_reversal(job_id).await?;
        }

        Ok(())
    }

    /// Fast path for unblocking jobs after a successful completion (return_code = 0).
    ///
    /// Uses bulk UPDATE statements instead of individual updates for better performance.
    /// Handles cancellation of jobs with failed dependencies before marking remaining jobs as ready.
    ///
    /// # Arguments
    /// * `tx` - The database transaction to use
    /// * `workflow_id` - The workflow ID containing the jobs
    /// * `completed_job_id` - The ID of the job that just completed
    /// * `workflow_has_failures` - Whether any job in this workflow has failed (from in-memory tracking)
    ///
    /// # Returns
    /// * `Ok(Vec<i64>)` - IDs of jobs that became ready
    /// * `Err(ApiError)` if there's a database error
    async fn unblock_jobs_fast_path_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        workflow_id: i64,
        completed_job_id: i64,
        workflow_has_failures: bool,
    ) -> Result<Vec<i64>, ApiError> {
        let completed_status = models::JobStatus::Completed.to_int();
        let failed_status = models::JobStatus::Failed.to_int();
        let canceled_status = models::JobStatus::Canceled.to_int();
        let terminated_status = models::JobStatus::Terminated.to_int();
        let ready_status = models::JobStatus::Ready.to_int();
        let blocked_status = models::JobStatus::Blocked.to_int();

        // Only run the expensive cancellation query if we know there are failed jobs.
        // The workflow_has_failures flag is tracked in memory to avoid database queries.
        if workflow_has_failures {
            let _canceled = sqlx::query!(
                r#"
                UPDATE job
                SET status = ?
                WHERE workflow_id = ?
                  AND status = ?
                  AND cancel_on_blocking_job_failure = 1
                  AND id IN (
                      SELECT jbb.job_id
                      FROM job_depends_on jbb
                      WHERE jbb.depends_on_job_id = ?
                        AND jbb.workflow_id = ?
                        AND NOT EXISTS (
                            SELECT 1
                            FROM job_depends_on jbb2
                            JOIN job j2 ON jbb2.depends_on_job_id = j2.id
                            WHERE jbb2.job_id = jbb.job_id
                              AND jbb2.depends_on_job_id != ?
                              AND j2.status NOT IN (?, ?, ?, ?)
                        )
                  )
                  AND EXISTS (
                      SELECT 1
                      FROM job_depends_on jbb_fail
                      JOIN job j_fail ON jbb_fail.depends_on_job_id = j_fail.id
                      JOIN result r_fail ON j_fail.id = r_fail.job_id
                      JOIN workflow_status ws ON j_fail.workflow_id = ws.id AND r_fail.run_id = ws.run_id
                      WHERE jbb_fail.job_id = job.id
                        AND jbb_fail.workflow_id = ?
                        AND j_fail.status IN (?, ?, ?)
                        AND r_fail.return_code != 0
                  )
                "#,
                canceled_status,
                workflow_id,
                blocked_status,
                completed_job_id,
                workflow_id,
                completed_job_id,
                completed_status,
                failed_status,
                canceled_status,
                terminated_status,
                workflow_id,
                failed_status,
                canceled_status,
                terminated_status
            )
            .execute(&mut **tx)
            .await;
        }

        // Then, mark remaining unblocked jobs as ready (those without failed dependencies
        // or without cancel_on_blocking_job_failure)
        let updated_jobs = match sqlx::query!(
            r#"
            UPDATE job
            SET status = ?
            WHERE workflow_id = ?
              AND status = ?
              AND id IN (
                  SELECT jbb.job_id
                  FROM job_depends_on jbb
                  WHERE jbb.depends_on_job_id = ?
                    AND jbb.workflow_id = ?
                    AND NOT EXISTS (
                        SELECT 1
                        FROM job_depends_on jbb2
                        JOIN job j2 ON jbb2.depends_on_job_id = j2.id
                        WHERE jbb2.job_id = jbb.job_id
                          AND jbb2.depends_on_job_id != ?
                          AND j2.status NOT IN (?, ?, ?, ?)
                    )
              )
            RETURNING id
            "#,
            ready_status,
            workflow_id,
            blocked_status,
            completed_job_id,
            workflow_id,
            completed_job_id,
            completed_status,
            failed_status,
            canceled_status,
            terminated_status
        )
        .fetch_all(&mut **tx)
        .await
        {
            Ok(rows) => rows,
            Err(e) => {
                debug!("Fast path bulk update failed: {}", e);
                return Err(ApiError(format!("Database error: {}", e)));
            }
        };

        let ready_job_ids: Vec<i64> = updated_jobs.iter().map(|r| r.id).collect();
        debug!(
            "unblock_jobs_fast_path_tx: updated {} jobs to ready after completion of job_id={}",
            ready_job_ids.len(),
            completed_job_id
        );
        Ok(ready_job_ids)
    }

    /// Slow path for unblocking jobs after a failed completion (return_code != 0).
    ///
    /// Uses a recursive CTE to handle cascading cancellations when a job fails.
    /// This is more expensive than the fast path but necessary for proper cancellation propagation.
    ///
    /// # Arguments
    /// * `tx` - The database transaction to use
    /// * `workflow_id` - The workflow ID containing the jobs
    /// * `completed_job_id` - The ID of the job that just completed
    ///
    /// # Returns
    /// * `Ok(Vec<i64>)` - IDs of jobs that became ready
    /// * `Err(ApiError)` if there's a database error
    async fn unblock_jobs_slow_path_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        workflow_id: i64,
        completed_job_id: i64,
    ) -> Result<Vec<i64>, ApiError> {
        let completed_status = models::JobStatus::Completed.to_int();
        let failed_status = models::JobStatus::Failed.to_int();
        let canceled_status = models::JobStatus::Canceled.to_int();
        let terminated_status = models::JobStatus::Terminated.to_int();
        let ready_status = models::JobStatus::Ready.to_int();
        let blocked_status = models::JobStatus::Blocked.to_int();

        // Use a recursive CTE to find all jobs that need status updates (including cascading
        // cancellations). The CTE propagates return codes through the dependency chain to
        // determine final status.
        let jobs_to_update = match sqlx::query(
            r#"
            WITH RECURSIVE jobs_to_process(job_id, should_cancel, level) AS (
                -- Base case: find jobs directly blocked by the completed job
                SELECT
                    jbb.job_id,
                    -- Cancel if: ANY dependency has failed (status is failed/canceled/terminated with non-zero return code)
                    -- AND cancel_on_blocking_job_failure = true. We check ALL failed dependencies, not just the current one.
                    CASE
                        WHEN j.cancel_on_blocking_job_failure != 0 AND EXISTS (
                            SELECT 1
                            FROM job_depends_on jbb_dep
                            JOIN job j_dep ON jbb_dep.depends_on_job_id = j_dep.id
                            JOIN result r_dep ON j_dep.id = r_dep.job_id
                            JOIN workflow_status ws ON j_dep.workflow_id = ws.id AND r_dep.run_id = ws.run_id
                            WHERE jbb_dep.job_id = jbb.job_id
                              AND jbb_dep.workflow_id = ?
                              AND j_dep.status IN (?, ?, ?)
                              AND r_dep.return_code != 0
                        ) THEN 1
                        ELSE 0
                    END as should_cancel,
                    0 as level
                FROM job_depends_on jbb
                JOIN job j ON jbb.job_id = j.id
                WHERE jbb.depends_on_job_id = ?
                  AND jbb.workflow_id = ?
                  AND j.status = ?
                  -- Only process if no other incomplete blocking jobs exist
                  AND NOT EXISTS (
                      SELECT 1
                      FROM job_depends_on jbb2
                      JOIN job j2 ON jbb2.depends_on_job_id = j2.id
                      WHERE jbb2.job_id = jbb.job_id
                        AND jbb2.depends_on_job_id != ?
                        AND j2.status NOT IN (?, ?, ?, ?)
                  )

                UNION ALL

                -- Recursive case: find jobs blocked by jobs that will be canceled
                SELECT
                    jbb.job_id,
                    -- Propagate cancellation: if parent is canceled and child has cancel_on_blocking_job_failure = true
                    CASE
                        WHEN jtp.should_cancel = 1 AND j.cancel_on_blocking_job_failure != 0 THEN 1
                        ELSE 0
                    END as should_cancel,
                    jtp.level + 1 as level
                FROM jobs_to_process jtp
                JOIN job_depends_on jbb ON jbb.depends_on_job_id = jtp.job_id
                JOIN job j ON jbb.job_id = j.id
                WHERE jtp.should_cancel = 1  -- Only cascade from jobs that will be canceled
                  AND jbb.workflow_id = ?
                  AND j.status = ?
                  AND jtp.level < 100  -- Prevent infinite loops
                  -- Only process if no other incomplete blocking jobs exist
                  AND NOT EXISTS (
                      SELECT 1
                      FROM job_depends_on jbb2
                      JOIN job j2 ON jbb2.depends_on_job_id = j2.id
                      WHERE jbb2.job_id = jbb.job_id
                        AND jbb2.depends_on_job_id != jtp.job_id
                        AND j2.status NOT IN (?, ?, ?, ?)
                  )
            )
            SELECT
                jtp.job_id,
                jtp.should_cancel,
                j.resource_requirements_id
            FROM jobs_to_process jtp
            JOIN job j ON jtp.job_id = j.id
            ORDER BY jtp.level ASC  -- Process in dependency order
            "#,
        )
        .bind(workflow_id)           // Base case: workflow_id for subquery
        .bind(failed_status)         // Base case: failed statuses for subquery (not completed - status is source of truth)
        .bind(canceled_status)
        .bind(terminated_status)
        .bind(completed_job_id)      // Base case: depends_on_job_id
        .bind(workflow_id)           // Base case: workflow_id
        .bind(blocked_status)        // Base case: only process blocked jobs
        .bind(completed_job_id)      // Base case: exclude the completed job itself
        .bind(completed_status)      // Base case: complete statuses
        .bind(failed_status)
        .bind(canceled_status)
        .bind(terminated_status)
        .bind(workflow_id)           // Recursive case: workflow_id
        .bind(blocked_status)        // Recursive case: only process blocked jobs
        .bind(completed_status)      // Recursive case: complete statuses
        .bind(failed_status)
        .bind(canceled_status)
        .bind(terminated_status)
        .fetch_all(&mut **tx)
        .await
        {
            Ok(rows) => rows,
            Err(e) => {
                error!("Database error finding jobs to unblock: {}", e);
                return Err(ApiError("Database error".to_string()));
            }
        };

        if jobs_to_update.is_empty() {
            debug!(
                "unblock_jobs_slow_path_tx: no jobs to unblock after completion of job_id={}",
                completed_job_id
            );
            return Ok(Vec::new());
        }

        debug!(
            "unblock_jobs_slow_path_tx: found {} jobs to update after completion of job_id={}",
            jobs_to_update.len(),
            completed_job_id
        );

        // Update job statuses
        let mut updated_count = 0;
        let mut ready_count = 0;
        let mut canceled_count = 0;
        let mut ready_job_ids = Vec::new();

        for row in jobs_to_update {
            let job_id: i64 = row.get("job_id");
            let should_cancel: i64 = row.get("should_cancel");
            let new_status = if should_cancel != 0 {
                canceled_status
            } else {
                ready_status
            };

            // Update job status
            match sqlx::query!(
                "UPDATE job SET status = ? WHERE id = ? AND workflow_id = ? AND status = ?",
                new_status,
                job_id,
                workflow_id,
                blocked_status
            )
            .execute(&mut **tx)
            .await
            {
                Ok(result) => {
                    if result.rows_affected() > 0 {
                        updated_count += 1;
                        if new_status == ready_status {
                            ready_count += 1;
                            ready_job_ids.push(job_id);
                        } else {
                            canceled_count += 1;
                        }

                        debug!(
                            "unblock_jobs_slow_path_tx: updated job_id={} to status={}",
                            job_id,
                            if new_status == ready_status {
                                "ready"
                            } else {
                                "canceled"
                            }
                        );
                    }
                }
                Err(e) => {
                    error!("Database error updating job {} status: {}", job_id, e);
                    return Err(ApiError("Database error".to_string()));
                }
            }
        }

        debug!(
            "unblock_jobs_slow_path_tx: updated {} jobs ({} ready, {} canceled) after completion of job_id={}",
            updated_count, ready_count, canceled_count, completed_job_id
        );

        Ok(ready_job_ids)
    }

    /// Unblock jobs using a provided transaction (for batch processing).
    ///
    /// This is the internal implementation that accepts a transaction parameter,
    /// allowing the background task to process multiple completions in one transaction.
    ///
    /// # Arguments
    /// * `tx` - The database transaction to use
    /// * `completed_job_id` - The ID of the job that just completed
    /// * `workflow_id` - The workflow ID containing the jobs
    /// * `return_code` - The return code of the completed job (0 = success, non-zero = failure)
    /// * `workflow_has_failures` - Whether any job in this workflow has failed (used to skip cancellation checks)
    ///
    /// # Returns
    /// * `Ok(Vec<i64>)` - IDs of jobs that became ready (for triggering actions)
    /// * `Err(ApiError)` if there's a database error
    async fn unblock_jobs_waiting_for_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        workflow_id: i64,
        completed_job_id: i64,
        return_code: i64,
        workflow_has_failures: bool,
    ) -> Result<Vec<i64>, ApiError> {
        debug!(
            "unblock_jobs_waiting_for_tx: checking jobs blocked by job_id={} in workflow={} with return_code={} workflow_has_failures={}",
            completed_job_id, workflow_id, return_code, workflow_has_failures
        );

        let completed_status = models::JobStatus::Completed.to_int();
        let failed_status = models::JobStatus::Failed.to_int();
        let canceled_status = models::JobStatus::Canceled.to_int();
        let terminated_status = models::JobStatus::Terminated.to_int();
        let blocked_status = models::JobStatus::Blocked.to_int();

        // Quick pre-check: Are there ANY blocked jobs that depend on this completed job
        // AND have no other incomplete dependencies? If not, skip the expensive CTE.
        // This optimization is critical for barrier patterns where 1000 jobs complete
        // but only the last one actually unblocks the barrier.
        let has_unblockable_jobs = match sqlx::query!(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM job_depends_on jbb
                JOIN job j ON jbb.job_id = j.id
                WHERE jbb.depends_on_job_id = ?
                  AND jbb.workflow_id = ?
                  AND j.status = ?
                  AND NOT EXISTS (
                      SELECT 1
                      FROM job_depends_on jbb2
                      JOIN job j2 ON jbb2.depends_on_job_id = j2.id
                      WHERE jbb2.job_id = jbb.job_id
                        AND jbb2.depends_on_job_id != ?
                        AND j2.status NOT IN (?, ?, ?, ?)
                  )
            ) as has_jobs
            "#,
            completed_job_id,
            workflow_id,
            blocked_status,
            completed_job_id,
            completed_status,
            failed_status,
            canceled_status,
            terminated_status
        )
        .fetch_one(&mut **tx)
        .await
        {
            Ok(row) => row.has_jobs != 0,
            Err(e) => {
                debug!("Pre-check query failed: {}", e);
                // If pre-check fails, fall through to full query
                true
            }
        };

        if !has_unblockable_jobs {
            debug!(
                "unblock_jobs_waiting_for_tx: quick check found no unblockable jobs for job_id={}",
                completed_job_id
            );
            return Ok(Vec::new());
        }

        // Dispatch to appropriate path based on return code
        if return_code == 0 {
            // Fast path for successful completions: uses bulk updates
            Self::unblock_jobs_fast_path_tx(
                tx,
                workflow_id,
                completed_job_id,
                workflow_has_failures,
            )
            .await
        } else {
            // Slow path for failed completions: uses recursive CTE for cascading cancellations
            Self::unblock_jobs_slow_path_tx(tx, workflow_id, completed_job_id).await
        }
    }

    /// Unblock jobs that were waiting for a completed job.
    ///
    /// This function finds all jobs that were blocked by the completed job and checks if they
    /// can now be unblocked. A job can be unblocked if it's no longer blocked by any incomplete jobs.
    /// Uses a recursive CTE to handle cascading cancellations in a single database transaction with
    /// an IMMEDIATE lock to prevent concurrent modifications.
    ///
    /// # Arguments
    /// * `completed_job_id` - The ID of the job that just completed
    /// * `workflow_id` - The workflow ID containing the jobs
    /// * `return_code` - The return code of the completed job (0 = success, non-zero = failure)
    ///
    /// # Returns
    /// * `Ok(())` if the operation succeeds
    /// * `Err(ApiError)` if there's a database error
    async fn unblock_jobs_waiting_for(
        &self,
        completed_job_id: i64,
        workflow_id: i64,
        return_code: i64,
    ) -> Result<(), ApiError> {
        // Begin a transaction
        let mut tx = match self.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => {
                error!("Failed to begin transaction: {}", e);
                return Err(ApiError("Database error".to_string()));
            }
        };

        // Call the transaction-based helper
        // Pass true for workflow_has_failures since this codepath doesn't have access to
        // in-memory failure tracking (used outside background batch processing)
        let ready_job_ids = Self::unblock_jobs_waiting_for_tx(
            &mut tx,
            workflow_id,
            completed_job_id,
            return_code,
            true, // Conservative: assume failures possible, let DB pre-check optimize
        )
        .await?;

        // Commit the transaction
        if let Err(e) = tx.commit().await {
            error!("Failed to commit transaction: {}", e);
            return Err(ApiError("Database error".to_string()));
        }

        // After committing, trigger on_jobs_ready actions for jobs that became ready
        // This must be done AFTER the transaction commit to avoid SQLite database locks
        if !ready_job_ids.is_empty() {
            debug!(
                "unblock_jobs_waiting_for: checking on_jobs_ready actions for {} jobs that became ready",
                ready_job_ids.len()
            );

            // Trigger on_jobs_ready actions that involve the newly ready jobs
            if let Err(e) = self
                .workflow_actions_api
                .check_and_trigger_actions(
                    workflow_id,
                    "on_jobs_ready",
                    Some(ready_job_ids.clone()),
                )
                .await
            {
                error!(
                    "Failed to check_and_trigger_actions for on_jobs_ready: {}",
                    e
                );
                // Don't fail the entire operation, but log the error
            }
        }

        Ok(())
    }

    /// Reinitialize downstream jobs when a completed job is set back to uninitialized.
    ///
    /// This function finds all jobs that were blocked by the specified job and changes
    /// any that are in a "done" status back to "uninitialized".
    ///
    /// # Arguments
    /// * `job_id` - The ID of the job that was reset to uninitialized
    /// * `workflow_id` - The workflow ID containing the jobs
    ///
    /// # Returns
    /// * `Ok(())` if the operation succeeds
    /// * `Err(ApiError)` if there's a database error
    async fn reinitialize_downstream_jobs(
        &self,
        job_id: i64,
        workflow_id: i64,
    ) -> Result<(), ApiError> {
        debug!(
            "reinitialize_downstream_jobs: resetting downstream jobs for job_id={} in workflow={}",
            job_id, workflow_id
        );

        let completed_status = models::JobStatus::Completed.to_int();
        let failed_status = models::JobStatus::Failed.to_int();
        let uninitialized_status = models::JobStatus::Uninitialized.to_int();

        // Update downstream jobs to uninitialized in a single query using a subquery
        let result = match sqlx::query!(
            r#"
            UPDATE job
            SET status = ?
            WHERE workflow_id = ?
            AND id IN (
                SELECT DISTINCT jbb.job_id
                FROM job_depends_on jbb
                JOIN job j ON jbb.job_id = j.id
                WHERE jbb.depends_on_job_id = ?
                AND jbb.workflow_id = ?
                AND j.status IN (?, ?)
            )
            "#,
            uninitialized_status,
            workflow_id,
            job_id,
            workflow_id,
            completed_status,
            failed_status
        )
        .execute(self.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("Database error reinitializing downstream jobs: {}", e);
                return Err(ApiError("Database error".to_string()));
            }
        };

        let affected_count = result.rows_affected();
        if affected_count == 0 {
            debug!(
                "reinitialize_downstream_jobs: no downstream jobs to reinitialize for job_id={}",
                job_id
            );
        } else {
            info!(
                "reinitialize_downstream_jobs: successfully reinitialized {} downstream jobs for job_id={}",
                affected_count, job_id
            );
        }

        Ok(())
    }

    /// Reset all jobs downstream of the given job to Uninitialized status
    /// This is called when a completed job needs to be reset, requiring all
    /// downstream jobs to also be reset recursively
    async fn update_jobs_from_completion_reversal(&self, job_id: i64) -> Result<(), ApiError> {
        debug!(
            "update_jobs_from_completion_reversal: resetting downstream jobs for job_id={}",
            job_id
        );

        let uninitialized_status = models::JobStatus::Uninitialized.to_int();

        // Begin a transaction with immediate lock to ensure atomicity
        // SQLx automatically uses BEGIN IMMEDIATE for SQLite when the first write occurs
        let mut tx = match self.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => {
                error!("Failed to begin transaction for completion reversal: {}", e);
                return Err(ApiError("Database error".to_string()));
            }
        };

        // Get the workflow_id for the given job_id first
        let workflow_id = match sqlx::query!("SELECT workflow_id FROM job WHERE id = ?", job_id)
            .fetch_optional(&mut *tx)
            .await
        {
            Ok(Some(row)) => row.workflow_id,
            Ok(None) => {
                error!("Job with id {} not found", job_id);
                return Err(ApiError("Job not found".to_string()));
            }
            Err(e) => {
                error!("Database error finding job {}: {}", job_id, e);
                return Err(ApiError("Database error".to_string()));
            }
        };

        // Use a recursive CTE to find all jobs downstream of the given job
        // and reset them to uninitialized status
        let result = sqlx::query!(
            r#"
            WITH RECURSIVE downstream_jobs(job_id, level) AS (
                -- Base case: find jobs directly blocked by the given job
                SELECT
                    jbb.job_id,
                    0 as level
                FROM job_depends_on jbb
                WHERE jbb.depends_on_job_id = ?
                  AND jbb.workflow_id = ?

                UNION ALL

                -- Recursive case: find jobs blocked by any downstream job
                SELECT
                    jbb.job_id,
                    dj.level + 1 as level
                FROM downstream_jobs dj
                JOIN job_depends_on jbb ON jbb.depends_on_job_id = dj.job_id
                WHERE jbb.workflow_id = ?
                  AND dj.level < 100  -- Prevent infinite loops
            )
            UPDATE job
            SET status = ?
            WHERE workflow_id = ?
              AND id IN (SELECT DISTINCT job_id FROM downstream_jobs)
            "#,
            job_id,
            workflow_id,
            workflow_id,
            uninitialized_status,
            workflow_id
        )
        .execute(&mut *tx)
        .await;

        match result {
            Ok(result) => {
                let affected_rows = result.rows_affected();
                debug!(
                    "update_jobs_from_completion_reversal: reset {} downstream jobs for job_id={}",
                    affected_rows, job_id
                );

                // Commit the transaction
                if let Err(e) = tx.commit().await {
                    error!(
                        "Failed to commit transaction for completion reversal: {}",
                        e
                    );
                    return Err(ApiError("Database error".to_string()));
                }

                info!(
                    "Successfully reset {} downstream jobs for job_id={} in workflow={}",
                    affected_rows, job_id, workflow_id
                );

                Ok(())
            }
            Err(e) => {
                error!(
                    "Database error during completion reversal for job {}: {}",
                    job_id, e
                );
                // Transaction will be automatically rolled back when tx is dropped
                Err(ApiError("Database error".to_string()))
            }
        }
    }
}

use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode,
    errors::Error as JwtError,
};
use serde::{Deserialize, Serialize};

use std::error::Error;
use swagger::ApiError;
use torc::server::routing::MakeService;
use torc::server::{
    Api, CancelWorkflowResponse, ClaimJobsBasedOnResources, ClaimNextJobsResponse,
    CompleteJobResponse, CreateComputeNodeResponse, CreateEventResponse, CreateFileResponse,
    CreateJobResponse, CreateJobsResponse, CreateLocalSchedulerResponse,
    CreateResourceRequirementsResponse, CreateResultResponse, CreateScheduledComputeNodeResponse,
    CreateSlurmSchedulerResponse, CreateUserDataResponse, CreateWorkflowResponse,
    DeleteAllResourceRequirementsResponse, DeleteAllUserDataResponse, DeleteComputeNodeResponse,
    DeleteComputeNodesResponse, DeleteEventResponse, DeleteEventsResponse, DeleteFileResponse,
    DeleteFilesResponse, DeleteJobResponse, DeleteJobsResponse, DeleteLocalSchedulerResponse,
    DeleteLocalSchedulersResponse, DeleteResourceRequirementsResponse, DeleteResultResponse,
    DeleteResultsResponse, DeleteScheduledComputeNodeResponse, DeleteScheduledComputeNodesResponse,
    DeleteSlurmSchedulerResponse, DeleteSlurmSchedulersResponse, DeleteUserDataResponse,
    DeleteWorkflowResponse, GetComputeNodeResponse, GetDotGraphResponse, GetEventResponse,
    GetFileResponse, GetJobResponse, GetLocalSchedulerResponse, GetReadyJobRequirementsResponse,
    GetResourceRequirementsResponse, GetResultResponse, GetScheduledComputeNodeResponse,
    GetSlurmSchedulerResponse, GetUserDataResponse, GetVersionResponse, GetWorkflowResponse,
    GetWorkflowStatusResponse, InitializeJobsResponse, IsWorkflowCompleteResponse,
    IsWorkflowUninitializedResponse, ListComputeNodesResponse, ListEventsResponse,
    ListFilesResponse, ListJobIdsResponse, ListJobsResponse, ListLocalSchedulersResponse,
    ListMissingUserDataResponse, ListRequiredExistingFilesResponse,
    ListResourceRequirementsResponse, ListResultsResponse, ListScheduledComputeNodesResponse,
    ListSlurmSchedulersResponse, ListUserDataResponse, ListWorkflowsResponse,
    ManageStatusChangeResponse, PingResponse, ProcessChangedJobInputsResponse,
    ResetJobStatusResponse, ResetWorkflowStatusResponse, RetryJobResponse, StartJobResponse,
    UpdateComputeNodeResponse, UpdateEventResponse, UpdateFileResponse, UpdateJobResponse,
    UpdateLocalSchedulerResponse, UpdateResourceRequirementsResponse, UpdateResultResponse,
    UpdateScheduledComputeNodeResponse, UpdateSlurmSchedulerResponse, UpdateUserDataResponse,
    UpdateWorkflowResponse, UpdateWorkflowStatusResponse,
};
use torc::time_utils::duration_string_to_seconds;

// Import the API implementations from torc library
use torc::server::api::{
    ApiContext, ComputeNodesApiImpl, EventsApiImpl, FilesApiImpl, JobsApiImpl,
    RemoteWorkersApiImpl, ResourceRequirementsApiImpl, ResultsApiImpl, SchedulersApiImpl,
    UserDataApiImpl, WorkflowActionsApiImpl, WorkflowsApiImpl,
};

impl<C> Server<C>
where
    C: Has<Option<Authorization>> + Send + Sync,
{
    /// Helper to extract authorization from context and check workflow access
    async fn check_workflow_access_for_context(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> AccessCheckResult {
        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        self.authorization_service
            .check_workflow_access(&auth, workflow_id)
            .await
    }

    /// Helper to extract authorization from context and check job access
    async fn check_job_access_for_context(&self, job_id: i64, context: &C) -> AccessCheckResult {
        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        self.authorization_service
            .check_job_access(&auth, job_id)
            .await
    }
}

#[async_trait]
impl<C> Api<C> for Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync,
{
    /// Store a compute node.
    async fn create_compute_node(
        &self,
        body: models::ComputeNodeModel,
        context: &C,
    ) -> Result<CreateComputeNodeResponse, ApiError> {
        // Check if workflow exists
        let workflow_exists = self
            .workflows_api
            .does_workflow_exist(body.workflow_id, context)
            .await?;
        if !workflow_exists {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", body.workflow_id)
            }));
            return Ok(CreateComputeNodeResponse::NotFoundErrorResponse(
                error_response,
            ));
        }

        let result = self
            .compute_nodes_api
            .create_compute_node(body.clone(), context)
            .await?;

        // Broadcast SSE event for compute node creation (start)
        if let CreateComputeNodeResponse::SuccessfulResponse(ref created) = result {
            self.event_broadcaster.broadcast(BroadcastEvent {
                workflow_id: body.workflow_id,
                timestamp: chrono::Utc::now().timestamp_millis(),
                event_type: "compute_node_started".to_string(),
                severity: models::EventSeverity::Info,
                data: serde_json::json!({
                    "compute_node_id": created.id,
                    "hostname": body.hostname,
                    "pid": body.pid,
                    "num_cpus": body.num_cpus,
                    "memory_gb": body.memory_gb,
                    "num_gpus": body.num_gpus,
                    "compute_node_type": body.compute_node_type,
                }),
            });
        }

        Ok(result)
    }

    /// Store an event.
    async fn create_event(
        &self,
        body: models::EventModel,
        context: &C,
    ) -> Result<CreateEventResponse, ApiError> {
        // Check if workflow exists
        let workflow_exists = self
            .workflows_api
            .does_workflow_exist(body.workflow_id, context)
            .await?;
        if !workflow_exists {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", body.workflow_id)
            }));
            return Ok(CreateEventResponse::NotFoundErrorResponse(error_response));
        }

        self.events_api.create_event(body, context).await
    }

    /// Store a file.
    async fn create_file(
        &self,
        file: models::FileModel,
        context: &C,
    ) -> Result<CreateFileResponse, ApiError> {
        // Check if workflow exists
        let workflow_exists = self
            .workflows_api
            .does_workflow_exist(file.workflow_id, context)
            .await?;
        if !workflow_exists {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", file.workflow_id)
            }));
            return Ok(CreateFileResponse::NotFoundErrorResponse(error_response));
        }

        self.files_api.create_file(file, context).await
    }

    /// Store a job.
    async fn create_job(
        &self,
        mut job: models::JobModel,
        context: &C,
    ) -> Result<CreateJobResponse, ApiError> {
        let workflow_exists = self
            .workflows_api
            .does_workflow_exist(job.workflow_id, context)
            .await?;
        if !workflow_exists {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", job.workflow_id)
            }));
            return Ok(CreateJobResponse::NotFoundErrorResponse(error_response));
        }

        if job.resource_requirements_id.is_none() {
            let default_id = self
                .get_default_resource_requirements_id(job.workflow_id, context)
                .await?;
            job.resource_requirements_id = Some(default_id);
        }

        self.jobs_api.create_job(job, context).await
    }

    /// Create jobs in bulk. Recommended max job count of 10,000.
    async fn create_jobs(
        &self,
        mut body: models::JobsModel,
        context: &C,
    ) -> Result<CreateJobsResponse, ApiError> {
        if body.jobs.is_empty() {
            return self.jobs_api.create_jobs(body, context).await;
        }

        // Validate that all jobs have the same workflow_id
        let first_workflow_id = body.jobs[0].workflow_id;
        for job in &body.jobs {
            if job.workflow_id != first_workflow_id {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!(
                        "All jobs in a batch must have the same workflow_id. Found workflow_ids: {} and {}",
                        first_workflow_id, job.workflow_id
                    )
                }));
                return Ok(CreateJobsResponse::UnprocessableContentErrorResponse(
                    error_response,
                ));
            }
        }

        // Check if workflow exists
        let workflow_exists = self
            .workflows_api
            .does_workflow_exist(first_workflow_id, context)
            .await?;
        if !workflow_exists {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", first_workflow_id)
            }));
            return Ok(CreateJobsResponse::NotFoundErrorResponse(error_response));
        }

        // Get default resource requirements for this workflow once
        let default_resource_requirements_id = self
            .get_default_resource_requirements_id(first_workflow_id, context)
            .await?;

        // Set default resource requirements for any jobs that don't have one
        for job in &mut body.jobs {
            if job.resource_requirements_id.is_none() {
                job.resource_requirements_id = Some(default_resource_requirements_id);
            }
        }

        self.jobs_api.create_jobs(body, context).await
    }

    /// Store a local scheduler.
    async fn create_local_scheduler(
        &self,
        body: models::LocalSchedulerModel,
        context: &C,
    ) -> Result<CreateLocalSchedulerResponse, ApiError> {
        // Check if workflow exists
        let workflow_exists = self
            .workflows_api
            .does_workflow_exist(body.workflow_id, context)
            .await?;
        if !workflow_exists {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", body.workflow_id)
            }));
            return Ok(CreateLocalSchedulerResponse::DefaultErrorResponse(
                error_response,
            ));
        }

        self.schedulers_api
            .create_local_scheduler(body, context)
            .await
    }

    /// Store a failure handler.
    async fn create_failure_handler(
        &self,
        body: models::FailureHandlerModel,
        context: &C,
    ) -> Result<CreateFailureHandlerResponse, ApiError> {
        // Check if workflow exists
        let workflow_exists = self
            .workflows_api
            .does_workflow_exist(body.workflow_id, context)
            .await?;
        if !workflow_exists {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", body.workflow_id)
            }));
            return Ok(CreateFailureHandlerResponse::NotFoundErrorResponse(
                error_response,
            ));
        }

        self.failure_handlers_api
            .create_failure_handler(body, context)
            .await
    }

    /// Retrieve a failure handler by ID.
    async fn get_failure_handler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetFailureHandlerResponse, ApiError> {
        self.failure_handlers_api
            .get_failure_handler(id, context)
            .await
    }

    /// Retrieve all failure handlers for one workflow.
    async fn list_failure_handlers(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListFailureHandlersResponse, ApiError> {
        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.failure_handlers_api
            .list_failure_handlers(workflow_id, offset, limit, context)
            .await
    }

    /// Delete a failure handler.
    async fn delete_failure_handler(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteFailureHandlerResponse, ApiError> {
        self.failure_handlers_api
            .delete_failure_handler(id, body, context)
            .await
    }

    /// Store one resource requirements record.
    async fn create_resource_requirements(
        &self,
        body: models::ResourceRequirementsModel,
        context: &C,
    ) -> Result<CreateResourceRequirementsResponse, ApiError> {
        // Prevent external API calls from creating "default" resource requirements
        if body.name == "default" {
            error!(
                "Attempt to create resource requirement with reserved name 'default' via external API for workflow_id={}",
                body.workflow_id
            );
            return Err(ApiError(
                "Cannot create resource requirement named 'default' via external API".to_string(),
            ));
        }

        // Check if workflow exists
        let workflow_exists = self
            .workflows_api
            .does_workflow_exist(body.workflow_id, context)
            .await?;
        if !workflow_exists {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", body.workflow_id)
            }));
            return Ok(CreateResourceRequirementsResponse::NotFoundErrorResponse(
                error_response,
            ));
        }

        self.resource_requirements_api
            .create_resource_requirements(body, context)
            .await
    }

    /// Store a job result.
    async fn create_result(
        &self,
        body: models::ResultModel,
        context: &C,
    ) -> Result<CreateResultResponse, ApiError> {
        // Check if workflow exists
        let workflow_exists = self
            .workflows_api
            .does_workflow_exist(body.workflow_id, context)
            .await?;
        if !workflow_exists {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", body.workflow_id)
            }));
            return Ok(CreateResultResponse::NotFoundErrorResponse(error_response));
        }

        self.results_api.create_result(body, context).await
    }

    /// Store a scheduled compute node.
    async fn create_scheduled_compute_node(
        &self,
        body: models::ScheduledComputeNodesModel,
        context: &C,
    ) -> Result<CreateScheduledComputeNodeResponse, ApiError> {
        // Check if workflow exists
        let workflow_exists = self
            .workflows_api
            .does_workflow_exist(body.workflow_id, context)
            .await?;
        if !workflow_exists {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", body.workflow_id)
            }));
            return Ok(CreateScheduledComputeNodeResponse::DefaultErrorResponse(
                error_response,
            ));
        }

        let workflow_id = body.workflow_id;
        let scheduler_id = body.scheduler_id;
        let scheduler_config_id = body.scheduler_config_id;
        let scheduler_type = body.scheduler_type.clone();

        let result = self
            .schedulers_api
            .create_scheduled_compute_node(body, context)
            .await?;

        // Broadcast SSE event for scheduled compute node creation
        if let CreateScheduledComputeNodeResponse::SuccessfulResponse(ref created) = result {
            self.event_broadcaster.broadcast(BroadcastEvent {
                workflow_id,
                timestamp: chrono::Utc::now().timestamp_millis(),
                event_type: "scheduler_node_created".to_string(),
                severity: models::EventSeverity::Info,
                data: serde_json::json!({
                    "category": "scheduler",
                    "scheduled_compute_node_id": created.id,
                    "scheduler_id": scheduler_id,
                    "scheduler_config_id": scheduler_config_id,
                    "scheduler_type": scheduler_type,
                    "status": created.status,
                }),
            });
        }

        Ok(result)
    }

    /// Store a Slurm compute node configuration.
    async fn create_slurm_scheduler(
        &self,
        body: models::SlurmSchedulerModel,
        context: &C,
    ) -> Result<CreateSlurmSchedulerResponse, ApiError> {
        // Check if workflow exists
        let workflow_exists = self
            .workflows_api
            .does_workflow_exist(body.workflow_id, context)
            .await?;
        if !workflow_exists {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", body.workflow_id)
            }));
            return Ok(CreateSlurmSchedulerResponse::NotFoundErrorResponse(
                error_response,
            ));
        }

        self.schedulers_api
            .create_slurm_scheduler(body, context)
            .await
    }

    /// Store remote workers for a workflow.
    async fn create_remote_workers(
        &self,
        workflow_id: i64,
        workers: Vec<String>,
        context: &C,
    ) -> Result<CreateRemoteWorkersResponse, ApiError> {
        self.remote_workers_api
            .create_remote_workers(workflow_id, workers, context)
            .await
    }

    /// List remote workers for a workflow.
    async fn list_remote_workers(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<ListRemoteWorkersResponse, ApiError> {
        self.remote_workers_api
            .list_remote_workers(workflow_id, context)
            .await
    }

    /// Delete a remote worker from a workflow.
    async fn delete_remote_worker(
        &self,
        workflow_id: i64,
        worker: String,
        context: &C,
    ) -> Result<DeleteRemoteWorkerResponse, ApiError> {
        self.remote_workers_api
            .delete_remote_worker(workflow_id, worker, context)
            .await
    }

    /// Store a user data record.
    async fn create_user_data(
        &self,
        body: models::UserDataModel,
        consumer_job_id: Option<i64>,
        producer_job_id: Option<i64>,
        context: &C,
    ) -> Result<CreateUserDataResponse, ApiError> {
        // Check if workflow exists
        let workflow_exists = self
            .workflows_api
            .does_workflow_exist(body.workflow_id, context)
            .await?;
        if !workflow_exists {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", body.workflow_id)
            }));
            return Ok(CreateUserDataResponse::NotFoundErrorResponse(
                error_response,
            ));
        }

        self.user_data_api
            .create_user_data(body, consumer_job_id, producer_job_id, context)
            .await
    }

    /// Store a workflow.
    async fn create_workflow(
        &self,
        body: models::WorkflowModel,
        context: &C,
    ) -> Result<CreateWorkflowResponse, ApiError> {
        let response = self.workflows_api.create_workflow(body, context).await?;
        match response {
            CreateWorkflowResponse::SuccessfulResponse(w) => {
                let rr = models::ResourceRequirementsModel {
                    id: None,
                    workflow_id: w.id.expect("Failed to get workflow ID"),
                    name: "default".to_string(),
                    num_cpus: 1,
                    num_gpus: 0,
                    num_nodes: 1,
                    memory: "1m".to_string(),
                    runtime: "P0DT1M".to_string(),
                };
                let _result = self
                    .resource_requirements_api
                    .create_resource_requirements(rr, context)
                    .await?;
                Ok(CreateWorkflowResponse::SuccessfulResponse(w))
            }
            CreateWorkflowResponse::DefaultErrorResponse(err) => {
                Ok(CreateWorkflowResponse::DefaultErrorResponse(err))
            }
        }
    }

    /// Create a workflow action.
    async fn create_workflow_action(
        &self,
        workflow_id: i64,
        body: serde_json::Value,
        context: &C,
    ) -> Result<CreateWorkflowActionResponse, ApiError> {
        // Parse body into WorkflowActionModel
        let action_model: models::WorkflowActionModel = match serde_json::from_value(body) {
            Ok(model) => model,
            Err(e) => {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Invalid workflow action data: {}", e)
                }));
                return Ok(CreateWorkflowActionResponse::DefaultErrorResponse(
                    error_response,
                ));
            }
        };

        // Check if workflow exists
        let workflow_exists = self
            .workflows_api
            .does_workflow_exist(workflow_id, context)
            .await?;
        if !workflow_exists {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", workflow_id)
            }));
            return Ok(CreateWorkflowActionResponse::NotFoundErrorResponse(
                error_response,
            ));
        }

        self.workflow_actions_api
            .create_workflow_action(workflow_id, action_model, context)
            .await
    }

    /// Get all workflow actions for a workflow.
    async fn get_workflow_actions(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<GetWorkflowActionsResponse, ApiError> {
        let workflow_exists = self
            .workflows_api
            .does_workflow_exist(workflow_id, context)
            .await?;
        if !workflow_exists {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", workflow_id)
            }));
            return Ok(GetWorkflowActionsResponse::NotFoundErrorResponse(
                error_response,
            ));
        }
        self.workflow_actions_api
            .get_workflow_actions(workflow_id, context)
            .await
    }

    /// Get pending (unexecuted) workflow actions for a workflow.
    #[instrument(level = "debug", skip(self, context), fields(workflow_id))]
    async fn get_pending_actions(
        &self,
        workflow_id: i64,
        trigger_types: Option<Vec<String>>,
        context: &C,
    ) -> Result<GetPendingActionsResponse, ApiError> {
        let workflow_exists = self
            .workflows_api
            .does_workflow_exist(workflow_id, context)
            .await?;
        if !workflow_exists {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", workflow_id)
            }));
            return Ok(GetPendingActionsResponse::NotFoundErrorResponse(
                error_response,
            ));
        }
        self.workflow_actions_api
            .get_pending_actions(workflow_id, trigger_types, context)
            .await
    }

    /// Atomically claim a workflow action for execution.
    #[instrument(
        level = "debug",
        skip(self, body, context),
        fields(workflow_id, action_id)
    )]
    async fn claim_action(
        &self,
        workflow_id: i64,
        action_id: i64,
        body: serde_json::Value,
        context: &C,
    ) -> Result<ClaimActionResponse, ApiError> {
        // Parse compute_node_id from body (optional - can be null for login node submissions)
        let compute_node_id = body.get("compute_node_id").and_then(|v| v.as_i64());

        // Check if workflow exists
        let workflow_exists = self
            .workflows_api
            .does_workflow_exist(workflow_id, context)
            .await?;
        if !workflow_exists {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", workflow_id)
            }));
            return Ok(ClaimActionResponse::NotFoundErrorResponse(error_response));
        }

        self.workflow_actions_api
            .claim_action(workflow_id, action_id, compute_node_id, context)
            .await
    }

    /// Return the version of the service.
    async fn get_version(&self, context: &C) -> Result<GetVersionResponse, ApiError> {
        debug!(
            "get_version() - X-Span-ID: {:?}",
            Has::<XSpanIdString>::get(context).0.clone()
        );
        Ok(GetVersionResponse::SuccessfulResponse(serde_json::json!(
            full_version()
        )))
    }

    /// Retrieve all workflows.
    async fn list_workflows(
        &self,
        offset: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        limit: Option<i64>,
        name: Option<String>,
        user: Option<String>,
        description: Option<String>,
        is_archived: Option<bool>,
        context: &C,
    ) -> Result<ListWorkflowsResponse, ApiError> {
        let (processed_offset, processed_limit) = process_pagination_params(offset, limit)?;

        // When access control is enforced and no user filter is provided,
        // restrict results to workflows the authenticated user can access.
        let accessible_ids = if self.authorization_service.is_enforced() && user.is_none() {
            let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
            match self
                .authorization_service
                .get_accessible_workflow_ids(&auth)
                .await
            {
                Ok(ids) => ids,
                Err(e) => {
                    return Err(ApiError(format!(
                        "Failed to get accessible workflows: {}",
                        e
                    )));
                }
            }
        } else {
            None
        };

        self.workflows_api
            .list_workflows_filtered(
                processed_offset,
                sort_by,
                reverse_sort,
                processed_limit,
                name,
                user,
                description,
                is_archived,
                accessible_ids,
                context,
            )
            .await
    }

    /// Check if the service is running.
    async fn ping(&self, context: &C) -> Result<PingResponse, ApiError> {
        debug!(
            "ping() - X-Span-ID: {:?}",
            Has::<XSpanIdString>::get(context).0.clone()
        );
        let response = PingResponse::SuccessfulResponse(serde_json::json!({"status": "ok"}));
        Ok(response)
    }

    /// Cancel a workflow. Workers will detect the status change and cancel jobs.
    async fn cancel_workflow(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<CancelWorkflowResponse, ApiError> {
        info!(
            "cancel_workflow(workflow_id={}) - X-Span-ID: {:?}",
            id,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        // Check access control
        match self.check_workflow_access_for_context(id, context).await {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(CancelWorkflowResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(CancelWorkflowResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }

        let result = self
            .workflows_api
            .cancel_workflow(id, body, context)
            .await?;

        Ok(result)
    }

    /// Delete all compute node records for one workflow.
    async fn delete_compute_nodes(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteComputeNodesResponse, ApiError> {
        self.compute_nodes_api
            .delete_compute_nodes(workflow_id, body, context)
            .await
    }

    /// Delete all events for one workflow.
    async fn delete_events(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteEventsResponse, ApiError> {
        self.events_api
            .delete_events(workflow_id, body, context)
            .await
    }

    /// Delete all files for one workflow.
    async fn delete_files(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteFilesResponse, ApiError> {
        self.files_api
            .delete_files(workflow_id, body, context)
            .await
    }

    /// Delete all jobs for one workflow.
    async fn delete_jobs(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteJobsResponse, ApiError> {
        self.jobs_api.delete_jobs(workflow_id, body, context).await
    }

    /// Delete all local schedulers for one workflow.
    async fn delete_local_schedulers(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteLocalSchedulersResponse, ApiError> {
        self.schedulers_api
            .delete_local_schedulers(workflow_id, body, context)
            .await
    }

    /// Delete all resource requirements records for one workflow.
    async fn delete_all_resource_requirements(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteAllResourceRequirementsResponse, ApiError> {
        self.resource_requirements_api
            .delete_all_resource_requirements(workflow_id, body, context)
            .await
    }

    /// Delete all job results for one workflow.
    async fn delete_results(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteResultsResponse, ApiError> {
        self.results_api
            .delete_results(workflow_id, body, context)
            .await
    }

    /// Delete all scheduled compute node records for one workflow.
    async fn delete_scheduled_compute_nodes(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteScheduledComputeNodesResponse, ApiError> {
        self.schedulers_api
            .delete_scheduled_compute_nodes(workflow_id, body, context)
            .await
    }

    /// Retrieve all Slurm compute node configurations for one workflow.
    async fn delete_slurm_schedulers(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteSlurmSchedulersResponse, ApiError> {
        self.schedulers_api
            .delete_slurm_schedulers(workflow_id, body, context)
            .await
    }

    /// Delete all user data records for one workflow.
    async fn delete_all_user_data(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteAllUserDataResponse, ApiError> {
        self.user_data_api
            .delete_all_user_data(workflow_id, body, context)
            .await
    }

    /// Retrieve all compute node records for one workflow.
    async fn list_compute_nodes(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        hostname: Option<String>,
        is_active: Option<bool>,
        scheduled_compute_node_id: Option<i64>,
        context: &C,
    ) -> Result<ListComputeNodesResponse, ApiError> {
        let (processed_offset, processed_limit) = process_pagination_params(offset, limit)?;
        self.compute_nodes_api
            .list_compute_nodes(
                workflow_id,
                processed_offset,
                processed_limit,
                sort_by,
                reverse_sort,
                hostname,
                is_active,
                scheduled_compute_node_id,
                context,
            )
            .await
    }

    /// Retrieve all events for one workflow.
    async fn list_events(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        category: Option<String>,
        after_timestamp: Option<i64>,
        context: &C,
    ) -> Result<ListEventsResponse, ApiError> {
        let (processed_offset, processed_limit) = process_pagination_params(offset, limit)?;
        self.events_api
            .list_events(
                workflow_id,
                processed_offset,
                processed_limit,
                sort_by,
                reverse_sort,
                category,
                after_timestamp,
                context,
            )
            .await
    }

    /// Retrieve all files for one workflow.
    async fn list_files(
        &self,
        workflow_id: i64,
        produced_by_job_id: Option<i64>,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        name: Option<String>,
        path: Option<String>,
        is_output: Option<bool>,
        context: &C,
    ) -> Result<ListFilesResponse, ApiError> {
        let (processed_offset, processed_limit) = process_pagination_params(offset, limit)?;
        self.files_api
            .list_files(
                workflow_id,
                produced_by_job_id,
                processed_offset,
                processed_limit,
                sort_by,
                reverse_sort,
                name,
                path,
                is_output,
                context,
            )
            .await
    }

    /// Retrieve all jobs for one workflow.
    async fn list_jobs(
        &self,
        workflow_id: i64,
        status: Option<models::JobStatus>,
        needs_file_id: Option<i64>,
        upstream_job_id: Option<i64>,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        include_relationships: Option<bool>,
        active_compute_node_id: Option<i64>,
        context: &C,
    ) -> Result<ListJobsResponse, ApiError> {
        // Check access control
        match self
            .check_workflow_access_for_context(workflow_id, context)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(ListJobsResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(ListJobsResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }

        let (processed_offset, processed_limit) = process_pagination_params(offset, limit)?;
        self.jobs_api
            .list_jobs(
                workflow_id,
                status,
                needs_file_id,
                upstream_job_id,
                processed_offset,
                processed_limit,
                sort_by,
                reverse_sort,
                include_relationships,
                active_compute_node_id,
                context,
            )
            .await
    }

    /// Retrieve all job dependencies for one workflow.
    async fn list_job_dependencies(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListJobDependenciesResponse, ApiError> {
        // Check access control
        match self
            .check_workflow_access_for_context(workflow_id, context)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(ListJobDependenciesResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(ListJobDependenciesResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }

        self.workflows_api
            .list_job_dependencies(workflow_id, offset, limit, context)
            .await
    }

    /// Retrieve job-file relationships for one workflow.
    async fn list_job_file_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListJobFileRelationshipsResponse, ApiError> {
        // Check access control
        match self
            .check_workflow_access_for_context(workflow_id, context)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(ListJobFileRelationshipsResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(ListJobFileRelationshipsResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }

        self.workflows_api
            .list_job_file_relationships(workflow_id, offset, limit, context)
            .await
    }

    /// Retrieve job-user_data relationships for one workflow.
    async fn list_job_user_data_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListJobUserDataRelationshipsResponse, ApiError> {
        // Check access control
        match self
            .check_workflow_access_for_context(workflow_id, context)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(
                    ListJobUserDataRelationshipsResponse::ForbiddenErrorResponse(
                        models::ErrorResponse::new(serde_json::json!({
                            "error": "Forbidden",
                            "message": reason
                        })),
                    ),
                );
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(ListJobUserDataRelationshipsResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }

        self.workflows_api
            .list_job_user_data_relationships(workflow_id, offset, limit, context)
            .await
    }

    /// Retrieve local schedulers for one workflow.
    async fn list_local_schedulers(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        memory: Option<String>,
        num_cpus: Option<i64>,
        context: &C,
    ) -> Result<ListLocalSchedulersResponse, ApiError> {
        // Check access control
        match self
            .check_workflow_access_for_context(workflow_id, context)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(ListLocalSchedulersResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(ListLocalSchedulersResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }

        let (processed_offset, processed_limit) = process_pagination_params(offset, limit)?;
        self.schedulers_api
            .list_local_schedulers(
                workflow_id,
                processed_offset,
                processed_limit,
                sort_by,
                reverse_sort,
                memory,
                num_cpus,
                context,
            )
            .await
    }

    /// Retrieve all resource requirements records for one workflow.
    async fn list_resource_requirements(
        &self,
        workflow_id: i64,
        job_id: Option<i64>,
        name: Option<String>,
        memory: Option<String>,
        num_cpus: Option<i64>,
        num_gpus: Option<i64>,
        num_nodes: Option<i64>,
        runtime: Option<i64>,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListResourceRequirementsResponse, ApiError> {
        // Check access control
        match self
            .check_workflow_access_for_context(workflow_id, context)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(ListResourceRequirementsResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(ListResourceRequirementsResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }

        let (processed_offset, processed_limit) = process_pagination_params(offset, limit)?;
        self.resource_requirements_api
            .list_resource_requirements(
                workflow_id,
                job_id,
                name,
                memory,
                num_cpus,
                num_gpus,
                num_nodes,
                runtime,
                processed_offset,
                processed_limit,
                sort_by,
                reverse_sort,
                context,
            )
            .await
    }

    /// Retrieve all job results for one workflow.
    async fn list_results(
        &self,
        workflow_id: i64,
        job_id: Option<i64>,
        run_id: Option<i64>,
        return_code: Option<i64>,
        status: Option<models::JobStatus>,
        compute_node_id: Option<i64>,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        all_runs: Option<bool>,
        context: &C,
    ) -> Result<ListResultsResponse, ApiError> {
        debug!(
            "list_results({}, {:?}, {:?}, {:?}, {:?}, compute_node_id={:?}, {:?}, {:?}, {:?}, {:?}, all_runs={:?}) - X-Span-ID: {:?}",
            workflow_id,
            job_id,
            run_id,
            return_code,
            status,
            compute_node_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            all_runs,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        // Check access control
        match self
            .check_workflow_access_for_context(workflow_id, context)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(ListResultsResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(ListResultsResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }

        let (processed_offset, processed_limit) = process_pagination_params(offset, limit)?;
        self.results_api
            .list_results(
                workflow_id,
                job_id,
                run_id,
                return_code,
                status,
                compute_node_id,
                processed_offset,
                processed_limit,
                sort_by,
                reverse_sort,
                all_runs,
                context,
            )
            .await
    }

    /// Retrieve scheduled compute node records for one workflow.
    async fn list_scheduled_compute_nodes(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        scheduler_id: Option<String>,
        scheduler_config_id: Option<String>,
        status: Option<String>,
        context: &C,
    ) -> Result<ListScheduledComputeNodesResponse, ApiError> {
        // Check access control
        match self
            .check_workflow_access_for_context(workflow_id, context)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(ListScheduledComputeNodesResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(ListScheduledComputeNodesResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }

        let (processed_offset, processed_limit) = process_pagination_params(offset, limit)?;
        self.schedulers_api
            .list_scheduled_compute_nodes(
                workflow_id,
                processed_offset,
                processed_limit,
                sort_by,
                reverse_sort,
                scheduler_id,
                scheduler_config_id,
                status,
                context,
            )
            .await
    }

    /// Retrieve a Slurm compute node configuration.
    async fn list_slurm_schedulers(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        _: Option<String>,
        _: Option<String>,
        _: Option<String>,
        _: Option<String>,
        _: Option<i64>,
        _: Option<String>,
        _: Option<String>,
        _: Option<String>,
        _: Option<String>,
        context: &C,
    ) -> Result<ListSlurmSchedulersResponse, ApiError> {
        // Check access control
        match self
            .check_workflow_access_for_context(workflow_id, context)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(ListSlurmSchedulersResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(ListSlurmSchedulersResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }

        let (processed_offset, processed_limit) = process_pagination_params(offset, limit)?;
        self.schedulers_api
            .list_slurm_schedulers(
                workflow_id,
                processed_offset,
                processed_limit,
                sort_by,
                reverse_sort,
                context,
            )
            .await
    }

    /// Retrieve all user data records for one workflow.
    async fn list_user_data(
        &self,
        workflow_id: i64,
        consumer_job_id: Option<i64>,
        producer_job_id: Option<i64>,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        name: Option<String>,
        is_ephemeral: Option<bool>,
        context: &C,
    ) -> Result<ListUserDataResponse, ApiError> {
        // Check access control
        match self
            .check_workflow_access_for_context(workflow_id, context)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(ListUserDataResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(ListUserDataResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }

        let (processed_offset, processed_limit) = process_pagination_params(offset, limit)?;
        self.user_data_api
            .list_user_data(
                workflow_id,
                consumer_job_id,
                producer_job_id,
                processed_offset,
                processed_limit,
                sort_by,
                reverse_sort,
                name,
                is_ephemeral,
                context,
            )
            .await
    }

    /// Retrieve a compute node by ID.
    async fn get_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetComputeNodeResponse, ApiError> {
        self.compute_nodes_api.get_compute_node(id, context).await
    }

    /// Retrieve an event by ID.
    async fn get_event(&self, id: i64, context: &C) -> Result<GetEventResponse, ApiError> {
        self.events_api.get_event(id, context).await
    }

    /// Retrieve a file.
    async fn get_file(&self, id: i64, context: &C) -> Result<GetFileResponse, ApiError> {
        self.files_api.get_file(id, context).await
    }

    /// Retrieve a job.
    async fn get_job(&self, id: i64, context: &C) -> Result<GetJobResponse, ApiError> {
        // Check access control (via workflow)
        match self.check_job_access_for_context(id, context).await {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(GetJobResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(GetJobResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }
        self.jobs_api.get_job(id, context).await
    }

    /// Retrieve a local scheduler.
    async fn get_local_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetLocalSchedulerResponse, ApiError> {
        self.schedulers_api.get_local_scheduler(id, context).await
    }

    /// Return the resource requirements for jobs with a status of ready.
    #[instrument(level = "debug", skip(self, context), fields(workflow_id = id, scheduler_config_id = ?scheduler_config_id))]
    async fn get_ready_job_requirements(
        &self,
        id: i64,
        scheduler_config_id: Option<i64>,
        context: &C,
    ) -> Result<GetReadyJobRequirementsResponse, ApiError> {
        debug!(
            "get_ready_job_requirements({}, {:?}) - X-Span-ID: {:?}",
            id,
            scheduler_config_id,
            Has::<XSpanIdString>::get(context).0.clone()
        );
        error!("get_ready_job_requirements operation is not implemented");
        Err(ApiError("Api-Error: Operation is NOT implemented".into()))
    }

    /// Retrieve one resource requirements record.
    async fn get_resource_requirements(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetResourceRequirementsResponse, ApiError> {
        self.resource_requirements_api
            .get_resource_requirements(id, context)
            .await
    }

    /// Retrieve a job result.
    async fn get_result(&self, id: i64, context: &C) -> Result<GetResultResponse, ApiError> {
        self.results_api.get_result(id, context).await
    }

    /// Retrieve a scheduled compute node.
    async fn get_scheduled_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetScheduledComputeNodeResponse, ApiError> {
        self.schedulers_api
            .get_scheduled_compute_node(id, context)
            .await
    }

    /// Retrieve a Slurm compute node configuration.
    async fn get_slurm_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetSlurmSchedulerResponse, ApiError> {
        self.schedulers_api.get_slurm_scheduler(id, context).await
    }

    /// Retrieve a user data record.
    async fn get_user_data(&self, id: i64, context: &C) -> Result<GetUserDataResponse, ApiError> {
        self.user_data_api.get_user_data(id, context).await
    }

    /// Retrieve a workflow.
    async fn get_workflow(&self, id: i64, context: &C) -> Result<GetWorkflowResponse, ApiError> {
        // Check access control
        match self.check_workflow_access_for_context(id, context).await {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(GetWorkflowResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(GetWorkflowResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }
        self.workflows_api.get_workflow(id, context).await
    }

    /// Return the workflow status.
    async fn get_workflow_status(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetWorkflowStatusResponse, ApiError> {
        // Check access control
        match self.check_workflow_access_for_context(id, context).await {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(GetWorkflowStatusResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(GetWorkflowStatusResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }
        self.workflows_api.get_workflow_status(id, context).await
    }

    /// Initialize job relationships based on file and user_data relationships.
    ///
    /// This operation wraps all initialization steps in a transaction to ensure atomicity.
    /// If any step fails, all changes will be rolled back.
    ///
    /// This function can be called multiple times (e.g., for workflow reruns). It will:
    /// - Reset all job statuses to uninitialized
    /// - Delete workflow_result records for incomplete jobs
    /// - Re-initialize job statuses based on dependencies
    async fn initialize_jobs(
        &self,
        id: i64,
        only_uninitialized: Option<bool>,
        clear_ephemeral_user_data: Option<bool>,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<InitializeJobsResponse, ApiError> {
        info!(
            "initialize_jobs({}, {:?}, {:?}, {:?}) - X-Span-ID: {:?}",
            id,
            only_uninitialized,
            clear_ephemeral_user_data,
            body,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        // Check access control
        match self.check_workflow_access_for_context(id, context).await {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(InitializeJobsResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(InitializeJobsResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }

        // Clear in-memory failure tracking for this workflow when (re)initializing
        if let Ok(mut set) = self.workflows_with_failures.write() {
            set.remove(&id);
        }

        // Begin a transaction to ensure all initialization steps are atomic
        let mut tx = match self.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => {
                error!("Failed to begin transaction for initialize_jobs: {}", e);
                return Err(ApiError("Database error".to_string()));
            }
        };

        // Step 1: Add depends-on associations based on file dependencies
        if let Err(e) = self
            .add_depends_on_associations_from_files(&mut *tx, id)
            .await
        {
            error!("Failed to add depends-on associations from files: {}", e);
            let _ = tx.rollback().await;
            return Err(e);
        }

        // Step 1b: Add depends-on associations from user_data
        if let Err(e) = self
            .add_depends_on_associations_from_user_data(&mut *tx, id)
            .await
        {
            error!(
                "Failed to add depends-on associations from user_data: {}",
                e
            );
            let _ = tx.rollback().await;
            return Err(e);
        }

        // Step 2: Uninitialize blocked jobs (only needed during reinitialization)
        // This is skipped during initial workflow start because Step 3 will set all job statuses anyway.
        // During reinitialization, this ensures jobs transitively blocked by reset jobs are also reset.
        let only_uninit = only_uninitialized.unwrap_or(false);
        if only_uninit && let Err(e) = self.uninitialize_blocked_jobs(&mut *tx, id).await {
            error!("Failed to uninitialize blocked jobs: {}", e);
            let _ = tx.rollback().await;
            return Err(e);
        }

        // Step 3: Initialize blocked jobs to blocked status
        if let Err(e) = self
            .initialize_blocked_jobs_to_blocked(&mut *tx, id, only_uninit)
            .await
        {
            error!("Failed to initialize blocked jobs to blocked: {}", e);
            let _ = tx.rollback().await;
            return Err(e);
        }

        // Step 4: Initialize unblocked jobs to ready status
        if let Err(e) = self.initialize_unblocked_jobs(&mut *tx, id).await {
            error!("Failed to initialize unblocked jobs: {}", e);
            let _ = tx.rollback().await;
            return Err(e);
        }

        // TODO: helper function
        // Step 5: Delete workflow_result records for jobs that are not complete
        // This is done after steps 1-4 to be future-proof in case those steps reset job completion statuses
        // Complete statuses are: Completed (5), Failed (6), Canceled (7), Terminated (8)
        let completed_status = models::JobStatus::Completed.to_int();
        let failed_status = models::JobStatus::Failed.to_int();
        let canceled_status = models::JobStatus::Canceled.to_int();
        let terminated_status = models::JobStatus::Terminated.to_int();

        match sqlx::query!(
            r#"
            DELETE FROM workflow_result
            WHERE workflow_id = $1
              AND job_id IN (
                SELECT id FROM job
                WHERE workflow_id = $1
                  AND status NOT IN ($2, $3, $4, $5)
              )
            "#,
            id,
            completed_status,
            failed_status,
            canceled_status,
            terminated_status
        )
        .execute(&mut *tx)
        .await
        {
            Ok(result) => {
                debug!(
                    "Deleted {} workflow_result records for incomplete jobs in workflow {}",
                    result.rows_affected(),
                    id
                );
            }
            Err(e) => {
                error!(
                    "Failed to delete workflow_result records for incomplete jobs: {}",
                    e
                );
                let _ = tx.rollback().await;
                return Err(ApiError("Database error".to_string()));
            }
        }

        // Commit the transaction
        // Hash computation must happen AFTER this commit so that compute_job_input_hash
        // can see the job_depends_on relationships that were inserted in this transaction
        if let Err(e) = tx.commit().await {
            error!("Failed to commit transaction for initialize_jobs: {}", e);
            return Err(ApiError("Database error".to_string()));
        }

        // Step 7: Compute and store input hashes for all jobs in the workflow
        // This tracks the baseline hash for this run to detect future input changes
        // IMPORTANT: This must happen AFTER the transaction commits so that the hash
        // computation sees the committed job_depends_on relationships
        let job_ids = match sqlx::query!(
            r#"
            SELECT id
            FROM job
            WHERE workflow_id = $1
            ORDER BY id
            "#,
            id
        )
        .fetch_all(self.pool.as_ref())
        .await
        {
            Ok(rows) => rows,
            Err(e) => {
                error!("Failed to query job IDs for hash computation: {}", e);
                return Err(ApiError("Database error".to_string()));
            }
        };

        let job_count = job_ids.len();
        debug!(
            "Computing and storing input hashes for {} jobs in workflow {}",
            job_count, id
        );

        for row in job_ids {
            let job_id = row.id;

            // Compute hash (reads from pool with committed data)
            match self.jobs_api.compute_job_input_hash(job_id).await {
                Ok(hash) => {
                    // Store hash (using pool, not transaction)
                    match sqlx::query!(
                        r#"
                        INSERT INTO job_internal (job_id, input_hash)
                        VALUES ($1, $2)
                        ON CONFLICT(job_id) DO UPDATE SET input_hash = excluded.input_hash
                        "#,
                        job_id,
                        hash
                    )
                    .execute(self.pool.as_ref())
                    .await
                    {
                        Ok(_) => {
                            debug!("Stored input hash for job {}", job_id);
                        }
                        Err(e) => {
                            error!("Failed to store input hash for job {}: {}", job_id, e);
                            return Err(ApiError("Database error".to_string()));
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to compute input hash for job {}: {}", job_id, e);
                    return Err(e);
                }
            }
        }

        debug!(
            "Completed computing and storing input hashes for {} jobs in workflow {}",
            job_count, id
        );

        debug!(
            "Successfully initialized jobs for workflow {} with transaction",
            id
        );

        // Reset workflow actions for reinitialization
        // This resets executed flags and pre-computes trigger_count based on current job states.
        // For on_jobs_ready/on_jobs_complete actions, trigger_count is set to the number of jobs
        // already in a satisfied state (e.g., job2 is Completed, so it counts toward trigger_count).
        if let Err(e) = self
            .workflow_actions_api
            .reset_actions_for_reinitialize(id)
            .await
        {
            error!(
                "Failed to reset workflow actions for workflow {}: {}",
                id, e
            );
            // Don't fail the request, just log the error
        }

        // Activate on_workflow_start actions (workflow has started with initialization)
        if let Err(e) = self
            .workflow_actions_api
            .check_and_trigger_actions(id, "on_workflow_start", None)
            .await
        {
            error!(
                "Failed to check_and_trigger_actions for on_workflow_start: {}",
                e
            );
            // Don't fail the request, just log the error
        }

        // Activate on_worker_start and on_worker_complete actions immediately
        // These are worker-lifecycle events that workers can claim when they start/complete
        for trigger_type in &["on_worker_start", "on_worker_complete"] {
            match sqlx::query(
                "UPDATE workflow_action SET trigger_count = required_triggers WHERE workflow_id = ? AND trigger_type = ?"
            )
            .bind(id)
            .bind(trigger_type)
            .execute(self.pool.as_ref())
            .await
            {
                Ok(result) => {
                    let count = result.rows_affected();
                    if count > 0 {
                        debug!("Activated {} {} actions for workflow {}", count, trigger_type, id);
                    }
                }
                Err(e) => {
                    error!("Failed to activate {} actions for workflow {}: {}", trigger_type, id, e);
                    // Don't fail the request, just log the error
                }
            }
        }

        // Check if any on_jobs_ready actions should be triggered based on job states
        if let Err(e) = self
            .workflow_actions_api
            .check_and_trigger_actions(id, "on_jobs_ready", None)
            .await
        {
            error!(
                "Failed to check_and_trigger_actions for on_jobs_ready: {}",
                e
            );
            // Don't fail the request, just log the error
        }

        // Broadcast SSE event for workflow initialization
        // Determine event type based on only_uninitialized flag
        let event_type = if only_uninitialized.unwrap_or(false) {
            "workflow_started"
        } else {
            "workflow_reinitialized"
        };

        // Get username from authorization context if available
        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        let username = auth
            .map(|a| a.subject)
            .unwrap_or_else(|| "unknown".to_string());

        self.event_broadcaster.broadcast(BroadcastEvent {
            workflow_id: id,
            timestamp: chrono::Utc::now().timestamp_millis(),
            event_type: event_type.to_string(),
            severity: models::EventSeverity::Info,
            data: serde_json::json!({
                "category": "workflow",
                "type": event_type,
                "user": username,
                "message": format!("{} workflow {}", event_type.replace('_', " "), id),
            }),
        });

        let response = InitializeJobsResponse::SuccessfulResponse(
            serde_json::json!({"message": "Initialized job status"}),
        );
        Ok(response)
    }

    /// Return true if all jobs in the workflow are complete.
    #[instrument(level = "debug", skip(self, context), fields(workflow_id = id))]
    async fn is_workflow_complete(
        &self,
        id: i64,
        context: &C,
    ) -> Result<IsWorkflowCompleteResponse, ApiError> {
        self.workflows_api.is_workflow_complete(id, context).await
    }

    async fn is_workflow_uninitialized(
        &self,
        id: i64,
        context: &C,
    ) -> Result<IsWorkflowUninitializedResponse, ApiError> {
        self.workflows_api
            .is_workflow_uninitialized(id, context)
            .await
    }

    /// Retrieve all job IDs for one workflow.
    async fn list_job_ids(&self, id: i64, context: &C) -> Result<ListJobIdsResponse, ApiError> {
        self.jobs_api.list_job_ids(id, context).await
    }

    /// List missing user data that should exist.
    async fn list_missing_user_data(
        &self,
        id: i64,
        context: &C,
    ) -> Result<ListMissingUserDataResponse, ApiError> {
        self.user_data_api.list_missing_user_data(id, context).await
    }

    /// List files that must exist.
    async fn list_required_existing_files(
        &self,
        id: i64,
        context: &C,
    ) -> Result<ListRequiredExistingFilesResponse, ApiError> {
        self.files_api
            .list_required_existing_files(id, context)
            .await
    }

    /// Update a compute node.
    async fn update_compute_node(
        &self,
        id: i64,
        body: models::ComputeNodeModel,
        context: &C,
    ) -> Result<UpdateComputeNodeResponse, ApiError> {
        let result = self
            .compute_nodes_api
            .update_compute_node(id, body.clone(), context)
            .await?;

        // Broadcast SSE event when compute node stops (is_active becomes false)
        if let UpdateComputeNodeResponse::SuccessfulResponse(ref _updated) = result
            && body.is_active == Some(false)
        {
            self.event_broadcaster.broadcast(BroadcastEvent {
                workflow_id: body.workflow_id,
                timestamp: chrono::Utc::now().timestamp_millis(),
                event_type: "compute_node_stopped".to_string(),
                severity: models::EventSeverity::Info,
                data: serde_json::json!({
                    "compute_node_id": id,
                    "hostname": body.hostname,
                    "pid": body.pid,
                    "duration_seconds": body.duration_seconds,
                    "compute_node_type": body.compute_node_type,
                }),
            });
        }

        Ok(result)
    }

    /// Update an event.
    async fn update_event(
        &self,
        id: i64,
        body: serde_json::Value,
        context: &C,
    ) -> Result<UpdateEventResponse, ApiError> {
        self.events_api.update_event(id, body, context).await
    }

    /// Update a file.
    async fn update_file(
        &self,
        id: i64,
        body: models::FileModel,
        context: &C,
    ) -> Result<UpdateFileResponse, ApiError> {
        self.files_api.update_file(id, body, context).await
    }

    /// Update a job.
    async fn update_job(
        &self,
        id: i64,
        body: models::JobModel,
        context: &C,
    ) -> Result<UpdateJobResponse, ApiError> {
        // Check access control (via workflow)
        match self.check_job_access_for_context(id, context).await {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(UpdateJobResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(UpdateJobResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }
        self.jobs_api.update_job(id, body, context).await
    }

    /// Update a local scheduler.
    async fn update_local_scheduler(
        &self,
        id: i64,
        body: models::LocalSchedulerModel,
        context: &C,
    ) -> Result<UpdateLocalSchedulerResponse, ApiError> {
        self.schedulers_api
            .update_local_scheduler(id, body, context)
            .await
    }

    /// Update one resource requirements record.
    async fn update_resource_requirements(
        &self,
        id: i64,
        body: models::ResourceRequirementsModel,
        context: &C,
    ) -> Result<UpdateResourceRequirementsResponse, ApiError> {
        let result = self
            .resource_requirements_api
            .update_resource_requirements(id, body, context)
            .await?;

        // Log event for successful update
        if let UpdateResourceRequirementsResponse::SuccessfulResponse(ref rr) = result {
            let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
            let username = auth
                .map(|a| a.subject)
                .unwrap_or_else(|| "unknown".to_string());

            let event = models::EventModel::new(
                rr.workflow_id,
                serde_json::json!({
                    "category": "user_action",
                    "action": "update_resource_requirements",
                    "user": username,
                    "resource_requirements_id": id,
                    "name": rr.name,
                    "num_cpus": rr.num_cpus,
                    "num_gpus": rr.num_gpus,
                    "num_nodes": rr.num_nodes,
                    "memory": rr.memory,
                    "runtime": rr.runtime,
                }),
            );
            if let Err(e) = self.events_api.create_event(event, context).await {
                error!(
                    "Failed to create event for update_resource_requirements: {:?}",
                    e
                );
            }
        }

        Ok(result)
    }

    /// Update a job result.
    async fn update_result(
        &self,
        id: i64,
        body: models::ResultModel,
        context: &C,
    ) -> Result<UpdateResultResponse, ApiError> {
        self.results_api.update_result(id, body, context).await
    }

    /// Update a scheduled compute node.
    async fn update_scheduled_compute_node(
        &self,
        id: i64,
        body: models::ScheduledComputeNodesModel,
        context: &C,
    ) -> Result<UpdateScheduledComputeNodeResponse, ApiError> {
        self.schedulers_api
            .update_scheduled_compute_node(id, body, context)
            .await
    }

    /// Update a Slurm compute node configuration.
    async fn update_slurm_scheduler(
        &self,
        id: i64,
        body: models::SlurmSchedulerModel,
        context: &C,
    ) -> Result<UpdateSlurmSchedulerResponse, ApiError> {
        self.schedulers_api
            .update_slurm_scheduler(id, body, context)
            .await
    }

    /// Update a user data record.
    async fn update_user_data(
        &self,
        id: i64,
        body: models::UserDataModel,
        context: &C,
    ) -> Result<UpdateUserDataResponse, ApiError> {
        self.user_data_api.update_user_data(id, body, context).await
    }

    /// Update a workflow.
    async fn update_workflow(
        &self,
        id: i64,
        body: models::WorkflowModel,
        context: &C,
    ) -> Result<UpdateWorkflowResponse, ApiError> {
        // Check access control
        match self.check_workflow_access_for_context(id, context).await {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(UpdateWorkflowResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(UpdateWorkflowResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }
        self.workflows_api.update_workflow(id, body, context).await
    }

    /// Update the workflow status.
    async fn update_workflow_status(
        &self,
        id: i64,
        body: models::WorkflowStatusModel,
        context: &C,
    ) -> Result<UpdateWorkflowStatusResponse, ApiError> {
        // Check access control
        match self.check_workflow_access_for_context(id, context).await {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(UpdateWorkflowStatusResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(UpdateWorkflowStatusResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }

        // Clear in-memory failure tracking when workflow is being archived
        if body.is_archived == Some(true)
            && let Ok(mut set) = self.workflows_with_failures.write()
        {
            set.remove(&id);
        }

        self.workflows_api
            .update_workflow_status(id, body, context)
            .await
    }

    /// Return jobs that are ready for submission and meet worker resource requirements. Set status to pending.
    #[instrument(level = "debug", skip(self, body, context), fields(workflow_id = id, limit))]
    async fn claim_jobs_based_on_resources(
        &self,
        id: i64,
        body: models::ComputeNodesResources,
        limit: i64,
        sort_method: Option<models::ClaimJobsSortMethod>,
        strict_scheduler_match: Option<bool>,
        context: &C,
    ) -> Result<ClaimJobsBasedOnResources, ApiError> {
        debug!(
            "claim_jobs_based_on_resources({}, {:?}, {:?}, {:?}, strict_scheduler_match={:?}) - X-Span-ID: {:?}",
            id,
            body,
            sort_method,
            limit,
            strict_scheduler_match,
            Has::<XSpanIdString>::get(context).0.clone()
        );
        let status = match self.get_workflow_status(id, context).await {
            Ok(GetWorkflowStatusResponse::SuccessfulResponse(status)) => status,
            Ok(_) => {
                error!(
                    "Unexpected response from get_workflow_status for workflow_id={}",
                    id
                );
                return Err(ApiError(
                    "Unexpected response from get_workflow_status".to_string(),
                ));
            }
            Err(e) => return Err(e),
        };

        if status.is_canceled {
            return Ok(ClaimJobsBasedOnResources::SuccessfulResponse(
                models::ClaimJobsBasedOnResources {
                    jobs: Some(vec![]),
                    reason: Some("Workflow is canceled".to_string()),
                },
            ));
        }

        self.prepare_ready_jobs(
            id,
            body,
            sort_method,
            limit,
            strict_scheduler_match,
            context,
        )
        .await
    }

    /// Return user-requested number of jobs that are ready for submission. Sets status to pending.
    #[instrument(level = "debug", skip(self, body, context), fields(workflow_id = id, limit = ?limit))]
    async fn claim_next_jobs(
        &self,
        id: i64,
        limit: Option<i64>,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<ClaimNextJobsResponse, ApiError> {
        debug!(
            "claim_next_jobs({}, {:?}, {:?}) - X-Span-ID: {:?}",
            id,
            limit,
            body,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        let workflow_id = id;
        let job_limit = limit.unwrap_or(10);

        // Check if workflow is canceled first
        let status = match self.get_workflow_status(workflow_id, context).await {
            Ok(GetWorkflowStatusResponse::SuccessfulResponse(status)) => status,
            Ok(_) => {
                error!(
                    "Unexpected response from get_workflow_status for workflow_id={}",
                    workflow_id
                );
                return Err(ApiError(
                    "Unexpected response from get_workflow_status".to_string(),
                ));
            }
            Err(e) => return Err(e),
        };

        if status.is_canceled {
            return Ok(ClaimNextJobsResponse::SuccessfulResponse(
                models::ClaimNextJobsResponse { jobs: Some(vec![]) },
            ));
        }

        // Use BEGIN IMMEDIATE TRANSACTION to acquire a database write lock.
        // This ensures thread safety at the database level for the entire job selection process.
        // The IMMEDIATE mode acquires a reserved lock immediately, preventing other writers
        // and ensuring that concurrent reads see a consistent snapshot.
        // This prevents race conditions where multiple clients could:
        // 1. Select the same ready jobs from the job table
        // 2. Double-allocate jobs to different clients
        // The lock is held for the entire transaction duration.

        // Start an IMMEDIATE transaction using raw SQL
        let mut conn = self.pool.acquire().await.map_err(|e| {
            error!("Failed to acquire database connection: {}", e);
            ApiError("Database connection error".to_string())
        })?;

        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(|e| {
                error!("Failed to begin immediate transaction: {}", e);
                ApiError("Database lock error".to_string())
            })?;

        debug!(
            "claim_next_jobs: workflow_id={}, limit={}",
            workflow_id, job_limit
        );

        // First check if the workflow exists
        let workflow_exists = sqlx::query("SELECT id FROM workflow WHERE id = $1")
            .bind(workflow_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| {
                error!("Database error checking workflow existence: {}", e);
                ApiError("Database error".to_string())
            })?;

        if workflow_exists.is_none() {
            // Rollback the transaction since we're returning early
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;

            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", workflow_id)
            }));
            return Ok(ClaimNextJobsResponse::DefaultErrorResponse(error_response));
        }

        // Query the job table directly for ready jobs using the indexed status column
        let ready_status = models::JobStatus::Ready.to_int();
        let query = r#"
            SELECT
                id as job_id,
                workflow_id,
                name,
                command,
                status,
                cancel_on_blocking_job_failure,
                supports_termination,
                resource_requirements_id,
                failure_handler_id,
                attempt_id
            FROM job
            WHERE workflow_id = $1 AND status = $2
            LIMIT $3
            "#;

        let rows = sqlx::query(query)
            .bind(workflow_id)
            .bind(ready_status)
            .bind(job_limit)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| {
                error!("Database error in claim_next_jobs: {}", e);
                ApiError("Database error".to_string())
            })?;

        debug!(
            "claim_next_jobs: Found {} jobs for workflow {}",
            rows.len(),
            workflow_id
        );

        let mut selected_jobs = Vec::new();
        let mut job_ids_to_update = Vec::new();

        // Process all returned jobs (all are guaranteed to be in Ready status)
        for row in rows {
            let job_id: i64 = row.get("job_id");
            job_ids_to_update.push(job_id);

            let job = models::JobModel {
                id: Some(job_id),
                workflow_id: row.get("workflow_id"),
                name: row.get("name"),
                command: row.get("command"),
                invocation_script: None,
                status: Some(models::JobStatus::Pending),
                schedule_compute_nodes: None,
                cancel_on_blocking_job_failure: Some(row.get("cancel_on_blocking_job_failure")),
                supports_termination: Some(row.get("supports_termination")),
                depends_on_job_ids: None,
                input_file_ids: None,
                output_file_ids: None,
                input_user_data_ids: None,
                output_user_data_ids: None,
                resource_requirements_id: Some(row.get("resource_requirements_id")),
                scheduler_id: None,
                failure_handler_id: row.get("failure_handler_id"),
                attempt_id: row.get("attempt_id"),
            };

            selected_jobs.push(job);
        }

        // Query output file and user_data relationships for all selected jobs
        let mut output_files_map: std::collections::HashMap<i64, Vec<i64>> =
            std::collections::HashMap::new();
        let mut output_user_data_map: std::collections::HashMap<i64, Vec<i64>> =
            std::collections::HashMap::new();

        if !job_ids_to_update.is_empty() {
            // Query output files
            let output_files =
                sqlx::query("SELECT job_id, file_id FROM job_output_file WHERE workflow_id = $1")
                    .bind(workflow_id)
                    .fetch_all(&mut *conn)
                    .await
                    .map_err(|e| {
                        error!("Failed to query output files: {}", e);
                        ApiError("Database query error".to_string())
                    })?;

            for row in output_files {
                let job_id: i64 = row.get("job_id");
                let file_id: i64 = row.get("file_id");
                if job_ids_to_update.contains(&job_id) {
                    output_files_map.entry(job_id).or_default().push(file_id);
                }
            }

            // Query output user_data
            let output_user_data = sqlx::query("SELECT job_id, user_data_id FROM job_output_user_data WHERE job_id IN (SELECT id FROM job WHERE workflow_id = $1)")
                .bind(workflow_id)
                .fetch_all(&mut *conn)
                .await
                .map_err(|e| {
                    error!("Failed to query output user_data: {}", e);
                    ApiError("Database query error".to_string())
                })?;

            for row in output_user_data {
                let job_id: i64 = row.get("job_id");
                let user_data_id: i64 = row.get("user_data_id");
                if job_ids_to_update.contains(&job_id) {
                    output_user_data_map
                        .entry(job_id)
                        .or_default()
                        .push(user_data_id);
                }
            }
        }

        // Populate the output file and user_data IDs in the selected jobs
        for job in &mut selected_jobs {
            if let Some(job_id) = job.id {
                job.output_file_ids = output_files_map.get(&job_id).cloned();
                job.output_user_data_ids = output_user_data_map.get(&job_id).cloned();
            }
        }

        // If we have jobs to update, update their status to pending using bulk UPDATE
        if !job_ids_to_update.is_empty() {
            let pending = models::JobStatus::Pending.to_int();
            // SAFETY: job_ids are i64 from database query results.
            // i64::to_string() only produces numeric strings - SQL injection impossible.
            // Using string formatting because sqlx doesn't support parameterized IN clauses.
            let job_ids_str = job_ids_to_update
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "UPDATE job SET status = {} WHERE id IN ({})",
                pending, job_ids_str
            );
            sqlx::query(&sql).execute(&mut *conn).await.map_err(|e| {
                error!("Failed to update job status: {}", e);
                ApiError("Database update error".to_string())
            })?;

            debug!(
                "Updated {} jobs to pending status for workflow {}",
                job_ids_to_update.len(),
                workflow_id
            );
        }

        // Commit the transaction to release the database lock
        sqlx::query("COMMIT")
            .execute(&mut *conn)
            .await
            .map_err(|e| {
                error!("Failed to commit transaction: {}", e);
                ApiError("Database commit error".to_string())
            })?;

        let response = models::ClaimNextJobsResponse {
            jobs: Some(selected_jobs),
        };

        Ok(ClaimNextJobsResponse::SuccessfulResponse(response))
    }

    /// Check for changed job inputs and update status accordingly.
    #[instrument(level = "debug", skip(self, body, context), fields(workflow_id = id, dry_run = ?dry_run))]
    async fn process_changed_job_inputs(
        &self,
        id: i64,
        dry_run: Option<bool>,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<ProcessChangedJobInputsResponse, ApiError> {
        let dry_run_value = dry_run.unwrap_or(false);
        self.jobs_api
            .process_changed_job_inputs(id, body, dry_run_value, context)
            .await
    }

    /// Delete a compute node.
    async fn delete_compute_node(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteComputeNodeResponse, ApiError> {
        self.compute_nodes_api
            .delete_compute_node(id, body, context)
            .await
    }

    /// Delete an event.
    async fn delete_event(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteEventResponse, ApiError> {
        self.events_api.delete_event(id, body, context).await
    }

    /// Delete a file.
    async fn delete_file(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteFileResponse, ApiError> {
        self.files_api.delete_file(id, body, context).await
    }

    /// Delete a job.
    async fn delete_job(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteJobResponse, ApiError> {
        // Check access control (via workflow)
        match self.check_job_access_for_context(id, context).await {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(DeleteJobResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(DeleteJobResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }
        self.jobs_api.delete_job(id, body, context).await
    }

    /// Delete a local scheduler.
    async fn delete_local_scheduler(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteLocalSchedulerResponse, ApiError> {
        self.schedulers_api
            .delete_local_scheduler(id, body, context)
            .await
    }

    /// Delete a resource requirements record.
    async fn delete_resource_requirements(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteResourceRequirementsResponse, ApiError> {
        self.resource_requirements_api
            .delete_resource_requirements(id, body, context)
            .await
    }

    /// Delete a job result.
    async fn delete_result(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteResultResponse, ApiError> {
        self.results_api.delete_result(id, body, context).await
    }

    /// Delete a scheduled compute node.
    async fn delete_scheduled_compute_node(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteScheduledComputeNodeResponse, ApiError> {
        self.schedulers_api
            .delete_scheduled_compute_node(id, body, context)
            .await
    }

    /// Delete Slurm compute node configuration.
    async fn delete_slurm_scheduler(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteSlurmSchedulerResponse, ApiError> {
        self.schedulers_api
            .delete_slurm_scheduler(id, body, context)
            .await
    }

    /// Delete a user data record.
    async fn delete_user_data(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteUserDataResponse, ApiError> {
        self.user_data_api.delete_user_data(id, body, context).await
    }

    /// Delete a workflow.
    async fn delete_workflow(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteWorkflowResponse, ApiError> {
        info!(
            "delete_workflow(workflow_id={}) - X-Span-ID: {:?}",
            id,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        // Check access control
        match self.check_workflow_access_for_context(id, context).await {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(DeleteWorkflowResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(DeleteWorkflowResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }

        // Clear in-memory failure tracking for this workflow
        if let Ok(mut set) = self.workflows_with_failures.write() {
            set.remove(&id);
        }

        self.workflows_api.delete_workflow(id, body, context).await
    }

    /// Reset status for jobs to uninitialized.
    /// If failed_only is true, only jobs with a failed result will be reset.
    /// If failed_only is false, all jobs will be reset.
    async fn reset_job_status(
        &self,
        id: i64,
        failed_only: Option<bool>,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<ResetJobStatusResponse, ApiError> {
        info!(
            "reset_job_status(workflow_id={}, failed_only={:?}) - X-Span-ID: {:?}",
            id,
            failed_only,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        // Check if workflow exists
        let workflow_exists = self.workflows_api.does_workflow_exist(id, context).await?;
        if !workflow_exists {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", id)
            }));
            return Ok(ResetJobStatusResponse::NotFoundErrorResponse(
                error_response,
            ));
        }

        let failed_only_value = failed_only.unwrap_or(false);
        let result = self
            .jobs_api
            .reset_job_status(id, failed_only_value, body, context)
            .await?;

        // Log event for successful reset
        if let ResetJobStatusResponse::SuccessfulResponse(ref response) = result {
            let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
            let username = auth
                .map(|a| a.subject)
                .unwrap_or_else(|| "unknown".to_string());

            let event = models::EventModel::new(
                id,
                serde_json::json!({
                    "category": "user_action",
                    "action": "reset_job_status",
                    "user": username,
                    "workflow_id": id,
                    "failed_only": failed_only_value,
                    "updated_count": response.updated_count,
                }),
            );
            if let Err(e) = self.events_api.create_event(event, context).await {
                error!("Failed to create event for reset_job_status: {:?}", e);
            }
        }

        Ok(result)
    }

    /// Reset worklow status.
    /// Rules:
    /// - Not allowed if any job is running or SubmittedPending (unless force=true)
    /// Actions:
    /// - Reset fields in WorkflowStatusModel
    async fn reset_workflow_status(
        &self,
        id: i64,
        force: Option<bool>,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<ResetWorkflowStatusResponse, ApiError> {
        info!(
            "reset_workflow_status(workflow_id={}, force={:?}) - X-Span-ID: {:?}",
            id,
            force,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        // Clear in-memory failure tracking for this workflow
        if let Ok(mut set) = self.workflows_with_failures.write() {
            set.remove(&id);
        }

        // TODO: don't allow this if any nodes are scheduled
        let force_value = force.unwrap_or(false);
        let result = self
            .workflows_api
            .reset_workflow_status(id, force, body, context)
            .await?;

        // Log event for successful reset
        if let ResetWorkflowStatusResponse::SuccessfulResponse(_) = result {
            let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
            let username = auth
                .map(|a| a.subject)
                .unwrap_or_else(|| "unknown".to_string());

            let event = models::EventModel::new(
                id,
                serde_json::json!({
                    "category": "user_action",
                    "action": "reset_workflow_status",
                    "user": username,
                    "workflow_id": id,
                    "force": force_value,
                }),
            );
            if let Err(e) = self.events_api.create_event(event, context).await {
                error!("Failed to create event for reset_workflow_status: {:?}", e);
            }
        }

        Ok(result)
    }

    /// Build a string for a DOT graph.
    async fn get_dot_graph(
        &self,
        id: i64,
        name: String,
        context: &C,
    ) -> Result<GetDotGraphResponse, ApiError> {
        debug!(
            "get_dot_graph({}, \"{}\") - X-Span-ID: {:?}",
            id,
            name,
            Has::<XSpanIdString>::get(context).0.clone()
        );
        error!("get_dot_graph operation is not implemented");
        Err(ApiError("Api-Error: Operation is NOT implemented".into()))
    }

    /// Change the status of a job and manage side effects.
    #[instrument(level = "debug", skip(self, body, context), fields(job_id = id, status = ?status, run_id))]
    async fn manage_status_change(
        &self,
        id: i64,
        status: models::JobStatus,
        run_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<ManageStatusChangeResponse, ApiError> {
        debug!(
            "manage_status_change({}, {:?}, {}, {:?}) - X-Span-ID: {:?}",
            id,
            status,
            run_id,
            body,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        // Guard: Reject completion statuses - those must go through complete_job
        // Completion statuses trigger unblocking of dependent jobs via a background task,
        // which requires proper result records to be created first.
        if status.is_complete() {
            error!(
                "manage_status_change: cannot set completion status '{}' for job_id={}. Use complete_job instead.",
                status, id
            );
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "Cannot set completion status '{}' via manage_status_change. Use complete_job API instead.",
                    status
                )
            }));
            return Ok(
                ManageStatusChangeResponse::UnprocessableContentErrorResponse(error_response),
            );
        }

        // 1. Call get_job. If the job doesn't exist, return a 404.
        let mut job = match self.jobs_api.get_job(id, context).await? {
            GetJobResponse::SuccessfulResponse(job) => job,
            GetJobResponse::ForbiddenErrorResponse(err) => {
                return Ok(ManageStatusChangeResponse::DefaultErrorResponse(err));
            }
            GetJobResponse::NotFoundErrorResponse(err) => {
                return Ok(ManageStatusChangeResponse::NotFoundErrorResponse(err));
            }
            GetJobResponse::DefaultErrorResponse(err) => {
                return Ok(ManageStatusChangeResponse::DefaultErrorResponse(err));
            }
        };

        let current_status = *job.status.as_ref().ok_or_else(|| {
            error!("Job status is missing for job_id={}", id);
            ApiError("Job status is required".to_string())
        })?;

        if current_status == status {
            debug!(
                "manage_status_change: job_id={} already has status '{}', no change needed",
                id, status
            );
            return Ok(ManageStatusChangeResponse::SuccessfulResponse(job));
        }

        // 2. Validate run_id matches workflow status run_id
        if let Err(e) = self.validate_run_id(job.workflow_id, run_id).await {
            error!("manage_status_change: job_id={}, {}", id, e);
            let error_response = models::ErrorResponse::new(serde_json::json!({ "message": e }));
            return Ok(
                ManageStatusChangeResponse::UnprocessableContentErrorResponse(error_response),
            );
        }

        job.status = Some(status);

        // 3. Call self.update_job to set the new status
        let updated_job = match self
            .jobs_api
            .update_job_status(id, job.status.expect("Job status must be set"), context)
            .await?
        {
            UpdateJobResponse::SuccessfulResponse(job) => job,
            UpdateJobResponse::ForbiddenErrorResponse(err) => {
                return Ok(ManageStatusChangeResponse::DefaultErrorResponse(err));
            }
            UpdateJobResponse::NotFoundErrorResponse(err) => {
                return Ok(ManageStatusChangeResponse::NotFoundErrorResponse(err));
            }
            UpdateJobResponse::UnprocessableContentErrorResponse(err) => {
                return Ok(ManageStatusChangeResponse::UnprocessableContentErrorResponse(err));
            }
            UpdateJobResponse::DefaultErrorResponse(err) => {
                return Ok(ManageStatusChangeResponse::DefaultErrorResponse(err));
            }
        };

        // Handle reversion from complete to uninitialized
        if current_status.is_complete() && status == models::JobStatus::Uninitialized {
            // Current status is complete and new status is Uninitialized
            // Change all downstream jobs accordingly - jobs blocked by this job that are "done"
            // should also be changed to JobStatus::Uninitialized
            if let Err(e) = self.reinitialize_downstream_jobs(id, job.workflow_id).await {
                error!(
                    "Failed to reinitialize downstream jobs for job {}: {}",
                    id, e
                );
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": "Failed to reinitialize downstream jobs"
                }));
                return Ok(ManageStatusChangeResponse::DefaultErrorResponse(
                    error_response,
                ));
            }
        }

        debug!(
            "manage_status_change: successfully changed job_id={} status from '{}' to '{}'",
            id, current_status, status
        );

        Ok(ManageStatusChangeResponse::SuccessfulResponse(updated_job))
    }

    /// Start a job and manage side effects.
    #[instrument(level = "debug", skip(self, body, context), fields(job_id = id, run_id, compute_node_id))]
    async fn start_job(
        &self,
        id: i64,
        run_id: i64,
        compute_node_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<StartJobResponse, ApiError> {
        debug!(
            "start_job({}, {}, {}, {:?}) - X-Span-ID: {:?}",
            id,
            run_id,
            compute_node_id,
            body,
            Has::<XSpanIdString>::get(context).0.clone()
        );
        let mut job = match self.jobs_api.get_job(id, context).await? {
            GetJobResponse::SuccessfulResponse(job) => job,
            GetJobResponse::ForbiddenErrorResponse(err) => {
                error!("Access denied for job {}: {:?}", id, err);
                return Err(ApiError("Access denied for job".to_string()));
            }
            GetJobResponse::NotFoundErrorResponse(err) => {
                error!("Job not found {}: {:?}", id, err);
                return Err(ApiError("Job not found".to_string()));
            }
            GetJobResponse::DefaultErrorResponse(err) => {
                error!("Failed to get job {}: {:?}", id, err);
                return Err(ApiError("Failed to get job".to_string()));
            }
        };
        match job.status {
            Some(models::JobStatus::Pending) => {
                job.status = Some(models::JobStatus::Running);
            }
            Some(status) => {
                error!(
                    "start_job: Invalid job status for job_id={}. Expected SubmittedPending, got {:?}",
                    id, status
                );
                return Err(ApiError(format!(
                    "Job {} has invalid status {:?}. Expected SubmittedPending for job start.",
                    id, status
                )));
            }
            None => {
                error!("start_job: Job status not set for job_id={}", id);
                return Err(ApiError(format!(
                    "Job {} has no status set. Expected SubmittedPending for job start.",
                    id
                )));
            }
        }

        // Validate run_id matches workflow status run_id before proceeding
        if let Err(e) = self.validate_run_id(job.workflow_id, run_id).await {
            error!("start_job: job_id={}, {}", id, e);
            let error_response = models::ErrorResponse::new(serde_json::json!({ "message": e }));
            return Ok(StartJobResponse::UnprocessableContentErrorResponse(
                error_response,
            ));
        }

        // Set active_compute_node_id to track which compute node is running this job
        match sqlx::query!(
            "UPDATE job_internal SET active_compute_node_id = ? WHERE job_id = ?",
            compute_node_id,
            id
        )
        .execute(self.pool.as_ref())
        .await
        {
            Ok(_) => {
                debug!(
                    "Set active_compute_node_id={} for job_id={}",
                    compute_node_id, id
                );
            }
            Err(e) => {
                error!(
                    "Failed to set active_compute_node_id for job_id={}: {}",
                    id, e
                );
                // Continue anyway - this is not critical for job execution
            }
        }

        self.manage_job_status_change(&job, run_id).await?;

        // Broadcast job_started event to SSE clients (ephemeral, not persisted to DB)
        self.event_broadcaster.broadcast(BroadcastEvent {
            workflow_id: job.workflow_id,
            timestamp: chrono::Utc::now().timestamp_millis(),
            event_type: "job_started".to_string(),
            severity: models::EventSeverity::Info,
            data: serde_json::json!({
                "job_id": id,
                "job_name": job.name,
                "compute_node_id": compute_node_id,
                "run_id": run_id,
            }),
        });
        debug!("Broadcast job_started event for job_id={}", id);

        Ok(StartJobResponse::SuccessfulResponse(job))
    }

    /// Complete a job, connect it to a result, and manage side effects.
    #[instrument(level = "debug", skip(self, result, context), fields(job_id = id, status = ?status, run_id))]
    async fn complete_job(
        &self,
        id: i64,
        status: models::JobStatus,
        run_id: i64,
        result: models::ResultModel,
        context: &C,
    ) -> Result<CompleteJobResponse, ApiError> {
        debug!(
            "complete_job({}, {:?}, {}, {:?}) - X-Span-ID: {:?}",
            id,
            status,
            run_id,
            result,
            Has::<XSpanIdString>::get(context).0.clone()
        );
        // 1. Verify job status is terminal (finished executing)
        if !status.is_terminal() {
            error!(
                "Attempted to complete job {} with non-terminal status '{}'",
                id, status
            );
            return Err(ApiError(format!(
                "Status '{}' is not a terminal status for job completion",
                status
            )));
        }

        // 1. Ensure the job exists.
        let mut job = match self.jobs_api.get_job(id, context).await? {
            GetJobResponse::SuccessfulResponse(job) => job,
            GetJobResponse::ForbiddenErrorResponse(err) => {
                error!("Access denied for job {}: {:?}", id, err);
                return Err(ApiError("Access denied for job".to_string()));
            }
            GetJobResponse::NotFoundErrorResponse(err) => {
                error!("Job not found {}: {:?}", id, err);
                return Err(ApiError("Job not found".to_string()));
            }
            GetJobResponse::DefaultErrorResponse(err) => {
                error!("Failed to get job {}: {:?}", id, err);
                return Err(ApiError("Failed to get job".to_string()));
            }
        };

        // Check if job is already complete
        if let Some(current_status) = &job.status
            && current_status.is_complete()
        {
            error!(
                "Job {} is already complete with status {:?}",
                id, current_status
            );
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Job {} is already complete with status {:?}", id, current_status)
            }));
            return Ok(CompleteJobResponse::UnprocessableContentErrorResponse(
                error_response,
            ));
        }

        job.status = Some(status);

        // Clear active_compute_node_id since the job is no longer running
        match sqlx::query!(
            "UPDATE job_internal SET active_compute_node_id = NULL WHERE job_id = ?",
            id
        )
        .execute(self.pool.as_ref())
        .await
        {
            Ok(_) => {
                debug!("Cleared active_compute_node_id for job_id={}", id);
            }
            Err(e) => {
                error!(
                    "Failed to clear active_compute_node_id for job_id={}: {}",
                    id, e
                );
                // Continue anyway - this is not critical
            }
        }

        // Capture return_code before moving result to create_result
        let result_return_code = result.return_code;

        // 2. Add the result to the database
        let result_response = self.results_api.create_result(result, context).await?;

        let result_id = match result_response {
            CreateResultResponse::SuccessfulResponse(result) => {
                debug!(
                    "complete_job: added result with ID {:?} for job_id={}",
                    result.id, id
                );
                result.id
            }
            CreateResultResponse::NotFoundErrorResponse(err) => {
                error!("Failed to add result for job {}: {:?}", id, err);
                return Err(ApiError("Failed to add result".to_string()));
            }
            CreateResultResponse::DefaultErrorResponse(err) => {
                error!("Failed to add result for job {}: {:?}", id, err);
                return Err(ApiError("Failed to add result".to_string()));
            }
        };

        // 3. Add/update workflow_result record using INSERT OR REPLACE for atomic upsert.
        // This handles the case where a job is being re-run or a result is being replaced.
        // The table has PRIMARY KEY (workflow_id, job_id), so conflicts are automatically resolved.
        let workflow_id = job.workflow_id;
        let result_id_value = result_id.ok_or_else(|| {
            error!("Result ID is missing after creating result");
            ApiError("Result ID is missing".to_string())
        })?;

        match sqlx::query!(
            r#"
            INSERT OR REPLACE INTO workflow_result (workflow_id, job_id, result_id)
            VALUES (?, ?, ?)
            "#,
            workflow_id,
            id,
            result_id_value
        )
        .execute(self.pool.as_ref())
        .await
        {
            Ok(_) => {
                debug!(
                    "complete_job: added workflow_result record for workflow_id={}, job_id={}, result_id={}",
                    workflow_id, id, result_id_value
                );
            }
            Err(e) => {
                error!(
                    "Failed to insert workflow_result for workflow_id={}, job_id={}, result_id={}: {}",
                    workflow_id, id, result_id_value, e
                );
                return Err(ApiError("Database error".to_string()));
            }
        }

        // 4. Call manage_job_status_change for validation and side effects
        self.manage_job_status_change(&job, run_id).await?;

        // 5. Broadcast job completion event to SSE clients (ephemeral, not persisted to DB)
        let event_type = format!("job_{}", status.to_string().to_lowercase());
        let severity = match status {
            models::JobStatus::Completed => models::EventSeverity::Info,
            models::JobStatus::Failed => models::EventSeverity::Error,
            models::JobStatus::Terminated | models::JobStatus::Canceled => {
                models::EventSeverity::Warning
            }
            _ => models::EventSeverity::Info,
        };
        self.event_broadcaster.broadcast(BroadcastEvent {
            workflow_id: job.workflow_id,
            timestamp: chrono::Utc::now().timestamp_millis(),
            event_type,
            severity,
            data: serde_json::json!({
                "job_id": id,
                "job_name": job.name,
                "status": status.to_string(),
                "return_code": result_return_code,
            }),
        });
        debug!("Broadcast job completion event for job_id={}", id);

        debug!(
            "complete_job: successfully completed job_id={} with status={}, result_id={:?}",
            id, status, result_id
        );

        // Note: We intentionally do NOT update the job input hash on completion.
        // The hash is stored at initialization time and represents the baseline for
        // detecting input changes during reinitialize. Updating it here would be
        // redundant since inputs shouldn't change during execution.

        // Check if any on_jobs_complete actions should be triggered
        // Only check actions that involve this specific completed job for efficiency
        if let Err(e) = self
            .workflow_actions_api
            .check_and_trigger_actions(workflow_id, "on_jobs_complete", Some(vec![id]))
            .await
        {
            error!(
                "Failed to check_and_trigger_actions for on_jobs_complete: {}",
                e
            );
        }

        // Note: on_jobs_ready actions are triggered by the background unblock thread
        // (process_workflow_unblocks) when jobs transition to Ready status.

        // Check if workflow is now complete and trigger on_workflow_complete actions
        match self
            .workflows_api
            .is_workflow_complete(workflow_id, context)
            .await
        {
            Ok(response) => {
                if let IsWorkflowCompleteResponse::SuccessfulResponse(completion_status) = response
                    && completion_status.is_complete
                {
                    debug!(
                        "Workflow {} is complete, triggering on_workflow_complete actions",
                        workflow_id
                    );
                    if let Err(e) = self
                        .workflow_actions_api
                        .check_and_trigger_actions(workflow_id, "on_workflow_complete", None)
                        .await
                    {
                        error!(
                            "Failed to check_and_trigger_actions for on_workflow_complete: {}",
                            e
                        );
                    }
                }
            }
            Err(e) => {
                error!(
                    "Failed to check if workflow {} is complete: {}",
                    workflow_id, e
                );
            }
        }

        Ok(CompleteJobResponse::SuccessfulResponse(job))
    }

    /// Retry a failed job by resetting it to ready status and incrementing attempt_id.
    async fn retry_job(
        &self,
        id: i64,
        run_id: i64,
        max_retries: i32,
        context: &C,
    ) -> Result<RetryJobResponse, ApiError> {
        debug!(
            "retry_job({}, {}, {}) - X-Span-ID: {:?}",
            id,
            run_id,
            max_retries,
            Has::<XSpanIdString>::get(context).0.clone()
        );
        let result = self
            .jobs_api
            .retry_job(id, run_id, max_retries, context)
            .await?;

        Ok(result)
    }

    /// Get ready jobs that fit within the specified resource constraints.
    ///
    /// This function performs the following operations:
    /// 1. Queries job and resource_requirements tables for ready jobs
    /// 2. Filters jobs based on resource constraints:
    ///    - memory_bytes <= resources.memory_gb (converted from GiB to bytes)
    ///    - num_cpus <= resources.num_cpus
    ///    - num_gpus <= resources.num_gpus
    ///    - num_nodes <= resources.num_nodes (only multi-node jobs consume dedicated nodes)
    ///    - runtime_s < resources.time_limit (converted to seconds using duration_string_to_seconds)
    /// 3. Sorts results according to the sort_method:
    ///    - None: No sorting applied
    ///    - GpusRuntimeMemory: Sort by num_gpus DESC, runtime_s DESC, memory_bytes DESC
    ///    - GpusMemoryRuntime: Sort by num_gpus DESC, memory_bytes DESC, runtime_s DESC
    /// 4. Loops through returned records and accumulates resource consumption
    /// 5. Selects jobs that can fit within total available resources
    /// 6. Atomically updates selected jobs to "pending" status
    ///
    /// # Parameters
    /// - `workflow_id`: ID of the workflow to get jobs for
    /// - `resources`: Available compute resources (CPUs, memory, GPUs, nodes, time limit)
    /// - `sort_method`: Optional sorting method for job prioritization
    /// - `limit`: Maximum number of jobs to return
    ///
    /// # Returns
    /// A `ClaimJobsBasedOnResources` containing the list of jobs that were selected and updated,
    /// or an error if the operation fails. The `reason` field is set to an empty string.
    ///
    /// # Implementation Notes
    /// - Uses SQLite BEGIN IMMEDIATE TRANSACTION to acquire a database write lock
    /// - This ensures thread-safe access at the database level, preventing race conditions
    /// - The lock prevents concurrent job selection and ensures consistent resource accounting
    /// - Leverages the time_utils::duration_string_to_seconds function for time parsing
    /// - All selected jobs are changed from "ready" to "pending" status atomically
    #[instrument(
        level = "debug",
        skip(self, resources, context),
        fields(workflow_id, limit)
    )]
    async fn prepare_ready_jobs(
        &self,
        workflow_id: i64,
        resources: models::ComputeNodesResources,
        sort_method: Option<models::ClaimJobsSortMethod>,
        limit: i64,
        strict_scheduler_match: Option<bool>,
        context: &C,
    ) -> Result<ClaimJobsBasedOnResources, ApiError> {
        let strict_scheduler_match = strict_scheduler_match.unwrap_or(false);
        // Use BEGIN IMMEDIATE TRANSACTION to acquire a database write lock.
        // This ensures thread safety at the database level for the entire job selection process.
        // The IMMEDIATE mode acquires a reserved lock immediately, preventing other writers
        // and ensuring that concurrent reads see a consistent snapshot.
        // This prevents race conditions where multiple clients could:
        // 1. Select the same ready jobs from the job table
        // 2. Double-allocate jobs to different clients
        // 3. Create inconsistent resource counting
        // The lock is held for the entire transaction duration.

        // Start an IMMEDIATE transaction using raw SQL
        // We can't use pool.begin() because it starts a regular transaction, not IMMEDIATE
        let mut conn = self.pool.acquire().await.map_err(|e| {
            error!("Failed to acquire database connection: {}", e);
            ApiError("Database connection error".to_string())
        })?;

        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(|e| {
                error!("Failed to begin immediate transaction: {}", e);
                ApiError("Database lock error".to_string())
            })?;

        let actual_sort_method = sort_method.unwrap_or(models::ClaimJobsSortMethod::None);
        debug!(
            "get_ready_jobs: workflow_id={}, limit={}, sort_method={:?}, resources={:?} - X-Span-ID: {:?}",
            workflow_id,
            limit,
            actual_sort_method,
            resources,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        // First check if the workflow exists
        let workflow_exists = sqlx::query("SELECT id FROM workflow WHERE id = $1")
            .bind(workflow_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| {
                error!("Database error checking workflow existence: {}", e);
                ApiError("Database error".to_string())
            })?;

        if workflow_exists.is_none() {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;

            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", workflow_id)
            }));
            return Ok(ClaimJobsBasedOnResources::NotFoundErrorResponse(
                error_response,
            ));
        }

        let time_limit_seconds = if let Some(ref time_limit) = resources.time_limit {
            match duration_string_to_seconds(time_limit) {
                Ok(seconds) => seconds,
                Err(e) => {
                    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;

                    let error_response = models::ErrorResponse::new(serde_json::json!({
                        "message": format!("Invalid time_limit format '{}': {}", time_limit, e),
                        "field": "time_limit",
                        "value": time_limit
                    }));
                    return Ok(
                        ClaimJobsBasedOnResources::UnprocessableContentErrorResponse(
                            error_response,
                        ),
                    );
                }
            }
        } else {
            // If no time limit specified, use a very large value (effectively unlimited)
            i64::MAX
        };

        let memory_bytes = (resources.memory_gb * 1024.0 * 1024.0 * 1024.0) as i64;

        let ready_status = models::JobStatus::Ready.to_int();
        let order_by_clause = match actual_sort_method {
            models::ClaimJobsSortMethod::None => "",
            models::ClaimJobsSortMethod::GpusRuntimeMemory => {
                "ORDER BY rr.num_gpus DESC, rr.runtime_s DESC, rr.memory_bytes DESC"
            }
            models::ClaimJobsSortMethod::GpusMemoryRuntime => {
                "ORDER BY rr.num_gpus DESC, rr.memory_bytes DESC, rr.runtime_s DESC"
            }
        };

        // Query with scheduler filter
        let query_with_scheduler = format!(
            r#"
            SELECT
                job.workflow_id,
                job.id AS job_id,
                job.name,
                job.command,
                job.invocation_script,
                job.status,
                job.cancel_on_blocking_job_failure,
                job.supports_termination,
                job.failure_handler_id,
                job.attempt_id,
                rr.id AS resource_requirements_id,
                rr.memory_bytes,
                rr.num_cpus,
                rr.num_gpus,
                rr.num_nodes,
                rr.runtime_s
            FROM job
            JOIN resource_requirements rr ON job.resource_requirements_id = rr.id
            WHERE job.workflow_id = $1
            AND job.status = $2
            AND rr.memory_bytes <= $3
            AND rr.num_cpus <= $4
            AND rr.num_gpus <= $5
            AND rr.num_nodes <= $6
            AND rr.runtime_s <= $7
            AND (job.scheduler_id IS NULL OR job.scheduler_id = $8)
            {}
            LIMIT $9
            "#,
            order_by_clause
        );

        // First try with scheduler filter
        let mut rows = sqlx::query(&query_with_scheduler)
            .bind(workflow_id)
            .bind(ready_status)
            .bind(memory_bytes)
            .bind(resources.num_cpus)
            .bind(resources.num_gpus)
            .bind(resources.num_nodes)
            .bind(time_limit_seconds)
            .bind(resources.scheduler_config_id)
            .bind(limit)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| {
                error!("Database error in get_ready_jobs: {}", e);
                ApiError("Database error".to_string())
            })?;

        // If no jobs found with scheduler filter and strict_scheduler_match is false,
        // retry without the scheduler filter
        if rows.is_empty() && !strict_scheduler_match {
            // Query without scheduler filter
            let query_without_scheduler = format!(
                r#"
                SELECT
                    job.workflow_id,
                    job.id AS job_id,
                    job.name,
                    job.command,
                    job.invocation_script,
                    job.status,
                    job.cancel_on_blocking_job_failure,
                    job.supports_termination,
                    job.failure_handler_id,
                    job.attempt_id,
                    rr.id AS resource_requirements_id,
                    rr.memory_bytes,
                    rr.num_cpus,
                    rr.num_gpus,
                    rr.num_nodes,
                    rr.runtime_s
                FROM job
                JOIN resource_requirements rr ON job.resource_requirements_id = rr.id
                WHERE job.workflow_id = $1
                AND job.status = $2
                AND rr.memory_bytes <= $3
                AND rr.num_cpus <= $4
                AND rr.num_gpus <= $5
                AND rr.num_nodes <= $6
                AND rr.runtime_s <= $7
                {}
                LIMIT $8
                "#,
                order_by_clause
            );

            rows = sqlx::query(&query_without_scheduler)
                .bind(workflow_id)
                .bind(ready_status)
                .bind(memory_bytes)
                .bind(resources.num_cpus)
                .bind(resources.num_gpus)
                .bind(resources.num_nodes)
                .bind(time_limit_seconds)
                .bind(limit)
                .fetch_all(&mut *conn)
                .await
                .map_err(|e| {
                    error!(
                        "Database error in get_ready_jobs (no scheduler filter): {}",
                        e
                    );
                    ApiError("Database error".to_string())
                })?;

            if !rows.is_empty() {
                info!(
                    "Worker with scheduler_config_id={:?} found {} ready jobs after removing scheduler filter \
                     (strict_scheduler_match=false).",
                    resources.scheduler_config_id,
                    rows.len()
                );
            }
        }

        // Track consumed resources
        let mut consumed_memory_bytes = 0i64;
        let mut consumed_cpus = 0i64;
        let mut consumed_gpus = 0i64;
        let mut consumed_nodes = 0i64; // Only tracks dedicated nodes consumed by multi-node jobs
        let mut selected_jobs = Vec::new();
        let mut job_ids_to_update = Vec::new();

        debug!(
            "get_ready_jobs: Found {} potential jobs for workflow {} with resources: memory_gb={}, cpus={}, gpus={}, nodes={}, time_limit={:?}",
            rows.len(),
            workflow_id,
            resources.memory_gb,
            resources.num_cpus,
            resources.num_gpus,
            resources.num_nodes,
            resources.time_limit
        );
        // Loop through jobs and select those that fit within resource limits
        for row in rows {
            let job_memory: i64 = row.get("memory_bytes");
            let job_cpus: i64 = row.get("num_cpus");
            let job_gpus: i64 = row.get("num_gpus");
            let job_nodes: i64 = row.get("num_nodes");

            // Check if this job would exceed resource limits
            // For multi-node jobs (num_nodes > 1), check if we have enough dedicated nodes
            // For single-node jobs (num_nodes = 1), they can share nodes so only check other resources
            let would_exceed_nodes = if job_nodes > 1 {
                consumed_nodes + job_nodes > resources.num_nodes
            } else {
                false // Single-node jobs don't count against node limit
            };

            if consumed_memory_bytes + job_memory <= memory_bytes
                && consumed_cpus + job_cpus <= resources.num_cpus
                && consumed_gpus + job_gpus <= resources.num_gpus
                && !would_exceed_nodes
            {
                // Add this job to the selection
                consumed_memory_bytes += job_memory;
                consumed_cpus += job_cpus;
                consumed_gpus += job_gpus;
                // Only increment consumed_nodes for multi-node jobs
                if job_nodes > 1 {
                    consumed_nodes += job_nodes;
                }

                let job_id: i64 = row.get("job_id");
                job_ids_to_update.push(job_id);

                let status = models::JobStatus::from_int(row.get::<i64, _>("status") as i32)
                    .map_err(|e| {
                        error!("Failed to parse job status: {}", e);
                        ApiError("Invalid job status".to_string())
                    })?;

                if status != models::JobStatus::Ready {
                    error!("Expected job status to be Ready, but got: {}", status);
                    // Rollback the transaction since we're returning early
                    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                    return Err(ApiError("Invalid job status in ready queue".to_string()));
                }
                let job = models::JobModel {
                    id: Some(job_id),
                    workflow_id: row.get("workflow_id"),
                    name: row.get("name"),
                    command: row.get("command"),
                    invocation_script: None,
                    status: Some(models::JobStatus::Pending),
                    schedule_compute_nodes: None,
                    cancel_on_blocking_job_failure: Some(row.get("cancel_on_blocking_job_failure")),
                    supports_termination: Some(row.get("supports_termination")),
                    depends_on_job_ids: None,
                    input_file_ids: None,
                    output_file_ids: None,
                    input_user_data_ids: None,
                    output_user_data_ids: None,
                    resource_requirements_id: Some(row.get("resource_requirements_id")),
                    scheduler_id: None,
                    failure_handler_id: row.get("failure_handler_id"),
                    attempt_id: row.get("attempt_id"),
                };

                selected_jobs.push(job);
            } else {
                // Job would exceed resource limits, skip it
                let node_limit_reason = if job_nodes > 1
                    && consumed_nodes + job_nodes > resources.num_nodes
                {
                    format!(
                        " [multi-node job needs {} nodes but only {} available after {} consumed]",
                        job_nodes,
                        resources.num_nodes - consumed_nodes,
                        consumed_nodes
                    )
                } else {
                    String::new()
                };

                debug!(
                    "Skipping job {} - would exceed resource limits (memory: {}/{}, cpus: {}/{}, gpus: {}/{}){}",
                    row.get::<i64, _>("job_id"),
                    consumed_memory_bytes + job_memory,
                    memory_bytes,
                    consumed_cpus + job_cpus,
                    resources.num_cpus,
                    consumed_gpus + job_gpus,
                    resources.num_gpus,
                    node_limit_reason
                );
            }
        }

        // Query output file and user_data relationships for all selected jobs
        let mut output_files_map: std::collections::HashMap<i64, Vec<i64>> =
            std::collections::HashMap::new();
        let mut output_user_data_map: std::collections::HashMap<i64, Vec<i64>> =
            std::collections::HashMap::new();

        if !job_ids_to_update.is_empty() {
            // Query output files
            let output_files =
                sqlx::query("SELECT job_id, file_id FROM job_output_file WHERE workflow_id = $1")
                    .bind(workflow_id)
                    .fetch_all(&mut *conn)
                    .await
                    .map_err(|e| {
                        error!("Failed to query output files: {}", e);
                        ApiError("Database query error".to_string())
                    })?;

            for row in output_files {
                let job_id: i64 = row.get("job_id");
                let file_id: i64 = row.get("file_id");
                if job_ids_to_update.contains(&job_id) {
                    output_files_map.entry(job_id).or_default().push(file_id);
                }
            }

            // Query output user_data
            let output_user_data = sqlx::query("SELECT job_id, user_data_id FROM job_output_user_data WHERE job_id IN (SELECT id FROM job WHERE workflow_id = $1)")
                .bind(workflow_id)
                .fetch_all(&mut *conn)
                .await
                .map_err(|e| {
                    error!("Failed to query output user_data: {}", e);
                    ApiError("Database query error".to_string())
                })?;

            for row in output_user_data {
                let job_id: i64 = row.get("job_id");
                let user_data_id: i64 = row.get("user_data_id");
                if job_ids_to_update.contains(&job_id) {
                    output_user_data_map
                        .entry(job_id)
                        .or_default()
                        .push(user_data_id);
                }
            }
        }

        // Populate the output file and user_data IDs in the selected jobs
        for job in &mut selected_jobs {
            if let Some(job_id) = job.id {
                job.output_file_ids = output_files_map.get(&job_id).cloned();
                job.output_user_data_ids = output_user_data_map.get(&job_id).cloned();
            }
        }

        // If we have jobs to update, update their status to pending using bulk UPDATE
        if !job_ids_to_update.is_empty() {
            let pending = models::JobStatus::Pending.to_int();
            // SAFETY: job_ids are i64 from database query results.
            // i64::to_string() only produces numeric strings - SQL injection impossible.
            // Using string formatting because sqlx doesn't support parameterized IN clauses.
            let job_ids_str = job_ids_to_update
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "UPDATE job SET status = {} WHERE id IN ({})",
                pending, job_ids_str
            );
            sqlx::query(&sql).execute(&mut *conn).await.map_err(|e| {
                error!("Failed to update job status: {}", e);
                ApiError("Database update error".to_string())
            })?;

            debug!(
                "Updated {} jobs to pending status for workflow {}",
                job_ids_to_update.len(),
                workflow_id
            );
        }

        // Commit the transaction to release the database lock
        sqlx::query("COMMIT")
            .execute(&mut *conn)
            .await
            .map_err(|e| {
                error!("Failed to commit transaction: {}", e);
                ApiError("Database commit error".to_string())
            })?;

        // Note: The `reason` field is not populated because generating a useful
        // single-string reason is impractical when multiple jobs may be skipped
        // for different reasons (memory, CPUs, GPUs, nodes). Detailed per-job
        // skip reasons are logged at debug level during job selection above.
        let response = models::ClaimJobsBasedOnResources {
            jobs: Some(selected_jobs),
            reason: None,
        };

        Ok(ClaimJobsBasedOnResources::SuccessfulResponse(response))
    }

    // Access Groups API

    async fn create_access_group(
        &self,
        body: models::AccessGroupModel,
        context: &C,
    ) -> Result<CreateAccessGroupResponse, ApiError> {
        // Only system administrators can create access groups
        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        match self.authorization_service.check_admin_access(&auth).await {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(CreateAccessGroupResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                // This shouldn't happen for admin access check, but handle it
                return Ok(CreateAccessGroupResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
        }

        self.access_groups_api
            .create_access_group(body, context)
            .await
    }

    async fn get_access_group(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetAccessGroupResponse, ApiError> {
        self.access_groups_api.get_access_group(id, context).await
    }

    async fn list_access_groups(
        &self,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListAccessGroupsApiResponse, ApiError> {
        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.access_groups_api
            .list_access_groups(offset, limit, context)
            .await
    }

    async fn delete_access_group(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteAccessGroupResponse, ApiError> {
        // Only system administrators can delete access groups
        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        match self.authorization_service.check_admin_access(&auth).await {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(DeleteAccessGroupResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(DeleteAccessGroupResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }

        // Cannot delete system groups (like the admin group)
        match self.authorization_service.is_system_group(id).await {
            Ok(true) => {
                return Ok(DeleteAccessGroupResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": "Cannot delete system groups"
                    })),
                ));
            }
            Ok(false) => {}
            Err(e) => {
                return Ok(DeleteAccessGroupResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": e
                    })),
                ));
            }
        }

        self.access_groups_api
            .delete_access_group(id, context)
            .await
    }

    async fn add_user_to_group(
        &self,
        group_id: i64,
        body: models::UserGroupMembershipModel,
        context: &C,
    ) -> Result<AddUserToGroupResponse, ApiError> {
        // Group admins or system admins can add users to groups
        // Note: check_group_admin_access already blocks modifications to the admin group
        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        match self
            .authorization_service
            .check_group_admin_access(&auth, group_id)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(AddUserToGroupResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(AddUserToGroupResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }

        self.access_groups_api
            .add_user_to_group(group_id, body, context)
            .await
    }

    async fn remove_user_from_group(
        &self,
        group_id: i64,
        user_name: String,
        context: &C,
    ) -> Result<RemoveUserFromGroupResponse, ApiError> {
        // Group admins or system admins can remove users from groups
        // Note: check_group_admin_access already blocks modifications to the admin group
        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        match self
            .authorization_service
            .check_group_admin_access(&auth, group_id)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(RemoveUserFromGroupResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(RemoveUserFromGroupResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }

        self.access_groups_api
            .remove_user_from_group(group_id, &user_name, context)
            .await
    }

    async fn list_group_members(
        &self,
        group_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListGroupMembersResponse, ApiError> {
        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.access_groups_api
            .list_group_members(group_id, offset, limit, context)
            .await
    }

    async fn list_user_groups(
        &self,
        user_name: String,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListUserGroupsApiResponse, ApiError> {
        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.access_groups_api
            .list_user_groups(&user_name, offset, limit, context)
            .await
    }

    async fn add_workflow_to_group(
        &self,
        workflow_id: i64,
        group_id: i64,
        context: &C,
    ) -> Result<AddWorkflowToGroupResponse, ApiError> {
        // Workflow owner or group admin can add workflows to groups
        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        match self
            .authorization_service
            .check_workflow_group_access(&auth, workflow_id, group_id)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(AddWorkflowToGroupResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(AddWorkflowToGroupResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }

        self.access_groups_api
            .add_workflow_to_group(workflow_id, group_id, context)
            .await
    }

    async fn remove_workflow_from_group(
        &self,
        workflow_id: i64,
        group_id: i64,
        context: &C,
    ) -> Result<RemoveWorkflowFromGroupResponse, ApiError> {
        // Workflow owner or group admin can remove workflows from groups
        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        match self
            .authorization_service
            .check_workflow_group_access(&auth, workflow_id, group_id)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok(RemoveWorkflowFromGroupResponse::ForbiddenErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "Forbidden",
                        "message": reason
                    })),
                ));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok(RemoveWorkflowFromGroupResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NotFound",
                        "message": reason
                    })),
                ));
            }
        }

        self.access_groups_api
            .remove_workflow_from_group(workflow_id, group_id, context)
            .await
    }

    async fn list_workflow_groups(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListWorkflowGroupsResponse, ApiError> {
        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.access_groups_api
            .list_workflow_groups(workflow_id, offset, limit, context)
            .await
    }

    async fn check_workflow_access(
        &self,
        workflow_id: i64,
        user_name: String,
        context: &C,
    ) -> Result<CheckWorkflowAccessResponse, ApiError> {
        self.access_groups_api
            .check_workflow_access(workflow_id, &user_name, context)
            .await
    }

    /// Subscribe to the event broadcast channel for SSE streaming.
    fn subscribe_to_events(&self) -> tokio::sync::broadcast::Receiver<BroadcastEvent> {
        self.event_broadcaster.subscribe()
    }
}

// Helper methods for Server (not part of the Api trait)
impl<C> Server<C>
where
    C: Has<XSpanIdString> + Send + Sync,
{
    // No additional helper methods needed
}
