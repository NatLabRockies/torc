//! Configuration management for Torc
//!
//! This module provides layered configuration loading from multiple sources:
//! 1. Built-in defaults (lowest priority)
//! 2. System config (`/etc/torc/config.toml`)
//! 3. User config (`~/.config/torc/config.toml`)
//! 4. Project-local config (`./torc.toml`)
//! 5. Environment variables (`TORC_*`)
//! 6. CLI arguments (highest priority, handled by clap)
//!
//! # Example
//!
//! ```rust,ignore
//! use torc::client::config::TorcConfig;
//!
//! // Load configuration from all sources
//! let config = TorcConfig::load()?;
//!
//! // Access client settings
//! println!("API URL: {}", config.client.api_url);
//!
//! // Or load with custom paths
//! let config = TorcConfig::load_from_paths(&["/custom/config.toml"])?;
//! ```

mod client;
mod dash;
mod loader;
mod server;

pub use client::{
    ClientConfig, ClientHpcConfig, ClientRunConfig, ClientSlurmConfig, HpcPartitionConfig,
    HpcProfileConfig, HpcProfileOverride,
};
pub use dash::{DashChatConfig, DashConfig, user_chat_api_key};
pub use loader::{ConfigPaths, TorcConfig};
pub use server::{ServerConfig, ServerLoggingConfig};
