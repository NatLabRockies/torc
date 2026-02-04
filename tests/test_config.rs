//! Tests for the configuration management module

use rstest::rstest;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use torc::config::{
    ClientConfig, ClientRunConfig, ConfigPaths, DashConfig, ServerConfig, ServerLoggingConfig,
    TorcConfig,
};

// ============== Default Value Tests ==============

#[rstest]
fn test_client_config_defaults() {
    let config = ClientConfig::default();
    assert_eq!(config.api_url, "http://localhost:8080/torc-service/v1");
    assert_eq!(config.format, "table");
    assert_eq!(config.log_level, "info");
}

#[rstest]
fn test_client_run_config_defaults() {
    let config = ClientRunConfig::default();
    assert_eq!(config.poll_interval, 5.0);
    assert_eq!(config.output_dir, PathBuf::from("output"));
    assert!(config.max_parallel_jobs.is_none());
    assert!(config.num_cpus.is_none());
    assert!(config.memory_gb.is_none());
    assert!(config.num_gpus.is_none());
}

#[rstest]
fn test_server_config_defaults() {
    let config = ServerConfig::default();
    assert_eq!(config.log_level, "info");
    assert!(!config.https);
    assert_eq!(config.host, "0.0.0.0");
    assert_eq!(config.port, 8080);
    assert_eq!(config.threads, 1);
    assert!(config.database.is_none());
    assert!(config.auth_file.is_none());
    assert!(!config.require_auth);
    assert_eq!(config.completion_check_interval_secs, 30.0);
}

#[rstest]
fn test_server_logging_config_defaults() {
    let config = ServerLoggingConfig::default();
    assert!(config.log_dir.is_none());
    assert!(!config.json_logs);
}

#[rstest]
fn test_dash_config_defaults() {
    let config = DashConfig::default();
    assert_eq!(config.host, "127.0.0.1");
    assert_eq!(config.port, 8090);
    assert_eq!(config.api_url, "http://localhost:8080/torc-service/v1");
    assert_eq!(config.torc_bin, "torc");
    assert_eq!(config.torc_server_bin, "torc-server");
    assert!(!config.standalone);
    assert_eq!(config.server_port, 0);
    assert!(config.database.is_none());
    assert_eq!(config.completion_check_interval_secs, 5);
}

#[rstest]
fn test_torc_config_defaults() {
    let config = TorcConfig::default();
    assert_eq!(
        config.client.api_url,
        "http://localhost:8080/torc-service/v1"
    );
    assert_eq!(config.server.port, 8080);
    assert_eq!(config.dash.port, 8090);
}

// ============== Config Paths Tests ==============

#[rstest]
fn test_config_paths_new() {
    let paths = ConfigPaths::new();
    assert_eq!(paths.system, PathBuf::from("/etc/torc/config.toml"));
    assert!(paths.user.is_some());
    assert_eq!(paths.local, PathBuf::from("torc.toml"));
}

#[rstest]
fn test_config_paths_existing_paths_empty() {
    let paths = ConfigPaths {
        system: PathBuf::from("/nonexistent/system/config.toml"),
        user: Some(PathBuf::from("/nonexistent/user/config.toml")),
        local: PathBuf::from("/nonexistent/local/torc.toml"),
    };
    let existing = paths.existing_paths();
    assert!(existing.is_empty());
}

#[rstest]
fn test_config_paths_user_config_dir() {
    let paths = ConfigPaths::new();
    if let Some(user_path) = &paths.user {
        let user_dir = paths.user_config_dir();
        assert!(user_dir.is_some());
        assert_eq!(user_dir.unwrap(), user_path.parent().unwrap());
    }
}

// ============== Config Loading Tests ==============

#[rstest]
fn test_load_returns_defaults_when_no_files() {
    // Load should return defaults when no config files exist
    // Use non-existent paths to avoid reading user's actual config
    let paths = ConfigPaths {
        system: PathBuf::from("/nonexistent/system/config.toml"),
        user: Some(PathBuf::from("/nonexistent/user/config.toml")),
        local: PathBuf::from("/nonexistent/local/torc.toml"),
    };
    let config = TorcConfig::load_with_paths(&paths).unwrap_or_default();
    assert_eq!(
        config.client.api_url,
        "http://localhost:8080/torc-service/v1"
    );
    assert_eq!(config.server.port, 8080);
}

#[rstest]
fn test_load_from_toml_file() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let toml_content = r#"
[client]
api_url = "http://custom-server:9999/api"
format = "json"
log_level = "debug"

[client.run]
poll_interval = 10.0
output_dir = "custom_output"

