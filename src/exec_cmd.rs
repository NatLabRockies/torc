//! Implementation of `torc exec` — run inline commands as a synthesized workflow.
//!
//! Builds a `WorkflowSpec` from command-line inputs (`-c`, `-C`, `--param`),
//! creates the workflow via the standard client path, then runs it locally through
//! `run_jobs_cmd`. The workflow is persisted like any other; only the standalone
//! server (when `--standalone` is used) is ephemeral. The user-facing contract is
//! defined by the `Commands::Exec` variant in `cli.rs`.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, IsTerminal};
use std::path::PathBuf;

use crate::client::apis::configuration::Configuration;
use crate::client::parameter_expansion::{
    ParameterValue, cartesian_product, parse_parameter_value, substitute_parameters, zip_parameters,
};
use crate::client::resource_monitor::{
    ComputeNodeMonitorConfig, JobMonitorConfig, MonitorGranularity, ResourceMonitorConfig,
};
use crate::client::workflow_spec::{
    ExecutionConfig, JobSpec, StdioConfig, StdioMode, WorkflowSpec,
};
use crate::run_jobs_cmd;

/// Bundle of arguments for `torc exec`. Passed from `main.rs` after CLI parsing.
pub struct ExecArgs {
    pub name: Option<String>,
    pub description: Option<String>,
    pub commands: Vec<String>,
    pub commands_file: Option<String>,
    pub params: Vec<String>,
    pub link: String,
    pub max_parallel_jobs: Option<i64>,
    pub output_dir: PathBuf,
    pub dry_run: bool,
    pub monitor: String,
    pub monitor_compute_node: String,
    pub generate_plots: bool,
    pub sample_interval_seconds: Option<i32>,
    pub stdio: Option<String>,
    pub trailing: Vec<String>,
    pub shell_command_delimited: bool,
    pub format: String,
    pub log_level: String,
    pub url: String,
    pub password: Option<String>,
    pub tls_ca_cert: Option<String>,
    pub tls_insecure: bool,
    pub cookie_header: Option<String>,
}

/// Run the exec command. Returns on workflow completion; exits the process on errors.
pub fn run(args: ExecArgs, config: &Configuration, user: &str) {
    // Detect the `torc exec <file>` mistake and redirect users to `torc run`.
    // Skipped when the user passed `--`: everything after the delimiter is an intentional
    // shell command (e.g., `torc exec -- cat workflow.yaml`) and must not be second-guessed.
    if !args.shell_command_delimited
        && let Some(hint) = detect_spec_file_in_trailing(&args.trailing)
    {
        eprintln!(
            "torc exec: unexpected argument '{}' — this looks like a workflow spec file.",
            hint
        );
        eprintln!("Did you mean: torc run {}", hint);
        std::process::exit(2);
    }
    if !args.trailing.is_empty()
        && (!args.shell_command_delimited
            || !args.commands.is_empty()
            || args.commands_file.is_some())
    {
        eprintln!(
            "torc exec: unexpected trailing argument(s): {}",
            args.trailing.join(" ")
        );
        eprintln!(
            "Use either -c/--command, -C/--commands-file, or shell-style `torc exec -- <command>`."
        );
        std::process::exit(2);
    }

    let shell_command =
        if args.shell_command_delimited && args.commands.is_empty() && args.commands_file.is_none()
        {
            shell_command_from_trailing(&args.trailing)
        } else {
            Ok(None)
        };
    let shell_command = match shell_command {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("torc exec: {}", e);
            std::process::exit(1);
        }
    };

    let commands = match gather_commands(
        &args.commands,
        args.commands_file.as_deref(),
        shell_command.as_deref(),
    ) {
        Ok(cmds) => cmds,
        Err(e) => {
            eprintln!("torc exec: {}", e);
            std::process::exit(1);
        }
    };
    if commands.is_empty() {
        eprintln!("torc exec: no commands provided.");
        eprintln!("Use -c/--command (repeatable) or -C/--commands-file to supply commands.");
        eprintln!("Hint: to run a workflow from a spec file, use `torc run <file>`.");
        std::process::exit(2);
    }

    let parsed_params = match parse_params(&args.params) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("torc exec: {}", e);
            std::process::exit(1);
        }
    };

    let combos = match build_combinations(&parsed_params, &args.link) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("torc exec: {}", e);
            std::process::exit(1);
        }
    };

    let jobs = expand_jobs(&commands, &combos);
    if jobs.is_empty() {
        eprintln!("torc exec: expansion produced zero jobs (empty parameter list?).");
        std::process::exit(1);
    }

    let spec = build_spec(
        jobs,
        user,
        args.name.as_deref(),
        args.description.as_deref(),
        &args.monitor,
        &args.monitor_compute_node,
        args.generate_plots,
        args.sample_interval_seconds,
        args.stdio.as_deref(),
    );
    let spec = match spec {
        Ok(s) => s,
        Err(e) => {
            eprintln!("torc exec: {}", e);
            std::process::exit(1);
        }
    };

    if args.dry_run {
        print_dry_run(&spec, &args.format);
        return;
    }

    let workflow_id = match WorkflowSpec::create_from_validated_spec(config, spec, user, false) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("torc exec: error creating workflow: {}", e);
            std::process::exit(1);
        }
    };

    if args.format != "json" {
        println!("Created workflow {}", workflow_id);
    }

    let run_args = run_jobs_cmd::Args {
        workflow_id: Some(workflow_id),
        url: args.url,
        output_dir: args.output_dir,
        poll_interval: 5.0,
        max_parallel_jobs: args.max_parallel_jobs,
        time_limit: None,
        end_time: None,
        num_cpus: None,
        memory_gb: None,
        num_gpus: None,
        num_nodes: None,
        scheduler_config_id: None,
        log_prefix: None,
        cpu_affinity_cpus_per_job: None,
        log_level: args.log_level,
        password: args.password,
        tls_ca_cert: args.tls_ca_cert,
        tls_insecure: args.tls_insecure,
        cookie_header: args.cookie_header,
    };
    let log_stream = if args.format == "json" {
        run_jobs_cmd::LogStream::Stderr
    } else {
        run_jobs_cmd::LogStream::Stdout
    };
    let result = run_jobs_cmd::run_with_log_stream(&run_args, log_stream);
    if args.format == "json" {
        println!(
            "{}",
            serde_json::json!({
                "workflow_id": workflow_id,
                "status": if result.had_failures || result.had_terminations {
                    "failed"
                } else {
                    "completed"
                },
                "had_failures": result.had_failures,
                "had_terminations": result.had_terminations,
            })
        );
    }
    if result.had_failures || result.had_terminations {
        std::process::exit(1);
    }
}

