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

    // Rerun when the checked-out commit changes (new commit, branch switch).
    // NOTE: Do NOT watch .git/index — it is modified by nearly every git
    // operation (stage, stash, status) and causes constant rebuilds of all
    // test targets that depend on the env vars emitted above.
    println!("cargo:rerun-if-changed=.git/HEAD");
    if let Ok(head) = fs::read_to_string(".git/HEAD") {
        // HEAD usually contains "ref: refs/heads/<branch>"; watch that file
        // so we rebuild when the branch tip moves (i.e., a new commit).
        if let Some(refpath) = head.trim().strip_prefix("ref: ") {
            println!("cargo:rerun-if-changed=.git/{}", refpath);
        }
    }

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
