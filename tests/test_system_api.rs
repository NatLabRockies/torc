mod common;

use common::{start_server, start_server_with_access_control};
use torc::api_version::HTTP_API_VERSION;
use torc::client::apis;

#[test]
fn test_get_version_includes_git_hash_without_access_control() {
    let server = start_server();
    let version = apis::system_api::get_version(&server.config).expect("get_version should work");

    assert_eq!(version.api_version, HTTP_API_VERSION);
    assert!(version.git_hash.is_some(), "git_hash should be exposed");
}

#[test]
fn test_get_version_hides_git_hash_with_access_control() {
    let server = start_server_with_access_control();
    let version = apis::system_api::get_version(&server.config).expect("get_version should work");

    assert_eq!(version.api_version, HTTP_API_VERSION);
    assert_eq!(version.git_hash, None);
}
