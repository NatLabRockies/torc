mod common;

use serial_test::serial;

// Regression test for missing admin gates on `/admin/api-stats` and
// `/admin/api-events/stream`. Both endpoints sit under `/admin/...` but were
// wired directly into the router instead of going through the transport layer
// that runs `authorize_admin!`, so any authenticated user could read them.
// `dave` is configured as a non-admin user in `start_server_with_required_auth`.

fn http_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("blocking client")
}

#[test]
#[serial(auth)]
fn admin_api_stats_forbids_non_admin() {
    let server = common::start_server_with_required_auth();
    let url = format!(
        "{}/admin/api-stats?window_seconds=60&interval_seconds=60",
        server.config.base_path
    );

    let resp = http_client()
        .get(&url)
        .basic_auth("dave", Some("correct horse battery staple"))
        .send()
        .expect("request sent");
    assert_eq!(
        resp.status().as_u16(),
        403,
        "non-admin should be forbidden from /admin/api-stats"
    );
}

#[test]
#[serial(auth)]
fn admin_api_stats_allows_admin() {
    let server = common::start_server_with_required_auth();
    let url = format!(
        "{}/admin/api-stats?window_seconds=60&interval_seconds=60",
        server.config.base_path
    );

    let resp = http_client()
        .get(&url)
        .basic_auth("owner", Some("correct horse battery staple"))
        .send()
        .expect("request sent");
    assert_eq!(
        resp.status().as_u16(),
        200,
        "admin should be allowed on /admin/api-stats"
    );
}

#[test]
#[serial(auth)]
fn admin_api_events_stream_forbids_non_admin() {
    let server = common::start_server_with_required_auth();
    let url = format!("{}/admin/api-events/stream", server.config.base_path);

    let resp = http_client()
        .get(&url)
        .basic_auth("dave", Some("correct horse battery staple"))
        .header("Accept", "text/event-stream")
        .send()
        .expect("request sent");
    assert_eq!(
        resp.status().as_u16(),
        403,
        "non-admin should be forbidden from /admin/api-events/stream"
    );
}
