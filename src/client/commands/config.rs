//! Configuration management commands

use super::output::print_json;
use crate::client::config::{ConfigPaths, TorcConfig};
use clap::Subcommand;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Config subcommands
#[derive(Subcommand, Debug, Clone)]
#[command(after_long_help = "\
EXAMPLES:
    # Show current configuration
    torc config show

    # Show as JSON
    torc config show --format json

    # Show config file locations
    torc config paths

    # Initialize user config
    torc config init --user
")]
pub enum ConfigCommands {
    /// Show the effective configuration (merged from all sources)
    #[command(after_long_help = "\
EXAMPLES:
    torc config show
    torc config show --format json
")]
    Show {
        /// Output format (toml or json)
        #[arg(short, long, default_value = "toml")]
        format: String,
    },
    /// Show configuration file paths
    #[command(after_long_help = "\
EXAMPLES:
    torc config paths
")]
    Paths,
    /// Initialize a configuration file with defaults
    #[command(after_long_help = "\
EXAMPLES:
    torc config init --user
    torc config init --local
    torc config init --user --force
")]
    Init {
        /// Create system-wide config (/etc/torc/config.toml)
        #[arg(long)]
        system: bool,

        /// Create user config (~/.config/torc/config.toml)
        #[arg(long)]
        user: bool,

        /// Create project-local config (./torc.toml)
        #[arg(long)]
        local: bool,

        /// Force overwrite if file exists
        #[arg(short, long)]
        force: bool,
    },
    /// Validate the current configuration
    Validate,
}

/// Handle config commands
pub fn handle_config_commands(command: &ConfigCommands) {
    match command {
        ConfigCommands::Show { format } => show_config(format),
        ConfigCommands::Paths => show_paths(),
        ConfigCommands::Init {
            system,
            user,
            local,
            force,
        } => init_config(*system, *user, *local, *force),
        ConfigCommands::Validate => validate_config(),
    }
}

/// Show the effective configuration
fn show_config(format: &str) {
    let config = match TorcConfig::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error loading configuration: {}", e);
            std::process::exit(1);
        }
    };

    match format {
        "toml" => match config.to_toml() {
            Ok(toml) => println!("{}", toml),
            Err(e) => {
                eprintln!("Error serializing config to TOML: {}", e);
                std::process::exit(1);
            }
        },
        "json" => print_json(&config, "config"),
        _ => {
            eprintln!("Unknown format '{}'. Use 'toml' or 'json'.", format);
            std::process::exit(1);
        }
    }
}

/// Show configuration file paths
fn show_paths() {
    let paths = ConfigPaths::new();

    println!("Configuration file paths (in priority order):");
    println!();

    // System config
    let system_status = if paths.system.exists() {
        "exists"
    } else {
        "not found"
    };
    println!("  System:  {} ({})", paths.system.display(), system_status);

    // User config
    if let Some(user_path) = &paths.user {
        let user_status = if user_path.exists() {
            "exists"
        } else {
            "not found"
        };
        println!("  User:    {} ({})", user_path.display(), user_status);
    } else {
        println!("  User:    (not available - could not determine config directory)");
    }

    // Local config
    let local_status = if paths.local.exists() {
        "exists"
    } else {
        "not found"
    };
    println!("  Local:   {} ({})", paths.local.display(), local_status);

    eprintln!();
    eprintln!("Environment variables (highest priority):");
    eprintln!("  Use double underscore (__) to separate nested keys:");
    eprintln!("    TORC_CLIENT__API_URL, TORC_CLIENT__FORMAT, TORC_SERVER__PORT, etc.");
    eprintln!();

    let existing = paths.existing_paths();
    if existing.is_empty() {
        println!("No configuration files found.");
        eprintln!("Run 'torc config init --user' to create one.");
    } else {
        println!(
            "Active configuration files: {}",
            existing
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}

/// Initialize a configuration file
fn init_config(system: bool, user: bool, local: bool, force: bool) {
    let paths = ConfigPaths::new();

    // Default to user if no option specified
    let target_path = if system {
        paths.system.clone()
    } else if local {
        paths.local.clone()
    } else if user {
        paths.user.clone().unwrap_or_else(|| {
            eprintln!("Error: Could not determine user config directory");
            std::process::exit(1);
        })
    } else {
        // Default to user config
        paths.user.clone().unwrap_or_else(|| {
            eprintln!("Error: Could not determine user config directory");
            eprintln!("Hint: Use --local to create a project-local config instead");
            std::process::exit(1);
        })
    };

    write_config_file(&target_path, force);
}

/// Write a config file to the specified path
fn write_config_file(path: &PathBuf, force: bool) {
    // Check if file exists
    if path.exists() && !force {
        eprintln!("Configuration file already exists: {}", path.display());
        eprintln!("Use --force to overwrite");
        std::process::exit(1);
    }

    // Create parent directories if needed
    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        match fs::create_dir_all(parent) {
            Ok(_) => eprintln!("Created directory: {}", parent.display()),
            Err(e) => {
                eprintln!("Error creating directory {}: {}", parent.display(), e);
                std::process::exit(1);
            }
        }
    }

    // Write the config file
    let content = TorcConfig::generate_default_config();
    match fs::File::create(path) {
        Ok(mut file) => match file.write_all(content.as_bytes()) {
            Ok(_) => {
                println!("Created configuration file: {}", path.display());
                eprintln!();
                eprintln!("Edit this file to customize your Torc settings.");
                eprintln!("Run 'torc config show' to see the effective configuration.");
            }
            Err(e) => {
                eprintln!("Error writing to {}: {}", path.display(), e);
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("Error creating {}: {}", path.display(), e);
            if path.starts_with("/etc") {
                eprintln!("Hint: Creating system config may require root/sudo");
            }
            std::process::exit(1);
        }
    }
}

/// Validate the current configuration
fn validate_config() {
    let paths = ConfigPaths::new();

    eprintln!("Validating configuration...");
    eprintln!();

    // Show which files are being loaded
    let existing = paths.existing_paths();
    if existing.is_empty() {
        eprintln!("No configuration files found (using defaults)");
    } else {
        eprintln!("Loading configuration from:");
        for path in &existing {
            eprintln!("  - {}", path.display());
        }
    }
    eprintln!();

    // Load and validate
    let config = match TorcConfig::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error loading configuration: {}", e);
            std::process::exit(1);
        }
    };

    match config.validate() {
        Ok(_) => {
            println!("Configuration is valid.");
            println!();

            // Show key values
            println!("Key settings:");
            println!("  client.api_url = {}", config.client.api_url);
            println!("  client.format = {}", config.client.format);
            println!("  server.port = {}", config.server.port);
            println!("  dash.port = {}", config.dash.port);
        }
        Err(errors) => {
            eprintln!("Configuration has {} error(s):", errors.len());
            for error in errors {
                eprintln!("  - {}", error);
            }
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_paths() {
        let paths = ConfigPaths::new();
        assert!(paths.system.to_string_lossy().contains("torc"));
    }
}
