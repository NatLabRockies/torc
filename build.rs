// Build script to capture git commit hash at compile time

use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    // Get git commit hash (short form)
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GIT_HASH={}", git_hash);

    // Check if working directory is dirty
    let is_dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|output| !output.stdout.is_empty())
        .unwrap_or(false);

    let dirty_suffix = if is_dirty { "-dirty" } else { "" };
    println!("cargo:rustc-env=GIT_DIRTY={}", dirty_suffix);

    // Rerun if git HEAD changes or if any tracked files change
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");

    // Ensure binaries embedding SQLx migrations rebuild whenever migrations change.
    emit_rerun_if_changed_for_dir(Path::new("torc-server/migrations"));
}

fn emit_rerun_if_changed_for_dir(dir: &Path) {
    println!("cargo:rerun-if-changed={}", dir.display());

    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            emit_rerun_if_changed_for_dir(&path);
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}
