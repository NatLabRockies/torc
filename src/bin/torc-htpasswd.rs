use bcrypt::hash;
use clap::{Parser, Subcommand};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

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
    },

    /// Generate a password hash and output to stdout (for sending to admin)
    Hash {
        /// Username (defaults to $USER or $USERNAME from environment)
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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Add {
            file,
            username,
            password,
            cost,
        } => {
            if !(4..=31).contains(&cost) {
                eprintln!("Error: cost must be between 4 and 31");
                std::process::exit(1);
            }

            let password = match password {
                Some(pwd) => pwd,
                None => {
                    match rpassword::prompt_password(format!("Password for '{}': ", username)) {
                        Ok(pwd) => pwd,
                        Err(e) => {
                            eprintln!("Error reading password: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            };

            println!("Hashing password (cost={})...", cost);
            let hash = match hash(&password, cost) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("Error hashing password: {}", e);
                    std::process::exit(1);
                }
            };

            // Read existing file or create new entries
            let mut entries = std::collections::HashMap::new();
            if file.exists() {
                let file_handle = match File::open(&file) {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("Error opening file: {}", e);
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
                    eprintln!("Error opening file for writing: {}", e);
                    std::process::exit(1);
                }
            };

            writeln!(file_handle, "# Torc htpasswd file").unwrap();
            writeln!(file_handle, "# Format: username:bcrypt_hash").unwrap();
            for (user, hash) in entries {
                writeln!(file_handle, "{}:{}", user, hash).unwrap();
            }

            if is_update {
                println!("Updated user '{}' in {:?}", username, file);
            } else {
                println!("Added user '{}' to {:?}", username, file);
            }
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
                None => std::env::var("USER")
                    .or_else(|_| std::env::var("USERNAME"))
                    .unwrap_or_else(|_| {
                        eprintln!(
                            "Error: username not provided and could not read from $USER or $USERNAME"
                        );
                        std::process::exit(1);
                    }),
            };

            let password = match password {
                Some(pwd) => pwd,
                None => {
                    match rpassword::prompt_password(format!("Password for '{}': ", username)) {
                        Ok(pwd) => pwd,
                        Err(e) => {
                            eprintln!("Error reading password: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            };

            eprintln!("Hashing password (cost={})...", cost);
            let hash_result = match hash(&password, cost) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("Error hashing password: {}", e);
                    std::process::exit(1);
                }
            };

            // Output the htpasswd line to stdout (progress messages go to stderr)
            println!("{}:{}", username, hash_result);
            eprintln!("Send the line above to your server administrator.");
        }

        Commands::Remove { file, username } => {
            if !file.exists() {
                eprintln!("Error: file {:?} does not exist", file);
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
                eprintln!("Error: user '{}' not found in {:?}", username, file);
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
                writeln!(file_handle, "{}:{}", user, hash).unwrap();
            }

            println!("Removed user '{}' from {:?}", username, file);
        }

        Commands::List { file } => {
            if !file.exists() {
                eprintln!("Error: file {:?} does not exist", file);
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
                println!("No users found in {:?}", file);
            } else {
                println!("Users in {:?}:", file);
                for user in users {
                    println!("  - {}", user);
                }
            }
        }

        Commands::Verify {
            file,
            username,
            password,
        } => {
            if !file.exists() {
                eprintln!("Error: file {:?} does not exist", file);
                std::process::exit(1);
            }

            let password = match password {
                Some(pwd) => pwd,
                None => {
                    match rpassword::prompt_password(format!("Password for '{}': ", username)) {
                        Ok(pwd) => pwd,
                        Err(e) => {
                            eprintln!("Error reading password: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            };

            // Load htpasswd file
            match torc::server::htpasswd::HtpasswdFile::load(&file) {
                Ok(htpasswd) => {
                    if htpasswd.verify(&username, &password) {
                        println!("✓ Password is correct for user '{}'", username);
                    } else {
                        println!("✗ Password is incorrect for user '{}'", username);
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Error loading htpasswd file: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
