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
use std::sync::atomic::{AtomicU16, AtomicU64, Ordering};
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
    /// Have torc-dash start a private torc-server (--standalone).
    standalone: bool,
    /// Shared torc-server URL when not standalone. Empty: torc-dash uses TORC_API_URL or the
    /// torc config where it runs.
    server_url: String,
    /// Remote working directory (shell syntax such as `~/work` or `$SCRATCH` works). Empty: home.
    #[serde(default)]
    workdir: String,
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
                drop(ssh);
                kill_ssh(&state); // reap it and stop anything it left behind
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

    let local = args.mode == "local";
    let port = local_port(if local { "local" } else { args.host.trim() })?;
    // A local torc-dash listens on TCP, where it requires a token; over SSH it uses a private
    // UNIX socket and needs none. Passed by environment so other users cannot read it from ps.
    let token = local.then(random_hex);
    let (mut cmd, what) = if let Some(token) = &token {
        let mut cmd = local_command(app, args, port)?;
        cmd.env("TORC_DASH_TOKEN", token);
        (cmd, "torc-dash")
    } else {
        // ssh fails (ExitOnForwardFailure) rather than moving if this port gets taken.
        (ssh_command(&state, args, port)?, "ssh")
    };
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        // Own process group, so disconnecting also stops anything torc-dash started
        // (e.g. its managed torc-server) and the askpass helper.
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
    let pid = child.id();

    // Drain output (ssh errors + torc-dash logs) so the pipes never fill; keep the tail and
    // pick up the port torc-dash reports.
    let tail = Arc::new(Mutex::new(String::new()));
    let reported = Arc::new(AtomicU16::new(0));
    drain(child.stdout.take().unwrap(), &tail, &reported);
    drain(child.stderr.take().unwrap(), &tail, &reported);
    let tail_text = || tail.lock().unwrap().clone();

    // Register the child now so Disconnect or quitting the app can stop it mid-connect.
    *state.ssh.lock().unwrap() = Some(child);
    let deadline = Instant::now() + CONNECT_TIMEOUT;
    loop {
        let status = match state.ssh.lock().unwrap().as_mut() {
            Some(c) if c.id() == pid => c.try_wait(),
            _ => return Err("Connection cancelled".into()),
        };
        if let Ok(Some(status)) = status {
            std::thread::sleep(Duration::from_millis(200)); // let the readers catch up
            kill_ssh(&state); // reap it and stop anything it left behind
            return Err(format!("{what} exited ({status})\n{}", tail_text()));
        }
        // Locally, torc-dash reports the port it bound (the same one, barring a race).
        let port = if local {
            reported.load(Ordering::Relaxed)
        } else {
            port
        };
        if port != 0 && dashboard_ready(port) {
            if !local && let Err(e) = push_chat_settings(port) {
                // Not fatal: the AI Chat tab then shows its setup form.
                eprintln!("Could not send AI Chat settings to torc-dash: {e}");
            }
            let query = token
                .as_ref()
                .map_or(String::new(), |t| format!("?token={t}"));
            return Ok(format!("http://127.0.0.1:{port}/{query}"));
        }
        if Instant::now() > deadline {
            kill_ssh(&state);
            return Err(format!("Timed out waiting for torc-dash\n{}", tail_text()));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Local port for the dashboard. The same target (a host, or "local") gets the same port each
/// time it is free, so the dashboard keeps one origin and its saved settings (theme, editor draft,
/// chat setup) across reconnects. Falls back to any free port.
fn local_port(target: &str) -> Result<u16, String> {
    let preferred = stable_port(target);
    TcpListener::bind(("127.0.0.1", preferred))
        .or_else(|_| TcpListener::bind("127.0.0.1:0"))
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .map_err(|e| e.to_string())
}

/// A port in 20000..30000 from an FNV-1a hash of `target`: stable across runs and builds, and
/// outside the OS ephemeral ranges.
fn stable_port(target: &str) -> u16 {
    let hash = target.bytes().fold(0x811c_9dc5_u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    });
    20000 + (hash % 10000) as u16
}

/// Reads `stream` line by line on a thread, keeping the last few KB in `tail` and storing the
/// port from torc-dash's "Dashboard available at http://host:port" log line in `port`.
fn drain(stream: impl Read + Send + 'static, tail: &Arc<Mutex<String>>, port: &Arc<AtomicU16>) {
    let (tail, port) = (tail.clone(), port.clone());
    std::thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            if let Some(p) = reported_port(&line) {
                port.store(p, Ordering::Relaxed);
            }
            // ssh logs this for each readiness probe sent before the remote socket exists.
            if line.starts_with("channel ") && line.contains(": open failed: ") {
                continue;
            }
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
}

fn reported_port(line: &str) -> Option<u16> {
    let rest = line.split("Dashboard available at http://").nth(1)?;
    let digits: String = rest
        .rsplit_once(':')?
        .1
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok().filter(|&p| p != 0)
}

/// torc-dash on this computer, listening on 127.0.0.1:<port>. Runs through the user's login
/// shell so PATH from their profile applies (apps launched from Finder get a minimal PATH).
fn local_command(app: &AppHandle, args: &ConnectArgs, port: u16) -> Result<Command, String> {
    let flags = server_flags(args, quote_arg, || {
        // The private server's database lives in the app's data folder.
        let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(quote_arg(&dir.join("torc.db").to_string_lossy()))
    })?;
    let line = format!(
        "{} --host 127.0.0.1 --port {port}{flags}",
        args.command.trim()
    );
    #[cfg(unix)]
    {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        let mut cmd = Command::new(shell);
        // Common install locations that a GUI app's PATH lacks, in case the profile doesn't add
        // them. The script runs under /bin/sh because the login shell may be csh or fish.
        let script = format!(
            "PATH=\"$PATH:$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin\"; exec {line}"
        );
        cmd.arg("-lc")
            .arg(format!("exec /bin/sh -c {}", sh_quote(&script)));
        cmd.stdin(Stdio::null());
        set_work_dir(&mut cmd, &args.workdir)?;
        Ok(cmd)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = Command::new("cmd");
        // raw_arg: Rust's own quoting (\") is not what cmd.exe expects. /S strips the outer quotes.
        cmd.raw_arg(format!("/S /C \"{line}\""))
            .stdin(Stdio::null());
        set_work_dir(&mut cmd, &args.workdir)?;
        Ok(cmd)
    }
}

/// torc-dash flags selecting its torc-server: a private localhost-only one, a shared URL, or
/// (empty) whatever TORC_API_URL / the torc config says. `database` gives the quoted db path.
fn server_flags(
    args: &ConnectArgs,
    quote: fn(&str) -> String,
    database: impl FnOnce() -> Result<String, String>,
) -> Result<String, String> {
    let url = args.server_url.trim();
    Ok(if args.standalone {
        format!(
            " --standalone --server-host 127.0.0.1 --database {}",
            database()?
        )
    } else if url.is_empty() {
        String::new()
    } else if url.starts_with("http://") || url.starts_with("https://") {
        format!(" --api-url {}", quote(url))
    } else {
        return Err(format!(
            "torc-server URL must start with http:// or https://: {url}"
        ));
    })
}

/// Where local torc-dash runs: `workdir`, or the home directory when blank (apps launched from
/// Finder start in `/`, and torc writes relative paths such as `torc_output/`). `~/` and
/// relative paths resolve against home.
fn set_work_dir(cmd: &mut Command, workdir: &str) -> Result<(), String> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    if let Some(dir) = resolve_work_dir(home.map(std::path::PathBuf::from), workdir) {
        if !dir.is_dir() {
            return Err(format!("Working directory not found: {}", dir.display()));
        }
        cmd.current_dir(dir);
    }
    Ok(())
}

