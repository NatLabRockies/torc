//! Common API module with shared imports and traits

use crate::server::transport_types::context_types::ApiError;
use log::{debug, error, info};
use sqlx::sqlite::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;

pub use crate::MAX_RECORD_TRANSFER_COUNT;

/// Shared server context that all API modules can use
#[derive(Clone)]
pub struct ApiContext {
    pub pool: Arc<SqlitePool>,
}

impl ApiContext {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool: Arc::new(pool),
        }
    }
}

/// Common error handling utilities
pub fn database_error_with_msg(e: impl std::fmt::Display, msg: impl Into<String>) -> ApiError {
    let msg_str = msg.into();
    error!("Database error ({}): {}", msg_str, e);
    ApiError(msg_str)
}

/// Like `database_error_with_msg` but preserves "database is locked" in the `ApiError`
/// so that callers can detect lock contention and retry. Does not leak other database
/// error details. Lock contention is logged at debug level (expected transient condition)
/// while other database errors are logged at error level.
pub fn database_lock_aware_error(e: impl std::fmt::Display, msg: impl Into<String>) -> ApiError {
    let msg_str = msg.into();
    let error_string = e.to_string().to_lowercase();
    if error_string.contains("database is locked")
        || error_string.contains("database is busy")
        || error_string.contains("sqlite_busy")
    {
        debug!("Database lock contention ({}): {}", msg_str, e);
        ApiError(format!("{}: database is locked", msg_str))
    } else {
        error!("Database error ({}): {}", msg_str, e);
        ApiError(msg_str)
    }
}

pub fn json_parse_error(e: impl std::fmt::Display) -> ApiError {
    info!("Failed to parse JSON data: {}", e);
    ApiError("Failed to parse event data".to_string())
}

pub fn normalize_env_map(env: Option<HashMap<String, String>>) -> Option<HashMap<String, String>> {
    env.filter(|env_map| !env_map.is_empty())
}

pub fn serialize_env_map(
    env: Option<HashMap<String, String>>,
    field_name: &str,
) -> Result<Option<String>, ApiError> {
    normalize_env_map(env)
        .map(|env_map| {
            serde_json::to_string(&env_map)
                .map_err(|e| ApiError(format!("Failed to serialize {}: {}", field_name, e)))
        })
        .transpose()
}

