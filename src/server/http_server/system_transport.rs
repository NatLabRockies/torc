use super::*;
use crate::server::htpasswd::HtpasswdFile;

impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync,
{
    pub(super) async fn transport_get_version(
        &self,
        context: &C,
    ) -> Result<GetVersionResponse, ApiError> {
        log_call!(debug, context, "get_version()");
        if self.authorization_service.enforce_access_control() {
            Ok(GetVersionResponse::SuccessfulResponse(serde_json::json!({
                "version": full_version(),
                "api_version": API_VERSION,
            })))
        } else {
            Ok(GetVersionResponse::SuccessfulResponse(serde_json::json!({
                "version": full_version(),
                "api_version": API_VERSION,
                "git_hash": GIT_HASH
            })))
        }
    }

    pub(super) async fn transport_ping(&self, context: &C) -> Result<PingResponse, ApiError> {
        log_call!(debug, context, "ping()");
        Ok(PingResponse::SuccessfulResponse(
            serde_json::json!({"status": "ok"}),
        ))
    }

    pub(super) async fn transport_reload_auth(
        &self,
        context: &C,
    ) -> Result<ReloadAuthResponse, ApiError> {
        log_call!(debug, context, "reload_auth()");

        authorize_admin!(self, context, ReloadAuthResponse);

        let auth_file_path = match &self.auth_file_path {
            Some(path) => path.clone(),
            None => {
                return Ok(ReloadAuthResponse::DefaultErrorResponse(error_payload!(
                    "NoAuthFile",
                    "No auth file configured. Start the server with --auth-file to enable auth reloading."
                )));
            }
        };

        info!("Reloading htpasswd file from: {}", auth_file_path);

        let load_result = tokio::task::spawn_blocking(move || HtpasswdFile::load(&auth_file_path))
            .await
            .map_err(|e| ApiError(format!("spawn_blocking failed: {e}")))?;

        match load_result {
            Ok(new_htpasswd) => {
                let user_count = new_htpasswd.user_count();

                {
                    let mut htpasswd_guard = self.htpasswd.write();
                    *htpasswd_guard = Some(new_htpasswd);
                }

                {
                    let cache_guard = self.credential_cache.read();
                    if let Some(cache) = cache_guard.as_ref() {
                        cache.clear();
                    }
                }

                info!(
                    "Successfully reloaded htpasswd file with {} users, credential cache cleared",
                    user_count
                );

                Ok(ReloadAuthResponse::SuccessfulResponse(serde_json::json!({
                    "message": "Auth credentials reloaded successfully",
                    "user_count": user_count
                })))
            }
            Err(e) => {
                error!("Failed to reload htpasswd file: {}", e);
                Ok(ReloadAuthResponse::DefaultErrorResponse(error_payload!(
                    "ReloadFailed",
                    format!("Failed to reload htpasswd file: {}", e)
                )))
            }
        }
    }

    /// Execute a raw SQL statement on behalf of an admin (admin only).
    ///
    /// Reads run on a read-only connection; writes run in a transaction with an
    /// optional dry-run preview, a no-WHERE guard, and an audit-log record. See
    /// [`crate::server::api::admin`].
    pub(super) async fn transport_admin_sql(
        &self,
        body: models::AdminSqlRequest,
        context: &C,
    ) -> Result<AdminSqlResponse, ApiError> {
        log_call!(debug, context, "admin_sql(write={})", body.write);

        authorize_admin!(self, context, AdminSqlResponse);

        // Operator opt-out: the whole feature, or just writes, can be disabled
        // via `torc-server --disable-admin-sql[-writes]`. Audit-log listing is
        // intentionally not gated, so past activity stays reviewable.
        if !self.admin_sql.reads_enabled {
            return Ok(AdminSqlResponse::ForbiddenErrorResponse(forbidden_error!(
                "admin SQL is disabled on this server"
            )));
        }
        if body.write && !self.admin_sql.writes_enabled {
            return Ok(AdminSqlResponse::ForbiddenErrorResponse(forbidden_error!(
                "admin SQL writes are disabled on this server"
            )));
        }

        use crate::server::api::admin;

        if let Err(msg) = admin::validate_statement(&body.sql, body.write, body.allow_full_table) {
            return Ok(AdminSqlResponse::UnprocessableContentErrorResponse(
                error_payload!("InvalidStatement", msg),
            ));
        }

        if body.write {
            let user = username_from_context(context);
            match admin::execute_write(&self.pool, &body.sql, body.dry_run).await {
                Ok(rows_affected) => {
                    let committed = !body.dry_run;
                    if committed {
                        admin::record_audit(
                            &self.pool,
                            &user,
                            &body.sql,
                            body.allow_full_table,
                            Some(rows_affected),
                            true,
                            true,
                            None,
                        )
                        .await;
                        info!(
                            "admin_sql write committed by user={} rows_affected={} sql={:?}",
                            user, rows_affected, body.sql
                        );
                    }
                    let payload = models::AdminSqlResponse {
                        columns: Vec::new(),
                        items: Vec::new(),
                        rows_affected: Some(rows_affected),
                        committed,
                    };
                    Ok(AdminSqlResponse::SuccessfulResponse(
                        serde_json::to_value(payload).map_err(|e| ApiError(e.to_string()))?,
                    ))
                }
                Err(msg) => {
                    if !body.dry_run {
                        admin::record_audit(
                            &self.pool,
                            &user,
                            &body.sql,
                            body.allow_full_table,
                            None,
                            false,
                            false,
                            Some(&msg),
                        )
                        .await;
                    }
                    Ok(AdminSqlResponse::UnprocessableContentErrorResponse(
                        error_payload!("ExecutionFailed", msg),
                    ))
                }
            }
        } else {
            let limit = admin::clamp_limit(body.limit);
            match admin::execute_read_only(&self.pool, &body.sql, limit).await {
                Ok((columns, items)) => {
                    let payload = models::AdminSqlResponse {
                        columns,
                        items,
                        rows_affected: None,
                        committed: false,
                    };
                    Ok(AdminSqlResponse::SuccessfulResponse(
                        serde_json::to_value(payload).map_err(|e| ApiError(e.to_string()))?,
                    ))
                }
                Err(msg) => Ok(AdminSqlResponse::UnprocessableContentErrorResponse(
                    error_payload!("QueryFailed", msg),
                )),
            }
        }
    }

    /// List recent admin raw-SQL audit-log entries (admin only).
    ///
    /// Returns a page of `admin_audit_log` rows newest-first with the standard
    /// pagination metadata. See [`crate::server::api::admin::list_audit_log`].
    pub(super) async fn transport_list_admin_audit_log(
        &self,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListAdminAuditLogResponse, ApiError> {
        log_call!(debug, context, "list_admin_audit_log()");

        authorize_admin!(self, context, ListAdminAuditLogResponse);

        use crate::server::api::admin;

        let max_limit = admin::MAX_RESULT_ROWS;
        let offset = offset.unwrap_or(0).max(0);
        let limit = admin::clamp_limit(limit) as i64;

        match admin::list_audit_log(&self.pool, offset, limit).await {
            Ok((items, total_count)) => {
                let count = items.len() as i64;
                let has_more = offset + count < total_count;
                let payload = models::ListAdminAuditLogResponse {
                    items,
                    offset,
                    max_limit,
                    count,
                    total_count,
                    has_more,
                };
                Ok(ListAdminAuditLogResponse::SuccessfulResponse(
                    serde_json::to_value(payload).map_err(|e| ApiError(e.to_string()))?,
                ))
            }
            Err(msg) => Ok(ListAdminAuditLogResponse::DefaultErrorResponse(
                error_payload!("QueryFailed", msg),
            )),
        }
    }
}