fn resolve_work_dir(home: Option<std::path::PathBuf>, workdir: &str) -> Option<std::path::PathBuf> {
    let w = workdir.trim();
    let rel = w
        .strip_prefix("~/")
        .unwrap_or(if w == "~" { "" } else { w });
    match home {
        // join() keeps an absolute `rel` as is.
        Some(home) if !rel.is_empty() => Some(home.join(rel)),
        Some(home) => Some(home),
        None => (!rel.is_empty()).then(|| rel.into()),
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
        // Hide login banners and notices so real errors are visible in the app.
        "-o",
        "LogLevel=ERROR",
    ])
    .arg("-L")
    .arg(format!("127.0.0.1:{port}:{socket}"))
    .arg("--")
    .arg(host)
    .arg(remote_command(
        &args.setup,
        &args.workdir,
        &format!(
            "{}{}",
            args.command.trim(),
            // $HOME expands in the remote bash.
            server_flags(args, sh_quote, || Ok("\"$HOME/torc-desktop.db\"".into()))?
        ),
        &socket,
    ))
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
fn remote_command(setup: &str, workdir: &str, command: &str, socket: &str) -> String {
    let setup = setup.trim().trim_end_matches(';').trim();
    let setup = if setup.is_empty() {
        String::new()
    } else {
        format!("{setup}; ")
    };
    // Left unquoted, like `setup`, so `~` and `$VARS` expand.
    let workdir = workdir.trim();
    let cd = if workdir.is_empty() {
        String::new()
    } else {
        format!("cd {workdir} || exit 1; ")
    };
    let script = format!(
        "S={socket}; trap 'rm -f \"$S\"' EXIT; exec 3<&0; {cd}{setup}{command} --socket \"$S\" & \
         pid=$(jobs -p %%); {{ cat <&3; kill $pid; }} >/dev/null 2>&1 & wait $pid",
        command = command.trim(),
    );
    format!("bash -lc {}", sh_quote(&script))
}

