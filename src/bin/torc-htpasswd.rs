use bcrypt::hash;
use clap::{Parser, Subcommand};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use torc::client::apis::configuration::{Configuration, TlsConfig};
use zxcvbn::Score;

/// Minimum zxcvbn score required for passwords (0-4 scale).
/// Score 3 = "safely unguessable: moderate protection from offline slow-hash scenario"
const MIN_PASSWORD_SCORE: Score = Score::Three;

/// Minimum password length (NIST SP 800-63B recommendation).
const MIN_PASSWORD_LENGTH: usize = 8;

#[derive(Parser)]
#[command(name = "torc-htpasswd")]
#[command(about = "Manage htpasswd files for Torc server authentication")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add or update a user in the htpasswd file
    Add {
        /// Path to htpasswd file (will be created if it doesn't exist)
        #[arg(short, long)]
        file: PathBuf,

        /// Username to add or update
        username: String,

        /// Password (will be prompted if not provided)
        #[arg(short, long)]
        password: Option<String>,

        /// Bcrypt cost factor (4-31, default: 12, higher = more secure but slower)
        #[arg(short, long, default_value_t = 12)]
        cost: u32,

        /// Reload auth credentials on the server after modifying the file.
        /// Requires --url and credentials (TORC_PASSWORD or --server-password).
        #[arg(long)]
        reload_auth: bool,

        /// Server URL for reload-auth (defaults to TORC_API_URL or http://localhost:8080/torc-service/v1)
        #[arg(long, env = "TORC_API_URL")]
        url: Option<String>,

        /// Password for authenticating with the server (for reload-auth)
        #[arg(long, env = "TORC_PASSWORD")]
        server_password: Option<String>,
    },

    /// Generate a password hash and output to stdout (for sending to admin)
    Hash {
        /// Username (defaults to $TORC_USERNAME, $USER, or $USERNAME from environment)
        username: Option<String>,

        /// Password (will be prompted if not provided)
        #[arg(short, long)]
        password: Option<String>,

        /// Bcrypt cost factor (4-31, default: 12, higher = more secure but slower)
        #[arg(short, long, default_value_t = 12)]
        cost: u32,
    },

    /// Remove a user from the htpasswd file
    Remove {
        /// Path to htpasswd file
        #[arg(short, long)]
        file: PathBuf,

        /// Username to remove
        username: String,

        /// Reload auth credentials on the server after modifying the file.
        /// Requires --url and credentials (TORC_PASSWORD or --server-password).
        #[arg(long)]
        reload_auth: bool,

        /// Server URL for reload-auth (defaults to TORC_API_URL or http://localhost:8080/torc-service/v1)
        #[arg(long, env = "TORC_API_URL")]
        url: Option<String>,

        /// Password for authenticating with the server (for reload-auth)
        #[arg(long, env = "TORC_PASSWORD")]
        server_password: Option<String>,
    },

    /// List all users in the htpasswd file
    List {
        /// Path to htpasswd file
        #[arg(short, long)]
        file: PathBuf,
    },

    /// Verify a password for a user
    Verify {
        /// Path to htpasswd file
        #[arg(short, long)]
        file: PathBuf,

        /// Username to verify
        username: String,

        /// Password to verify (will be prompted if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
}

/// Validate password strength using zxcvbn. Returns Ok(()) if the password is
/// strong enough, or Err with a user-facing error message.
fn validate_password(password: &str, username: &str) -> Result<(), String> {
    if password.len() < MIN_PASSWORD_LENGTH {
        return Err(format!(
            "Password is too short ({} characters). Minimum length is {}.",
            password.len(),
            MIN_PASSWORD_LENGTH,
        ));
    }

    let estimate = zxcvbn::zxcvbn(password, &[username, "torc"]);
    let score = estimate.score();

    if score >= MIN_PASSWORD_SCORE {
        return Ok(());
    }

    let mut msg = format!(
        "Password is too weak (score {}/4, minimum required: {}/4).",
        score as u8, MIN_PASSWORD_SCORE as u8,
    );

    if let Some(feedback) = estimate.feedback() {
        if let Some(warning) = feedback.warning() {
            msg.push_str(&format!("\n  Warning: {warning}"));
        }
        for suggestion in feedback.suggestions() {
            msg.push_str(&format!("\n  Suggestion: {suggestion}"));
        }
    }

    Err(msg)
}

