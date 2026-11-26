# Torc Desktop

Native desktop app for torc-dash, on this computer or on a remote login node. In remote mode the app
opens an SSH session, starts `torc-dash` there on a private UNIX socket, forwards it to a local
port, and shows the dashboard in its window. No manual SSH tunnel.

Built with [Tauri 2](https://tauri.app); the connect page is a static HTML form (`ui/index.html`),
so there is no JavaScript build. It is a standalone crate, not part of the main torc workspace.

## Usage

Installing and using the app: [Desktop App](../docs/src/core/monitoring/desktop.md) in the user
docs.

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

## Connect page and dashboard extras

- Saved connections (named copies of the form) live in the connect page's `localStorage`
  (`torc-desktop.profiles`), next to the last-used form (`torc-desktop.connection`).
- Downloads from the dashboard go to a temporary file, then a native save dialog asks where to keep
  them (`on_download` in `main.rs`, tauri-plugin-dialog).
- tauri-plugin-notification polyfills `window.Notification` for the dashboard's workflow-finished
  notifications. `capabilities/dashboard.json` lets the dashboard origin (`http://127.0.0.1:*`) call
  only its permission and notify commands.

Design notes (why a UNIX socket, the remote watcher, the askpass relay, the stable port):
`docs/src/specialized/design/desktop.md`.

## Requirements

- Rust and the Tauri system dependencies (see <https://tauri.app/start/prerequisites/>), plus the
  Tauri CLI for installers: `cargo install tauri-cli --version "^2.12" --locked`
- `torc-dash` installed on the login node
- OpenSSH 8.4+ on the client (for `SSH_ASKPASS_REQUIRE`)

## Development

CI runs `cargo fmt`, clippy, and the tests for `src-tauri` on every pull request
(`.github/workflows/dashboard.yml`). Tagged releases build the installers
(`.github/workflows/release.yml`). The app's version follows torc's: `cargo release` bumps
`src-tauri/Cargo.toml` (`tauri.conf.json` takes it from there).

```bash
cd torc-desktop
cargo tauri dev        # run; edits to ui/index.html show on reload
cargo tauri build      # release bundle in src-tauri/target/release/bundle/
cargo tauri build --bundles dmg   # just one installer type
```

Without the bundled binaries, local mode runs `torc-dash` from `PATH`. To bundle them as the release
does, copy each of `torc-dash`, `torc`, `torc-server`, and `torc-mcp-server` (plus `.exe` on
Windows) to `src-tauri/binaries/<name>-<host triple>` (triple from `rustc -vV`), then:

```bash
cargo tauri build --config src-tauri/tauri.bundle.conf.json
```
