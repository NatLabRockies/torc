use super::*;
use crate::server::api::ResourceRequirementsApi;

impl<C> Server<C> {
    /// Create a depends-on association between two jobs based on file dependencies.
    pub(super) async fn add_depends_on_associations_from_files<'e, E>(
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
    pub(super) async fn add_depends_on_associations_from_user_data<'e, E>(
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

    pub(super) async fn uninitialize_blocked_jobs<'e, E>(
        &self,
        executor: E,
        workflow_id: i64,
    ) -> Result<(), ApiError>
    where
        E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
    {
        let uninitialized_status = models::JobStatus::Uninitialized.to_int();
        match sqlx::query!(
            r#"
            WITH RECURSIVE jobs_to_uninitialize(job_id) AS (
                SELECT id FROM job
                WHERE workflow_id = $1 AND status = $2
                UNION
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

    pub(super) async fn initialize_blocked_jobs_to_blocked<'e, E>(
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

    pub(super) async fn get_default_resource_requirements_id<Ctx>(
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
                None,
                Some("default".to_string()),
                None,
                None,
                None,
                None,
                None,
                0,
                1,
                None,
                None,
                context,
            )
            .await;

        match result {
            Ok(ListResourceRequirementsResponse::SuccessfulResponse(records)) => {
                let items = records.items;
                if items.len() != 1 {
                    return Err(ApiError(
                        "Expected exactly 1 default resource requirement, found different number"
                            .to_string(),
                    ));
                }
                if let Some(id) = items[0].id {
                    Ok(id)
                } else {
                    Err(ApiError(
                        "Default resource requirement has no ID".to_string(),
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

    pub(super) async fn initialize_unblocked_jobs<'e, E>(
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

    pub(super) async fn validate_run_id(
        &self,
        workflow_id: i64,
        provided_run_id: i64,
    ) -> Result<(), String> {
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
}
