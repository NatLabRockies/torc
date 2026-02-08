# Contributing

Contributions to Torc are welcome! This guide will help you get started.

## Development Setup

1. **Fork and clone the repository:**

```bash
git clone https://github.com/your-username/torc.git
cd torc
```

2. **Install Rust and dependencies:**

Make sure you have Rust 1.85 or later installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

3. **Install SQLx CLI:**

```bash
cargo install sqlx-cli --no-default-features --features sqlite
```

4. **Set up the database:**

```bash
# Create .env file
echo "DATABASE_URL=sqlite:torc.db" > .env

# Run migrations
sqlx migrate run --source torc-server/migrations
```

5. **Build and test:**

```bash
cargo build
cargo test
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
```

### Adding Tests

All new functionality should include tests:

```bash
# Run specific test
cargo test test_name -- --nocapture

# Run with logging
RUST_LOG=debug cargo test -- --nocapture
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
cargo test
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
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
