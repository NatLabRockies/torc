//! Version checking utilities for comparing client and server API versions.
//!
//! This module provides functions to check API version compatibility between
//! client applications and the torc-server, with appropriate warning levels.
//!
//! The HTTP API has its own semver version (e.g., "0.8.0") that is independent
//! of the crate/binary version. This allows the client to change frequently
//! without implying server incompatibility. The API version only bumps when the
//! HTTP contract changes.

use crate::client::apis;
use crate::client::apis::configuration::Configuration;

/// The current version of this binary, set at compile time.
pub const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The API version that this client expects from the server.
///
/// Bump this only when the HTTP API contract changes:
/// - Patch: bug fix in an existing endpoint (response field fix, etc.)
/// - Minor: new endpoint, new optional field, new query parameter
/// - Major: removed endpoint, renamed field, changed semantics
pub const CLIENT_API_VERSION: &str = crate::api_version::HTTP_API_VERSION;

/// The git commit hash of this binary, set at compile time via build.rs.
pub const GIT_HASH: &str = env!("GIT_HASH");

/// Returns the full version string including git hash (e.g., "0.8.0 (abc1234)")
pub fn full_version() -> String {
    format!("{} ({})", CLIENT_VERSION, GIT_HASH)
}

/// Returns just the version with git hash suffix (e.g., "0.8.0-abc1234")
pub fn version_with_hash() -> String {
    format!("{}-{}", CLIENT_VERSION, GIT_HASH)
}

/// Severity level for API version mismatches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionMismatchSeverity {
    /// API versions match exactly - no warning needed.
    None,
    /// Only patch version differs - minor warning.
    Patch,
    /// Client API minor version is higher than server - some features may not work.
    Minor,
    /// API major version differs - incompatible.
    Major,
}

impl VersionMismatchSeverity {
    /// Returns true if this severity level should prevent operation.
    pub fn is_blocking(&self) -> bool {
        matches!(self, VersionMismatchSeverity::Major)
    }

    /// Returns true if any warning should be displayed.
    pub fn has_warning(&self) -> bool {
        !matches!(self, VersionMismatchSeverity::None)
    }
}

/// Information retrieved from the server's /version endpoint.
#[derive(Debug, Clone)]
pub struct ServerInfo {
    /// The server's binary version (e.g., "0.14.0 (abc1234)").
    pub version: String,
    /// The server's API version (e.g., "0.8.0").
    pub api_version: Option<String>,
}

/// Result of an API version check operation.
#[derive(Debug, Clone)]
pub struct VersionCheckResult {
    /// The client binary version.
    pub client_version: String,
    /// The server binary version (if successfully retrieved).
    pub server_version: Option<String>,
    /// The client API version.
    pub client_api_version: String,
    /// The server API version (if successfully retrieved).
    pub server_api_version: Option<String>,
    /// The severity of any API version mismatch.
    pub severity: VersionMismatchSeverity,
    /// A human-readable message describing the result.
    pub message: String,
}

impl VersionCheckResult {
    /// Creates a new result for when the server couldn't be reached.
    pub fn server_unreachable() -> Self {
        Self {
            client_version: CLIENT_VERSION.to_string(),
            server_version: None,
            client_api_version: CLIENT_API_VERSION.to_string(),
            server_api_version: None,
            severity: VersionMismatchSeverity::None,
            message: "Could not check server version".to_string(),
        }
    }

    /// Creates a new result for a successful version check.
    pub fn from_server_info(server_info: &ServerInfo) -> Self {
        let (severity, message) = match &server_info.api_version {
            Some(server_api) => {
                let severity = compare_versions(CLIENT_API_VERSION, server_api);
                let message = format_api_version_message(
                    CLIENT_API_VERSION,
                    server_api,
                    &server_info.version,
                    severity,
                );
                (severity, message)
            }
            None => {
                // Old server that doesn't report api_version — fall back to
                // comparing binary versions (pre-API-versioning behavior).
                let severity = compare_versions(CLIENT_VERSION, &server_info.version);
                let message =
                    format_legacy_version_message(CLIENT_VERSION, &server_info.version, severity);
                (severity, message)
            }
        };

        Self {
            client_version: CLIENT_VERSION.to_string(),
            server_version: Some(server_info.version.clone()),
            client_api_version: CLIENT_API_VERSION.to_string(),
            server_api_version: server_info.api_version.clone(),
            severity,
            message,
        }
    }
}

/// Parses a version string into (major, minor, patch) components.
/// Returns None if parsing fails.
/// Handles formats like "0.8.0", "v0.8.0", "0.8.0-beta", "0.8.0 (abc1234)"
pub fn parse_version(version: &str) -> Option<(u32, u32, u32)> {
    // Strip any leading 'v' if present
    let version = version.strip_prefix('v').unwrap_or(version);

    // Strip git hash suffix like " (abc1234)" or " (abc1234-dirty)"
    let version = version.split(" (").next().unwrap_or(version);

    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() < 3 {
        return None;
    }

    let major = parts[0].parse().ok()?;
    let minor = parts[1].parse().ok()?;
    // Handle patch versions that may have suffixes like "-beta"
    let patch_str = parts[2].split('-').next().unwrap_or(parts[2]);
    let patch = patch_str.parse().ok()?;

    Some((major, minor, patch))
}

