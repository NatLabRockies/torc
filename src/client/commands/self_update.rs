use std::error::Error;
use std::fmt;

use axoupdater::{AxoUpdater, AxoupdateError, UpdateRequest, Version};
use clap::{Args, Subcommand};

const APP_NAME: &str = "torc";
const RELEASE_URL_PREFIX: &str = "https://github.com/NatLabRockies/torc/releases/tag/";

#[derive(Subcommand, Debug, Clone)]
pub enum SelfCommands {
    /// Update torc when installed by the standalone installer
    #[command(after_long_help = "\
EXAMPLES:
    # Update to the latest available release
    torc self update

    # Update to a specific release tag
    torc self update v0.36.0

    # Use a GitHub token if unauthenticated API requests are rate-limited
    torc self update --token <TOKEN>
")]
    Update(SelfUpdateArgs),
}

#[derive(Args, Debug, Clone)]
pub struct SelfUpdateArgs {
    /// Release tag or version to install (defaults to the latest stable release)
    #[arg(value_name = "TARGET_VERSION")]
    pub target_version: Option<String>,

    /// GitHub token for release API requests; also read from GITHUB_TOKEN
    #[arg(long, env = "GITHUB_TOKEN", hide_env_values = true)]
    pub token: Option<String>,
}

#[derive(Debug)]
pub enum SelfUpdateError {
    StandaloneInstallerRequired { detail: String },
    GitHubRateLimited,
    Updater(Box<AxoupdateError>),
}

impl fmt::Display for SelfUpdateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SelfUpdateError::StandaloneInstallerRequired { detail } => {
                write!(
                    formatter,
                    "Self-update is only available for torc binaries installed by the standalone installer.\n\n{detail}\n\n{}",
                    unsupported_install_guidance()
                )
            }
            SelfUpdateError::GitHubRateLimited => write!(
                formatter,
                "GitHub API rate limit exceeded while checking torc releases. Provide a token with `--token` or GITHUB_TOKEN and try again."
            ),
            SelfUpdateError::Updater(error) => {
                write!(formatter, "torc self-update failed: {error}")
            }
        }
    }
}

impl Error for SelfUpdateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SelfUpdateError::Updater(error) => Some(error.as_ref()),
            SelfUpdateError::StandaloneInstallerRequired { .. }
            | SelfUpdateError::GitHubRateLimited => None,
        }
    }
}

impl From<AxoupdateError> for SelfUpdateError {
    fn from(error: AxoupdateError) -> Self {
        SelfUpdateError::Updater(Box::new(error))
    }
}

pub fn handle_self_commands(command: &SelfCommands) -> Result<(), SelfUpdateError> {
    match command {
        SelfCommands::Update(args) => update(args),
    }
}

fn update(args: &SelfUpdateArgs) -> Result<(), SelfUpdateError> {
    let mut updater = AxoUpdater::new_for(APP_NAME);
    updater.disable_installer_output();

    if let Some(token) = args.token.as_deref() {
        updater.set_github_token(token);
    }

    if let Err(error) = updater.load_receipt() {
        return Err(SelfUpdateError::StandaloneInstallerRequired {
            detail: format!("No matching standalone installer receipt was found: {error}"),
        });
    }

    if !updater.check_receipt_is_for_this_executable()? {
        return Err(SelfUpdateError::StandaloneInstallerRequired {
            detail: "A standalone installer receipt exists, but it does not belong to the current torc executable."
                .to_string(),
        });
    }

    eprintln!("Checking for torc updates...");
    updater.configure_version_specifier(update_request(args.target_version.as_deref()));

    match updater.run_sync() {
        Ok(Some(result)) => {
            if let Some(old_version) = result.old_version {
                eprintln!(
                    "Updated torc from v{old_version} to v{}.",
                    result.new_version
                );
            } else {
                eprintln!("Updated torc to v{}.", result.new_version);
            }
            eprintln!("{}{}", RELEASE_URL_PREFIX, result.new_version_tag);
            Ok(())
        }
        Ok(None) => {
            eprintln!(
                "torc is already up to date (v{}).",
                env!("CARGO_PKG_VERSION")
            );
            Ok(())
        }
        Err(error) if args.token.is_none() && is_github_rate_limit(&error) => {
            Err(SelfUpdateError::GitHubRateLimited)
        }
        Err(error) => Err(SelfUpdateError::Updater(Box::new(error))),
    }
}

fn update_request(target_version: Option<&str>) -> UpdateRequest {
    match target_version {
        Some(target) if Version::parse(target).is_ok() => {
            UpdateRequest::SpecificVersion(target.to_string())
        }
        Some(target) => UpdateRequest::SpecificTag(target.to_string()),
        None => UpdateRequest::Latest,
    }
}

fn is_github_rate_limit(error: &AxoupdateError) -> bool {
    match error {
        AxoupdateError::Reqwest(error) => error
            .status()
            .is_some_and(|status| matches!(status.as_u16(), 403 | 429)),
        _ => false,
    }
}

fn unsupported_install_guidance() -> &'static str {
    concat!(
        "Update this torc installation with the tool that installed it:\n",
        "  - cargo/crates.io: run `cargo install torc --force`\n",
        "  - package manager: use that package manager's upgrade command\n",
        "  - Docker: run `docker pull ghcr.io/natlabrockies/torc:<tag>` and recreate containers\n",
        "  - site-managed shared install: ask your site administrator to update torc\n",
        "  - manual release archive: download a new archive from GitHub Releases and replace the binaries",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn self_update_plain_semver_uses_specific_version() {
        assert!(matches!(
            update_request(Some("0.36.0")),
            UpdateRequest::SpecificVersion(version) if version == "0.36.0"
        ));
    }

    #[test]
    fn self_update_tag_uses_specific_tag() {
        assert!(matches!(
            update_request(Some("v0.36.0")),
            UpdateRequest::SpecificTag(tag) if tag == "v0.36.0"
        ));
    }

    #[test]
    fn self_update_without_target_uses_latest() {
        assert!(matches!(update_request(None), UpdateRequest::Latest));
    }

    #[test]
    #[serial]
    fn self_update_missing_receipt_reports_standalone_installer_requirement() {
        let tempdir = tempfile::tempdir().expect("create tempdir");
        let previous_config_path = std::env::var_os("AXOUPDATER_CONFIG_PATH");
        unsafe { std::env::set_var("AXOUPDATER_CONFIG_PATH", tempdir.path()) };

        let error = update(&SelfUpdateArgs {
            target_version: None,
            token: None,
        })
        .expect_err("missing receipt should fail before network access");

        match previous_config_path {
            Some(value) => unsafe { std::env::set_var("AXOUPDATER_CONFIG_PATH", value) },
            None => unsafe { std::env::remove_var("AXOUPDATER_CONFIG_PATH") },
        }

        let message = error.to_string();
        assert!(message.contains("standalone installer"));
        assert!(message.contains("cargo install torc --force"));
        assert!(message.contains("docker pull ghcr.io/natlabrockies/torc:<tag>"));
        assert!(message.contains("site administrator"));
        assert!(message.contains("manual release archive"));
    }
}