/// If any trailing argument looks like a workflow spec file, return it.
/// Only matches by known spec extensions; other file paths fall through to the
/// generic "unexpected argument" error (so e.g. `torc exec commands.txt` doesn't
/// get incorrectly steered toward `torc run`).
fn detect_spec_file_in_trailing(trailing: &[String]) -> Option<String> {
    for arg in trailing {
        let lower = arg.to_lowercase();
        let extension_hit = lower.ends_with(".yaml")
            || lower.ends_with(".yml")
            || lower.ends_with(".json")
            || lower.ends_with(".json5")
            || lower.ends_with(".kdl");
        if extension_hit {
            return Some(arg.clone());
        }
    }
    None
}

/// Collect commands from `-c` entries plus an optional commands file (or stdin if `-`).
/// Blank lines and lines starting with `#` in the file are skipped.
fn gather_commands(
    inline: &[String],
    commands_file: Option<&str>,
    shell_command: Option<&str>,
) -> Result<Vec<String>, String> {
    let mut cmds: Vec<String> = inline.iter().map(|s| s.to_string()).collect();

    if let Some(source) = commands_file {
        let content = if source == "-" {
            if std::io::stdin().is_terminal() {
                return Err(
                    "commands-file '-' requires piped stdin, but stdin is a terminal".into(),
                );
            }
            let mut buf = String::new();
            let stdin = std::io::stdin();
            for line in stdin.lock().lines() {
                let line = line.map_err(|e| format!("error reading stdin: {}", e))?;
                buf.push_str(&line);
                buf.push('\n');
            }
            buf
        } else {
            fs::read_to_string(source)
                .map_err(|e| format!("error reading commands file '{}': {}", source, e))?
        };
        for raw in content.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            cmds.push(line.to_string());
        }
    }

    if let Some(cmd) = shell_command {
        cmds.push(cmd.to_string());
    }

    Ok(cmds)
}

fn shell_command_from_trailing(trailing: &[String]) -> Result<Option<String>, String> {
    if trailing.is_empty() {
        return Ok(None);
    }
    let words: Vec<&str> = trailing.iter().map(String::as_str).collect();
    shlex::try_join(words)
        .map(Some)
        .map_err(|e| format!("could not quote shell-style command: {}", e))
}

