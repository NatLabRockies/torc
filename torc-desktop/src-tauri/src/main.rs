//! Torc Desktop: opens an SSH connection to an HPC login node, starts `torc-dash` there on a
//! private UNIX socket, forwards it to a local port, and shows the dashboard in a native window.
//!
//! Password/MFA prompts are routed to the GUI by pointing `SSH_ASKPASS` at this same executable;
//! when started with `TORC_DESKTOP_ASKPASS_ADDR` set it acts as the askpass helper and relays the
//! prompt to the running app over loopback.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{BuildHasher, RandomState};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, Url};

const ASKPASS_ADDR: &str = "TORC_DESKTOP_ASKPASS_ADDR";
const ASKPASS_TOKEN: &str = "TORC_DESKTOP_ASKPASS_TOKEN";
/// Generous because the user may be typing a password and an MFA code.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(180);

struct AppState {
    ssh: Mutex<Option<Child>>,
    /// The local connect page, to return to on disconnect.
    home_url: OnceLock<Url>,
    askpass_addr: String,
    askpass_token: String,
    next_prompt_id: AtomicU64,
    pending_prompts: Mutex<HashMap<u64, mpsc::Sender<Option<String>>>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectArgs {
    /// "ssh" (remote login node) or "local" (this computer).
    mode: String,
    host: String,
    setup: String,
    command: String,
    /// Local mode: torc-server to use, or None to have torc-dash start one (--standalone).
    server_url: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct AskpassRequest {
    token: String,
    prompt: String,
}

#[derive(Serialize, Deserialize)]
struct AskpassResponse {
    answer: Option<String>,
}

#[derive(Clone, Serialize)]
struct AskpassPrompt {
    id: u64,
    prompt: String,
}

fn main() {
    if let Ok(addr) = std::env::var(ASKPASS_ADDR) {
        std::process::exit(run_askpass_helper(&addr));
    }

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind askpass listener");
    let state = AppState {
        ssh: Mutex::new(None),
        home_url: OnceLock::new(),
        askpass_addr: listener.local_addr().unwrap().to_string(),
        askpass_token: random_hex(),
        next_prompt_id: AtomicU64::new(0),
        pending_prompts: Mutex::new(HashMap::new()),
    };

    tauri::Builder::default()
        .manage(state)
        .setup(move |app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                for stream in listener.incoming().flatten() {
                    let handle = handle.clone();
                    std::thread::spawn(move || {
                        let _ = handle_askpass(&handle, stream);
                    });
                }
            });

            if let Some(window) = app.get_webview_window("main") {
                let _ = app.state::<AppState>().home_url.set(window.url()?);
            }
            let menu = Menu::default(app.handle())?;
            menu.append(&Submenu::with_items(
                app,
                "Connection",
                true,
                &[
                    &MenuItem::with_id(app, "reload", "Reload", true, Some("CmdOrCtrl+R"))?,
                    &PredefinedMenuItem::separator(app)?,
                    &MenuItem::with_id(
                        app,
                        "disconnect",
                        "Disconnect",
                        true,
                        Some("CmdOrCtrl+Shift+D"),
                    )?,
                ],
            )?)?;
            app.set_menu(menu)?;
            app.on_menu_event(|app, event| match event.id().as_ref() {
                "disconnect" => {
                    kill_ssh(&app.state::<AppState>());
                    go_home(app, false);
                }
                "reload" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.eval("location.reload()");
                    }
                }
                _ => {}
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![connect, askpass_reply])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                kill_ssh(&app.state::<AppState>());
            }
        });
}

#[tauri::command]
async fn connect(app: AppHandle, args: ConnectArgs) -> Result<(), String> {
    let handle = app.clone();
    let url = tauri::async_runtime::spawn_blocking(move || start_tunnel(&handle, &args))
        .await
        .map_err(|e| e.to_string())??;
    let window = app
        .get_webview_window("main")
        .ok_or("main window not found")?;
    window
        .navigate(url.parse().map_err(|e| format!("bad url {url}: {e}"))?)
        .map_err(|e| e.to_string())?;
    watch_connection(app);
    Ok(())
}

/// Returns to the connect page when ssh exits on its own (network drop, sleep, remote exit).
/// ServerAliveInterval makes ssh notice a dead link within ~90 s.
fn watch_connection(app: AppHandle) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(2));
            let state = app.state::<AppState>();
            let mut ssh = state.ssh.lock().unwrap();
            let Some(child) = ssh.as_mut() else {
                return; // disconnected on purpose
            };
            if !matches!(child.try_wait(), Ok(None)) {
                ssh.take();
                drop(ssh);
                go_home(&app, true);
                return;
            }
        }
    });
}