/// Quotes one argument for the local shell: POSIX `sh` on Unix, `cmd.exe` on Windows
/// (Windows paths cannot contain `"`).
fn quote_arg(s: &str) -> String {
    if cfg!(windows) {
        format!("\"{s}\"")
    } else {
        sh_quote(s)
    }
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

/// Sends the AI Chat settings in this computer's torc user config file to the remote torc-dash at
/// `port`, overriding any it read on the login node, so they need not be set there. The key goes
/// through the SSH tunnel into torc-dash's memory only: never to disk or a command line there.
fn push_chat_settings(port: u16) -> Result<(), String> {
    let Some(path) = dirs::config_dir().map(|d| d.join("torc").join("config.toml")) else {
        return Ok(());
    };
    let Some(body) = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| chat_settings(&t))
    else {
        return Ok(());
    };
    let body = body.to_string();
    let mut s = TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(10)));
    write!(
        s,
        "POST /api/chat/configure HTTP/1.0\r\nHost: localhost\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .map_err(|e| e.to_string())?;
    let mut response = String::new();
    let _ = s.read_to_string(&mut response);
    if response.split_whitespace().nth(1) == Some("200") {
        Ok(())
    } else {
        let status = response.lines().next().unwrap_or("no response");
        let detail = response.split("\r\n\r\n").nth(1).unwrap_or("");
        Err(format!("{status} {detail}"))
    }
}

/// The body of torc-dash's POST /api/chat/configure from a torc config file's `[dash.chat]`, or
/// None if it has no such section or no key (Ollama needs none).
fn chat_settings(config: &str) -> Option<serde_json::Value> {
    let table: toml::Table = config.parse().ok()?;
    let chat = table.get("dash")?.get("chat")?.as_table()?;
    let get = |k: &str| {
        chat.get(k)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    };
    let mut provider = get("provider").unwrap_or("anthropic").to_lowercase();
    let api_key = match get("api_key") {
        Some(key) => key,
        None if provider == "ollama" => "ollama",
        None => return None,
    };
    // The endpoint's "anthropic" setup ignores base_url and auth_header; "custom" uses them.
    if provider == "anthropic" && get("base_url").is_some() {
        provider = "custom".into();
    }
    let mut body = serde_json::json!({ "provider": provider, "api_key": api_key });
    for k in ["model", "base_url", "auth_header", "foundry_resource"] {
        if let Some(v) = get(k) {
            body[k] = v.into();
        }
    }
    Some(body)
}