fn print_dry_run(spec: &WorkflowSpec, _format: &str) {
    println!(
        "{}",
        serde_json::to_string_pretty(spec).expect("WorkflowSpec should serialize")
    );
}

fn parse_params(specs: &[String]) -> Result<HashMap<String, Vec<ParameterValue>>, String> {
    let mut out = HashMap::new();
    for raw in specs {
        let (name, value) = raw
            .split_once('=')
            .ok_or_else(|| format!("invalid --param '{}': expected NAME=VALUE", raw))?;
        let name = name.trim();
        if name.is_empty() {
            return Err(format!(
                "invalid --param '{}': parameter name is empty",
                raw
            ));
        }
        if out.contains_key(name) {
            return Err(format!("duplicate --param '{}'", name));
        }
        let values = parse_parameter_value(value)
            .map_err(|e| format!("invalid value for --param {}: {}", name, e))?;
        out.insert(name.to_string(), values);
    }
    Ok(out)
}

fn build_combinations(
    params: &HashMap<String, Vec<ParameterValue>>,
    link: &str,
) -> Result<Vec<HashMap<String, ParameterValue>>, String> {
    if params.is_empty() {
        return Ok(vec![HashMap::new()]);
    }
    match link {
        "zip" => zip_parameters(params),
        "product" => Ok(cartesian_product(params)),
        other => Err(format!(
            "invalid --link '{}': expected 'product' or 'zip'",
            other
        )),
    }
}

fn expand_jobs(commands: &[String], combos: &[HashMap<String, ParameterValue>]) -> Vec<JobSpec> {
    let mut jobs = Vec::with_capacity(commands.len() * combos.len());
    let mut counter = 1usize;
    for template in commands {
        for combo in combos {
            let cmd = if combo.is_empty() {
                template.clone()
            } else {
                substitute_parameters(template, combo)
            };
            jobs.push(JobSpec::new(format!("job{}", counter), cmd));
            counter += 1;
        }
    }
    jobs
}

#[allow(clippy::too_many_arguments)]
fn build_spec(
    jobs: Vec<JobSpec>,
    user: &str,
    name: Option<&str>,
    description: Option<&str>,
    monitor: &str,
    monitor_compute_node: &str,
    generate_plots: bool,
    sample_interval_seconds: Option<i32>,
    stdio: Option<&str>,
) -> Result<WorkflowSpec, String> {
    let wf_name = name
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("exec_{}", chrono::Local::now().format("%Y%m%d_%H%M%S")));
    let mut spec = WorkflowSpec::new(
        wf_name,
        user.to_string(),
        description.map(|s| s.to_string()),
        jobs,
    );

    let jobs_cfg = parse_job_monitor(monitor)?;
    let node_cfg = parse_node_monitor(monitor_compute_node)?;

    if generate_plots {
        let has_timeseries = jobs_cfg
            .as_ref()
            .is_some_and(|j| matches!(j.granularity, MonitorGranularity::TimeSeries))
            || node_cfg
                .as_ref()
                .is_some_and(|n| matches!(n.granularity, MonitorGranularity::TimeSeries));
        if !has_timeseries {
            return Err(
                "--generate-plots requires --monitor time-series or --monitor-compute-node time-series"
                    .into(),
            );
        }
    }

    if jobs_cfg.is_some() || node_cfg.is_some() {
        spec.resource_monitor = Some(ResourceMonitorConfig {
            enabled: jobs_cfg.as_ref().is_some_and(|j| j.enabled),
            granularity: jobs_cfg
                .as_ref()
                .map(|j| j.granularity.clone())
                .unwrap_or(MonitorGranularity::Summary),
            sample_interval_seconds: sample_interval_seconds.unwrap_or(10),
            generate_plots,
            jobs: jobs_cfg,
            compute_node: node_cfg,
            ..ResourceMonitorConfig::default()
        });
    }

    if let Some(mode_str) = stdio {
        let mode = parse_stdio_mode(mode_str)?;
        let ec = spec
            .execution_config
            .get_or_insert_with(ExecutionConfig::default);
        ec.stdio = Some(StdioConfig {
            mode,
            delete_on_success: None,
        });
    }

    Ok(spec)
}

fn parse_job_monitor(mode: &str) -> Result<Option<JobMonitorConfig>, String> {
    match mode {
        "off" => Ok(None),
        "summary" => Ok(Some(JobMonitorConfig {
            enabled: true,
            granularity: MonitorGranularity::Summary,
        })),
        "time-series" => Ok(Some(JobMonitorConfig {
            enabled: true,
            granularity: MonitorGranularity::TimeSeries,
        })),
        other => Err(format!("invalid --monitor '{}'", other)),
    }
}

