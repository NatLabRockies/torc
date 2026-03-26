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
        debug!(
            "get_version() - X-Span-ID: {:?}",
            Has::<XSpanIdString>::get(context).0.clone()
        );
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
        debug!(
            "ping() - X-Span-ID: {:?}",
            Has::<XSpanIdString>::get(context).0.clone()
        );
        Ok(PingResponse::SuccessfulResponse(
            serde_json::json!({"status": "ok"}),
        ))
    }

    pub(super) async fn transport_reload_auth(
        &self,
        context: &C,
    ) -> Result<ReloadAuthResponse, ApiError> {
        debug!(
            "reload_auth() - X-Span-ID: {:?}",
            Has::<XSpanIdString>::get(context).0.clone()
        );

        authorize_admin!(self, context, ReloadAuthResponse);

        let auth_file_path = match &self.auth_file_path {
            Some(path) => path.clone(),
            None => {
                return Ok(ReloadAuthResponse::DefaultErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "NoAuthFile",
                        "message": "No auth file configured. Start the server with --auth-file to enable auth reloading."
                    })),
                ));
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
                Ok(ReloadAuthResponse::DefaultErrorResponse(
                    models::ErrorResponse::new(serde_json::json!({
                        "error": "ReloadFailed",
                        "message": format!("Failed to reload htpasswd file: {}", e)
                    })),
                ))
            }
        }
    }
}