/// Navigates back to the connect page; `#lost` tells it to show a reconnect prompt.
fn go_home(app: &AppHandle, lost: bool) {
    let (Some(window), Some(url)) = (
        app.get_webview_window("main"),
        app.state::<AppState>().home_url.get().cloned(),
    ) else {
        return;
    };
    let mut url = url;
    url.set_fragment(lost.then_some("lost"));
    let _ = window.navigate(url);
}

#[tauri::command]
fn askpass_reply(state: State<AppState>, id: u64, answer: Option<String>) {
    if let Some(tx) = state.pending_prompts.lock().unwrap().remove(&id) {
        let _ = tx.send(answer);
    }
}

/// Starts torc-dash (over SSH or locally) and waits until it answers on a local port.
/// Returns the dashboard URL.
fn start_tunnel(app: &AppHandle, args: &ConnectArgs) -> Result<String, String> {
    let state = app.state::<AppState>();
    kill_ssh(&state);

    let port = TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map_err(|e| e.to_string())?
        .port();
    let (mut cmd, what) = if args.mode == "local" {
        (local_command(app, args, port)?, "torc-dash")
    } else {
        (ssh_command(&state, args, port)?, "ssh")
    };
    cmd.stdout(Stdio::null()).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        // Own process group, so disconnecting also stops anything torc-dash started
        // (e.g. its managed torc-server).
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to run {what}: {e}"))?;

    // Drain stderr (ssh errors + remote torc-dash logs) so the pipe never fills; keep the tail.
    let stderr_tail = Arc::new(Mutex::new(String::new()));
    let stderr = child.stderr.take().unwrap();
    let tail = stderr_tail.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let mut t = tail.lock().unwrap();
            t.push_str(&line);
            t.push('\n');
            if t.len() > 8192 {
                let cut = t.len() - 4096;
                let cut = (cut..t.len()).find(|&i| t.is_char_boundary(i)).unwrap();
                t.drain(..cut);
            }
        }
    });

    let deadline = Instant::now() + CONNECT_TIMEOUT;
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            std::thread::sleep(Duration::from_millis(200)); // let the stderr reader catch up
            return Err(format!(
                "{what} exited ({status})\n{}",
                stderr_tail.lock().unwrap()
            ));
        }
        if dashboard_ready(port) {
            *state.ssh.lock().unwrap() = Some(child);
            return Ok(format!("http://127.0.0.1:{port}/"));
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            return Err(format!(
                "Timed out waiting for torc-dash\n{}",
                stderr_tail.lock().unwrap()
            ));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// torc-dash on this computer, listening on 127.0.0.1:<port>. Runs through the user's login
/// shell so PATH from their profile applies (apps launched from Finder get a minimal PATH).
fn local_command(app: &AppHandle, args: &ConnectArgs, port: u16) -> Result<Command, String> {
    let mut line = format!("{} --port {port}", args.command.trim());
    match args.server_url.as_deref().map(str::trim) {
        Some(url) if !url.is_empty() => line += &format!(" --api-url {}", sh_quote(url)),
        _ => {
            // Standalone: a localhost-only server with its database in the app's data folder.
            let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            let db = dir.join("torc.db");
            line += &format!(
                " --standalone --server-host 127.0.0.1 --database {}",
                sh_quote(&db.to_string_lossy())
            );
        }
    }
    #[cfg(unix)]
    {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        let mut cmd = Command::new(shell);
        // Common install locations that a GUI app's PATH lacks, in case the profile doesn't add them.
        cmd.arg("-lc").arg(format!(
            "PATH=\"$PATH:$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin\"; exec {line}"
        ));
        cmd.stdin(Stdio::null());
        set_home_dir(&mut cmd);
        Ok(cmd)
    }
    #[cfg(windows)]
    {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(line).stdin(Stdio::null());
        set_home_dir(&mut cmd);
        Ok(cmd)
    }
}

/// Run in the home directory: apps launched from Finder start in `/`, and torc writes
/// relative paths such as `torc_output/`.
fn set_home_dir(cmd: &mut Command) {
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        cmd.current_dir(home);
    }
}

/// `ssh -L 127.0.0.1:<port>:<remote socket> host <script>`: starts torc-dash on the login node
/// on a private UNIX socket and forwards it here.
fn ssh_command(state: &AppState, args: &ConnectArgs, port: u16) -> Result<Command, String> {
    let host = args.host.trim();
    if host.is_empty() || host.starts_with('-') {
        return Err("Enter a host such as user@login.example.org".into());
    }
    let socket = format!("/tmp/torc-desktop-{}.sock", random_hex());
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = Command::new("ssh");
    cmd.args([
        "-T",
        "-o",
        "ExitOnForwardFailure=yes",
        "-o",
        "ServerAliveInterval=30",
    ])
    .arg("-L")
    .arg(format!("127.0.0.1:{port}:{socket}"))
    .arg("--")
    .arg(host)
    .arg(remote_command(&args.setup, &args.command, &socket))
    .env("SSH_ASKPASS", exe)
    .env("SSH_ASKPASS_REQUIRE", "force")
    .env(ASKPASS_ADDR, &state.askpass_addr)
    .env(ASKPASS_TOKEN, &state.askpass_token)
    .stdin(Stdio::piped()); // held open; the remote side stops torc-dash when it closes
    Ok(cmd)
}

