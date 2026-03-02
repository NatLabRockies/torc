use clap::{CommandFactory, Parser};

use torc::cli::{Cli, Commands};
use torc::client::apis::configuration::{Configuration, TlsConfig};
use torc::client::apis::default_api;
use torc::client::commands::access_groups::handle_access_group_commands;
use torc::client::commands::admin::handle_admin_commands;
use torc::client::commands::compute_nodes::handle_compute_node_commands;
use torc::client::commands::config::handle_config_commands;
use torc::client::commands::events::handle_event_commands;
use torc::client::commands::failure_handlers::handle_failure_handler_commands;
use torc::client::commands::files::handle_file_commands;
use torc::client::commands::hpc::handle_hpc_commands;
use torc::client::commands::job_dependencies::handle_job_dependency_commands;
use torc::client::commands::jobs::handle_job_commands;
use torc::client::commands::logs::handle_log_commands;
use torc::client::commands::recover::{
    RecoverArgs, RecoveryReport, diagnose_failures, recover_workflow,
};
use torc::client::commands::remote::handle_remote_commands;
use torc::client::commands::reports::handle_report_commands;
use torc::client::commands::resource_requirements::handle_resource_requirements_commands;
use torc::client::commands::results::handle_result_commands;
use torc::client::commands::scheduled_compute_nodes::handle_scheduled_compute_node_commands;
use torc::client::commands::slurm::handle_slurm_commands;
use torc::client::commands::user_data::handle_user_data_commands;
use torc::client::commands::watch::{WatchArgs, run_watch};
use torc::client::commands::workflows::handle_workflow_commands;
use torc::client::config::TorcConfig;
use torc::client::version_check;
use torc::client::workflow_manager::WorkflowManager;
use torc::client::workflow_spec::WorkflowSpec;

// Import the binary command modules from the library
use torc::plot_resources_cmd;
use torc::run_jobs_cmd;
use torc::tui_runner;

/// Helper to print a workflow message in the appropriate format (JSON or plain text).
fn print_workflow_message(format: &str, workflow_id: i64, message: &str) {
    if format == "json" {
        println!(
            "{}",
            serde_json::json!({"workflow_id": workflow_id, "message": message})
        );
    } else {
        println!("{}", message);
    }
}

/// Helper function to determine if a string is a file path or workflow ID
fn is_spec_file(arg: &str) -> bool {
    arg.ends_with(".yaml")
        || arg.ends_with(".yml")
        || arg.ends_with(".json")
        || arg.ends_with(".json5")
        || std::path::Path::new(arg).is_file()
}

