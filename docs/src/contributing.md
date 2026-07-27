# Contributing

Contributions to Torc are welcome! This guide will help you get started.

## Development Setup

1. **Fork and clone the repository:**

```bash
git clone https://github.com/your-username/torc.git
cd torc
```

2. **Install Rust and dependencies:**

Make sure you have Rust 1.95 or later installed. Hawk requires its analysis to run with the exact
compiler toolchain it was built against, currently Rust 1.97.1.

3. **Install cargo-nextest:**

```bash
cargo install cargo-nextest
```

4. **Install SQLx CLI:**

```bash
cargo install sqlx-cli --no-default-features --features sqlite
```

5. **Install cargo-release** (only needed when cutting a release):

```bash
cargo install cargo-release
```

6. **Install Hawk:**

```bash
rustup toolchain install 1.97.1
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/astral-sh/hawk/releases/latest/download/cargo-hawk-installer.sh | sh
```

7. **Set up the database:**

```bash
# Create .env file
echo "DATABASE_URL=sqlite:torc.db" > .env

# Run migrations
sqlx migrate run --source torc-server/migrations
```

8. **Build and test:**

```bash
cargo build
cargo nextest run --all-features
```

## Making Changes

### Code Style

Run formatting and linting before committing:

```bash
# Format code
cargo fmt

# Run clippy
cargo clippy --all --all-targets --all-features -- -D warnings

# Run all checks
cargo fmt --check && cargo clippy --all --all-targets --all-features -- -D warnings

# Check public APIs with Hawk (run with the pinned Hawk toolchain)
cargo +1.97.1 hawk check -D warnings
```

### Adding Tests

All new functionality should include tests:

```bash
# Run specific test
cargo nextest run -E 'test(test_name)'

# Run with logging
RUST_LOG=debug cargo nextest run -E 'test(test_name)'
```

### Database Migrations

If you need to modify the database schema:

```bash
# Create new migration
sqlx migrate add --source torc-server/migrations <migration_name>

# Edit the generated SQL file in torc-server/migrations/

# Run migration
sqlx migrate run --source torc-server/migrations

# To revert
sqlx migrate revert --source torc-server/migrations
```

## Submitting Changes

1. **Create a feature branch:**

```bash
git checkout -b feature/my-new-feature
```

2. **Make your changes and commit:**

```bash
git add .
git commit -m "Add feature: description"
```

3. **Ensure all tests pass:**

```bash
cargo nextest run --all-features
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo +1.97.1 hawk check -D warnings
```

4. **Push to your fork:**

```bash
git push origin feature/my-new-feature
```

5. **Open a Pull Request:**

Go to the original repository and open a pull request with:

- Clear description of changes
- Reference to any related issues
- Test results

## Pre-Release: Slurm Integration Tests

Before tagging a release, run the Slurm integration test suite on an HPC cluster to verify that job
submission, multi-node execution, failure recovery, and resource monitoring work correctly under a
real Slurm scheduler.

### Prerequisites

- Access to an HPC cluster with Slurm
- `torc`, `torc-server`, and `torc-htpasswd` binaries built
  (`cargo build --release
  --all-features`)
- `jq` and Slurm CLI tools (`sbatch`, etc.) available on the login node

### Running the Suite

From the repository root on a login node:

```bash
./slurm-tests/run_all.sh --account <SLURM_ACCOUNT> --host <LOGIN_NODE_HOSTNAME>
```

Common options:

```bash
# Use a specific partition (default: debug)
./slurm-tests/run_all.sh --account myproject --host login1.hpc.example.com \
  --partition gpu

# Run a single test by name substring
./slurm-tests/run_all.sh --account myproject --host login1.hpc.example.com \
  --test oom_detection

# Adjust timeout and parallelism
./slurm-tests/run_all.sh --account myproject --host login1.hpc.example.com \
  --timeout 60 --max-parallel-jobs 8
```

The runner starts a temporary Torc server, executes each test as a child workflow under Slurm, and
writes results to `slurm-tests/output/run_<timestamp>/results.json`. All tests must pass before a
release is tagged.

## Cutting a Release

