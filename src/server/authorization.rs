//! Authorization service for access control checks
//!
//! This module provides authorization utilities that can be used by API handlers
//! to enforce access control based on user identity and group memberships.

use log::{debug, warn};
use sqlx::Row;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use swagger::auth::Authorization;

/// Result type for authorization checks
#[derive(Debug, Clone, PartialEq)]
pub enum AccessCheckResult {
    /// User is allowed to access the resource
    Allowed,
    /// User is not allowed to access the resource
    Denied(String),
    /// Resource was not found
    NotFound(String),
}

impl AccessCheckResult {
    pub fn is_allowed(&self) -> bool {
        matches!(self, AccessCheckResult::Allowed)
    }
}

/// Authorization service for checking user permissions
#[derive(Clone)]
pub struct AuthorizationService {
    pool: Arc<SqlitePool>,
    /// If true, authorization checks are enforced
    /// If false, all access is allowed (for backward compatibility)
    enforce_access_control: bool,
}

impl AuthorizationService {
    /// If true, authorization checks are enforced
    /// If false, all access is allowed (for backward compatibility)
    pub fn enforce_access_control(&self) -> bool {
        self.enforce_access_control
    }

    /// Create a new authorization service
    pub fn new(pool: Arc<SqlitePool>, enforce_access_control: bool) -> Self {
        Self {
            pool,
            enforce_access_control,
        }
    }

    /// Extract the username from the authorization context
    /// Returns None if no authorization is present or user is anonymous
    pub fn get_username(auth: &Option<Authorization>) -> Option<&str> {
        auth.as_ref().and_then(|a| {
            if a.subject == "anonymous" {
                None
            } else {
                Some(a.subject.as_str())
            }
        })
    }

    /// Check if a user can access a workflow
    ///
    /// Access is granted if:
    /// 1. Access control is not enforced (backward compatibility mode)
    /// 2. The user is the owner of the workflow
    /// 3. The user belongs to a group that has access to the workflow
    pub async fn check_workflow_access(
        &self,
        auth: &Option<Authorization>,
        workflow_id: i64,
    ) -> AccessCheckResult {
        // If access control is not enforced, allow everything
        if !self.enforce_access_control {
            return AccessCheckResult::Allowed;
        }

        let username = match Self::get_username(auth) {
            Some(name) => name,
            None => {
                // Anonymous users have no access when access control is enforced
                return AccessCheckResult::Denied(
                    "Anonymous access not allowed when access control is enabled".to_string(),
                );
            }
        };

        debug!(
            "Checking workflow access for user '{}' on workflow {}",
            username, workflow_id
        );

        // Check if workflow exists and get owner
        let workflow_owner: Option<String> =
            match sqlx::query("SELECT user FROM workflow WHERE id = $1")
                .bind(workflow_id)
                .fetch_optional(self.pool.as_ref())
                .await
            {
                Ok(Some(row)) => Some(row.get("user")),
                Ok(None) => {
                    return AccessCheckResult::NotFound(format!(
                        "Workflow not found with ID: {}",
                        workflow_id
                    ));
                }
                Err(e) => {
                    warn!("Database error checking workflow owner: {}", e);
                    return AccessCheckResult::Denied(format!("Database error: {}", e));
                }
            };

        // Check if user is the owner
        if let Some(owner) = workflow_owner
            && owner == username
        {
            debug!("User '{}' is owner of workflow {}", username, workflow_id);
            return AccessCheckResult::Allowed;
        }

        // Check if user has group-based access
        let has_group_access: bool = match sqlx::query(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM workflow_access_group wag
                INNER JOIN user_group_membership ugm ON wag.group_id = ugm.group_id
                WHERE wag.workflow_id = $1 AND ugm.user_name = $2
            ) as has_access
            "#,
        )
        .bind(workflow_id)
        .bind(username)
        .fetch_one(self.pool.as_ref())
        .await
        {
            Ok(row) => row.get::<i32, _>("has_access") == 1,
            Err(e) => {
                warn!("Database error checking group access: {}", e);
                return AccessCheckResult::Denied(format!("Database error: {}", e));
            }
        };

