# Torc Documentation

This directory contains the source files for Torc's user documentation, built with
[mdBook](https://rust-lang.github.io/mdBook/).

## Building the Documentation

### Prerequisites

Install mdBook:

```bash
cargo install mdbook
```

### Build Commands

**Build the documentation:**

```bash
mdbook build
```

This must be run from the `docs/` directory. Output will be written to `docs/book/`.

**Serve locally with live reload:**

```bash
mdbook serve
```

This will:

- Build the documentation
- Start a local web server at `http://localhost:3000`
- Watch for file changes and rebuild automatically
- Open your browser automatically

**Serve on custom address:**

```bash
mdbook serve --hostname 0.0.0.0 --port 8080
```

**Clean build artifacts:**

```bash
mdbook clean
```

### Testing

Check for broken links and other issues:

```bash
mdbook test
```

## Documentation Structure

The documentation follows the [Diataxis](https://diataxis.fr/) framework:

```
src/
├── SUMMARY.md              # Table of contents
├── introduction.md         # Landing page
├── getting-started.md      # Quick start guide
├── installation.md         # Installation instructions
├── quick-start.md          # Basic usage
│
├── explanation/            # Understanding-oriented
│   ├── README.md
│   ├── architecture.md
│   ├── database.md
│   ├── server.md
│   ├── client.md
│   ├── job-runners.md
│   ├── job-states.md
│   ├── intelligent-restart.md
│   ├── dependencies.md
│
├── how-to/                 # Problem-oriented
│   ├── README.md
│   ├── creating-workflows.md
│   ├── slurm.md
│   └── resources.md
│
├── reference/              # Information-oriented
│   ├── README.md
│   ├── openapi.md
│   ├── parameterization.md
│   └── configuration.md
│
├── tutorials/              # Learning-oriented
│   ├── README.md
│   ├── many-jobs.md
│   ├── diamond.md
│   ├── user-data.md
│   ├── simple-params.md
│   └── advanced-params.md
│
└── contributing.md         # Contributing guide
```

## Editing Documentation

1. Edit Markdown files in `src/`
2. If adding new pages, update `src/SUMMARY.md`
3. Run `mdbook serve` to preview changes
4. Build with `mdbook build` before committing

## OpenAPI And Client Generation

The OpenAPI contract is emitted from Rust, not edited by hand.

From the repository root:

```bash
cd api

# Emit Rust-owned spec only
bash sync_openapi.sh emit

# Verify both checked-in spec files match Rust output
bash sync_openapi.sh check

# Promote the Rust spec into api/openapi.yaml and regenerate clients
bash sync_openapi.sh all --promote

# Regenerate Rust, Python, and Julia clients from the current checked-in spec
bash sync_openapi.sh clients
```

### Markdown Features

mdBook supports:

- **Standard Markdown** - headings, lists, links, images
- **Code blocks with syntax highlighting** - Specify language after ```
- **Tables** - GitHub-flavored markdown tables
- **Admonitions** - Using blockquotes with specific prefixes
- **Links** - Relative links between pages
- **Anchor links** - `#heading-name` within pages

Example code block:

```yaml
name: my_workflow
jobs:
  - name: hello
    command: echo "Hello World"
```

### Adding New Pages

1. Create new `.md` file in appropriate directory
2. Add entry to `SUMMARY.md`:

```markdown
- [New Page Title](./path/to/new-page.md)
```

3. Test build: `mdbook build`

## Deployment

### GitHub Pages

To deploy to GitHub Pages:

1. Build the documentation:
   ```bash
   mdbook build
   ```

2. The `book/` directory contains the static site

3. Configure GitHub Pages to serve from `docs/book/` or use GitHub Actions to build and deploy

### Custom Deployment

The `book/` directory is a self-contained static website. Deploy it to any web server:

```bash
# Example: Copy to web server
scp -r book/* user@server:/var/www/torc-docs/

# Example: Deploy to S3
aws s3 sync book/ s3://my-bucket/torc-docs/ --delete
```

## Configuration

Edit `book.toml` to customize:

- Site title and description
- GitHub repository links
- Theme and styling
- Search settings
- Output format options

See [mdBook documentation](https://rust-lang.github.io/mdBook/format/configuration/index.html) for
all options.

## Troubleshooting

**Build fails with "File not found":**

- Check that all files referenced in `SUMMARY.md` exist
- Verify paths are relative to `src/` directory

**Links broken in generated site:**

- Use relative links: `[Link](./page.md)` not absolute paths
- Check link anchors match actual heading IDs

**Styles not applying:**

- Custom CSS goes in `theme/` directory
- See mdBook theme documentation

**Search not working:**

- Search is enabled by default in `book.toml`
- Rebuild if search index seems stale

## Additional Resources

- [mdBook Documentation](https://rust-lang.github.io/mdBook/)
- [Diataxis Framework](https://diataxis.fr/)
- [Markdown Guide](https://www.markdownguide.org/)