/// Compares two version strings and returns the severity of any mismatch.
pub fn compare_versions(client_version: &str, server_version: &str) -> VersionMismatchSeverity {
    let client = match parse_version(client_version) {
        Some(v) => v,
        None => {
            eprintln!(
                "Warning: failed to parse client version '{}'; skipping version comparison",
                client_version
            );
            return VersionMismatchSeverity::None;
        }
    };

    let server = match parse_version(server_version) {
        Some(v) => v,
        None => {
            eprintln!(
                "Warning: failed to parse server version '{}'; skipping version comparison",
                server_version
            );
            return VersionMismatchSeverity::None;
        }
    };

    // Check major version difference
    if client.0 != server.0 {
        return VersionMismatchSeverity::Major;
    }

    // Check if minor versions differ
    if client.1 != server.1 {
        return VersionMismatchSeverity::Minor;
    }

    // Check if versions differ in patch
    if client.2 != server.2 {
        return VersionMismatchSeverity::Patch;
    }

    VersionMismatchSeverity::None
}

/// Formats a human-readable message for an API version mismatch.
fn format_api_version_message(
    client_api: &str,
    server_api: &str,
    server_version: &str,
    severity: VersionMismatchSeverity,
) -> String {
    match severity {
        VersionMismatchSeverity::None => {
            format!(
                "API version {} matches server (server {})",
                client_api, server_version
            )
        }
        VersionMismatchSeverity::Patch => {
            format!(
                "API version mismatch: client API {} vs server API {} \
                 (server {}) - patch difference, should be compatible",
                client_api, server_api, server_version
            )
        }
        VersionMismatchSeverity::Minor => {
            let client_parsed = parse_version(client_api);
            let server_parsed = parse_version(server_api);
            let direction = if client_parsed > server_parsed {
                "client is newer than server"
            } else {
                "server is newer than client"
            };
            format!(
                "API version mismatch: client API {} vs server API {} \
                 (server {}) - minor version difference ({direction}), should be compatible",
                client_api, server_api, server_version
            )
        }
        VersionMismatchSeverity::Major => {
            format!(
                "API version incompatible: client API {} vs server API {} \
                 (server {}) - major version mismatch, client and server are not compatible",
                client_api, server_api, server_version
            )
        }
    }
}

/// Formats a human-readable message when the server doesn't report an API version
/// (pre-API-versioning server). Falls back to comparing binary versions.
fn format_legacy_version_message(
    client_version: &str,
    server_version: &str,
    severity: VersionMismatchSeverity,
) -> String {
    match severity {
        VersionMismatchSeverity::None => {
            format!("Version {} matches server", client_version)
        }
        VersionMismatchSeverity::Patch => {
            format!(
                "Version mismatch: client {} vs server {} (patch difference)",
                client_version, server_version
            )
        }
        VersionMismatchSeverity::Minor => {
            format!(
                "Client version {} is newer than server {} \
                 - server does not report API version, some features may not work",
                client_version, server_version
            )
        }
        VersionMismatchSeverity::Major => {
            format!(
                "Major version mismatch: client {} vs server {} \
                 - server does not report API version, client and server are likely incompatible",
                client_version, server_version
            )
        }
    }
}

/// Fetches server information from the /version endpoint.
pub fn get_server_info(config: &Configuration) -> Option<ServerInfo> {
    match apis::system_api::get_version(config) {
        Ok(value) => Some(ServerInfo {
            version: value.version,
            api_version: Some(value.api_version),
        }),
        Err(_) => None,
    }
}

/// Fetches the server version string from the API.
/// Returns the binary version for display purposes (e.g., in log messages).
pub fn get_server_version(config: &Configuration) -> Option<String> {
    get_server_info(config).map(|info| info.version)
}

/// Performs an API version check between the client and server.
pub fn check_version(config: &Configuration) -> VersionCheckResult {
    match get_server_info(config) {
        Some(server_info) => VersionCheckResult::from_server_info(&server_info),
        None => VersionCheckResult::server_unreachable(),
    }
}

/// Prints a version warning to stderr if appropriate.
/// Returns the severity level for programmatic use.
pub fn print_version_warning(result: &VersionCheckResult) -> VersionMismatchSeverity {
    match result.severity {
        VersionMismatchSeverity::None | VersionMismatchSeverity::Patch => {}
        VersionMismatchSeverity::Minor => {
            eprintln!("Warning: {}", result.message);
        }
        VersionMismatchSeverity::Major => {
            eprintln!("Error: {}", result.message);
        }
    }
    result.severity
}

/// Checks the server version and prints appropriate warnings.
/// Returns true if the version check passed (no major incompatibility).
pub fn check_and_warn(config: &Configuration) -> bool {
    let result = check_version(config);
    let severity = print_version_warning(&result);
    !severity.is_blocking()
}