Releases are driven by [`cargo-release`](https://github.com/crate-ci/cargo-release) with the
configuration in [`release.toml`](https://github.com/NREL/torc/blob/main/release.toml). The tool
bumps the version in every place it needs to be kept in sync (`Cargo.toml`, `Cargo.lock`,
`Dockerfile`, `python_client/pyproject.toml`, and the install snippets in
`docs/src/getting-started/installation.md`) and creates a single "chore: Release" commit.

Prerequisite: `cargo install cargo-release` (see step 5 of [Development Setup](#development-setup)).

From a clean `main` checkout:

```bash
# Dry run -- shows exactly what will change without touching anything.
# (cargo-release defaults to dry-run; --execute is what actually applies the bump.)
cargo release patch
cargo release minor
cargo release major

# Apply the bump. The flags below match how releases are cut today:
# tags and pushes are handled separately so the bump commit can be reviewed first.
cargo release patch --no-publish --no-push --no-tag --execute   # 0.30.2 -> 0.30.3
cargo release minor --no-publish --no-push --no-tag --execute   # 0.30.2 -> 0.31.0
cargo release major --no-publish --no-push --no-tag --execute   # 0.30.2 -> 1.0.0
```

After the bump commit lands on `main`, tag it and push the tag to trigger the
[release workflow](https://github.com/NREL/torc/blob/main/.github/workflows/release.yml), which
builds binaries for all supported platforms and creates a draft GitHub release:

```bash
git push origin main
git tag v0.30.3
git push origin v0.30.3
```

See [`.github/RELEASE.md`](https://github.com/NREL/torc/blob/main/.github/RELEASE.md) for details on
which binaries are produced, supported platforms, and how to publish the draft release.

## Rebuilding the README TUI Demo GIF

The animated TUI screencast in `README.md` is regenerated from a script:

```bash
./docs/assets/record_tui_demo.sh
```

Requires a running `torc-server` and `vhs`, `sqlite3`, and `python3` on PATH. Install VHS with
`brew install vhs`.

Environment overrides:

- `TORC_API_URL` — full server URL **including the `/torc-service/v1` prefix** (default:
  `http://localhost:8080/torc-service/v1`). Exported so the torc CLI and TUI hit the same server the
  script probes.
- `TORC_DEMO_DB` — path to the SQLite DB the server is writing to. Defaults to `db/sqlite/dev.db`
  relative to the repo root; override if `torc-server` was started with `--database` pointing
  elsewhere.

The script creates a `Simulation demo` workflow, seeds three stages of progressively-completed job
status (mid-flight → failures appear → finished) directly into the SQLite database, drives the TUI
with VHS, and writes the result to `docs/assets/tui-demo.gif`. It also verifies the final DB state
after recording — if the backgrounded stage transitions didn't land, the rendered GIF is left in
`$REPO_ROOT/tui-demo.gif` and not promoted. Re-running the script wipes any prior `Simulation demo`
workflow, so it is safe to invoke repeatedly.

To tweak the recording (timings, terminal size, theme), edit the VHS tape heredoc near the bottom of
the script.

## Pull Request Guidelines

- **Keep PRs focused** - One feature or fix per PR
- **Add tests** - All new code should be tested
- **Update documentation** - Update README.md, DOCUMENTATION.md, or inline docs as needed
- **Follow style guidelines** - Run `cargo fmt` and `cargo clippy`
- **Write clear commit messages** - Describe what and why, not just how

## Areas for Contribution

### High Priority

- Performance optimizations for large workflows
- Additional job runner implementations (Kubernetes, etc.)
- Improved error messages and logging
- Documentation improvements

### Features

- Workflow visualization tools
- Job retry policies and error handling
- Workflow templates and libraries
- Integration with external systems

### Testing

- Additional integration tests
- Performance benchmarks
- Stress testing with large workflows

## Code of Conduct

Be respectful and constructive in all interactions. We're all here to make Torc better.

## Questions?

- Open an issue for bugs or feature requests
- Start a discussion for questions or ideas
- Check existing issues and discussions first

## License

By contributing, you agree that your contributions will be licensed under the BSD 3-Clause License.