fn main() {
    let cli = Cli::parse();

    // Load configuration from files (system, user, local) and environment variables
    // CLI arguments take precedence over file/env config
    let file_config = TorcConfig::load().unwrap_or_default();

    // Resolve log level with priority: CLI arg > file config > default
    let log_level = cli
        .log_level
        .clone()
        .unwrap_or_else(|| file_config.client.log_level.clone());

    // Initialize logger with CLI argument or RUST_LOG env var
    // Skip initialization for commands that set up their own logging (e.g., Run, Watch, Tui)
    // or output to stdout (e.g., Completions)
    let skip_logger_init = matches!(
        cli.command,
        Commands::Run { .. }
            | Commands::Watch { .. }
            | Commands::Tui(..)
            | Commands::Completions { .. }
    );

    if !skip_logger_init {
        env_logger::Builder::new().parse_filters(&log_level).init();
    }

    // Resolve format with priority: CLI arg (non-default) > file config > CLI default
    // Note: clap sets default to "table", so we check if user explicitly provided it
    let format = if cli.format != "table" {
        // User explicitly provided a format
        cli.format.clone()
    } else {
        // Use file config if available, otherwise CLI default
        file_config.client.format.clone()
    };

    // Validate format option for API commands
    if !matches!(format.as_str(), "table" | "json") {
        eprintln!("Error: format must be either 'table' or 'json'");
        std::process::exit(1);
    }

    // Resolve URL with priority: CLI arg > file config > default
    let url = cli
        .url
        .clone()
        .unwrap_or_else(|| file_config.client.api_url.clone());

    // Resolve TLS settings with priority: CLI arg > config file > defaults
    let tls_ca_cert = cli
        .tls_ca_cert
        .clone()
        .or_else(|| file_config.client.tls.ca_cert.clone());
    let tls_insecure = cli.tls_insecure || file_config.client.tls.insecure;
    let tls = TlsConfig {
        ca_cert_path: tls_ca_cert.as_ref().map(std::path::PathBuf::from),
        insecure: tls_insecure,
    };

    // Create configuration for API commands with TLS settings
    let mut config = Configuration::with_tls(tls);
    config.base_path = url.clone();

    // Handle authentication: use USER env var as username, password from CLI/env or prompt
    let password = if cli.prompt_password {
        // Prompt for password securely
        match rpassword::prompt_password("Password: ") {
            Ok(pwd) => Some(pwd),
            Err(e) => {
                eprintln!("Error reading password: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        cli.password.clone()
    };

    if let Some(password) = password {
        let username = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown".to_string());
        config.basic_auth = Some((username, Some(password)));
    }

    // Check server version for commands that communicate with the server
    // Skip for local-only commands or if --skip-version-check is set
    let requires_server = !matches!(
        cli.command,
        Commands::Completions { .. }
            | Commands::PlotResources(..)
            | Commands::Tui(..)
            | Commands::Config { .. }
            | Commands::Hpc { .. }
    );

    if requires_server && !cli.skip_version_check {
        let result = version_check::check_version(&config);
        if result.server_version.is_some() {
            let severity = version_check::print_version_warning(&result);
            if severity.is_blocking() {
                eprintln!("Use --skip-version-check to bypass this check (not recommended)");
                std::process::exit(1);
            }
        }
        // If server is unreachable, we'll let the actual command fail with a better error
    }

    match &cli.command {
        Commands::Run {
            workflow_spec_or_id,
            max_parallel_jobs,
            num_cpus,
            memory_gb,
            num_gpus,
            poll_interval,
            output_dir,
            skip_checks,
        } => {
            let workflow_id = if is_spec_file(workflow_spec_or_id) {
                // Create workflow from spec file
                let user = std::env::var("USER")
                    .or_else(|_| std::env::var("USERNAME"))
                    .unwrap_or_else(|_| "unknown".to_string());
                match WorkflowSpec::create_workflow_from_spec(
                    &config,
                    workflow_spec_or_id,
                    &user,
                    true,
                    *skip_checks,
                ) {
                    Ok(id) => {
                        print_workflow_message(&format, id, &format!("Created workflow {}", id));
                        id
                    }
                    Err(e) => {
                        eprintln!("Error creating workflow from spec: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                // Parse as workflow ID
                match workflow_spec_or_id.parse::<i64>() {
                    Ok(id) => id,
                    Err(_) => {
                        eprintln!(
                            "Error: '{}' is neither a valid workflow spec file nor a workflow ID",
                            workflow_spec_or_id
                        );
                        std::process::exit(1);
                    }
                }
            };

            // Build args for run_jobs_cmd with config file fallbacks
            let run_config = &file_config.client.run;
            // Pass through authentication from config
            let password = config.basic_auth.as_ref().and_then(|(_, p)| p.clone());
            let args = run_jobs_cmd::Args {
                workflow_id: Some(workflow_id),
                url: url.clone(),
                output_dir: output_dir
                    .clone()
                    .unwrap_or_else(|| run_config.output_dir.clone()),
                poll_interval: poll_interval.unwrap_or(run_config.poll_interval),
                max_parallel_jobs: max_parallel_jobs.or(run_config.max_parallel_jobs),
                time_limit: None,
                end_time: None,
                num_cpus: num_cpus.or(run_config.num_cpus),
                memory_gb: memory_gb.or(run_config.memory_gb),
                num_gpus: num_gpus.or(run_config.num_gpus),
                num_nodes: None,
                scheduler_config_id: None,
                log_prefix: None,
                cpu_affinity_cpus_per_job: None,
                log_level: log_level.clone(),
                password,
                tls_ca_cert: tls_ca_cert.clone(),
                tls_insecure,
            };

            run_jobs_cmd::run(&args);
        }
        Commands::Submit {
            workflow_spec_or_id,
            ignore_missing_data,
            skip_checks,
        } => {
            let workflow_id = if is_spec_file(workflow_spec_or_id) {
                // Load and validate spec file
                let spec = match WorkflowSpec::from_spec_file(workflow_spec_or_id) {
                    Ok(spec) => spec,
                    Err(e) => {
                        eprintln!("Error loading workflow spec: {}", e);
                        std::process::exit(1);
                    }
                };

                // Check if spec has schedule_nodes action
                if !spec.has_schedule_nodes_action() {
                    eprintln!("Error: Cannot submit workflow");
                    eprintln!();
                    eprintln!(
                        "The spec does not define an on_workflow_start action with schedule_nodes."
                    );
                    eprintln!("To submit to Slurm, either:");
                    eprintln!();
                    eprintln!("  1. Use 'torc submit-slurm' to auto-generate schedulers:");
                    eprintln!(
                        "     torc submit-slurm --account <account> {}",
                        workflow_spec_or_id
                    );
                    eprintln!();
                    eprintln!("  2. Add a workflow action manually:");
                    eprintln!("     actions:");
                    eprintln!("       - trigger_type: on_workflow_start");
                    eprintln!("         action_type: schedule_nodes");
                    eprintln!("         scheduler_type: slurm");
                    eprintln!("         scheduler: \"my-scheduler\"");
                    eprintln!();
                    eprintln!("Or run locally instead:");
                    eprintln!("  torc run {}", workflow_spec_or_id);
                    std::process::exit(1);
                }

                // Create workflow from spec
                let user = std::env::var("USER")
                    .or_else(|_| std::env::var("USERNAME"))
                    .unwrap_or_else(|_| "unknown".to_string());

                match WorkflowSpec::create_workflow_from_spec(
                    &config,
                    workflow_spec_or_id,
                    &user,
                    true,
                    *skip_checks,
                ) {
                    Ok(id) => {
                        print_workflow_message(&format, id, &format!("Created workflow {}", id));
                        id
                    }
                    Err(e) => {
                        eprintln!("Error creating workflow from spec: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                // Parse as workflow ID
                match workflow_spec_or_id.parse::<i64>() {
                    Ok(id) => id,
                    Err(_) => {
                        eprintln!(
                            "Error: '{}' is neither a valid workflow spec file nor a workflow ID",
                            workflow_spec_or_id
                        );
                        std::process::exit(1);
                    }
                }
            };

            // Check if workflow has schedule_nodes actions (for existing workflows)
            if !is_spec_file(workflow_spec_or_id) {
                match default_api::get_workflow_actions(&config, workflow_id) {
                    Ok(actions) => {
                        let has_schedule_nodes = actions.iter().any(|action| {
                            action.trigger_type == "on_workflow_start"
                                && action.action_type == "schedule_nodes"
                        });

                        if !has_schedule_nodes {
                            eprintln!("Error: Cannot submit workflow {}", workflow_id);
                            eprintln!();
                            eprintln!(
                                "The workflow does not define an on_workflow_start action with schedule_nodes."
                            );
                            eprintln!(
                                "To submit to a scheduler, the workflow must have an action configured."
                            );
                            eprintln!();
                            eprintln!("Or run locally instead:");
                            eprintln!("  torc run {}", workflow_id);
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error getting workflow actions: {}", e);
                        std::process::exit(1);
                    }
                }
            }

            // Submit the workflow
            match default_api::get_workflow(&config, workflow_id) {
                Ok(workflow) => {
                    let torc_config = TorcConfig::load().unwrap_or_default();
                    let workflow_manager =
                        WorkflowManager::new(config.clone(), torc_config, workflow);
                    match workflow_manager.start(*ignore_missing_data) {
                        Ok(()) => {
                            print_workflow_message(
                                &format,
                                workflow_id,
                                &format!("Successfully submitted workflow {}", workflow_id),
                            );
                        }
                        Err(e) => {
                            eprintln!("Error submitting workflow {}: {}", workflow_id, e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error getting workflow {}: {}", workflow_id, e);
                    std::process::exit(1);
                }
            }
        }
        Commands::SubmitSlurm {
            workflow_spec,
            account,
            hpc_profile,
            single_allocation,
            group_by,
            ignore_missing_data,
            skip_checks,
            overwrite,
        } => {
            use torc::client::commands::slurm::{
                WalltimeStrategy, generate_schedulers_for_workflow,
            };

            // Load the workflow spec
            let mut spec = match WorkflowSpec::from_spec_file(workflow_spec) {
                Ok(spec) => spec,
                Err(e) => {
                    eprintln!("Error loading workflow spec: {}", e);
                    std::process::exit(1);
                }
            };

            // Resolve account: CLI option takes precedence, then slurm_defaults
            let resolved_account = if let Some(acct) = account {
                acct.clone()
            } else if let Some(ref defaults) = spec.slurm_defaults {
                defaults
                    .0
                    .get("account")
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_else(|| {
                        eprintln!(
                            "Error: No account specified. Use --account or set 'account' in slurm_defaults."
                        );
                        std::process::exit(1);
                    })
            } else {
                eprintln!(
                    "Error: No account specified. Use --account or set 'account' in slurm_defaults."
                );
                std::process::exit(1);
            };

            // Get HPC profile
            let torc_config = TorcConfig::load().unwrap_or_default();
            let registry = torc::client::commands::hpc::create_registry_with_config_public(
                &torc_config.client.hpc,
            );

            let profile = if let Some(name) = hpc_profile {
                registry.get(name)
            } else {
                registry.detect()
            };

            let profile = match profile {
                Some(p) => p,
                None => {
                    if hpc_profile.is_some() {
                        eprintln!("Unknown HPC profile: {}", hpc_profile.as_ref().unwrap());
                    } else {
                        eprintln!("No HPC profile specified and no system detected.");
                        eprintln!("Use --hpc-profile <name> to specify a profile.");
                    }
                    std::process::exit(1);
                }
            };

            // Generate schedulers
            match generate_schedulers_for_workflow(
                &mut spec,
                profile,
                &resolved_account,
                *single_allocation,
                *group_by,
                WalltimeStrategy::MaxJobRuntime,
                1.5, // Default walltime multiplier
                true,
                *overwrite,
            ) {
                Ok(result) => {
                    eprintln!(
                        "Auto-generated {} scheduler(s) and {} action(s) using {} profile",
                        result.scheduler_count, result.action_count, profile.name
                    );
                    for warning in &result.warnings {
                        eprintln!("  Warning: {}", warning);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }

            // Print warning about auto-generated configuration
            eprintln!();
            eprintln!("WARNING: Schedulers and actions were auto-generated using heuristics.");
            eprintln!("         For complex workflows, this may not be optimal.");
            eprintln!();
            eprintln!("TIP: To preview and validate the configuration before submitting, use:");
            eprintln!(
                "     torc slurm generate --account {} {}",
                resolved_account, workflow_spec
            );
            eprintln!();

            // Write modified spec to temp file
            let temp_dir = std::env::temp_dir();
            let temp_file =
                temp_dir.join(format!("torc_submit_workflow_{}.yaml", std::process::id()));
            std::fs::write(&temp_file, serde_yaml::to_string(&spec).unwrap())
                .expect("Failed to write temporary workflow file");

            // Create workflow from spec
            let user = std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .unwrap_or_else(|_| "unknown".to_string());

            let workflow_id = match WorkflowSpec::create_workflow_from_spec(
                &config,
                &temp_file,
                &user,
                true,
                *skip_checks,
            ) {
                Ok(id) => {
                    print_workflow_message(&format, id, &format!("Created workflow {}", id));
                    id
                }
                Err(e) => {
                    eprintln!("Error creating workflow from spec: {}", e);
                    std::process::exit(1);
                }
            };

            // Submit the workflow
            match default_api::get_workflow(&config, workflow_id) {
                Ok(workflow) => {
                    let workflow_manager =
                        WorkflowManager::new(config.clone(), torc_config, workflow);
                    match workflow_manager.start(*ignore_missing_data) {
                        Ok(()) => {
                            print_workflow_message(
                                &format,
                                workflow_id,
                                &format!("Successfully submitted workflow {}", workflow_id),
                            );
                        }
                        Err(e) => {
                            eprintln!("Error submitting workflow {}: {}", workflow_id, e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error getting workflow {}: {}", workflow_id, e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Watch {
            workflow_id,
            poll_interval,
            recover,
            max_retries,
            memory_multiplier,
            runtime_multiplier,
            retry_unknown,
            recovery_hook,
            output_dir,
            show_job_counts,
            auto_schedule,
            auto_schedule_threshold,
            auto_schedule_cooldown,
            auto_schedule_stranded_timeout,
            ai_recovery,
            ai_agent,
        } => {
            let args = WatchArgs {
                workflow_id: *workflow_id,
                poll_interval: *poll_interval,
                recover: *recover,
                max_retries: *max_retries,
                memory_multiplier: *memory_multiplier,
                runtime_multiplier: *runtime_multiplier,
                retry_unknown: *retry_unknown,
                recovery_hook: recovery_hook.clone(),
                output_dir: output_dir.clone(),
                show_job_counts: *show_job_counts,
                log_level: log_level.clone(),
                auto_schedule: *auto_schedule,
                auto_schedule_threshold: *auto_schedule_threshold,
                auto_schedule_cooldown: *auto_schedule_cooldown,
                auto_schedule_stranded_timeout: *auto_schedule_stranded_timeout,
                ai_recovery: *ai_recovery,
                ai_agent: ai_agent.clone(),
            };
            run_watch(&config, &args);
        }
        Commands::Recover {
            workflow_id,
            output_dir,
            memory_multiplier,
            runtime_multiplier,
            retry_unknown,
            recovery_hook,
            dry_run,
            ai_recovery,
            ai_agent,
        } => {
            let args = RecoverArgs {
                workflow_id: *workflow_id,
                output_dir: output_dir.clone(),
                memory_multiplier: *memory_multiplier,
                runtime_multiplier: *runtime_multiplier,
                retry_unknown: *retry_unknown,
                recovery_hook: recovery_hook.clone(),
                dry_run: *dry_run,
                ai_recovery: *ai_recovery,
                ai_agent: ai_agent.clone(),
            };

            // For JSON output, get diagnosis data to include in the report
            let diagnosis = if format == "json" {
                diagnose_failures(*workflow_id, output_dir).ok()
            } else {
                None
            };

            match recover_workflow(&config, &args) {
                Ok(result) => {
                    if format == "json" {
                        // Output structured JSON report
                        let report = RecoveryReport {
                            workflow_id: *workflow_id,
                            dry_run: *dry_run,
                            memory_multiplier: *memory_multiplier,
                            runtime_multiplier: *runtime_multiplier,
                            result,
                            diagnosis,
                        };
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&report).unwrap_or_else(|e| {
                                format!("{{\"error\": \"Failed to serialize: {}\"}}", e)
                            })
                        );
                    } else if *dry_run {
                        println!("[DRY RUN] Summary for workflow {}", workflow_id);
                        if result.oom_fixed > 0 {
                            println!(
                                "  - {} job(s) would have memory increased",
                                result.oom_fixed
                            );
                        }
                        if result.timeout_fixed > 0 {
                            println!(
                                "  - {} job(s) would have runtime increased",
                                result.timeout_fixed
                            );
                        }
                        if result.unknown_retried > 0 {
                            println!(
                                "  - {} job(s) with unknown failures would be reset",
                                result.unknown_retried
                            );
                        }
                        if result.jobs_to_retry.is_empty() {
                            println!("No recoverable jobs found.");
                        } else {
                            println!(
                                "Would reset {} job(s) and regenerate Slurm schedulers.",
                                result.jobs_to_retry.len()
                            );
                        }
                        println!("\nRun without --dry-run to apply these changes.");
                    } else {
                        println!("Recovery complete for workflow {}", workflow_id);
                        if result.oom_fixed > 0 {
                            println!("  - {} job(s) had memory increased", result.oom_fixed);
                        }
                        if result.timeout_fixed > 0 {
                            println!("  - {} job(s) had runtime increased", result.timeout_fixed);
                        }
                        if result.unknown_retried > 0 {
                            println!(
                                "  - {} job(s) with unknown failures reset",
                                result.unknown_retried
                            );
                        }
                        if result.jobs_to_retry.is_empty() {
                            println!("No recoverable jobs found.");
                        } else {
                            println!(
                                "Reset {} job(s). Slurm schedulers regenerated and submitted.",
                                result.jobs_to_retry.len()
                            );
                        }
                    }
                }
                Err(e) => {
                    if format == "json" {
                        println!(
                            "{}",
                            serde_json::json!({
                                "error": e,
                                "workflow_id": workflow_id,
                                "dry_run": dry_run,
                            })
                        );
                        std::process::exit(1);
                    } else {
                        eprintln!("Recovery failed: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        Commands::Workflows { command } => {
            handle_workflow_commands(&config, command, &format);
        }
        Commands::ComputeNodes { command } => {
            handle_compute_node_commands(&config, command, &format);
        }
        Commands::Files { command } => {
            handle_file_commands(&config, command, &format);
        }
        Commands::Jobs { command } => {
            handle_job_commands(&config, command, &format);
        }
        Commands::JobDependencies { command } => {
            handle_job_dependency_commands(command, &config, &format);
        }
        Commands::ResourceRequirements { command } => {
            handle_resource_requirements_commands(&config, command, &format);
        }
        Commands::FailureHandlers { command } => {
            handle_failure_handler_commands(&config, command, &format);
        }
        Commands::Events { command } => {
            handle_event_commands(&config, command, &format);
        }
        Commands::Results { command } => {
            handle_result_commands(&config, command, &format);
        }
        Commands::UserData { command } => {
            handle_user_data_commands(&config, command, &format);
        }
        Commands::Slurm { command } => {
            handle_slurm_commands(&config, command, &format);
        }
        Commands::Remote { command } => {
            handle_remote_commands(&config, command);
        }
        Commands::ScheduledComputeNodes { command } => {
            handle_scheduled_compute_node_commands(&config, command, &format);
        }
        Commands::Hpc { command } => {
            handle_hpc_commands(command, &format);
        }
        Commands::Reports { command } => {
            handle_report_commands(&config, command, &format);
        }
        Commands::Logs { command } => {
            handle_log_commands(&config, command);
        }
        Commands::AccessGroups { command } => {
            handle_access_group_commands(&config, command, &format);
        }
        Commands::Admin { command } => {
            handle_admin_commands(&config, command, &format);
        }
        Commands::Config { command } => {
            handle_config_commands(command);
        }
        Commands::Tui(args) => {
            let basic_auth = config.basic_auth.clone();
            if let Err(e) = tui_runner::run(args, basic_auth) {
                eprintln!("Error running TUI: {}", e);
                std::process::exit(1);
            }
        }
        Commands::PlotResources(args) => {
            if let Err(e) = plot_resources_cmd::run(args) {
                eprintln!("Error generating plots: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Ping => match default_api::ping(&config) {
            Ok(_) => {
                if cli.format == "json" {
                    println!(r#"{{"status": "Server is running"}}"#);
                } else {
                    println!("Server is running");
                }
            }
            Err(e) => {
                if cli.format == "json" {
                    println!(
                        r#"{{"status": "error", "message": "{}"}}"#,
                        e.to_string().replace('"', "\\\"")
                    );
                } else {
                    eprintln!("Failed to connect to server: {}", e);
                }
                std::process::exit(1);
            }
        },
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(*shell, &mut cmd, "torc", &mut std::io::stdout());
        }
    }
}
