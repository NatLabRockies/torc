//! Cross-platform remote shell abstraction for remote worker execution.
//!
//! `torc remote` issues commands to worker hosts over SSH. The remote login
//! shell may be a POSIX shell with coreutils (Linux, macOS, *BSD) or a Windows
//! host where the default shell is `cmd.exe` or PowerShell. The original
//! implementation assumed bash unconditionally (`bash -c 'nohup ... & disown'`,
//! `pgrep`, `kill -0`, `tail`, `tar`, ...), so none of the remote commands
//! worked against Windows workers.
//!
//! This module detects which shell family a host belongs to and builds the
//! appropriate command string for each remote operation. Windows commands are
//! delivered as PowerShell `-EncodedCommand` payloads (base64 of a UTF-16LE
//! script). Encoding sidesteps the nested-quoting problem entirely: the SSH
//! argument that reaches the remote default shell (`cmd.exe` or PowerShell) is
//! plain ASCII with no quotes or shell metacharacters, so it survives whichever
//! shell interprets it before launching PowerShell.

use base64::Engine;
use log::debug;

use super::ssh::ssh_execute;
use super::types::WorkerEntry;

/// The remote shell family used to interpret commands on a worker host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteShell {
    /// POSIX shell with coreutils (Linux, macOS, *BSD).
    Posix,
    /// Windows host; commands are run via `powershell -EncodedCommand`.
    Windows,
}

/// Wrap a PowerShell script as a `powershell -EncodedCommand <base64>` invocation.
///
/// `-EncodedCommand` expects standard base64 of the UTF-16LE script bytes. Using
/// it means the command string sent over SSH contains no quotes or shell
/// metacharacters, so it is interpreted identically whether the remote default
/// shell is `cmd.exe` or PowerShell.
fn powershell_encoded(script: &str) -> String {
    let utf16_le: Vec<u8> = script
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(utf16_le);
    format!(
        "powershell -NoProfile -NonInteractive -EncodedCommand {}",
        encoded
    )
}

/// Detect the remote shell family for a worker by probing over SSH.
///
/// POSIX hosts respond to `uname`. If `uname` is unavailable but the host is
/// reachable, we confirm a usable PowerShell before classifying it as Windows.
/// This doubles as the connectivity check: a transport failure (SSH exit 255)
/// is reported as an unreachable host rather than an unknown shell.
pub fn detect_remote_shell(worker: &WorkerEntry) -> Result<RemoteShell, String> {
    let uname = ssh_execute(worker, "uname", Some(15))?;

    if uname.status.success() {
        let kernel = String::from_utf8_lossy(&uname.stdout);
        if !kernel.trim().is_empty() {
            debug!(
                "Detected POSIX shell on {} (uname={})",
                worker.display_name(),
                kernel.trim()
            );
            return Ok(RemoteShell::Posix);
        }
    }

    // SSH transport failures use exit code 255 -- distinguish "unreachable"
    // from "reachable but no uname" (i.e. a likely Windows host).
    if uname.status.code() == Some(255) {
        let stderr = String::from_utf8_lossy(&uname.stderr);
        return Err(format!(
            "SSH connection failed to {}: {}",
            worker.display_name(),
            stderr.trim()
        ));
    }

    // Reachable, but `uname` is not available. Confirm PowerShell works before
    // assuming a Windows host so we fail clearly on truly unsupported shells.
    let probe = powershell_encoded("Write-Output 'torc_powershell_ok'");
    let ps = ssh_execute(worker, &probe, Some(20))?;
    if ps.status.success() && String::from_utf8_lossy(&ps.stdout).contains("torc_powershell_ok") {
        debug!(
            "Detected Windows/PowerShell shell on {}",
            worker.display_name()
        );
        return Ok(RemoteShell::Windows);
    }

    Err(format!(
        "Could not determine the remote shell on {}: the host responded to neither \
         'uname' (POSIX) nor PowerShell. 'torc remote' supports POSIX shells and \
         Windows PowerShell.",
        worker.display_name()
    ))
}

