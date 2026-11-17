# Torc Desktop

Native desktop app for torc-dash on a remote login node. The user enters a login node; the app opens
an SSH session, starts `torc-dash` there on a private UNIX socket, forwards it to a local port, and
shows the dashboard in its window. No manual SSH tunnel.

Built with [Tauri 2](https://tauri.app) and Svelte 5. It is a standalone crate, not part of the main
torc workspace.

## Modes

- **This computer**: starts `torc-dash` locally on a private port. torc-dash runs through your login
  shell in your home directory, so your profile's `PATH` applies.
- **Remote (SSH)**: described below.

In both modes, **Torc server** chooses what the dashboard talks to:

- **Use a shared torc-server**: enter its URL. The app remembers it. Leave it blank to use
  `TORC_API_URL` or `api_url` in the `[dash]` section of the torc config
  (`~/.config/torc/config.toml`, or `/etc/torc/config.toml` for a site-wide default) on the machine
  where torc-dash runs.
- **Start a private torc-server**: torc-dash starts a localhost-only server. Its database is
  `~/torc-desktop.db` on the login node, or in the app's data folder locally
  (`~/Library/Application Support/com.github.natlabrockies.torc-desktop/torc.db` on macOS).

If the shared server requires a login, enter your password under the URL. The app saves it in the
system keychain (Keychain on macOS), never in its settings, and gives it to torc-dash as
`TORC_PASSWORD`: through the environment locally, or as the first line on the SSH session's stdin
remotely, so it never appears on a command line. The user name is `TORC_USERNAME` or `USER` where
torc-dash runs, as for the torc CLI. torc-dash adds this login to the requests it forwards to the
server, and the torc commands it runs inherit it.

Disconnecting or quitting stops torc-dash and any server it started.

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
