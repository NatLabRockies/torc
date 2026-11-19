# Torc Desktop

Native desktop app for torc-dash, on this computer or on a remote login node. In remote mode the app
opens an SSH session, starts `torc-dash` there on a private UNIX socket, forwards it to a local
port, and shows the dashboard in its window. No manual SSH tunnel.

Built with [Tauri 2](https://tauri.app) and Svelte 5. It is a standalone crate, not part of the main
torc workspace.

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

Design notes (why a UNIX socket, the remote watcher, the askpass relay, the stable port):
`docs/src/specialized/design/desktop.md`.

## Requirements

- Rust and Node.js (see <https://tauri.app/start/prerequisites/>)
- `torc-dash` installed on the login node
- OpenSSH 8.4+ on the client (for `SSH_ASKPASS_REQUIRE`)

## Development

CI type-checks the connect page and runs `cargo fmt`, clippy, and the tests for `src-tauri` on every
pull request (`.github/workflows/dashboard.yml`). Tagged releases build the installers
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
