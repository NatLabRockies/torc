use anyhow::{Context, Result};
use bcrypt::verify;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Represents an htpasswd file with user credentials
#[derive(Debug, Clone, Default)]
pub struct HtpasswdFile {
    /// Map of username to bcrypt password hash
    users: HashMap<String, String>,
}

impl HtpasswdFile {
    /// Load an htpasswd file from disk
    /// Format: username:bcrypt_hash (one entry per line)
    /// Lines starting with # are ignored as comments
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path.as_ref()).with_context(|| {
            format!("Failed to open htpasswd file: {}", path.as_ref().display())
        })?;
        let reader = BufReader::new(file);

        let mut users = HashMap::new();

        for (line_num, line) in reader.lines().enumerate() {
            let line = line.with_context(|| format!("Failed to read line {}", line_num + 1))?;
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse username:hash format
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() != 2 {
                return Err(anyhow::anyhow!(
                    "Invalid htpasswd format at line {}: expected 'username:hash'",
                    line_num + 1
                ));
            }

            let username = parts[0].trim().to_string();
            let hash = parts[1].trim().to_string();

            if username.is_empty() {
                return Err(anyhow::anyhow!("Empty username at line {}", line_num + 1));
            }

            // Validate that it looks like a bcrypt hash ($2b$, $2a$, or $2y$ followed by cost and hash)
            if !hash.starts_with("$2") {
                return Err(anyhow::anyhow!(
                    "Password hash for user '{}' doesn't appear to be a bcrypt hash (should start with $2a$, $2b$, or $2y$)",
                    username
                ));
            }

            users.insert(username, hash);
        }

        if users.is_empty() {
            return Err(anyhow::anyhow!("No valid users found in htpasswd file"));
        }

        Ok(HtpasswdFile { users })
    }

    /// Verify a username and password against the htpasswd file
    /// Returns true if the credentials are valid, false otherwise
    pub fn verify(&self, username: &str, password: &str) -> bool {
        if let Some(hash) = self.users.get(username) {
            // Use bcrypt to verify the password against the stored hash
            match verify(password, hash) {
                Ok(valid) => valid,
                Err(e) => {
                    log::warn!("Bcrypt verification error for user '{}': {}", username, e);
                    false
                }
            }
        } else {
            // User not found
            false
        }
    }

    /// Get the number of users in the file
    pub fn user_count(&self) -> usize {
        self.users.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_htpasswd_file() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, "# This is a comment")?;
        writeln!(
            file,
            "user1:$2b$10$abcdefghijklmnopqrstuvwxyz1234567890ABCDEFGHIJKLMNOP"
        )?;
        writeln!(
            file,
            "user2:$2a$10$abcdefghijklmnopqrstuvwxyz1234567890ABCDEFGHIJKLMNOP"
        )?;
        writeln!(file)?; // Empty line
        writeln!(
            file,
            "user3:$2y$10$abcdefghijklmnopqrstuvwxyz1234567890ABCDEFGHIJKLMNOP"
        )?;
        file.flush()?;

        let htpasswd = HtpasswdFile::load(file.path())?;

        assert_eq!(htpasswd.user_count(), 3);

        Ok(())
    }

    #[test]
    fn test_invalid_format() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "invalid_line_without_colon").unwrap();
        file.flush().unwrap();

        let result = HtpasswdFile::load(file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_hash_format() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "user1:not_a_bcrypt_hash").unwrap();
        file.flush().unwrap();

        let result = HtpasswdFile::load(file.path());
        assert!(result.is_err());
    }
}
