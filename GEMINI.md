# GEMINI.md

This file provides guidance to Gemini CLI when working with code in this repository. It establishes
foundational mandates that take precedence over general defaults.

## Development Lifecycle

1. **Research**: Map the codebase and validate assumptions using `grep_search` and `read_file`.
2. **Strategy**: Formulate a plan and share a concise summary.
3. **Execution**:
   - **Plan**: Define implementation and testing strategy.
   - **Act**: Apply targeted changes.
   - **Validate**: Run tests and quality checks.

## Code Quality Requirements

All changes MUST pass these checks:

```bash
# Rust formatting
cargo fmt -- --check

# Rust linting (MUST pass with no warnings)
cargo clippy --all --all-targets --all-features -- -D warnings

# Markdown formatting
dprint check
```

## Component-Specific Guidance

### Rust Client (`src/client/`)

- Command handlers are in `src/client/commands/`.
- Use the `Tabled` trait for tabular CLI output.
- Follow the established `\x1b[1;36m` (cyan) for commands and `\x1b[1;32m` (green) for categories in
  help templates.
