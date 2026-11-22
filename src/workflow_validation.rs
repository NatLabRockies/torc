//! Workflow checks shared by the client (`torc create`, `torc create --dry-run`) and the
//! server's `finalize_workflow` endpoint.
//!
//! The checks run on a [`WorkflowCheckInput`], which the client builds from a parsed spec and
//! the server builds from database rows, so both sides apply the same rules. Spec parsing,
//! parameter expansion, and name resolution stay in the client.

use crate::models::{ExecutionConfig, ExecutionMode};

/// A Slurm scheduler, as far as the checks need it.
#[derive(Clone, Debug, Default)]
pub struct SchedulerInfo {
    pub name: String,
    pub nodes: i64,
    pub walltime: String,
    pub mem: Option<String>,
    pub gres: Option<String>,
}

/// A resource requirements record, as far as the checks need it.
#[derive(Clone, Debug, Default)]
pub struct ResourceInfo {
    pub name: String,
    pub num_nodes: i64,
    pub num_gpus: i64,
    pub memory: String,
    pub runtime: String,
}

/// A job. References are indexes into [`WorkflowCheckInput`]'s vectors.
#[derive(Clone, Debug, Default)]
pub struct JobInfo {
    pub name: String,
    pub resource_requirements: Option<usize>,
    pub scheduler: Option<usize>,
}

/// A workflow action. `scheduler` is an index into [`WorkflowCheckInput::schedulers`].
#[derive(Clone, Debug, Default)]
pub struct ActionInfo {
    pub action_type: String,
    pub scheduler_type: Option<String>,
    pub scheduler: Option<usize>,
    pub start_one_worker_per_node: bool,
}

/// Everything the checks look at.
#[derive(Clone, Debug, Default)]
pub struct WorkflowCheckInput {
    pub execution_config: Option<ExecutionConfig>,
    pub schedulers: Vec<SchedulerInfo>,
    pub resource_requirements: Vec<ResourceInfo>,
    pub jobs: Vec<JobInfo>,
    pub actions: Vec<ActionInfo>,
}

/// Errors block creation; warnings block it unless the caller forces it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkflowCheckFindings {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Run every check.
pub fn check_workflow(input: &WorkflowCheckInput) -> WorkflowCheckFindings {
    let has_worker_per_node_action = input
        .actions
        .iter()
        .any(|a| a.action_type == "schedule_nodes" && a.start_one_worker_per_node);
    let mut errors = execution_config_errors(
        input.execution_config.as_ref(),
        !input.schedulers.is_empty(),
        has_worker_per_node_action,
    );
    errors.extend(scheduler_node_errors(input));
    WorkflowCheckFindings {
        errors,
        warnings: scheduler_resource_warnings(input),
    }
}

