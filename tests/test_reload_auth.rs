mod common;

use serial_test::serial;
use std::process::Command;
use torc::client::apis::configuration::Configuration;
use torc::client::apis::default_api;

fn get_exe_path(name: &str) -> String {
    common::get_exe_path(name)
}

/// Admin can call reload-auth and get 200 with user count.
#[test]
#[serial]
fn test_reload_auth_success() {
    let server = common::start_server_with_required_auth();
    let admin_config = server.config_for_user("owner");

    let result = default_api::reload_auth(&admin_config);
    assert!(result.is_ok(), "reload_auth should succeed for admin");

    let body = result.unwrap();
    assert_eq!(
        body.get("message").and_then(|v| v.as_str()).unwrap_or(""),
        "Auth credentials reloaded successfully"
    );
    assert!(
        body.get("user_count").and_then(|v| v.as_u64()).unwrap_or(0) > 0,
        "user_count should be > 0"
    );
}

/// Non-admin user gets 403 when calling reload-auth.
#[test]
#[serial]
fn test_reload_auth_forbidden() {
    let server = common::start_server_with_required_auth();

    // "dave" is not in the admin group
    let dave_config = server.config_for_user("dave");
    let result = default_api::reload_auth(&dave_config);
    assert!(result.is_err(), "reload_auth should fail for non-admin");
}

/// Add a user to htpasswd on disk, verify they can't auth yet, reload, then verify they can.
#[test]
#[serial]
fn test_reload_auth_new_user_can_authenticate() {
    let server = common::start_server_with_required_auth();
    let admin_config = server.config_for_user("owner");
    let htpasswd_path = server.htpasswd_path();

    // Add a new user "eve" to the htpasswd file
    let status = Command::new(get_exe_path("./target/debug/torc-htpasswd"))
        .arg("add")
        .arg("--file")
        .arg(&htpasswd_path)
        .arg("--password")
        .arg("correct horse battery staple")
        .arg("eve")
        .status()
        .expect("Failed to run torc-htpasswd");
    assert!(status.success(), "torc-htpasswd add should succeed");

    // Eve can't authenticate yet (server hasn't reloaded)
    let eve_config = server.config_for_user("eve");
    let eve_result = default_api::ping(&eve_config);
    assert!(
        eve_result.is_err(),
        "Eve should NOT be able to authenticate before reload"
    );

    // Admin reloads auth
    let reload_result = default_api::reload_auth(&admin_config);
    assert!(reload_result.is_ok(), "reload_auth should succeed");

    // Now eve can authenticate
    let eve_result = default_api::ping(&eve_config);
    assert!(
        eve_result.is_ok(),
        "Eve should be able to authenticate after reload"
    );
}

/// Remove a user from htpasswd on disk, reload, verify they get 401.
#[test]
#[serial]
fn test_reload_auth_removed_user_rejected() {
    let server = common::start_server_with_required_auth();
    let admin_config = server.config_for_user("owner");
    let htpasswd_path = server.htpasswd_path();

    // Verify "carol" can authenticate currently
    // First ensure carol is in the htpasswd file (may have been removed by a previous test)
    let _ = Command::new(get_exe_path("./target/debug/torc-htpasswd"))
        .arg("add")
        .arg("--file")
        .arg(&htpasswd_path)
        .arg("--password")
        .arg("correct horse battery staple")
        .arg("carol")
        .status();
    let _ = default_api::reload_auth(&admin_config);

    let carol_config = server.config_for_user("carol");
    let carol_result = default_api::ping(&carol_config);
    assert!(
        carol_result.is_ok(),
        "Carol should be able to authenticate initially"
    );

    // Remove carol from the htpasswd file
    let status = Command::new(get_exe_path("./target/debug/torc-htpasswd"))
        .arg("remove")
        .arg("--file")
        .arg(&htpasswd_path)
        .arg("carol")
        .status()
        .expect("Failed to run torc-htpasswd");
    assert!(status.success(), "torc-htpasswd remove should succeed");

    // Admin reloads auth
    let reload_result = default_api::reload_auth(&admin_config);
    assert!(reload_result.is_ok(), "reload_auth should succeed");

    // Carol can no longer authenticate
    let carol_result = default_api::ping(&carol_config);
    assert!(
        carol_result.is_err(),
        "Carol should NOT be able to authenticate after being removed and reloaded"
    );
}

/// Credential cache is cleared after reload: changing a user's password and reloading
/// should invalidate the old cached credentials.
#[test]
#[serial]
fn test_reload_auth_clears_credential_cache() {
    let server = common::start_server_with_required_auth();
    let admin_config = server.config_for_user("owner");
    let htpasswd_path = server.htpasswd_path();

    // Ensure bob has original password (may have been changed by a previous test)
    let _ = Command::new(get_exe_path("./target/debug/torc-htpasswd"))
        .arg("add")
        .arg("--file")
        .arg(&htpasswd_path)
        .arg("--password")
        .arg("correct horse battery staple")
        .arg("bob")
        .status();
    let _ = default_api::reload_auth(&admin_config);

    // Bob authenticates successfully (caches credentials)
    let bob_config = server.config_for_user("bob");
    let bob_result = default_api::ping(&bob_config);
    assert!(bob_result.is_ok(), "Bob should authenticate successfully");

    // Change Bob's password in the htpasswd file
    let status = Command::new(get_exe_path("./target/debug/torc-htpasswd"))
        .arg("add")
        .arg("--file")
        .arg(&htpasswd_path)
        .arg("--password")
        .arg("new super secret password!!")
        .arg("bob")
        .status()
        .expect("Failed to run torc-htpasswd");
    assert!(
        status.success(),
        "torc-htpasswd add (update) should succeed"
    );

    // Admin reloads auth (clears cache)
    let reload_result = default_api::reload_auth(&admin_config);
    assert!(reload_result.is_ok(), "reload_auth should succeed");

    // Bob's old password should no longer work
    let bob_old_result = default_api::ping(&bob_config);
    assert!(
        bob_old_result.is_err(),
        "Bob's old password should NOT work after reload"
    );

    // Bob's new password should work
    let mut bob_new_config = Configuration::new();
    bob_new_config.base_path = server.config.base_path.clone();
    bob_new_config.basic_auth = Some((
        "bob".to_string(),
        Some("new super secret password!!".to_string()),
    ));
    let bob_new_result = default_api::ping(&bob_new_config);
    assert!(
        bob_new_result.is_ok(),
        "Bob's new password should work after reload"
    );
}

/// Server started without --auth-file returns error when reload-auth is called.
#[test]
#[serial]
fn test_reload_auth_no_auth_file() {
    // Use the standard server (no auth file configured)
    let server = common::start_server();
    let result = default_api::reload_auth(&server.config);
    // Without access control, any user can call admin endpoints but reload will fail
    // because there's no auth file configured
    assert!(result.is_err(), "reload_auth should fail without auth file");
}