fn kill_ssh(state: &AppState) {
    // Dropping the senders answers any open password prompt with "cancel", which unblocks its
    // handler thread and makes the askpass helper exit.
    state.pending_prompts.lock().unwrap().clear();
    let child = state.ssh.lock().unwrap().take();
    if let Some(mut child) = child {
        // Stop the whole process group/tree (torc-dash plus any server it started), then make
        // sure the direct child is gone.
        #[cfg(unix)]
        let _ = Command::new("kill")
            .args(["-TERM", &format!("-{}", child.id())])
            .stderr(Stdio::null()) // the group may already be gone
            .status();
        #[cfg(windows)]
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
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
    fn chat_settings_from_config() {
        let body = chat_settings(
            "[dash]\nport = 1\n[dash.chat]\nprovider = \"custom\"\nbase_url = \"https://gw\"\n\
             auth_header = \"Authorization\"\nmodel = \"m\"\napi_key = \"k\"\n",
        )
        .unwrap();
        assert_eq!(
            body,
            serde_json::json!({"provider": "custom", "api_key": "k", "model": "m",
                "base_url": "https://gw", "auth_header": "Authorization"})
        );
        // Anthropic with a base URL goes through the endpoint's "custom" setup.
        let body =
            chat_settings("[dash.chat]\nbase_url = \"https://gw\"\napi_key = \"k\"").unwrap();
        assert_eq!(body["provider"], "custom");
        // Nothing to send without a section or a key; Ollama needs no key.
        assert_eq!(chat_settings("[dash]\nport = 1\n"), None);
        assert_eq!(chat_settings("[dash.chat]\nprovider = \"openai\"\n"), None);
        assert_eq!(
            chat_settings("[dash.chat]\nprovider = \"ollama\"\n").unwrap()["api_key"],
            "ollama"
        );
    }

    #[test]
    fn remote_command_quotes_for_login_shell() {
        let cmd = remote_command(
            "module load torc;",
            "~/work",
            "torc-dash --standalone",
            "/tmp/x.sock",
        );
        assert!(cmd.starts_with("bash -lc '"));
        assert!(
            cmd.contains("cd ~/work || exit 1; module load torc; torc-dash --standalone --socket")
        );
        assert!(!cmd.contains('\n') && !cmd.contains('!'));
        // The embedded single quotes from `trap` must be escaped for the outer quoting.
        assert!(cmd.contains(r"trap '\''rm -f"));
    }

    #[test]
    fn work_dir_resolves_against_home() {
        let home = || Some(std::path::PathBuf::from("/h"));
        let r = |h, w| resolve_work_dir(h, w).map(|p| p.to_string_lossy().into_owned());
        assert_eq!(r(home(), " "), Some("/h".into()));
        assert_eq!(r(home(), "~"), Some("/h".into()));
        assert_eq!(r(home(), "~/work"), Some("/h/work".into()));
        assert_eq!(r(home(), "work"), Some("/h/work".into()));
        assert_eq!(r(home(), "/scratch/x"), Some("/scratch/x".into()));
        assert_eq!(r(None, ""), None);
        assert_eq!(r(None, "/scratch/x"), Some("/scratch/x".into()));
    }

    #[test]
    fn server_flags_by_choice() {
        let args = |standalone, url: &str| ConnectArgs {
            mode: "ssh".into(),
            host: String::new(),
            setup: String::new(),
            command: String::new(),
            standalone,
            server_url: url.into(),
            workdir: String::new(),
        };
        let db = || Ok("DB".to_string());
        assert_eq!(server_flags(&args(false, " "), sh_quote, db).unwrap(), "");
        assert_eq!(
            server_flags(&args(false, "https://h/torc-service/v1"), sh_quote, db).unwrap(),
            " --api-url 'https://h/torc-service/v1'"
        );
        assert!(server_flags(&args(false, "h; rm -rf ~"), sh_quote, db).is_err());
        assert_eq!(
            server_flags(&args(true, "ignored"), sh_quote, db).unwrap(),
            " --standalone --server-host 127.0.0.1 --database DB"
        );
    }

    #[test]
    fn stable_port_is_fixed_per_target() {
        assert_eq!(stable_port("user@kl3"), stable_port("user@kl3"));
        assert_ne!(stable_port("user@kl3"), stable_port("local"));
        assert!((20000..30000).contains(&stable_port("")));
    }

    #[test]
    fn reported_port_parses_dash_log_line() {
        let line = "\x1b[32m INFO\x1b[0m torc_dash: Dashboard available at http://127.0.0.1:54321";
        assert_eq!(reported_port(line), Some(54321));
        assert_eq!(
            reported_port("Dashboard available at unix:/tmp/x.sock"),
            None
        );
        assert_eq!(
            reported_port("Dashboard available at http://127.0.0.1:0"),
            None
        );
    }
}