impl RemoteShell {
    /// Command to create `dir` (including parents), succeeding if it exists.
    pub fn mkdir_p(&self, dir: &str) -> String {
        match self {
            RemoteShell::Posix => format!("mkdir -p {}", dir),
            RemoteShell::Windows => powershell_encoded(&format!(
                "New-Item -ItemType Directory -Force -Path '{}' | Out-Null",
                dir
            )),
        }
    }

    /// Command to launch a detached `torc` worker, redirecting output to
    /// `log_file` and writing the worker PID to `pid_file`.
    ///
    /// `program` is the worker executable (`torc`) and `args` its arguments.
    /// The worker's `env_logger` output (including the "Starting torc job
    /// runner" startup line) goes to stderr, so both shells route stderr to
    /// `log_file`, which the liveness/tail checks inspect.
    pub fn start_detached(
        &self,
        program: &str,
        args: &[String],
        log_file: &str,
        pid_file: &str,
    ) -> String {
        match self {
            RemoteShell::Posix => {
                let cmd = std::iter::once(program.to_string())
                    .chain(args.iter().cloned())
                    .collect::<Vec<_>>()
                    .join(" ");
                // nohup + background + disown so the worker outlives the SSH session.
                format!(
                    "bash -c 'nohup {} > {} 2>&1 & echo $! > {}; disown'",
                    cmd, log_file, pid_file
                )
            }
            RemoteShell::Windows => {
                // Start-Process cannot redirect stdout and stderr to the same
                // file, so stderr (where the torc log lives) goes to `log_file`
                // and stdout to a sibling `.out` file.
                let arg_list = args
                    .iter()
                    .map(|a| format!("'{}'", a))
                    .collect::<Vec<_>>()
                    .join(",");
                let out_file = format!("{}.out", log_file);
                powershell_encoded(&format!(
                    "$p = Start-Process -FilePath '{program}' -ArgumentList {arg_list} \
                     -RedirectStandardError '{log}' -RedirectStandardOutput '{out}' \
                     -WindowStyle Hidden -PassThru; \
                     $p.Id | Out-File -Encoding ascii -FilePath '{pid}'",
                    program = program,
                    arg_list = arg_list,
                    log = log_file,
                    out = out_file,
                    pid = pid_file,
                ))
            }
        }
    }

    /// Command to print the contents of `path`. Fails (non-zero exit) if the
    /// file does not exist, so callers can treat that as "no PID file".
    pub fn read_file(&self, path: &str) -> String {
        match self {
            RemoteShell::Posix => format!("cat {}", path),
            RemoteShell::Windows => powershell_encoded(&format!("Get-Content '{}'", path)),
        }
    }

    /// Command that prints `running` if the given PID is alive, else `stopped`.
    pub fn is_process_alive(&self, pid: u32) -> String {
        match self {
            RemoteShell::Posix => {
                format!(
                    "kill -0 {} 2>/dev/null && echo running || echo stopped",
                    pid
                )
            }
            RemoteShell::Windows => powershell_encoded(&format!(
                "if (Get-Process -Id {} -ErrorAction SilentlyContinue) {{ 'running' }} \
                 else {{ 'stopped' }}",
                pid
            )),
        }
    }

    /// Command that prints `started` if `log_file` contains the worker startup
    /// line, else `waiting`. A missing log file prints `waiting`.
    pub fn log_shows_startup(&self, log_file: &str) -> String {
        match self {
            RemoteShell::Posix => format!(
                "grep -q 'Starting torc job runner' {} 2>/dev/null && echo started || echo waiting",
                log_file
            ),
            RemoteShell::Windows => powershell_encoded(&format!(
                "if ((Test-Path '{log}') -and \
                 (Select-String -Quiet -Pattern 'Starting torc job runner' -Path '{log}')) \
                 {{ 'started' }} else {{ 'waiting' }}",
                log = log_file
            )),
        }
    }

