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

    /// Extra hostnames the dashboard may be reached by (beyond localhost, 127.0.0.1, ::1, and a
    /// specific `host`), e.g. a reverse proxy's name. `"*"` allows any.
    pub allowed_hosts: Vec<String>,

    /// UNIX domain socket path (alternative to TCP host:port)
    #[cfg(unix)]
    pub socket: Option<String>,

    /// Deprecated and ignored: torc-server processes job completions as they arrive.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_check_interval_secs: Option<u32>,

    /// AI Chat provider settings
    pub chat: DashChatConfig,
}

/// AI Chat provider settings (`[dash.chat]`). Command-line flags and their environment variables
/// take precedence. The API key is deliberately not a field here: it is read only from the user
/// config file, by [`user_chat_api_key`], so it never lands in a project `torc.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DashChatConfig {
    /// "anthropic", "openai", "foundry", "ollama", "github", or "custom" (an Anthropic-compatible
    /// gateway such as LiteLLM)
    pub provider: Option<String>,
    /// Model name
    pub model: Option<String>,
    /// API base URL (required for "custom")
    pub base_url: Option<String>,
    /// Azure AI Foundry resource name (required for "foundry")
    pub foundry_resource: Option<String>,
    /// Auth header name for Anthropic-compatible providers (default "x-api-key")
    pub auth_header: Option<String>,
}

/// `dash.chat.api_key` from the user config file, plus warnings for the caller to log: the key is
/// ignored in the system and project config files, and the user file should not be readable by
/// other users.
pub fn user_chat_api_key(paths: &super::ConfigPaths) -> (Option<String>, Vec<String>) {
    let mut warnings = Vec::new();
    let read = |path: &std::path::Path| -> Option<String> {
        let text = std::fs::read_to_string(path).ok()?;
        let table: toml::Table = text.parse().ok()?;
        let key = table.get("dash")?.get("chat")?.get("api_key")?.as_str()?;
        Some(key.to_string())
    };
    for path in [&paths.system, &paths.local] {
        if read(path).is_some() {
            warnings.push(format!(
                "Ignoring dash.chat.api_key in {}: put it in the user config file{} instead",
                path.display(),
                paths
                    .user
                    .as_ref()
                    .map(|p| format!(" ({})", p.display()))
                    .unwrap_or_default()
            ));
        }
    }
    let Some(path) = paths.user.as_ref() else {
        return (None, warnings);
    };
    let Some(key) = read(path) else {
        return (None, warnings);
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path)
            && meta.permissions().mode() & 0o077 != 0
        {
            warnings.push(format!(
                "{} holds an API key but other users can read it; run: chmod 600 '{}'",
                path.display(),
                path.display()
            ));
        }
    }
    (Some(key), warnings)
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
            allowed_hosts: Vec::new(),
            #[cfg(unix)]
            socket: None,
            completion_check_interval_secs: None,
            chat: DashChatConfig::default(),
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
        #[cfg(unix)]
        assert!(config.socket.is_none());
        assert_eq!(config.completion_check_interval_secs, None);
    }

    #[test]
    fn test_user_chat_api_key_only_from_user_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = |name: &str, body: &str| {
            let p = dir.path().join(name);
            std::fs::write(&p, body).unwrap();
            p
        };
        let body = "[dash.chat]\nprovider = \"openai\"\napi_key = \"sk-test\"\n";
        let paths = super::super::ConfigPaths {
            system: file("system.toml", body),
            user: Some(file("user.toml", body)),
            local: file("local.toml", body),
        };
        let (key, warnings) = user_chat_api_key(&paths);
        assert_eq!(key.as_deref(), Some("sk-test"));
        assert_eq!(
            warnings
                .iter()
                .filter(|w| w.starts_with("Ignoring"))
                .count(),
            2
        );
        let paths = super::super::ConfigPaths {
            user: Some(dir.path().join("missing.toml")),
            ..paths
        };
        assert_eq!(user_chat_api_key(&paths).0, None);

        // The key is not part of the merged config, so it can't leak via Debug or Serialize.
        let config: DashConfig =
            toml::from_str("[chat]\nprovider = \"openai\"\napi_key = \"x\"").unwrap();
        assert_eq!(config.chat.provider.as_deref(), Some("openai"));
        assert!(!format!("{config:?}").contains("api_key"));
    }
}
