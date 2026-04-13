//! Tests for version checking utilities.

use torc::api_version::HTTP_API_VERSION;
use torc::client::version_check::{
    ServerInfo, VersionCheckResult, VersionMismatchSeverity, compare_versions, parse_version,
};

/// Helper to derive test versions relative to HTTP_API_VERSION.
/// This ensures tests don't break when the API version is bumped.
fn api_version_parts() -> (u32, u32, u32) {
    parse_version(HTTP_API_VERSION).expect("HTTP_API_VERSION should be valid semver")
}

/// Returns a version string with the same major.minor but patch + 1.
fn api_version_with_patch_bump() -> String {
    let (major, minor, patch) = api_version_parts();
    format!("{}.{}.{}", major, minor, patch + 1)
}

/// Returns a version string with minor + 1 (server newer than client).
fn api_version_with_minor_bump() -> String {
    let (major, minor, _patch) = api_version_parts();
    format!("{}.{}.0", major, minor + 1)
}

/// Returns a version string with minor - 1 (client newer than server).
fn api_version_with_minor_decrement() -> String {
    let (major, minor, _patch) = api_version_parts();
    // Ensure we don't go negative
    let older_minor = if minor > 0 { minor - 1 } else { 0 };
    format!("{}.{}.0", major, older_minor)
}

#[test]
fn test_parse_version() {
    assert_eq!(parse_version("0.8.0"), Some((0, 8, 0)));
    assert_eq!(parse_version("1.2.3"), Some((1, 2, 3)));
    assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
    assert_eq!(parse_version("1.2.3-beta"), Some((1, 2, 3)));
    // Versions with git hash suffix
    assert_eq!(parse_version("0.8.0 (abc1234)"), Some((0, 8, 0)));
    assert_eq!(parse_version("0.8.0 (abc1234-dirty)"), Some((0, 8, 0)));
    assert_eq!(parse_version("v1.2.3 (def5678)"), Some((1, 2, 3)));
    assert_eq!(parse_version("invalid"), None);
}

#[test]
fn test_compare_versions_exact_match() {
    assert_eq!(
        compare_versions("0.8.0", "0.8.0"),
        VersionMismatchSeverity::None
    );
}

#[test]
fn test_compare_versions_patch_diff() {
    assert_eq!(
        compare_versions("0.8.1", "0.8.0"),
        VersionMismatchSeverity::Patch
    );
    assert_eq!(
        compare_versions("0.8.0", "0.8.1"),
        VersionMismatchSeverity::Patch
    );
}

#[test]
fn test_compare_versions_minor_client_higher() {
    assert_eq!(
        compare_versions("0.9.0", "0.8.0"),
        VersionMismatchSeverity::Minor
    );
}

#[test]
fn test_compare_versions_minor_server_higher() {
    assert_eq!(
        compare_versions("0.8.0", "0.9.0"),
        VersionMismatchSeverity::Minor
    );
}

#[test]
fn test_compare_versions_major_diff() {
    assert_eq!(
        compare_versions("1.0.0", "0.8.0"),
        VersionMismatchSeverity::Major
    );
    assert_eq!(
        compare_versions("0.8.0", "1.0.0"),
        VersionMismatchSeverity::Major
    );
}

// --- Tests for API version checking via VersionCheckResult ---

#[test]
fn test_version_check_with_api_version_match() {
    let info = ServerInfo {
        version: "0.14.0 (abc1234)".to_string(),
        api_version: Some(HTTP_API_VERSION.to_string()),
    };
    let result = VersionCheckResult::from_server_info(&info);
    assert_eq!(result.severity, VersionMismatchSeverity::None);
    assert!(result.message.contains("API version"));
    assert!(result.message.contains("matches"));
}

#[test]
fn test_version_check_with_api_version_patch_diff() {
    // Server has same major.minor but different patch than client
    let info = ServerInfo {
        version: "0.14.0 (abc1234)".to_string(),
        api_version: Some(api_version_with_patch_bump()),
    };
    let result = VersionCheckResult::from_server_info(&info);
    assert_eq!(result.severity, VersionMismatchSeverity::Patch);
    assert!(result.message.contains("patch difference"));
}

#[test]
fn test_version_check_with_api_version_client_newer() {
    // Server has an older minor API version than client
    let info = ServerInfo {
        version: "0.12.0 (abc1234)".to_string(),
        api_version: Some(api_version_with_minor_decrement()),
    };
    let result = VersionCheckResult::from_server_info(&info);
    assert_eq!(result.severity, VersionMismatchSeverity::Minor);
    assert!(result.message.contains("client is newer than server"));
    assert!(result.message.contains("should be compatible"));
}

#[test]
fn test_version_check_with_api_version_major_diff() {
    let info = ServerInfo {
        version: "2.0.0 (abc1234)".to_string(),
        api_version: Some("1.0.0".to_string()),
    };
    let result = VersionCheckResult::from_server_info(&info);
    assert_eq!(result.severity, VersionMismatchSeverity::Major);
    assert!(result.message.contains("incompatible"));
}

#[test]
fn test_version_check_legacy_server_no_api_version() {
    // Pre-API-versioning server returns no api_version — falls back to
    // comparing binary versions.
    let info = ServerInfo {
        version: "0.13.0 (abc1234)".to_string(),
        api_version: None,
    };
    let result = VersionCheckResult::from_server_info(&info);
    assert!(result.server_api_version.is_none());
    // Severity depends on CLIENT_VERSION vs "0.13.0" — we just check
    // that it doesn't panic and produces a legacy-style message.
    assert!(
        result.message.contains("Version")
            || result.message.contains("version")
            || result.message.contains("matches")
    );
}

#[test]
fn test_version_check_server_newer_api_is_minor() {
    // Server has a newer minor API version than client — reported as Minor severity.
    let info = ServerInfo {
        version: "0.15.0 (abc1234)".to_string(),
        api_version: Some(api_version_with_minor_bump()),
    };
    let result = VersionCheckResult::from_server_info(&info);
    assert_eq!(result.severity, VersionMismatchSeverity::Minor);
    assert!(result.message.contains("server is newer than client"));
}

#[test]
fn test_version_check_result_fields() {
    let info = ServerInfo {
        version: "0.14.0 (abc1234)".to_string(),
        api_version: Some(HTTP_API_VERSION.to_string()),
    };
    let result = VersionCheckResult::from_server_info(&info);
    assert_eq!(result.server_version, Some("0.14.0 (abc1234)".to_string()));
    assert_eq!(
        result.server_api_version,
        Some(HTTP_API_VERSION.to_string())
    );
    assert_eq!(result.client_api_version, HTTP_API_VERSION);
}