[server]
port = 9090
threads = 4
"#;

    fs::write(&config_path, toml_content).unwrap();

    let config = TorcConfig::load_from_files(&[config_path]).unwrap();
    assert_eq!(config.client.api_url, "http://custom-server:9999/api");
    assert_eq!(config.client.format, "json");
    assert_eq!(config.client.log_level, "debug");
    assert_eq!(config.client.run.poll_interval, 10.0);
    assert_eq!(config.client.run.output_dir, PathBuf::from("custom_output"));
    assert_eq!(config.server.port, 9090);
    assert_eq!(config.server.threads, 4);
}

#[rstest]
fn test_load_partial_config() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    // Only specify some values, others should be defaults
    let toml_content = r#"
[client]
api_url = "http://partial:8080/api"
"#;

    fs::write(&config_path, toml_content).unwrap();

    let config = TorcConfig::load_from_files(&[config_path]).unwrap();
    assert_eq!(config.client.api_url, "http://partial:8080/api");
    // Should have defaults for unspecified values
    assert_eq!(config.client.format, "table");
    assert_eq!(config.server.port, 8080);
}

#[rstest]
fn test_load_with_priority_order() {
    let temp_dir = TempDir::new().unwrap();

    // Create two config files
    let config1_path = temp_dir.path().join("config1.toml");
    let config2_path = temp_dir.path().join("config2.toml");

    let toml1 = r#"
[client]
api_url = "http://first:8080/api"
format = "table"
"#;

    let toml2 = r#"
[client]
api_url = "http://second:9090/api"
"#;

    fs::write(&config1_path, toml1).unwrap();
    fs::write(&config2_path, toml2).unwrap();

    // Second file should override first
    let config = TorcConfig::load_from_files(&[config1_path, config2_path]).unwrap();
    assert_eq!(config.client.api_url, "http://second:9090/api");
    // Format not in second file, should use first file's value
    assert_eq!(config.client.format, "table");
}

// ============== Validation Tests ==============

#[rstest]
fn test_validate_valid_config() {
    let config = TorcConfig::default();
    assert!(config.validate().is_ok());
}

#[rstest]
fn test_validate_invalid_format() {
    let mut config = TorcConfig::default();
    config.client.format = "invalid_format".to_string();
    let result = config.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("format")));
}

#[rstest]
fn test_validate_invalid_poll_interval() {
    let mut config = TorcConfig::default();
    config.client.run.poll_interval = -1.0;
    let result = config.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("poll_interval")));
}

#[rstest]
fn test_validate_invalid_server_port() {
    let mut config = TorcConfig::default();
    config.server.port = 0;
    let result = config.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("server.port")));
}

#[rstest]
fn test_validate_invalid_threads() {
    let mut config = TorcConfig::default();
    config.server.threads = 0;
    let result = config.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("threads")));
}

#[rstest]
fn test_validate_invalid_completion_interval() {
    let mut config = TorcConfig::default();
    config.server.completion_check_interval_secs = 0.0;
    let result = config.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.contains("completion_check_interval"))
    );
}

#[rstest]
fn test_validate_invalid_dash_port() {
    let mut config = TorcConfig::default();
    config.dash.port = 0;
    let result = config.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("dash.port")));
}

#[rstest]
fn test_validate_multiple_errors() {
    let mut config = TorcConfig::default();
    config.client.format = "invalid".to_string();
    config.server.port = 0;
    config.dash.port = 0;

    let result = config.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.len() >= 3);
}

// ============== Serialization Tests ==============

#[rstest]
fn test_generate_default_config() {
    let config_content = TorcConfig::generate_default_config();
    assert!(config_content.contains("[client]"));
    assert!(config_content.contains("[server]"));
    assert!(config_content.contains("[dash]"));
    assert!(config_content.contains("api_url"));
    assert!(config_content.contains("port"));
}

#[rstest]
fn test_to_toml_serialization() {
    let config = TorcConfig::default();
    let toml_str = config.to_toml().unwrap();

    assert!(toml_str.contains("[client]"));
    assert!(toml_str.contains("api_url"));
    assert!(toml_str.contains("[server]"));
    assert!(toml_str.contains("port = 8080"));
    assert!(toml_str.contains("[dash]"));
}

#[rstest]
fn test_roundtrip_serialization() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    // Create a custom config
    let mut original = TorcConfig::default();
    original.client.api_url = "http://test:1234/api".to_string();
    original.client.format = "json".to_string();
    original.server.port = 9999;
    original.dash.port = 7777;

    // Serialize to TOML
    let toml_str = original.to_toml().unwrap();
    fs::write(&config_path, toml_str).unwrap();

    // Load back
    let loaded = TorcConfig::load_from_files(&[config_path]).unwrap();

    assert_eq!(loaded.client.api_url, original.client.api_url);
    assert_eq!(loaded.client.format, original.client.format);
    assert_eq!(loaded.server.port, original.server.port);
    assert_eq!(loaded.dash.port, original.dash.port);
}

// ============== Config Paths with Temp Files Tests ==============