fn prompt_password(username: &str) -> String {
    let password = match rpassword::prompt_password(format!("Password for '{username}': ")) {
        Ok(pwd) => pwd,
        Err(e) => {
            eprintln!("Error reading password: {e}");
            std::process::exit(1);
        }
    };

    let confirm = match rpassword::prompt_password("Confirm password: ") {
        Ok(pwd) => pwd,
        Err(e) => {
            eprintln!("Error reading password: {e}");
            std::process::exit(1);
        }
    };

    if password != confirm {
        eprintln!("Error: passwords do not match.");
        std::process::exit(1);
    }

    password
}

/// Call the server's reload-auth endpoint if --reload-auth was specified.
fn maybe_reload_auth(reload_auth: bool, url: &Option<String>, server_password: &Option<String>) {
    if !reload_auth {
        return;
    }

    let base_path = url
        .clone()
        .unwrap_or_else(|| "http://localhost:8080/torc-service/v1".to_string());

    let mut config = Configuration::with_tls(TlsConfig::default());
    config.base_path = base_path;

    // Set up auth using the current username and server_password
    if let Some(password) = server_password {
        let username = torc::get_username();
        config.basic_auth = Some((username, Some(password.clone())));
    }

    match torc::client::apis::default_api::reload_auth(&config) {
        Ok(response) => {
            let message = response
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Auth reloaded");
            let user_count = response
                .get("user_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            println!("Server: {} ({} users)", message, user_count);
        }
        Err(e) => {
            eprintln!("Warning: Failed to reload auth on server: {e}");
            eprintln!("The htpasswd file was modified but the server has not reloaded it.");
            eprintln!("Run 'torc admin reload-auth' manually to apply the changes.");
        }
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Add {
            file,
            username,
            password,
            cost,
            reload_auth,
            url,
            server_password,
        } => {
            if !(4..=31).contains(&cost) {
                eprintln!("Error: cost must be between 4 and 31");
                std::process::exit(1);
            }

            let password = password.unwrap_or_else(|| prompt_password(&username));

            if let Err(msg) = validate_password(&password, &username) {
                eprintln!("Error: {msg}");
                std::process::exit(1);
            }

            println!("Hashing password (cost={cost})...");
            let hash = match hash(&password, cost) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("Error hashing password: {e}");
                    std::process::exit(1);
                }
            };

            // Read existing file or create new entries
            let mut entries = std::collections::HashMap::new();
            if file.exists() {
                let file_handle = match File::open(&file) {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("Error opening file: {e}");
                        std::process::exit(1);
                    }
                };
                let reader = BufReader::new(file_handle);
                for line in reader.lines() {
                    let line = line.unwrap();
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    let parts: Vec<&str> = line.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        entries.insert(parts[0].to_string(), parts[1].to_string());
                    }
                }
            }

            // Add or update user
            let is_update = entries.contains_key(&username);
            entries.insert(username.clone(), hash);

            // Write back to file
            let mut file_handle = match OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&file)
            {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Error opening file for writing: {e}");
                    std::process::exit(1);
                }
            };

            writeln!(file_handle, "# Torc htpasswd file").unwrap();
            writeln!(file_handle, "# Format: username:bcrypt_hash").unwrap();
            for (user, hash) in entries {
                writeln!(file_handle, "{user}:{hash}").unwrap();
            }

            if is_update {
                println!("Updated user '{username}' in {file:?}");
            } else {
                println!("Added user '{username}' to {file:?}");
            }

            maybe_reload_auth(reload_auth, &url, &server_password);
        }

        Commands::Hash {
            username,
            password,
            cost,
        } => {
            if !(4..=31).contains(&cost) {
                eprintln!("Error: cost must be between 4 and 31");
                std::process::exit(1);
            }

            // Resolve username from argument or environment
            let username = match username {
                Some(u) => u,
                None => {
                    let u = torc::get_username();
                    if u == "unknown" {
                        eprintln!(
                            "Error: username not provided and could not read from $TORC_USERNAME, $USER, or $USERNAME"
                        );
                        std::process::exit(1);
                    }
                    u
                }
            };

            let password = password.unwrap_or_else(|| prompt_password(&username));

            if let Err(msg) = validate_password(&password, &username) {
                eprintln!("Error: {msg}");
                std::process::exit(1);
            }

            eprintln!("Hashing password (cost={cost})...");
            let hash_result = match hash(&password, cost) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("Error hashing password: {e}");
                    std::process::exit(1);
                }
            };

            // Output the htpasswd line to stdout (progress messages go to stderr)
            println!("{username}:{hash_result}");
            eprintln!("Send the line above to your server administrator.");
        }

        Commands::Remove {
            file,
            username,
            reload_auth,
            url,
            server_password,
        } => {
            if !file.exists() {
                eprintln!("Error: file {file:?} does not exist");
                std::process::exit(1);
            }

            let mut entries = std::collections::HashMap::new();
            let file_handle = File::open(&file).unwrap();
            let reader = BufReader::new(file_handle);
            for line in reader.lines() {
                let line = line.unwrap();
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    entries.insert(parts[0].to_string(), parts[1].to_string());
                }
            }

            if !entries.contains_key(&username) {
                eprintln!("Error: user '{username}' not found in {file:?}");
                std::process::exit(1);
            }

            entries.remove(&username);

            // Write back to file
            let mut file_handle = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&file)
                .unwrap();

            writeln!(file_handle, "# Torc htpasswd file").unwrap();
            writeln!(file_handle, "# Format: username:bcrypt_hash").unwrap();
            for (user, hash) in entries {
                writeln!(file_handle, "{user}:{hash}").unwrap();
            }

            println!("Removed user '{username}' from {file:?}");

            maybe_reload_auth(reload_auth, &url, &server_password);
        }

        Commands::List { file } => {
            if !file.exists() {
                eprintln!("Error: file {file:?} does not exist");
                std::process::exit(1);
            }

            let file_handle = File::open(&file).unwrap();
            let reader = BufReader::new(file_handle);
            let mut users = Vec::new();
            for line in reader.lines() {
                let line = line.unwrap();
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    users.push(parts[0].to_string());
                }
            }

            if users.is_empty() {
                println!("No users found in {file:?}");
            } else {
                println!("Users in {file:?}:");
                for user in users {
                    println!("  - {user}");
                }
            }
        }

        Commands::Verify {
            file,
            username,
            password,
        } => {
            if !file.exists() {
                eprintln!("Error: file {file:?} does not exist");
                std::process::exit(1);
            }

            let password = match password {
                Some(pwd) => pwd,
                None => match rpassword::prompt_password(format!("Password for '{username}': ")) {
                    Ok(pwd) => pwd,
                    Err(e) => {
                        eprintln!("Error reading password: {e}");
                        std::process::exit(1);
                    }
                },
            };

            // Load htpasswd file
            match torc::server::htpasswd::HtpasswdFile::load(&file) {
                Ok(htpasswd) => {
                    if htpasswd.verify(&username, &password) {
                        println!("Password is correct for user '{username}'");
                    } else {
                        println!("Password is incorrect for user '{username}'");
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Error loading htpasswd file: {e}");
                    std::process::exit(1);
                }
            }
        }
    }
}