/// One-line script for the remote login shell. Kept free of newlines and `!` so it also survives
/// csh/tcsh login shells. torc-dash runs under `bash -l` so `module`/PATH setup is available; a
/// watcher stops it when our ssh stdin closes (app quit or dropped connection).
fn remote_command(setup: &str, command: &str, socket: &str) -> String {
    let setup = setup.trim().trim_end_matches(';').trim();
    let setup = if setup.is_empty() {
        String::new()
    } else {
        format!("{setup}; ")
    };
    let script = format!(
        "S={socket}; trap 'rm -f \"$S\"' EXIT; exec 3<&0; {setup}{command} --socket \"$S\" & \
         pid=$(jobs -p %%); {{ cat <&3; kill $pid; }} >/dev/null 2>&1 & wait $pid",
        command = command.trim(),
    );
    format!("bash -lc {}", sh_quote(&script))
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// The SSH forward accepts connections before the remote socket exists, so check for a real
/// HTTP 200 rather than just a TCP connect.
fn dashboard_ready(port: u16) -> bool {
    let Ok(mut s) = TcpStream::connect(("127.0.0.1", port)) else {
        return false;
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
    let mut status = [0u8; 12]; // "HTTP/1.1 200"
    s.write_all(b"GET / HTTP/1.0\r\nHost: localhost\r\n\r\n")
        .is_ok()
        && s.read_exact(&mut status).is_ok()
        && status.ends_with(b" 200")
}

fn kill_ssh(state: &AppState) {
    if let Some(mut child) = state.ssh.lock().unwrap().take() {
        // Ask the whole process group to stop (torc-dash plus any server it started), then make
        // sure the direct child is gone.
        #[cfg(unix)]
        let _ = Command::new("kill")
            .args(["-TERM", &format!("-{}", child.id())])
            .status();
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// App side of askpass: forward the prompt to the UI and wait for the user's answer.
fn handle_askpass(app: &AppHandle, mut stream: TcpStream) -> io::Result<()> {
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line)?;
    let req: AskpassRequest = serde_json::from_str(&line)?;
    let state = app.state::<AppState>();
    if req.token != state.askpass_token {
        return Ok(());
    }
    let id = state.next_prompt_id.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = mpsc::channel();
    state.pending_prompts.lock().unwrap().insert(id, tx);
    app.emit(
        "askpass",
        AskpassPrompt {
            id,
            prompt: req.prompt,
        },
    )
    .map_err(io::Error::other)?;
    let answer = rx.recv().ok().flatten();
    serde_json::to_writer(&mut stream, &AskpassResponse { answer })?;
    Ok(())
}

/// Helper side of askpass, invoked by ssh as `<exe> "<prompt>"`. Prints the answer on success;
/// a non-zero exit tells ssh the user cancelled.
fn run_askpass_helper(addr: &str) -> i32 {
    let request = AskpassRequest {
        token: std::env::var(ASKPASS_TOKEN).unwrap_or_default(),
        prompt: std::env::args().nth(1).unwrap_or_default(),
    };
    let result = (|| -> io::Result<AskpassResponse> {
        let mut s = TcpStream::connect(addr)?;
        writeln!(s, "{}", serde_json::to_string(&request)?)?;
        s.shutdown(Shutdown::Write)?;
        Ok(serde_json::from_reader(s)?)
    })();
    match result {
        Ok(AskpassResponse {
            answer: Some(answer),
        }) => {
            println!("{answer}");
            0
        }
        _ => 1,
    }
}

/// 128 bits from std's randomly keyed SipHash; enough for a socket name and a loopback token.
fn random_hex() -> String {
    let a = RandomState::new().hash_one(0u8);
    let b = RandomState::new().hash_one(1u8);
    format!("{a:016x}{b:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_command_quotes_for_login_shell() {
        let cmd = remote_command("module load torc;", "torc-dash --standalone", "/tmp/x.sock");
        assert!(cmd.starts_with("bash -lc '"));
        assert!(cmd.contains("module load torc; torc-dash --standalone --socket"));
        assert!(!cmd.contains('\n') && !cmd.contains('!'));
        // The embedded single quotes from `trap` must be escaped for the outer quoting.
        assert!(cmd.contains(r"trap '\''rm -f"));
    }
}