/// Check that `execution_config` fields match the effective execution mode, and that
/// `start_one_worker_per_node` is only used in direct mode. For mode `auto`, the mode is slurm
/// when the workflow has Slurm schedulers.
pub fn execution_config_errors(
    execution_config: Option<&ExecutionConfig>,
    has_slurm_schedulers: bool,
    has_worker_per_node_action: bool,
) -> Vec<String> {
    let mut errors = Vec::new();
    if let Some(ec) = execution_config {
        let will_use_slurm = match ec.mode {
            ExecutionMode::Slurm => true,
            ExecutionMode::Auto => has_slurm_schedulers,
            ExecutionMode::Direct => false,
        };

        if let Some(value) = ec.srun_mpi.as_deref() {
            if let Err(err) = validate_srun_mpi_value(value) {
                errors.push(err);
            }
            if !has_worker_per_node_action {
                errors.push(
                    "srun_mpi requires schedule_nodes.start_one_worker_per_node = true. \
                    It only applies to the outer srun that launches one job runner per node."
                        .to_string(),
                );
            }
        }

        if will_use_slurm {
            if ec.limit_resources == Some(false) {
                errors.push(
                    "limit_resources: false is only supported in direct mode. \
                    Slurm mode requires resource limits for correct srun behavior."
                        .to_string(),
                );
            }
            if ec.termination_signal.is_some() {
                errors.push(
                    "termination_signal is only supported in direct mode. \
                    In slurm mode, use srun_termination_signal instead."
                        .to_string(),
                );
            }
            if ec.sigterm_lead_seconds.is_some() {
                errors.push(
                    "sigterm_lead_seconds is only supported in direct mode. \
                    In slurm mode, termination timing is controlled by \
                    srun_termination_signal."
                        .to_string(),
                );
            }
            if ec.oom_exit_code.is_some() {
                errors.push(
                    "oom_exit_code is only supported in direct mode. \
                    In slurm mode, Slurm manages OOM detection."
                        .to_string(),
                );
            }
        } else {
            if ec.srun_termination_signal.is_some() {
                errors.push(
                    "srun_termination_signal is only supported in slurm mode. \
                    In direct mode, use termination_signal instead."
                        .to_string(),
                );
            }
            if ec.enable_cpu_bind == Some(true) {
                errors.push(
                    "enable_cpu_bind is only supported in slurm mode. \
                    It has no effect in direct mode."
                        .to_string(),
                );
            }
        }
    }

    let mode = execution_config
        .map(|ec| &ec.mode)
        .unwrap_or(&ExecutionMode::Direct);
    if has_worker_per_node_action && *mode != ExecutionMode::Direct {
        errors
            .push("start_one_worker_per_node requires execution_config.mode to be 'direct'".into());
    }
    errors
}

/// Check that multi-node jobs bound to a multi-node Slurm scheduler request the same node count.
///
/// Two allocation patterns are valid: single-node jobs (`num_nodes` 1) placed on the nodes of a
/// multi-node allocation, and true multi-node jobs whose `num_nodes` equals the scheduler's
/// `nodes`. Jobs that request a different multi-node count are rejected.
pub fn scheduler_node_errors(input: &WorkflowCheckInput) -> Vec<String> {
    let mut errors = Vec::new();
    for action in &input.actions {
        if action.action_type != "schedule_nodes"
            || action.scheduler_type.as_deref() != Some("slurm")
        {
            continue;
        }
        let Some(sched_idx) = action.scheduler else {
            continue;
        };
        let scheduler = &input.schedulers[sched_idx];
        if scheduler.nodes <= 1 {
            continue;
        }
        let mismatched_jobs: Vec<&str> = input
            .jobs
            .iter()
            .filter(|job| job.scheduler == Some(sched_idx))
            .filter(|job| {
                let job_num_nodes = job
                    .resource_requirements
                    .map(|i| input.resource_requirements[i].num_nodes)
                    .unwrap_or(1);
                job_num_nodes > 1 && job_num_nodes != scheduler.nodes
            })
            .map(|job| job.name.as_str())
            .collect();
        if !mismatched_jobs.is_empty() {
            errors.push(format!(
                "Scheduler '{}' allocates {} nodes but jobs ({}) request a different \
                 multi-node count in their resource requirements. Set num_nodes={} \
                 on job resource requirements to match the scheduler, or use \
                 num_nodes=1 for single-node jobs.",
                scheduler.name,
                scheduler.nodes,
                mismatched_jobs.join(", "),
                scheduler.nodes,
            ));
        }
    }
    errors
}

