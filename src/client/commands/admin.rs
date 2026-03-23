use clap::Subcommand;

use crate::client::apis;
use crate::client::apis::configuration::Configuration;
use crate::client::commands::print_error;

#[derive(Subcommand)]
pub enum AdminCommands {
    /// Reload the htpasswd file from disk without restarting the server
    #[command(
        name = "reload-auth",
        after_long_help = "\
EXAMPLES:
    # Reload auth credentials after adding a user
    torc admin reload-auth

    # With JSON output
    torc -f json admin reload-auth
"
    )]
    ReloadAuth,
}

pub fn handle_admin_commands(config: &Configuration, command: &AdminCommands, format: &str) {
    match command {
        AdminCommands::ReloadAuth => match apis::access_control_api::reload_auth(config) {
            Ok(response) => {
                if format == "json" {
                    println!("{}", serde_json::to_string_pretty(&response).unwrap());
                } else {
                    println!("{} ({} users)", response.message, response.user_count);
                }
            }
            Err(e) => {
                print_error("reloading auth", &e);
                std::process::exit(1);
            }
        },
    }
}