        if has_group_access {
            debug!(
                "User '{}' has group access to workflow {}",
                username, workflow_id
            );
            AccessCheckResult::Allowed
        } else {
            debug!(
                "User '{}' denied access to workflow {}",
                username, workflow_id
            );
            AccessCheckResult::Denied(format!(
                "User '{}' does not have access to workflow {}",
                username, workflow_id
            ))
        }
    }

    /// Check if a user can access a job (via workflow access)
    pub async fn check_job_access(
        &self,
        auth: &Option<Authorization>,
        job_id: i64,
    ) -> AccessCheckResult {
        if !self.enforce_access_control {
            return AccessCheckResult::Allowed;
        }

        // Get the workflow ID for this job
        let workflow_id: Option<i64> =
            match sqlx::query("SELECT workflow_id FROM job WHERE id = $1")
                .bind(job_id)
                .fetch_optional(self.pool.as_ref())
                .await
            {
                Ok(Some(row)) => Some(row.get("workflow_id")),
                Ok(None) => {
                    return AccessCheckResult::NotFound(format!(
                        "Job not found with ID: {}",
                        job_id
                    ));
                }
                Err(e) => {
                    warn!("Database error getting job workflow: {}", e);
                    return AccessCheckResult::Denied(format!("Database error: {}", e));
                }
            };

        match workflow_id {
            Some(wf_id) => self.check_workflow_access(auth, wf_id).await,
            None => AccessCheckResult::NotFound(format!("Job not found with ID: {}", job_id)),
        }
    }

    /// Check if a user can access a resource that has a workflow_id column
    pub async fn check_resource_access(
        &self,
        auth: &Option<Authorization>,
        resource_id: i64,
        table_name: &str,
    ) -> AccessCheckResult {
        if !self.enforce_access_control {
            return AccessCheckResult::Allowed;
        }

        // Validate table name to prevent SQL injection via identifier interpolation.
        // Only allow ASCII alphanumeric characters and underscores, which are safe
        // for use as SQL identifiers and disallow any metacharacters.
        if !table_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            warn!(
                "Invalid table name provided to check_resource_access: {}",
                table_name
            );
            return AccessCheckResult::Denied("Invalid resource type".to_string());
        }

        // Get the workflow ID for this resource
        let sql = format!("SELECT workflow_id FROM {} WHERE id = $1", table_name);
        let workflow_id: Option<i64> = match sqlx::query(&sql)
            .bind(resource_id)
            .fetch_optional(self.pool.as_ref())
            .await
        {
            Ok(Some(row)) => Some(row.get("workflow_id")),
            Ok(None) => {
                return AccessCheckResult::NotFound(format!(
                    "Resource not found in {} with ID: {}",
                    table_name, resource_id
                ));
            }
            Err(e) => {
                warn!(
                    "Database error getting workflow for {} ID {}: {}",
                    table_name, resource_id, e
                );
                return AccessCheckResult::Denied(format!("Database error: {}", e));
            }
        };

        match workflow_id {
            Some(wf_id) => self.check_workflow_access(auth, wf_id).await,
            None => AccessCheckResult::NotFound(format!(
                "Resource not found in {} with ID: {}",
                table_name, resource_id
            )),
        }
    }

    /// Check if a user can access a workflow status
    pub async fn check_workflow_status_access(
        &self,
        auth: &Option<Authorization>,
        status_id: i64,
    ) -> AccessCheckResult {
        if !self.enforce_access_control {
            return AccessCheckResult::Allowed;
        }

        // workflow_status ID is the same as workflow ID
        self.check_workflow_access(auth, status_id).await
    }

    /// Get all workflow IDs that a user can access
    /// This is useful for filtering list queries
    pub async fn get_accessible_workflow_ids(
        &self,
        auth: &Option<Authorization>,
    ) -> Result<Option<Vec<i64>>, String> {
        if !self.enforce_access_control {
            // Return None to indicate no filtering needed
            return Ok(None);
        }

        let username = match Self::get_username(auth) {
            Some(name) => name,
            None => {
                // Anonymous users have no access
                return Ok(Some(Vec::new()));
            }
        };

        // Get all workflows the user owns OR has group access to
        let records = match sqlx::query(
            r#"
            SELECT DISTINCT w.id
            FROM workflow w
            WHERE w.user = $1
            UNION
            SELECT DISTINCT wag.workflow_id
            FROM workflow_access_group wag
            INNER JOIN user_group_membership ugm ON wag.group_id = ugm.group_id
            WHERE ugm.user_name = $1
            "#,
        )
        .bind(username)
        .fetch_all(self.pool.as_ref())
        .await
        {
            Ok(rows) => rows,
            Err(e) => {
                return Err(format!("Database error: {}", e));
            }
        };

        let ids: Vec<i64> = records.into_iter().map(|row| row.get("id")).collect();
        Ok(Some(ids))
    }

    /// Build a SQL WHERE clause fragment for filtering by accessible workflows
    /// Returns None if no filtering is needed, or Some(clause, bind_values) if filtering is needed
    pub async fn build_workflow_access_filter(
        &self,
        auth: &Option<Authorization>,
        workflow_id_column: &str,
    ) -> Result<Option<(String, Vec<i64>)>, String> {
        match self.get_accessible_workflow_ids(auth).await? {
            None => Ok(None), // No filtering needed
            Some(ids) if ids.is_empty() => {
                // User has no access to any workflows - return impossible condition
                Ok(Some(("1 = 0".to_string(), Vec::new())))
            }
            Some(ids) => {
                // Build IN clause
                let placeholders: Vec<String> =
                    (0..ids.len()).map(|i| format!("${}", i + 1)).collect();
                let clause = format!("{} IN ({})", workflow_id_column, placeholders.join(", "));
                Ok(Some((clause, ids)))
            }
        }
    }

    /// Check if access control is enforced
    pub fn is_enforced(&self) -> bool {
        self.enforce_access_control
    }

    /// Check if a user is a system administrator
    ///
    /// A user is an admin if they are a member of the "admin" group (is_system = 1)
    pub async fn check_admin_access(&self, auth: &Option<Authorization>) -> AccessCheckResult {
        if !self.enforce_access_control {
            return AccessCheckResult::Allowed;
        }

        let username = match Self::get_username(auth) {
            Some(name) => name,
            None => {
                return AccessCheckResult::Denied(
                    "Anonymous access not allowed for admin operations".to_string(),
                );
            }
        };

        let is_admin: bool = match sqlx::query(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM user_group_membership ugm
                INNER JOIN access_group ag ON ugm.group_id = ag.id
                WHERE ugm.user_name = $1 AND ag.is_system = 1 AND ag.name = 'admin'
            ) as is_admin
            "#,
        )
        .bind(username)
        .fetch_one(self.pool.as_ref())
        .await
        {
            Ok(row) => row.get::<i32, _>("is_admin") == 1,
            Err(e) => {
                warn!("Database error checking admin status: {}", e);
                return AccessCheckResult::Denied(format!("Database error: {}", e));
            }
        };

        if is_admin {
            debug!("User '{}' is a system administrator", username);
            AccessCheckResult::Allowed
        } else {
            debug!("User '{}' is not a system administrator", username);
            AccessCheckResult::Denied(format!("User '{}' is not a system administrator", username))
        }
    }

    /// Check if a user can manage a specific access group
    ///
    /// A user can manage a group if:
    /// 1. They are a system administrator
    /// 2. They are an admin of that specific group (role = 'admin')
    ///
    /// Note: The system 'admin' group can only be managed via config
    pub async fn check_group_admin_access(
        &self,
        auth: &Option<Authorization>,
        group_id: i64,
    ) -> AccessCheckResult {
        if !self.enforce_access_control {
            return AccessCheckResult::Allowed;
        }

        let username = match Self::get_username(auth) {
            Some(name) => name,
            None => {
                return AccessCheckResult::Denied(
                    "Anonymous access not allowed for group management".to_string(),
                );
            }
        };

        // Check if the target group is the system admin group
        let is_system_group: bool =
            match sqlx::query("SELECT is_system FROM access_group WHERE id = $1")
                .bind(group_id)
                .fetch_optional(self.pool.as_ref())
                .await
            {
                Ok(Some(row)) => row.get::<i32, _>("is_system") == 1,
                Ok(None) => {
                    return AccessCheckResult::NotFound(format!(
                        "Group not found with ID: {}",
                        group_id
                    ));
                }
                Err(e) => {
                    warn!("Database error checking group: {}", e);
                    return AccessCheckResult::Denied(format!("Database error: {}", e));
                }
            };

        // System admin group membership is managed via config only
        if is_system_group {
            return AccessCheckResult::Denied(
                "Admin group membership is managed via server configuration".to_string(),
            );
        }

        // Check if user is a system admin
        if let AccessCheckResult::Allowed = self.check_admin_access(auth).await {
            return AccessCheckResult::Allowed;
        }

        // Check if user is an admin of this specific group
        let is_group_admin: bool = match sqlx::query(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM user_group_membership
                WHERE user_name = $1 AND group_id = $2 AND role = 'admin'
            ) as is_admin
            "#,
        )
        .bind(username)
        .bind(group_id)
        .fetch_one(self.pool.as_ref())
        .await
        {
            Ok(row) => row.get::<i32, _>("is_admin") == 1,
            Err(e) => {
                warn!("Database error checking group admin status: {}", e);
                return AccessCheckResult::Denied(format!("Database error: {}", e));
            }
        };

        if is_group_admin {
            debug!("User '{}' is an admin of group {}", username, group_id);
            AccessCheckResult::Allowed
        } else {
            debug!("User '{}' is not an admin of group {}", username, group_id);
            AccessCheckResult::Denied(format!(
                "User '{}' is not an admin of group {}",
                username, group_id
            ))
        }
    }

    /// Check if a user can add a workflow to a group
    ///
    /// A user can add a workflow to a group if:
    /// 1. They are the owner of the workflow, OR
    /// 2. They are an admin of the group (or system admin)
    pub async fn check_workflow_group_access(
        &self,
        auth: &Option<Authorization>,
        workflow_id: i64,
        group_id: i64,
    ) -> AccessCheckResult {
        if !self.enforce_access_control {
            return AccessCheckResult::Allowed;
        }

        let username = match Self::get_username(auth) {
            Some(name) => name,
            None => {
                return AccessCheckResult::Denied(
                    "Anonymous access not allowed for workflow-group operations".to_string(),
                );
            }
        };

        // Check if user owns the workflow
        let workflow_owner: Option<String> =
            match sqlx::query("SELECT user FROM workflow WHERE id = $1")
                .bind(workflow_id)
                .fetch_optional(self.pool.as_ref())
                .await
            {
                Ok(Some(row)) => Some(row.get("user")),
                Ok(None) => {
                    return AccessCheckResult::NotFound(format!(
                        "Workflow not found with ID: {}",
                        workflow_id
                    ));
                }
                Err(e) => {
                    warn!("Database error checking workflow owner: {}", e);
                    return AccessCheckResult::Denied(format!("Database error: {}", e));
                }
            };

        if let Some(owner) = workflow_owner
            && owner == username
        {
            debug!(
                "User '{}' is owner of workflow {}, allowed to manage group access",
                username, workflow_id
            );
            return AccessCheckResult::Allowed;
        }

        // Check if user is admin of the group
        self.check_group_admin_access(auth, group_id).await
    }

    /// Check if a group is a system group (cannot be deleted)
    pub async fn is_system_group(&self, group_id: i64) -> Result<bool, String> {
        match sqlx::query("SELECT is_system FROM access_group WHERE id = $1")
            .bind(group_id)
            .fetch_optional(self.pool.as_ref())
            .await
        {
            Ok(Some(row)) => Ok(row.get::<i32, _>("is_system") == 1),
            Ok(None) => Err(format!("Group not found with ID: {}", group_id)),
            Err(e) => Err(format!("Database error: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_username_with_auth() {
        let auth = Some(Authorization {
            subject: "testuser".to_string(),
            scopes: swagger::auth::Scopes::All,
            issuer: None,
        });
        assert_eq!(AuthorizationService::get_username(&auth), Some("testuser"));
    }

    #[test]
    fn test_get_username_anonymous() {
        let auth = Some(Authorization {
            subject: "anonymous".to_string(),
            scopes: swagger::auth::Scopes::All,
            issuer: None,
        });
        assert_eq!(AuthorizationService::get_username(&auth), None);
    }

    #[test]
    fn test_get_username_none() {
        let auth: Option<Authorization> = None;
        assert_eq!(AuthorizationService::get_username(&auth), None);
    }

    #[test]
    fn test_access_check_result() {
        assert!(AccessCheckResult::Allowed.is_allowed());
        assert!(!AccessCheckResult::Denied("test".to_string()).is_allowed());
        assert!(!AccessCheckResult::NotFound("test".to_string()).is_allowed());
    }
}
