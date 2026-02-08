//! Generate CLI reference documentation in Markdown format.
//!
//! This binary generates documentation for the torc CLI using clap-markdown.
//! Run with: cargo run --bin generate-cli-docs --features "client,tui,plot_resources"
//!
//! The output is written to docs/src/reference/cli.md

use std::fs;
use std::path::Path;
use torc::cli::Cli;

fn main() {
    let markdown = clap_markdown::help_markdown::<Cli>();

    // Add a header with generation note
    let output = format!(
        r#"# CLI Reference

This documentation is automatically generated from the CLI help text.

To regenerate, run:
```bash
cargo run --bin generate-cli-docs --features "client,tui,plot_resources"
```

{}
"#,
        markdown
    );

    let output_path = Path::new("docs/src/core/reference/cli.md");

    // Ensure parent directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).expect("Failed to create output directory");
    }

    fs::write(output_path, output).expect("Failed to write CLI documentation");

    println!("Generated CLI documentation at {}", output_path.display());
}
