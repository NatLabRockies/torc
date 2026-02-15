# Installation

## Precompiled Binaries (Recommended)

1. Download the appropriate archive for your platform from the
   [releases page](https://github.com/NatLabRockies/torc/releases):
   - **Linux**: `torc-<version>-x86_64-unknown-linux-gnu.tar.gz`
   - **macOS (Intel)**: `torc-<version>-x86_64-apple-darwin.tar.gz`
   - **macOS (Apple Silicon)**: `torc-<version>-aarch64-apple-darwin.tar.gz`
   - **Windows**: `torc-<version>-x86_64-pc-windows-msvc.zip`

2. Extract the archive:

   ```bash
   # For .tar.gz files
   tar -xzf torc-<version>-<platform>.tar.gz

   # For .zip files
   unzip torc-<version>-<platform>.zip
   ```

3. Add the binaries to a directory in your system PATH:

   ```bash
   # Option 1: Copy to an existing PATH directory
   cp torc* ~/.local/bin/

   # Option 2: Add the extracted directory to your PATH
   export PATH="/path/to/extracted/torc:$PATH"
   ```

   To make the PATH change permanent, add the export line to your shell configuration file
   (`~/.bashrc`, `~/.zshrc`, etc.).

**macOS users**: The precompiled binaries are not signed with an Apple Developer certificate. macOS
Gatekeeper will block them by default. To allow the binaries to run, remove the quarantine attribute
after downloading:

```bash
xattr -cr /path/to/torc*
```

Alternatively, you can right-click each binary and select "Open" to add a security exception.

## Site-Specific Installations

Some HPC facilities maintain pre-installed Torc binaries and shared servers. Check if your site is
listed below.

### NLR Kestrel

**Pre-installed binaries** are available at:

```
/scratch/dthom/torc/
├── 0.14.2/
├── ...
└── latest -> 0.14.2  (symlink to current version)
```

> **Recommended**: Use the `latest` directory. Torc maintains backwards compatibility, so you'll
> automatically receive updates and bug fixes without changing your configuration.

Add to your PATH:

```bash
export PATH="/scratch/dthom/torc/latest:$PATH"
```

Or add to your `~/.bashrc` for persistence:

```bash
echo 'export PATH="/scratch/dthom/torc/latest:$PATH"' >> ~/.bashrc
```

**Shared server**: A `torc-server` instance runs on a dedicated VM within the Kestrel environment at
`torc.hpc.nrel.gov`. Contact Daniel Thom if you would like to create an access group to store
private workflows for your team.

```bash
export TORC_API_URL="http://torc.hpc.nrel.gov:8080/torc-service/v1"
```

**Note:** Be mindful of how much data you store in this database. Delete workflows that you don't
need any longer. Refer to `torc workflows export --help` for making backups of your workflows.

## Install from crates.io (requires cargo to build)

```bash
# Install the torc CLI (default)
cargo install torc

# Install all binaries (server, dashboard, MCP server, Slurm runner)
cargo install torc --features "server-bin,mcp-server,dash,slurm-runner"
```

## Docker

The `torc` Docker image provides the server and all binaries in a single container. Images are
published to the GitHub Container Registry on every release.

```bash
docker pull ghcr.io/natlabrockies/torc:latest
```

> **Recommended**: Use the `latest` tag to automatically receive updates and bug fixes. Torc
> maintains backwards compatibility across releases. Pinned version tags (e.g., `0.14.2`) are also
> available.

### Running the Server

The container runs `torc-server` by default. Authentication is required, so you must provide an
htpasswd file and at least one admin user.

**1. Create an htpasswd file:**

```bash
# Using the bundled torc-htpasswd utility
docker run --rm -it ghcr.io/natlabrockies/torc:latest \
  torc-htpasswd /dev/stdout admin
```

Save the output to a local `htpasswd` file.

**2. Start the server:**

```bash
docker run -d --name torc-server \
  -p 8080:8080 \
  -e TORC_AUTH_FILE=/data/htpasswd \
  -e TORC_ADMIN_USERS=admin \
  -v ./htpasswd:/data/htpasswd:ro \
  -v torc-data:/data \
  ghcr.io/natlabrockies/torc:latest
```

The server will be available at `http://localhost:8080`.

### Environment Variables

| Variable                              | Default         | Description                                  |
| ------------------------------------- | --------------- | -------------------------------------------- |
| `TORC_AUTH_FILE`                      | _(required)_    | Path to htpasswd file inside the container   |
| `TORC_ADMIN_USERS`                    | _(required)_    | Comma-separated list of admin usernames      |
| `TORC_PORT`                           | `8080`          | Server listen port                           |
| `TORC_DATABASE`                       | `/data/torc.db` | SQLite database path                         |
| `TORC_LOG_DIR`                        | `/data`         | Log file directory                           |
| `TORC_LOG_LEVEL`                      | `info`          | Log level (`debug`, `info`, `warn`, `error`) |
| `TORC_THREADS`                        | `4`             | Number of worker threads                     |
| `TORC_COMPLETION_CHECK_INTERVAL_SECS` | `30`            | Job completion polling interval              |

### Running CLI Commands

You can also use the container to run any `torc` CLI command:

```bash
# Check version
docker run --rm ghcr.io/natlabrockies/torc:latest torc --version

# List workflows (pointing at a running server)
docker run --rm ghcr.io/natlabrockies/torc:latest \
  torc --url http://torc.hpc.nrel.gov:8080/torc-service/v1 workflows list
```

## Building from Source

### Prerequisites

- Rust 1.85 or later (required for the 2024 edition)
- SQLite 3.35 or later (usually included with Rust via sqlx)

### Clone the Repository

```bash
git clone https://github.com/NLR/torc.git
cd torc
```

## Building All Components

Note that the file `.env` designates the database URL as `./db/sqlite/dev.db` Change as desired or
set the environment variable `DATABASE_URL`.

**Initialize the database**

```bash
# Install sqlx-cli if needed
cargo install sqlx-cli --no-default-features --features sqlite
sqlx database setup --source torc-server/migrations
```

**Build everything (server, client, dashboard, job runners):**

```bash
# Development build (all features)
cargo build --all-features

# Release build (optimized, recommended)
cargo build --all-features --release
```

**Build individual components using feature flags:**

```bash
# Client CLI (default features)
cargo build --release

# Server + htpasswd utility
cargo build --release --features server-bin

# Web Dashboard
cargo build --release --features dash

# MCP Server
cargo build --release --features mcp-server

# Slurm job runner
cargo build --release --features slurm-runner
```

Binaries will be in `target/release/`.

**Required**: Add this directory to your system path or copy the binaries to a directory already in
your path (e.g., `~/.local/bin/`).

## Python Client

The Python client provides programmatic workflow management for Python users.

### Prerequisites

- Python 3.11 or later

### Installation

```bash
pip install torc-client
```

The `pytorc` command will be available after installation.

## Julia Client

The Julia client provides programmatic workflow management for Julia users.

### Prerequisites

- Julia 1.10 or later

### Installation

Since the package is not yet registered in the Julia General registry, install it directly from
GitHub:

```julia
using Pkg
Pkg.add(url="https://github.com/NatLabRockies/torc.git", subdir="julia_client/Torc")
```

Then use it in your code:

```julia
using Torc
```

## For Developers

### Running Tests

# Run all tests

```bash
cargo test -- --test-threads=1

# Run specific test
cargo test --test test_workflow_manager test_initialize_files_with_updated_files

# Run with debug logging
RUST_LOG=debug cargo test -- --nocapture
```

### Setting Up the Server

**Start the server:**

```bash
# Development mode
cargo run --features server-bin --bin torc-server -- run

# Production mode (release build)
./target/release/torc-server run

# Custom port
./target/release/torc-server run --port 8080
```

Server will start on `http://localhost:8080`.

When running small workflows for testing and demonstration purposes, we recommend setting this
option so that the server detects job completions faster than the default value of 30 seconds.

```bash
./target/release/torc-server run --completion-check-interval-secs 5
```