/// Check that job resource requirements (runtime, memory, GPUs) fit the Slurm schedulers.
///
/// A job with a scheduler is checked against that scheduler in each dimension. A job without
/// one needs at least one scheduler that fits it in every dimension. Scheduler fields that are
/// not set (`mem`, `gres`) are not checked.
pub fn scheduler_resource_warnings(input: &WorkflowCheckInput) -> Vec<String> {
    if input.resource_requirements.is_empty() || input.schedulers.is_empty() {
        return Vec::new();
    }

    struct ParsedScheduler {
        walltime_secs: Option<u64>,
        memory_bytes: Option<i64>,
        gpu_count: Option<u32>,
    }

    let mut warnings = Vec::new();
    let parsed: Vec<ParsedScheduler> = input
        .schedulers
        .iter()
        .map(|sched| {
            let walltime_secs = match parse_walltime_secs(&sched.walltime) {
                Ok(v) => Some(v),
                Err(e) => {
                    warnings.push(format!(
                        "Scheduler '{}': invalid walltime '{}': {}",
                        sched.name, sched.walltime, e,
                    ));
                    None
                }
            };
            let memory_bytes = match sched.mem.as_ref() {
                Some(m) => match crate::memory_utils::memory_string_to_bytes(m) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        warnings.push(format!(
                            "Scheduler '{}': invalid memory '{}': {}",
                            sched.name, m, e,
                        ));
                        None
                    }
                },
                None => None,
            };
            // Non-empty but unparseable gres counts as 0 GPUs so the check isn't bypassed.
            let gpu_count = sched
                .gres
                .as_ref()
                .filter(|g| !g.trim().is_empty())
                .map(|_| parse_gres(&sched.gres).0.unwrap_or(0));
            ParsedScheduler {
                walltime_secs,
                memory_bytes,
                gpu_count,
            }
        })
        .collect();

    for job in &input.jobs {
        let Some(rr) = job
            .resource_requirements
            .map(|i| &input.resource_requirements[i])
        else {
            continue;
        };
        let job_runtime_secs = match crate::time_utils::duration_string_to_seconds(&rr.runtime) {
            Ok(secs) => Some(secs as u64),
            Err(e) => {
                warnings.push(format!(
                    "Job '{}': invalid runtime '{}': {}",
                    job.name, rr.runtime, e,
                ));
                None
            }
        };
        let job_memory_bytes = match crate::memory_utils::memory_string_to_bytes(&rr.memory) {
            Ok(bytes) => Some(bytes),
            Err(e) => {
                warnings.push(format!(
                    "Job '{}': invalid memory '{}': {}",
                    job.name, rr.memory, e,
                ));
                None
            }
        };
        let job_gpus = rr.num_gpus;
        if job_gpus < 0 {
            warnings.push(format!(
                "Job '{}': invalid negative num_gpus {}",
                job.name, job_gpus,
            ));
            continue;
        }

        // (runtime exceeded as (runtime s, walltime s), memory exceeded, GPUs exceeded)
        let mismatches = |ps: &ParsedScheduler| {
            let runtime = match (job_runtime_secs, ps.walltime_secs) {
                (Some(rt), Some(wt)) if rt > wt => Some((rt, wt)),
                _ => None,
            };
            let mem =
                matches!((job_memory_bytes, ps.memory_bytes), (Some(jm), Some(sm)) if jm > sm);
            let gpu = matches!(ps.gpu_count, Some(sg) if job_gpus > sg as i64);
            (runtime, mem, gpu)
        };

        if let Some(sched_idx) = job.scheduler {
            let sched = &input.schedulers[sched_idx];
            let (runtime, mem, gpu) = mismatches(&parsed[sched_idx]);
            if let Some((rt, wt)) = runtime {
                warnings.push(format!(
                    "Job '{}': runtime '{}' ({} s) exceeds scheduler '{}' walltime '{}' ({} s)",
                    job.name, rr.runtime, rt, sched.name, sched.walltime, wt,
                ));
            }
            if mem {
                warnings.push(format!(
                    "Job '{}': memory '{}' exceeds scheduler '{}' mem '{}'",
                    job.name,
                    rr.memory,
                    sched.name,
                    sched.mem.as_deref().unwrap_or("?"),
                ));
            }
            if gpu {
                warnings.push(format!(
                    "Job '{}': num_gpus {} exceeds scheduler '{}' gres '{}'",
                    job.name,
                    job_gpus,
                    sched.name,
                    sched.gres.as_deref().unwrap_or("?"),
                ));
            }
            continue;
        }

        let all: Vec<_> = parsed.iter().map(mismatches).collect();
        if all.contains(&(None, false, false)) {
            continue;
        }
        let reasons: Vec<String> = all
            .iter()
            .zip(&input.schedulers)
            .filter(|(m, _)| **m != (None, false, false))
            .map(|((runtime, mem, gpu), sched)| {
                let mut parts = Vec::new();
                if let Some((rt, wt)) = runtime {
                    parts.push(format!("runtime {} s > walltime {} s", rt, wt));
                }
                if *mem {
                    parts.push(format!(
                        "memory '{}' > mem '{}'",
                        rr.memory,
                        sched.mem.as_deref().unwrap_or("?"),
                    ));
                }
                if *gpu {
                    parts.push(format!(
                        "num_gpus {} > gres '{}'",
                        job_gpus,
                        sched.gres.as_deref().unwrap_or("?"),
                    ));
                }
                format!("'{}': {}", sched.name, parts.join(", "))
            })
            .collect();
        warnings.push(format!(
            "Job '{}' has no explicit scheduler and no scheduler can \
             accommodate its resource requirements. Mismatches: [{}]",
            job.name,
            reasons.join("; "),
        ));
    }
    warnings
}

