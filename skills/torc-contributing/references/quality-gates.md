# Quality gates

## Contents

- [The three gates](#the-three-gates)
- [Pre-commit hook](#pre-commit-hook)
- [What CI adds](#what-ci-adds)
- [Rust style](#rust-style)
- [Markdown and docs](#markdown-and-docs)
- [Shell scripts](#shell-scripts)
- [Releases](#releases)

## The three gates

```bash
cargo fmt -- --check
cargo clippy --all --all-targets --all-features -- -D warnings
dprint check
```

Run all three before finishing. `cargo fmt` and `dprint fmt` fix their own findings; clippy findings
need real changes.

`--all-features` is required. Server, dashboard, MCP, and Slurm-runner code is feature-gated, so a
default-feature clippy run silently skips most of the repository.

`--all-features` also pulls in `sqlx` compile-time query checking, which needs a database or the
checked-in `.sqlx/` cache:

```bash
echo "DATABASE_URL=sqlite:torc.db" > .env
cargo install sqlx-cli --no-default-features --features sqlite
sqlx migrate run --source torc-server/migrations
```

`dprint` is a separate binary; install it from <https://dprint.dev> if `dprint check` is not found.

## Pre-commit hook

`cargo-husky` installs `.cargo-husky/hooks/pre-commit` on first `cargo build`. It runs the three
gates plus `shellcheck` over every `.sh` file when shellcheck is available, and blocks the commit on
any failure.

Reinstall if hooks go missing:

```bash
cargo install cargo-husky
cargo build
```

There is also a `.pre-commit-config.yaml` covering whitespace, end-of-file, large files, and `ruff`
for Python. It excludes generated clients (`python_client/src/torc/openapi_client/`,
`julia_client/`).

## What CI adds

The lint workflow runs the three gates and then:

| Check                        | Command                                                 |
| ---------------------------- | ------------------------------------------------------- |
| OpenAPI codegen parity       | `bash api/check_openapi_codegen_parity.sh`              |
| Generated client parity      | `bash api/check_client_codegen_parity.sh`               |
| Doc-link checker self-test   | `python3 .github/scripts/test_check_doc_links.py`       |
| Internal documentation links | `python3 .github/scripts/check-doc-links.py --internal` |
| Shell lint                   | `shellcheck` over all `.sh` files                       |

The test workflow runs `cargo nextest run --all-features` on Unix, a reduced set on Windows, an SSH
loopback remote-worker test with `TORC_TEST_SSH_LOOPBACK=1`, and the Python client tests against a
live server.

Parity failures are the most common surprise. Reproduce with `cd api && bash sync_openapi.sh check`
and see `api-and-database.md`.

## Rust style

- 4-space indentation, 100-character lines, sorted imports, all enforced by `rustfmt`.
- Prefer `expect("descriptive message")` over `unwrap()`. Clippy flags `unwrap_used` patterns, and a
  message that names the invariant is worth more than a shorter line.
- Common findings: `clippy::clone_on_copy`, `clippy::needless_return`, `clippy::redundant_closure`.
- Log messages that reference database records use `key=value` pairs with the identifiers spelled
  `workflow_id=<id> job_id=<id>`, because parsing scripts and the doc examples depend on that form.
- Keep functions small enough to read in one pass. Long functions and duplicated blocks are called
  out in review.
- Watch for accidental performance regressions: repeated CLI subprocess invocations, per-item API
  calls where a batch endpoint exists, and unnecessary joins in server queries.

## Markdown and docs

Every Markdown file must satisfy `dprint check`, which wraps at 100 characters (`textWrap: always`).
`python_client/`, `julia_client/`, `target/`, and `node_modules/` are excluded.

```bash
dprint fmt      # rewrap and format
dprint check    # verify
```

Documentation lives in `docs/src/` and follows Diataxis: tutorials, how-to guides, concepts, and
reference. Add every new page to `docs/src/SUMMARY.md`, or the doc-link checker and mdbook build
will not see it.

```bash
cd docs && mdbook build        # build into docs/book/
cd docs && mdbook serve        # preview with reload
python3 .github/scripts/check-doc-links.py --internal
```

The book uses Mermaid diagrams, so a local build needs `mdbook-mermaid` installed
(`cargo install mdbook-mermaid`). CI pins mdBook 0.4.52 and mdbook-mermaid 0.16.0 and additionally
runs the link checker with `--external`.

Significant design decisions belong in `docs/src/specialized/design/` with problem statement, goals,
solution, implementation notes, and alternatives considered.

## Shell scripts

Every `.sh` file in the repository is linted by `shellcheck` in both the pre-commit hook and CI,
excluding `target/` and virtualenvs. Existing scripts use `set -euo pipefail`; follow that, and use
targeted `# shellcheck disable=` comments with a reason when a suppression is genuinely needed.

## Releases

Releases run through `cargo-release` with `release.toml`, which bumps the version everywhere it must
stay in sync: `Cargo.toml`, `Cargo.lock`, `Dockerfile`, `python_client/pyproject.toml`, and the
install snippets in `docs/src/getting-started/installation.md`.

```bash
cargo release patch                                             # dry run (the default)
cargo release patch --no-publish --no-push --no-tag --execute   # apply the bump commit
git push origin main
git tag v0.40.1 && git push origin v0.40.1                      # triggers the release workflow
```

Before tagging, run the Slurm integration suite on a cluster (see `testing.md`). Details on produced
binaries and publishing the draft release are in `.github/RELEASE.md`.
