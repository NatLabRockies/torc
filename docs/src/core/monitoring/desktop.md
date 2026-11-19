# Desktop App

Torc Desktop is a native app (macOS, Windows, Linux) that shows the [web dashboard](./dashboard.md)
in its own window. It can run the dashboard on your computer, or on an HPC login node over SSH
without a manual SSH tunnel.

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
- **Windows (x64)**: the `-setup.exe` installer, which installs for the current user and needs no
  admin rights. The `.msi` installs for all users and requires admin rights. SmartScreen may warn
  about an unrecognized app; choose **More info → Run anyway**.

The installers include `torc-dash`, `torc`, `torc-server`, and `torc-mcp-server`, so local mode
needs no separate install or `PATH` setup.

## Connecting

The connect page offers two modes.

### This computer

Starts the bundled `torc-dash` on a private local port. It runs through your login shell, so your
profile's environment (such as `TORC_API_URL` or `TORC_PASSWORD`) applies.

- **Working directory** (optional): where workflows run and write output. `~/` and relative paths
  resolve against your home directory. Default: your home directory.

### Remote (SSH)

Opens an SSH session to a login node, starts `torc-dash` there on a private UNIX socket, and
forwards it to the app. Nothing listens on a network port on the login node.

- **Login node**: `user@host`, or a host alias from `~/.ssh/config`. The app uses your system `ssh`
  (OpenSSH 8.4 or later; included in macOS, Linux, and Windows 10+), so `~/.ssh/config`,
  `ProxyJump`, keys, and Kerberos work as usual. Password, MFA, and host-key prompts appear in the
  app.
- **Environment setup** (optional): a command that runs first, such as `module load torc`. Use it to
  put `torc-dash` on the `PATH`.
- **Working directory** (optional): where workflows run and write output, such as `/scratch/$USER`.
  Shell syntax (`~`, `$VARS`) works. Default: your home directory.

The remote command runs under `bash -l`, so your login profile (`~/.bash_profile`) is read.
`torc-dash` must be installed on the login node; see
[Installation](../../getting-started/installation.md).

The app also sends the AI Chat settings from your computer's `[dash.chat]` config to the remote
dashboard; see [AI Chat Tab](./dashboard.md#ai-chat-tab).

### Torc server

In both modes, **Torc server** chooses which server the dashboard talks to:

- **Use a shared torc-server**: enter its URL, or leave it blank to use `TORC_API_URL` or `api_url`
  in the `[dash]` section of the torc config (`~/.config/torc/config.toml`, or
  `/etc/torc/config.toml` for a site-wide default) on the machine where torc-dash runs. With neither
  set, it uses `http://localhost:8080/torc-service/v1`. A URL entered here overrides `TORC_API_URL`.
- **Start a private torc-server**: torc-dash starts a server that listens on localhost only. Its
  database is `~/torc-desktop.db` on the login node, or in the app's data folder locally
  (`~/Library/Application Support/com.github.natlabrockies.torc-desktop/torc.db` on macOS).

If the shared server requires a login, set `TORC_PASSWORD` (and `TORC_USERNAME` if it differs from
`USER`) where torc-dash runs: in `~/.bash_profile` on the login node, or in your login-shell profile
locally (`~/.zprofile` for zsh; `~/.zshrc` is not read). See
[Set Up Authentication](../how-to/set-up-authentication.md).

The app remembers the form between launches.

## While connected

The **Connection** menu has:

- **Reload** (Cmd/Ctrl+R): reloads the dashboard.
- **Disconnect** (Cmd/Ctrl+Shift+D): stops torc-dash and returns to the connect page.

Disconnecting or quitting stops torc-dash and any server it started. If an SSH session drops
(network change, sleep), the app notices within about 90 seconds and returns to the connect page
with a **Reconnect** button. Reconnecting signs in again, so MFA users are prompted again.

Downloads and links that open new windows may not behave as they do in a browser.

## Troubleshooting

- **macOS says the app is damaged or can't be opened**: run the `xattr` command under
  [Install](#install).
- **Windows asks for admin rights**: use the `-setup.exe` installer instead of the `.msi`.
- **`torc-dash: command not found` (remote)**: install torc on the login node, or load it in
  **Environment setup** (for example, `module load torc`). The app does not install anything
  remotely.
- **The dashboard talks to the wrong server**: clear the URL field to use `TORC_API_URL`. Locally,
  set the variable in `~/.zprofile` (zsh) rather than `~/.zshrc`, which the app does not read.
- **Authentication errors**: set `TORC_PASSWORD` as described under [Torc server](#torc-server).