    /// Command that prints `running` if a `torc ... run <workflow_id>` process
    /// exists, else `stopped`. Used to confirm startup after the log line.
    pub fn torc_process_running(&self, workflow_id: i64) -> String {
        match self {
            RemoteShell::Posix => format!(
                "pgrep -f 'torc .* run {}( |$)' >/dev/null 2>&1 && echo running || echo stopped",
                workflow_id
            ),
            RemoteShell::Windows => powershell_encoded(&Self::win_find_torc_script(
                workflow_id,
                "if ($m) { 'running' } else { 'stopped' }",
            )),
        }
    }

    /// Command that prints the PID of a `torc ... run <workflow_id>` process if
    /// one exists, else nothing. Used as a fallback when the PID file is absent.
    pub fn torc_process_pid(&self, workflow_id: i64) -> String {
        match self {
            RemoteShell::Posix => {
                format!(
                    "pgrep -f 'torc .* run {}( |$)' 2>/dev/null | head -1",
                    workflow_id
                )
            }
            RemoteShell::Windows => powershell_encoded(&Self::win_find_torc_script(
                workflow_id,
                "if ($m) { @($m)[0].ProcessId }",
            )),
        }
    }

    /// Build the shared PowerShell prologue that locates torc worker processes
    /// for `workflow_id`, binding the matches to `$m`, then appends `tail`.
    fn win_find_torc_script(workflow_id: i64, tail: &str) -> String {
        format!(
            "$m = Get-CimInstance Win32_Process | Where-Object {{ \
             $_.Name -eq 'torc.exe' -and $_.CommandLine -match ' run {}( |$)' }}; {}",
            workflow_id, tail
        )
    }

    /// Command to send a stop signal to `pid`, printing `killed` on success or
    /// `not_found` if the process is already gone.
    ///
    /// `graceful` requests a SIGTERM-style stop on POSIX. Windows has no
    /// SIGTERM equivalent for a detached process, so it is always a hard stop;
    /// callers should surface that limitation to the user.
    pub fn kill_process(&self, pid: u32, graceful: bool) -> String {
        match self {
            RemoteShell::Posix => {
                let signal = if graceful { "TERM" } else { "KILL" };
                format!(
                    "kill -{} {} 2>/dev/null && echo killed || echo not_found",
                    signal, pid
                )
            }
            RemoteShell::Windows => powershell_encoded(&format!(
                "if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) \
                 {{ Stop-Process -Id {pid} -Force; 'killed' }} else {{ 'not_found' }}",
                pid = pid
            )),
        }
    }

    /// Command that prints `exists` if `dir` is a directory, else `missing`.
    pub fn dir_exists(&self, dir: &str) -> String {
        match self {
            RemoteShell::Posix => format!("test -d {} && echo exists || echo missing", dir),
            RemoteShell::Windows => powershell_encoded(&format!(
                "if (Test-Path -PathType Container '{}') {{ 'exists' }} else {{ 'missing' }}",
                dir
            )),
        }
    }

    /// Command to print the last `lines` lines of `file`, or a placeholder if
    /// the file is missing.
    pub fn tail(&self, file: &str, lines: usize) -> String {
        match self {
            RemoteShell::Posix => {
                format!(
                    "tail -{} {} 2>/dev/null || echo 'No log available'",
                    lines, file
                )
            }
            RemoteShell::Windows => powershell_encoded(&format!(
                "if (Test-Path '{file}') {{ Get-Content -Tail {lines} '{file}' }} \
                 else {{ 'No log available' }}",
                file = file,
                lines = lines
            )),
        }
    }

    /// Remote path for the temporary logs tarball named `name`.
    ///
    /// POSIX uses `/tmp`; Windows uses a home-relative path (resolved by the SSH
    /// session's working directory) so the path is concrete for `scp` without
    /// needing environment-variable expansion.
    pub fn temp_tarball_path(&self, name: &str) -> String {
        match self {
            RemoteShell::Posix => format!("/tmp/{}", name),
            RemoteShell::Windows => name.to_string(),
        }
    }

