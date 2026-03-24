use std::env;
use std::fs;
use std::io::{self, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);

    match args.next().as_deref() {
        Some("compare") => {
            let source_path = args.next().ok_or("missing source spec path")?;
            let source = fs::read_to_string(&source_path)?;
            let issues = torc::openapi_spec::parity_report(&source)?;
            if issues.is_empty() {
                writeln!(io::stdout(), "parity-check: ok")?;
                return Ok(());
            }

            for issue in issues {
                writeln!(io::stderr(), "{issue}")?;
            }
            Err("parity-check failed".into())
        }
        None => {
            let yaml = torc::openapi_spec::render_openapi_yaml()?;
            io::stdout().write_all(yaml.as_bytes())?;
            Ok(())
        }
        Some(other) => Err(format!("unknown subcommand: {other}").into()),
    }
}