#[rstest]
fn test_existing_paths_with_actual_file() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    fs::write(&config_path, "[client]\napi_url = \"http://test\"").unwrap();

    let paths = ConfigPaths {
        system: PathBuf::from("/nonexistent"),
        user: Some(config_path.clone()),
        local: PathBuf::from("/nonexistent"),
    };

    let existing = paths.existing_paths();
    assert_eq!(existing.len(), 1);
    assert_eq!(existing[0], &config_path);
}

// ============== JSON Serialization Tests ==============

#[rstest]
fn test_json_serialization() {
    let config = TorcConfig::default();
    let json_str = serde_json::to_string_pretty(&config).unwrap();

    assert!(json_str.contains("\"api_url\""));
    assert!(json_str.contains("\"client\""));
    assert!(json_str.contains("\"server\""));
    assert!(json_str.contains("\"dash\""));
}

#[rstest]
fn test_json_deserialization() {
    let json_str = r#"{
        "client": {
            "api_url": "http://json-test:8080/api",
            "format": "json",
            "log_level": "debug",
            "run": {
                "poll_interval": 15.0,
                "output_dir": "json_output"
            }
        },
        "server": {
            "port": 7777,
            "threads": 8,
            "log_level": "info",
            "https": false,
            "url": "localhost",
            "require_auth": false,
            "completion_check_interval_secs": 30.0,
            "logging": {
                "json_logs": true
            }
        },
        "dash": {
            "host": "0.0.0.0",
            "port": 5555,
            "api_url": "http://localhost:7777/api",
            "torc_bin": "torc",
            "torc_server_bin": "torc-server",
            "standalone": true,
            "server_port": 0,
            "completion_check_interval_secs": 10
        }
    }"#;

    let config: TorcConfig = serde_json::from_str(json_str).unwrap();
    assert_eq!(config.client.api_url, "http://json-test:8080/api");
    assert_eq!(config.client.format, "json");
    assert_eq!(config.client.run.poll_interval, 15.0);
    assert_eq!(config.server.port, 7777);
    assert_eq!(config.server.threads, 8);
    assert!(config.server.logging.json_logs);
    assert_eq!(config.dash.port, 5555);
    assert!(config.dash.standalone);
}

// ============== Edge Case Tests ==============

#[rstest]
fn test_empty_toml_file() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("empty.toml");
    fs::write(&config_path, "").unwrap();

    let config = TorcConfig::load_from_files(&[config_path]).unwrap();
    // Should return defaults for empty file
    assert_eq!(
        config.client.api_url,
        "http://localhost:8080/torc-service/v1"
    );
}

#[rstest]
fn test_nonexistent_file() {
    let config = TorcConfig::load_from_files(&[PathBuf::from("/nonexistent/config.toml")]).unwrap();
    // Should return defaults for nonexistent file
    assert_eq!(
        config.client.api_url,
        "http://localhost:8080/torc-service/v1"
    );
}

#[rstest]
fn test_config_with_optional_fields() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let toml_content = r#"
[client.run]
max_parallel_jobs = 4
num_cpus = 8
memory_gb = 32.5
num_gpus = 2

[server]
database = "/var/lib/torc/test.db"
auth_file = "/etc/torc/htpasswd"

[server.logging]
log_dir = "/var/log/torc"
json_logs = true

[dash]
database = "/var/lib/torc/dash.db"
"#;

    fs::write(&config_path, toml_content).unwrap();

    let config = TorcConfig::load_from_files(&[config_path]).unwrap();
    assert_eq!(config.client.run.max_parallel_jobs, Some(4));
    assert_eq!(config.client.run.num_cpus, Some(8));
    assert_eq!(config.client.run.memory_gb, Some(32.5));
    assert_eq!(config.client.run.num_gpus, Some(2));
    assert_eq!(
        config.server.database,
        Some("/var/lib/torc/test.db".to_string())
    );
    assert_eq!(
        config.server.auth_file,
        Some("/etc/torc/htpasswd".to_string())
    );
    assert_eq!(
        config.server.logging.log_dir,
        Some(PathBuf::from("/var/log/torc"))
    );
    assert!(config.server.logging.json_logs);
    assert_eq!(
        config.dash.database,
        Some("/var/lib/torc/dash.db".to_string())
    );
}

#[rstest]
#[case("table", true)]
#[case("json", true)]
#[case("TABLE", false)]
#[case("JSON", false)]
#[case("xml", false)]
#[case("", false)]
fn test_format_validation(#[case] format: &str, #[case] expected_valid: bool) {
    let mut config = TorcConfig::default();
    config.client.format = format.to_string();
    let result = config.validate();

    if expected_valid {
        assert!(result.is_ok(), "Format '{}' should be valid", format);
    } else {
        assert!(result.is_err(), "Format '{}' should be invalid", format);
    }
}
