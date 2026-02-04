//! Dashboard configuration for torc-dash

use serde::{Deserialize, Serialize};

/// Configuration for the torc-dash web dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DashConfig {
    /// Host to bind to
    pub host: String,

    /// Port to listen on
    pub port: u16,

    /// URL of the torc-server API
    pub api_url: String,

    /// Path to the torc CLI binary
    pub torc_bin: String,

    /// Path to the torc-server binary
    pub torc_server_bin: String,

    /// Run in standalone mode (auto-start torc-server)
    pub standalone: bool,

    /// Port for auto-started server (0 = auto-detect)
    pub server_port: u16,

    /// Host for auto-started server to bind to
    pub server_host: String,

    /// Path to the database (for standalone mode)
    pub database: Option<String>,

    /// Interval in seconds for job completion checks (standalone mode)
    pub completion_check_interval_secs: u32,
}

impl Default for DashConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8090,
            api_url: "http://localhost:8080/torc-service/v1".to_string(),
            torc_bin: "torc".to_string(),
            torc_server_bin: "torc-server".to_string(),
            standalone: false,
            server_port: 0,
            server_host: "0.0.0.0".to_string(),
            database: None,
            completion_check_interval_secs: 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dash_config_defaults() {
        let config = DashConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8090);
        assert_eq!(
            config.api_url,
            "http://localhost:8080/torc-service/v1".to_string()
        );
        assert_eq!(config.torc_bin, "torc");
        assert_eq!(config.torc_server_bin, "torc-server");
        assert!(!config.standalone);
        assert_eq!(config.server_port, 0);
        assert_eq!(config.server_host, "0.0.0.0");
        assert!(config.database.is_none());
        assert_eq!(config.completion_check_interval_secs, 5);
    }
}
