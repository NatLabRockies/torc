# Torc Desktop

Native desktop app for torc-dash on a remote login node. The user enters a login node; the app opens
an SSH session, starts `torc-dash` there on a private UNIX socket, forwards it to a local port, and
shows the dashboard in its window. No manual SSH tunnel.

Built with [Tauri 2](https://tauri.app) and Svelte 5. It is a standalone crate, not part of the main
torc workspace.

## Modes

- **This computer**: starts `torc-dash` locally on a private port, pointed at a torc-server URL you
  give (default `http://localhost:8080/torc-service/v1`). Or tick "Start a new torc-server for me"
  to run a localhost-only server whose database lives in the app's data folder
  (`~/Library/Application Support/com.github.natlabrockies.torc-desktop/torc.db` on macOS).
  torc-dash runs through your login shell in your home directory, so your profile's `PATH` applies.
  Disconnecting or quitting stops torc-dash and any server it started.
- **Remote (SSH)**: described below.

## How remote mode works

- Uses the system `ssh` (OpenSSH on macOS, Linux, and Windows 10+), so `~/.ssh/config`, `ProxyJump`,
  keys, and Kerberos work as usual.
- Password, MFA, and host-key prompts appear in the app. `SSH_ASKPASS` points at the app's own
  executable, which relays each prompt to the window over loopback, protected by a random token.
- The remote command runs under `bash -l`, so `module` and login-shell `PATH` setup work. An
  optional "Environment setup" field (e.g. `module load torc`) runs first.
- `torc-dash` listens on `/tmp/torc-desktop-<random>.sock` (mode 0600). It stops and the socket is
  removed when the SSH session ends, including when the app quits or the connection drops.

## Requirements

- Rust and Node.js (see <https://tauri.app/start/prerequisites/>)
- `torc-dash` installed on the login node
- OpenSSH 8.4+ on the client (for `SSH_ASKPASS_REQUIRE`)

## Development

```bash
cd torc-desktop
npm install
npm run tauri dev      # run with hot reload
npm run check          # svelte-check
npm run tauri build    # release bundle in src-tauri/target/release/bundle/
```

## Connection menu

- **Reload** (Cmd/Ctrl+R) reloads the dashboard.
- **Disconnect** (Cmd/Ctrl+Shift+D) ends the SSH session and returns to the connect page.
- If the session drops (network change, sleep, remote exit), the app returns to the connect page
  with a one-click **Reconnect**. `ServerAliveInterval=30` makes ssh notice a dead link within about
  90 seconds.

## Known limitations

- Reconnecting re-runs authentication, so MFA users are prompted again.
- Downloads and `window.open` from the dashboard may not behave as in a browser.