fn parse_node_monitor(mode: &str) -> Result<Option<ComputeNodeMonitorConfig>, String> {
    match mode {
        "off" => Ok(None),
        "summary" => Ok(Some(ComputeNodeMonitorConfig {
            enabled: true,
            granularity: MonitorGranularity::Summary,
            cpu: true,
            memory: true,
        })),
        "time-series" => Ok(Some(ComputeNodeMonitorConfig {
            enabled: true,
            granularity: MonitorGranularity::TimeSeries,
            cpu: true,
            memory: true,
        })),
        other => Err(format!("invalid --monitor-compute-node '{}'", other)),
    }
}

fn parse_stdio_mode(s: &str) -> Result<StdioMode, String> {
    match s {
        "separate" => Ok(StdioMode::Separate),
        "combined" => Ok(StdioMode::Combined),
        "no-stdout" => Ok(StdioMode::NoStdout),
        "no-stderr" => Ok(StdioMode::NoStderr),
        "none" => Ok(StdioMode::None),
        other => Err(format!("invalid --stdio '{}'", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gather_commands_inline_only() {
        let cmds = gather_commands(&["echo a".into(), "echo b".into()], None, None).unwrap();
        assert_eq!(cmds, vec!["echo a", "echo b"]);
    }

    #[test]
    fn gather_commands_from_file_skips_blanks_and_comments() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cmds.txt");
        std::fs::write(&path, "echo 1\n\n# comment\n  echo 2  \n").unwrap();
        let cmds = gather_commands(&[], Some(path.to_str().unwrap()), None).unwrap();
        assert_eq!(cmds, vec!["echo 1", "echo 2"]);
    }

    #[test]
    fn shell_command_from_trailing_quotes_words() {
        let cmd = shell_command_from_trailing(&[
            "python".into(),
            "train.py".into(),
            "--label".into(),
            "two words".into(),
        ])
        .unwrap()
        .unwrap();
        assert_eq!(cmd, "python train.py --label 'two words'");
    }

    #[test]
    fn parse_params_rejects_missing_equals() {
        let err = parse_params(&["foo".into()]).unwrap_err();
        assert!(err.contains("expected NAME=VALUE"));
    }

    #[test]
    fn parse_params_rejects_duplicate() {
        let err = parse_params(&["x=1".into(), "x=2".into()]).unwrap_err();
        assert!(err.contains("duplicate"));
    }

    #[test]
    fn expand_jobs_cartesian_produces_unique_names() {
        let mut params = HashMap::new();
        params.insert(
            "i".to_string(),
            vec![ParameterValue::Integer(1), ParameterValue::Integer(2)],
        );
        let combos = cartesian_product(&params);
        let jobs = expand_jobs(&["echo {i}".into()], &combos);
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].name, "job1");
        assert_eq!(jobs[0].command, "echo 1");
        assert_eq!(jobs[1].name, "job2");
        assert_eq!(jobs[1].command, "echo 2");
    }

    #[test]
    fn expand_jobs_multiple_commands_counter_spans_all() {
        let jobs = expand_jobs(&["a".into(), "b".into(), "c".into()], &[HashMap::new()]);
        assert_eq!(jobs.len(), 3);
        assert_eq!(jobs[0].name, "job1");
        assert_eq!(jobs[2].name, "job3");
    }

    #[test]
    fn detect_spec_file_by_extension() {
        let hit = detect_spec_file_in_trailing(&["workflow.yaml".into()]);
        assert_eq!(hit.as_deref(), Some("workflow.yaml"));
    }

    #[test]
    fn detect_spec_file_ignores_non_files() {
        let hit = detect_spec_file_in_trailing(&["hello world".into()]);
        assert!(hit.is_none());
    }

    #[test]
    fn detect_spec_file_ignores_non_spec_existing_files() {
        // A real file with a non-spec extension (e.g. `commands.txt`) should not
        // get redirected to `torc run`; the user probably meant `-C commands.txt`.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("commands.txt");
        std::fs::write(&path, "echo hi\n").unwrap();
        let hit = detect_spec_file_in_trailing(&[path.to_string_lossy().into_owned()]);
        assert!(
            hit.is_none(),
            "non-spec file should not be matched: {:?}",
            hit
        );
    }
}
