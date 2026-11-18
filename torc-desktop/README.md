# Torc Desktop

Native desktop app for torc-dash on a remote login node. The user enters a login node; the app opens
an SSH session, starts `torc-dash` there on a private UNIX socket, forwards it to a local port, and
shows the dashboard in its window. No manual SSH tunnel.

Built with [Tauri 2](https://tauri.app) and Svelte 5. It is a standalone crate, not part of the main
torc workspace.

## Install

Download the installer for your platform from the
[GitHub releases](https://github.com/NatLabRockies/torc/releases) page:

- **macOS (Apple Silicon)**: `Torc Desktop_<version>_aarch64.dmg`. Open it and drag the app to
  Applications. The app is not signed with an Apple Developer ID, so macOS blocks it on first launch
  ("damaged" or "can't be opened"). Clear the download's quarantine flag once:

  ```bash
  xattr -dr com.apple.quarantine "/Applications/Torc Desktop.app"
  ```

- **Linux (x86_64)**: the `.AppImage` (`chmod +x` it and run it) or the `.deb`
  (`sudo apt install ./<file>.deb`). Requires WebKitGTK 4.1, which current Ubuntu, Debian, and
  Fedora releases provide.
- **Windows (x64)**: the `.msi` or the `-setup.exe` installer. SmartScreen may warn about an
  unrecognized app; choose **More info → Run anyway**.

The installers include `torc-dash`, `torc`, `torc-server`, and `torc-mcp-server`, so local mode
needs no separate install or `PATH` setup. Remote mode needs `torc-dash` installed on the login
node.

## Modes

- **This computer**: starts the bundled `torc-dash` locally on a private port. It runs through your
  login shell, so your profile's environment (such as `TORC_PASSWORD`) applies. It starts in the
  optional **Working directory** (`~/` and relative paths resolve against your home directory), or
  your home directory if blank.
- **Remote (SSH)**: described below.

In both modes, **Torc server** chooses what the dashboard talks to:

- **Use a shared torc-server**: enter its URL. The app remembers it. Leave it blank to use
  `TORC_API_URL` or `api_url` in the `[dash]` section of the torc config
  (`~/.config/torc/config.toml`, or `/etc/torc/config.toml` for a site-wide default) on the machine
  where torc-dash runs.
- **Start a private torc-server**: torc-dash starts a localhost-only server. Its database is
  `~/torc-desktop.db` on the login node, or in the app's data folder locally
  (`~/Library/Application Support/com.github.natlabrockies.torc-desktop/torc.db` on macOS).

If the shared server requires a login, set `TORC_PASSWORD` (and `TORC_USERNAME` if it differs from
`USER`) where torc-dash runs: in `~/.bash_profile` on the login node, or in your login-shell profile
locally (`~/.zprofile` for zsh; `~/.zshrc` is not read). torc-dash adds this login to the requests
it forwards to the server, and the torc commands it runs inherit it.

Disconnecting or quitting stops torc-dash and any server it started.

## How remote mode works

- Uses the system `ssh` (OpenSSH on macOS, Linux, and Windows 10+), so `~/.ssh/config`, `ProxyJump`,
  keys, and Kerberos work as usual.
- Password, MFA, and host-key prompts appear in the app. `SSH_ASKPASS` points at the app's own
  executable, which relays each prompt to the window over loopback, protected by a random token.
- The remote command runs under `bash -l`, so `module` and login-shell `PATH` setup work. An
  optional "Environment setup" field (e.g. `module load torc`) runs first, in the optional "Working
  directory" (default: home), where workflows run and write output.
- `torc-dash` listens on `/tmp/torc-desktop-<random>.sock` (mode 0600). It stops and the socket is
  removed when the SSH session ends, including when the app quits or the connection drops.

Design notes (why a UNIX socket, the remote watcher, the askpass relay, the stable port):
`docs/src/specialized/design/desktop.md`.

## Requirements

- Rust and Node.js (see <https://tauri.app/start/prerequisites/>)
- `torc-dash` installed on the login node
- OpenSSH 8.4+ on the client (for `SSH_ASKPASS_REQUIRE`)

## Development

CI type-checks the connect page and runs `cargo fmt`, clippy, and the tests for `src-tauri` on every
pull request (`.github/workflows/dashboard.yml`). Tagged releases build the installers above
(`.github/workflows/release.yml`). The app's version follows torc's: `cargo release` bumps
`src-tauri/Cargo.toml` and `package.json` (`tauri.conf.json` takes it from `Cargo.toml`).

```bash
cd torc-desktop
npm install
npm run tauri dev      # run with hot reload
npm run check          # svelte-check
npm run tauri build    # release bundle in src-tauri/target/release/bundle/
npm run tauri -- build --bundles dmg   # just one installer type
```

Without the bundled binaries, local mode runs `torc-dash` from `PATH`. To bundle them as the release
does, copy each of `torc-dash`, `torc`, `torc-server`, and `torc-mcp-server` (plus `.exe` on
Windows) to `src-tauri/binaries/<name>-<host triple>` (triple from `rustc -vV`), then:

```bash
npm run tauri -- build --config src-tauri/tauri.bundle.conf.json
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