pub fn deserialize_env_map(
    env_json: Option<String>,
    field_name: &str,
) -> Result<Option<HashMap<String, String>>, ApiError> {
    env_json
        .and_then(|env| {
            let trimmed = env.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
        .map(|env| {
            serde_json::from_str::<HashMap<String, String>>(&env)
                .map_err(|e| ApiError(format!("Failed to parse {}: {}", field_name, e)))
        })
        .transpose()
        .map(normalize_env_map)
}

pub fn validate_env_map(
    env: Option<&HashMap<String, String>>,
    field_name: &str,
) -> Result<(), ApiError> {
    let Some(env) = env else {
        return Ok(());
    };

    for key in env.keys() {
        if !crate::models::is_valid_env_var_name(key) {
            return Err(ApiError(format!(
                "Invalid {} key '{}'; expected [A-Za-z_][A-Za-z0-9_]*",
                field_name, key
            )));
        }
    }

    Ok(())
}

/// Begin a SQLite transaction with `BEGIN IMMEDIATE`, taking the write lock up front.
///
/// In WAL mode, `pool.begin()` issues plain `BEGIN` (= `BEGIN DEFERRED`), which only
/// takes the database write lock when the first write statement runs. If a `SELECT`
/// runs first inside the transaction, the connection acquires a WAL read snapshot,
/// and a subsequent `INSERT`/`UPDATE`/`DELETE` can fail with `SQLITE_BUSY_SNAPSHOT`
/// (extended code 517) the moment any other connection commits in between. The
/// connection's `busy_timeout` does **not** retry `SQLITE_BUSY_SNAPSHOT` -- it
/// returns immediately.
///
/// `BEGIN IMMEDIATE` takes the write lock up front, so the full `busy_timeout`
/// applies to lock acquisition and the snapshot-conflict path is impossible. Use
/// this helper for any handler that mixes reads and writes inside a single
/// transaction.
pub async fn begin_immediate(
    pool: &SqlitePool,
) -> Result<sqlx::Transaction<'static, sqlx::Sqlite>, sqlx::Error> {
    pool.begin_with("BEGIN IMMEDIATE").await
}

/// Escape SQL LIKE wildcard characters in user input.
/// Escapes `%`, `_`, and `\` with a backslash prefix.
pub fn escape_like_pattern(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::{deserialize_env_map, escape_like_pattern, validate_env_map};
    use std::collections::HashMap;

    #[test]
    fn escapes_percent_sign() {
        assert_eq!(escape_like_pattern("100%"), "100\\%");
        assert_eq!(escape_like_pattern("%start"), "\\%start");
        assert_eq!(escape_like_pattern("middle%end"), "middle\\%end");
    }

    #[test]
    fn escapes_underscore() {
        assert_eq!(escape_like_pattern("user_name"), "user\\_name");
        assert_eq!(escape_like_pattern("_leading"), "\\_leading");
        assert_eq!(escape_like_pattern("trailing_"), "trailing\\_");
    }

    #[test]
    fn escapes_backslash() {
        assert_eq!(escape_like_pattern(r"c:\path"), r"c:\\path");
        assert_eq!(escape_like_pattern(r"\\"), r"\\\\");
    }

    #[test]
    fn escapes_multiple_consecutive_wildcards() {
        assert_eq!(escape_like_pattern("%%"), "\\%\\%");
        assert_eq!(escape_like_pattern("__"), "\\_\\_");
        assert_eq!(escape_like_pattern("%_%"), "\\%\\_\\%");
    }

    #[test]
    fn escapes_mixed_special_characters() {
        assert_eq!(escape_like_pattern(r"50%_done\path"), r"50\%\_done\\path");
    }

    #[test]
    fn leaves_normal_strings_unchanged() {
        assert_eq!(escape_like_pattern("simpletext"), "simpletext");
        assert_eq!(escape_like_pattern("123456"), "123456");
        assert_eq!(escape_like_pattern(""), "");
    }

    #[test]
    fn deserialize_env_map_treats_empty_string_as_none() {
        let env = deserialize_env_map(Some(String::new()), "job env").expect("empty env");
        assert_eq!(env, None);
    }

    #[test]
    fn deserialize_env_map_round_trips_json_object() {
        let env = deserialize_env_map(Some(r#"{"FOO":"bar"}"#.to_string()), "job env")
            .expect("valid env json");
        assert_eq!(
            env,
            Some(HashMap::from([("FOO".to_string(), "bar".to_string())]))
        );
    }

    #[test]
    fn validate_env_map_accepts_valid_names() {
        validate_env_map(
            Some(&HashMap::from([
                ("FOO".to_string(), "bar".to_string()),
                ("_CACHE1".to_string(), "/tmp".to_string()),
            ])),
            "job env",
        )
        .expect("valid env map");
    }

    #[test]
    fn validate_env_map_rejects_invalid_names() {
        let err = validate_env_map(
            Some(&HashMap::from([(
                "BAD-NAME".to_string(),
                "bar".to_string(),
            )])),
            "job env",
        )
        .expect_err("invalid env map should fail");
        assert!(err.0.contains("BAD-NAME"));
    }
}

/// Common pagination response structure
#[derive(Debug)]
pub struct PaginationInfo {
    pub offset: i64,
    pub limit: Option<i64>,
    pub total_count: i64,
}

impl PaginationInfo {
    pub fn new(offset: Option<i64>, limit: Option<i64>, total_count: i64) -> Self {
        Self {
            offset: offset.unwrap_or(0),
            limit,
            total_count,
        }
    }

    pub fn has_more(&self) -> bool {
        if let Some(limit) = self.limit {
            self.offset + limit < self.total_count
        } else {
            false
        }
    }
}

// Re-export submodules
pub mod access_groups;
pub mod compute_nodes;
pub mod events;
pub mod failure_handlers;
pub mod files;
pub mod jobs;
pub mod remote_workers;
pub mod resource_requirements;
pub mod results;
pub mod ro_crate;
pub mod schedulers;
pub mod slurm_stats;
pub mod sql_query_builder;
pub mod user_data;
pub mod workflow_actions;
pub mod workflows;

// Re-export API traits and implementations
pub use access_groups::{AccessGroupsApi, AccessGroupsApiImpl};
pub use compute_nodes::{ComputeNodesApi, ComputeNodesApiImpl};
pub use events::{EventsApi, EventsApiImpl};
pub use failure_handlers::{FailureHandlersApi, FailureHandlersApiImpl};
pub use files::{FilesApi, FilesApiImpl};
pub use jobs::{JobsApi, JobsApiImpl};
pub use remote_workers::{RemoteWorkersApi, RemoteWorkersApiImpl};
pub use resource_requirements::{ResourceRequirementsApi, ResourceRequirementsApiImpl};
pub use results::{ResultsApi, ResultsApiImpl};
pub use ro_crate::{RoCrateApi, RoCrateApiImpl};
pub use schedulers::{SchedulersApi, SchedulersApiImpl};
pub use slurm_stats::{SlurmStatsApi, SlurmStatsApiImpl};
pub use sql_query_builder::SqlQueryBuilder;
pub use user_data::{UserDataApi, UserDataApiImpl};
pub use workflow_actions::{WorkflowActionsApi, WorkflowActionsApiImpl};
pub use workflows::{WorkflowsApi, WorkflowsApiImpl};