/// Validate an `execution_config.srun_mpi` value: a single token of `[A-Za-z0-9+_.-]`.
pub fn validate_srun_mpi_value(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("srun_mpi must not be empty when provided. \
             Set a non-empty Slurm MPI mode such as 'pmix' or omit the field."
            .to_string());
    }
    let safe = |c: char| c.is_ascii_alphanumeric() || matches!(c, '+' | '_' | '.' | '-');
    if trimmed != value || !trimmed.chars().all(safe) {
        return Err(
            "srun_mpi must be a single safe token matching [A-Za-z0-9+_.-]+ \
             (for example 'none' or 'pmix')."
                .to_string(),
        );
    }
    Ok(())
}

/// Parse a Slurm walltime into seconds.
///
/// Supported formats:
/// - `MM` (minutes only, e.g., "30")
/// - `MM:SS` (e.g., "30:00")
/// - `HH:MM:SS` (e.g., "04:00:00")
/// - `D-HH:MM:SS` (e.g., "1-00:00:00")
pub fn parse_walltime_secs(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if let Some((days_str, rest)) = s.split_once('-') {
        let days: u64 = days_str
            .parse()
            .map_err(|_| format!("Invalid days: {}", days_str))?;
        return Ok(days * 24 * 3600 + parse_hms(rest)?);
    }
    parse_hms(s)
}

fn parse_hms(s: &str) -> Result<u64, String> {
    let parts: Vec<&str> = s.split(':').collect();
    let num = |p: &str, what: &str| -> Result<u64, String> {
        p.parse().map_err(|_| format!("Invalid {}: {}", what, p))
    };
    match parts.as_slice() {
        // Minutes only (Slurm convention)
        [m] => Ok(num(m, "minutes")? * 60),
        // MM:SS (Slurm convention)
        [m, sec] => Ok(num(m, "minutes")? * 60 + num(sec, "seconds")?),
        [h, m, sec] => Ok(num(h, "hours")? * 3600 + num(m, "minutes")? * 60 + num(sec, "seconds")?),
        _ => Err(format!("Invalid time format: {}", s)),
    }
}

/// Parse a GRES string and return the GPU count and type of its `gpu` entry.
pub fn parse_gres(gres: &Option<String>) -> (Option<u32>, Option<String>) {
    let Some(gres) = gres else {
        return (None, None);
    };
    // Entries are comma-separated; strip socket info like "(S:0-3)" before parsing.
    for entry in gres.split(',') {
        let entry = entry.split('(').next().unwrap_or(entry);
        let parts: Vec<&str> = entry.split(':').collect();
        match parts.as_slice() {
            ["gpu", count] => {
                let count: u32 = count.parse().unwrap_or(0);
                if count > 0 {
                    return (Some(count), None);
                }
            }
            ["gpu", gpu_type, count] => {
                let count: u32 = count.parse().unwrap_or(0);
                if count > 0 {
                    return (Some(count), Some(gpu_type.to_string()));
                }
            }
            _ => {}
        }
    }
    (None, None)
}