    /// Command to create a gzip tarball `tarball` from the contents of `dir`.
    ///
    /// Both shells use `tar`, which ships with modern Windows (bsdtar, Win10
    /// 1803+) and accepts the same `-czf -C` flags.
    pub fn create_tarball(&self, tarball: &str, dir: &str) -> String {
        match self {
            RemoteShell::Posix => format!("tar -czf {} -C {} . 2>/dev/null", tarball, dir),
            RemoteShell::Windows => format!("tar -czf {} -C {} .", tarball, dir),
        }
    }

    /// Command to remove the file at `path` (no error if absent).
    pub fn remove_file(&self, path: &str) -> String {
        match self {
            RemoteShell::Posix => format!("rm -f {}", path),
            RemoteShell::Windows => powershell_encoded(&format!(
                "Remove-Item -Force -ErrorAction SilentlyContinue '{}'",
                path
            )),
        }
    }

    /// Command to recursively remove the directory at `path` (no error if absent).
    pub fn remove_dir(&self, path: &str) -> String {
        match self {
            RemoteShell::Posix => format!("rm -rf {}", path),
            RemoteShell::Windows => powershell_encoded(&format!(
                "Remove-Item -Recurse -Force -ErrorAction SilentlyContinue '{}'",
                path
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decode a `powershell -EncodedCommand` invocation back to its script.
    fn decode_powershell(command: &str) -> String {
        let b64 = command
            .strip_prefix("powershell -NoProfile -NonInteractive -EncodedCommand ")
            .expect("expected an encoded PowerShell command");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .expect("valid base64");
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16(&units).expect("valid UTF-16")
    }

    #[test]
    fn posix_commands_are_bash() {
        let sh = RemoteShell::Posix;
        assert_eq!(sh.mkdir_p("out"), "mkdir -p out");
        assert!(
            sh.start_detached("torc", &["run".into()], "log", "pid")
                .contains("nohup torc run")
        );
        assert_eq!(sh.read_file("p"), "cat p");
        assert!(sh.is_process_alive(42).contains("kill -0 42"));
        assert!(sh.kill_process(42, true).contains("kill -TERM 42"));
        assert!(sh.kill_process(42, false).contains("kill -KILL 42"));
        assert_eq!(sh.temp_tarball_path("a.tgz"), "/tmp/a.tgz");
        assert_eq!(sh.remove_dir("d"), "rm -rf d");
    }

    #[test]
    fn windows_commands_are_encoded_powershell() {
        let sh = RemoteShell::Windows;

        let mkdir = sh.mkdir_p("out");
        assert!(mkdir.starts_with("powershell -NoProfile -NonInteractive -EncodedCommand "));
        assert!(
            decode_powershell(&mkdir).contains("New-Item -ItemType Directory -Force -Path 'out'")
        );

        let start = sh.start_detached(
            "torc",
            &["--url".into(), "u".into(), "run".into()],
            "log",
            "pid",
        );
        let start_script = decode_powershell(&start);
        assert!(start_script.contains("Start-Process -FilePath 'torc'"));
        assert!(start_script.contains("'--url','u','run'"));
        // stderr (where the torc log line goes) must land in the greppable log file.
        assert!(start_script.contains("-RedirectStandardError 'log'"));
        assert!(start_script.contains("-RedirectStandardOutput 'log.out'"));

        assert!(decode_powershell(&sh.is_process_alive(7)).contains("Get-Process -Id 7"));
        assert!(decode_powershell(&sh.kill_process(7, true)).contains("Stop-Process -Id 7 -Force"));
        assert!(decode_powershell(&sh.torc_process_pid(9)).contains("' run 9( |$)'"));
        assert_eq!(sh.temp_tarball_path("a.tgz"), "a.tgz");
    }

    #[test]
    fn windows_tarball_uses_tar() {
        let sh = RemoteShell::Windows;
        assert_eq!(sh.create_tarball("a.tgz", "out"), "tar -czf a.tgz -C out .");
    }
}
