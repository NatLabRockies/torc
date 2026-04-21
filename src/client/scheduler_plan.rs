//! Scheduler plan generation for Slurm workflows.
//!
//! This module provides a common abstraction for generating Slurm scheduler
//! configurations. It extracts the core logic that is shared between:
//! - `generate_schedulers_for_workflow` (for new workflows from specs)
//! - `handle_regenerate` (for existing workflows from database)
//!
//! The plan can then be applied to either a WorkflowSpec or to the database via API.

use std::collections::{HashMap, HashSet};

use log::warn;
use serde::{Deserialize, Serialize};

/// Threshold for auto-merging deferred and non-deferred scheduler groups.
///
/// When using `--group-by partition`, if a partition has both deferred (jobs with dependencies)
/// and non-deferred (jobs without dependencies) groups, and their combined allocation count
/// is at or below this threshold, they are merged into a single scheduler with an
/// `on_workflow_start` trigger. This reduces the number of Slurm job submissions.
///
/// When both groups exist, each needs at least 1 allocation, so the minimum total is 2.
/// A threshold of 2 means we merge when exactly 2 allocations are needed (the minimum case).
pub const MERGE_THRESHOLD: i64 = 2;

use crate::client::hpc::HpcProfile;
use crate::client::workflow_graph::{SchedulerGroup, WorkflowGraph};
use crate::time_utils::duration_string_to_seconds;

use super::commands::slurm::{
    GroupByStrategy, WalltimeStrategy, parse_memory_mb, secs_to_walltime,
};
use crate::client::hpc::HpcPartition;

/// Optional overrides for partition and walltime selection.
///
/// When set, these bypass the automatic size-fitting for partition and/or walltime,
/// using the user-specified values instead. Node count calculation still happens
/// dynamically based on the number of pending jobs.
#[derive(Debug, Clone, Default)]
pub struct SchedulerOverrides {
    /// Fixed partition name (bypasses `find_best_partition`)
    pub partition: Option<String>,
    /// Fixed walltime in seconds (bypasses `calculate_walltime`)
    pub walltime_secs: Option<u64>,
}

/// Parameters for calculating the number of allocations needed for a group of jobs.
struct AllocationParams {
    /// Maximum CPUs required by any job in the group
    max_cpus: u32,
    /// Maximum memory (MB) required by any job in the group
    max_memory_mb: u64,
    /// Maximum runtime (seconds) of any job in the group
    max_runtime_secs: u64,
    /// Maximum GPUs required by any job in the group (0 if none)
    max_gpus: u32,
    /// Number of nodes required per job
    nodes_per_job: u32,
    /// Total number of jobs in the group
    job_count: usize,
    /// Actual walltime (seconds) that will be assigned to each allocation.
    /// Used to calculate how many sequential job batches fit within an allocation.
    allocation_walltime_secs: u64,
}

/// Calculate the number of allocations needed for a group of jobs.
///
/// This calculation considers:
/// - Concurrent job capacity per node (based on CPUs, memory, GPUs)
/// - Sequential job capacity over the walltime (based on job runtime vs partition walltime)
///
/// Returns `None` if the parameters are invalid (e.g., zero CPUs or memory).
fn calculate_allocations(
    params: &AllocationParams,
    partition: &HpcPartition,
    single_allocation: bool,
) -> Option<i64> {
    // Guard against division by zero
    if params.max_cpus == 0 || params.max_memory_mb == 0 {
        return None;
    }

    // Calculate concurrent jobs per node based on resources
    let jobs_per_node_by_cpu = partition.cpus_per_node / params.max_cpus;
    let jobs_per_node_by_mem = (partition.memory_mb / params.max_memory_mb) as u32;
    let jobs_per_node_by_gpu = match (params.max_gpus, partition.gpus_per_node) {
        (job_gpus, Some(node_gpus)) if job_gpus > 0 => node_gpus / job_gpus,
        _ => u32::MAX,
    };
    let concurrent_jobs_per_node = std::cmp::max(
        1,
        std::cmp::min(
            jobs_per_node_by_cpu,
            std::cmp::min(jobs_per_node_by_mem, jobs_per_node_by_gpu),
        ),
    );

    // Factor in runtime: how many sequential batches can run within the allocation walltime
    let time_slots = params
        .allocation_walltime_secs
        .checked_div(params.max_runtime_secs)
        .map_or(1, |slots| std::cmp::max(1, slots));

    // Total jobs per allocation = concurrent capacity × time slots
    let jobs_per_allocation = (concurrent_jobs_per_node as u64) * time_slots;

    let total_nodes =
        (params.job_count as u64).div_ceil(jobs_per_allocation) * (params.nodes_per_job as u64);
    let total_nodes = std::cmp::max(1, total_nodes) as i64;

    if single_allocation {
        Some(1)
    } else {
        Some(total_nodes)
    }
}

/// Calculate walltime based on the selected strategy.
///
/// # Arguments
/// * `max_job_runtime_secs` - Maximum runtime of any job in the group
/// * `partition_max_walltime_secs` - Maximum walltime allowed by the partition
/// * `strategy` - Walltime calculation strategy
/// * `multiplier` - Multiplier for job runtime (only used with MaxJobRuntime strategy)
///
/// # Returns
/// The calculated walltime in seconds, capped at the partition maximum.
fn calculate_walltime(
    max_job_runtime_secs: u64,
    partition_max_walltime_secs: u64,
    strategy: WalltimeStrategy,
    multiplier: f64,
) -> u64 {
    match strategy {
        WalltimeStrategy::MaxPartitionTime => partition_max_walltime_secs,
        WalltimeStrategy::MaxJobRuntime => {
            // If runtime is 0 or not specified, fall back to partition max
            if max_job_runtime_secs == 0 {
                return partition_max_walltime_secs;
            }

            // Apply multiplier and cap at partition max
            let scaled_runtime = (max_job_runtime_secs as f64 * multiplier).ceil() as u64;
            std::cmp::min(scaled_runtime, partition_max_walltime_secs)
        }
    }
}

/// A planned Slurm scheduler configuration.
///
/// This is an intermediate representation that can be converted to either
/// a `SlurmSchedulerSpec` (for workflow specs) or a `SlurmSchedulerModel` (for database).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedScheduler {
    /// Scheduler name (includes suffix like "_deferred" for jobs with dependencies)
    pub name: String,
    /// Slurm account
    pub account: String,
    /// Partition (if explicit request required for sbatch --partition flag)
    pub partition: Option<String>,
    /// Resolved partition name (always set, used for cluster state queries and display)
    pub resolved_partition: String,
    /// Memory request
    pub mem: Option<String>,
    /// Walltime in HH:MM:SS format
    pub walltime: String,
    /// Nodes per allocation
    pub nodes: i64,
    /// GPU gres string (e.g., "gpu:2")
    pub gres: Option<String>,
    /// QOS
    pub qos: Option<String>,
    /// Resource requirements name this scheduler is for
    pub resource_requirements: String,
    /// Whether this scheduler is for jobs with dependencies
    pub has_dependencies: bool,
    /// Number of jobs this scheduler will handle
    pub job_count: usize,
    /// Job names that will use this scheduler
    pub job_names: Vec<String>,
    /// Job name patterns for action matching
    pub job_name_patterns: Vec<String>,
    /// Number of allocations to create
    pub num_allocations: i64,
}

/// A planned workflow action for scheduling nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedAction {
    /// Trigger type: "on_workflow_start" or "on_jobs_ready"
    pub trigger_type: String,
    /// Scheduler name this action references
    pub scheduler_name: String,
    /// Exact job names for this action (preferred over patterns for expanded jobs)
    pub job_names: Option<Vec<String>>,
    /// Job name regex patterns (for on_jobs_ready triggers with unexpanded parameterized jobs)
    pub job_name_patterns: Option<Vec<String>>,
    /// Number of allocations to submit
    pub num_allocations: i64,
    /// Whether this is a recovery action (ephemeral, deleted on reinitialize)
    pub is_recovery: bool,
}

/// A complete scheduler plan for a workflow.
///
/// Contains all the schedulers and actions needed to run the workflow,
/// plus job-to-scheduler assignments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerPlan {
    /// Schedulers to create
    pub schedulers: Vec<PlannedScheduler>,
    /// Actions to create
    pub actions: Vec<PlannedAction>,
    /// Map of job name -> scheduler name
    pub job_assignments: HashMap<String, String>,
    /// Warnings generated during planning
    pub warnings: Vec<String>,
}

impl SchedulerPlan {
    /// Create an empty plan
    pub fn new() -> Self {
        Self {
            schedulers: Vec::new(),
            actions: Vec::new(),
            job_assignments: HashMap::new(),
            warnings: Vec::new(),
        }
    }

    /// Get total number of allocations across all schedulers
    pub fn total_allocations(&self) -> i64 {
        self.schedulers.iter().map(|s| s.num_allocations).sum()
    }
}

impl Default for SchedulerPlan {
    fn default() -> Self {
        Self::new()
    }
}

/// Resource requirements abstraction for plan generation.
///
/// This trait allows both `ResourceRequirementsSpec` and `ResourceRequirementsModel`
/// to be used with the plan generator.
pub trait ResourceRequirements {
    fn name(&self) -> &str;
    fn memory(&self) -> &str;
    fn runtime(&self) -> &str;
    fn num_cpus(&self) -> i64;
    fn num_gpus(&self) -> i64;
    fn num_nodes(&self) -> i64;
}

/// Generate a scheduler plan from a workflow graph and resource requirements.
///
/// This is the core planning logic shared between workflow creation and regeneration.
///
/// # Arguments
/// * `graph` - The workflow graph with job dependency information
/// * `resource_requirements` - Map of RR name to RR data
/// * `profile` - HPC profile with partition information
/// * `account` - Slurm account to use
/// * `single_allocation` - If true, create 1 allocation with N nodes (1×N mode)
/// * `group_by` - Strategy for grouping jobs into schedulers
/// * `walltime_strategy` - Strategy for determining walltime
/// * `walltime_multiplier` - Multiplier for job runtime when using max-job-runtime strategy
/// * `add_actions` - Whether to add workflow actions for scheduling
/// * `scheduler_name_suffix` - Optional suffix for scheduler names (e.g., "_regen_20240101")
/// * `is_recovery` - Whether this is a recovery scenario (actions marked as recovery)
/// * `overrides` - Optional partition/walltime overrides that bypass auto-selection
#[allow(clippy::too_many_arguments)]
pub fn generate_scheduler_plan<RR: ResourceRequirements>(
    graph: &WorkflowGraph,
    resource_requirements: &HashMap<&str, &RR>,
    profile: &HpcProfile,
    account: &str,
    single_allocation: bool,
    group_by: GroupByStrategy,
    walltime_strategy: WalltimeStrategy,
    walltime_multiplier: f64,
    add_actions: bool,
    scheduler_name_suffix: Option<&str>,
    is_recovery: bool,
    overrides: &SchedulerOverrides,
) -> SchedulerPlan {
    let mut plan = SchedulerPlan::new();

    // Validate partition override up front to avoid repeated lookups and duplicate warnings
    if let Some(ref partition_name) = overrides.partition
        && profile.find_partition_by_name(partition_name).is_none()
    {
        plan.warnings.push(format!(
            "Partition '{}' not found in HPC profile. No schedulers will be generated.",
            partition_name
        ));
        return plan;
    }

    // Get scheduler groups from the graph
    // Groups jobs by (resource_requirements, has_dependencies)
    let scheduler_groups = graph.scheduler_groups();

    match group_by {
        GroupByStrategy::Partition => {
            // Group by partition: merge scheduler groups that map to the same partition
            generate_plan_grouped_by_partition(
                &scheduler_groups,
                resource_requirements,
                profile,
                account,
                single_allocation,
                walltime_strategy,
                walltime_multiplier,
                add_actions,
                scheduler_name_suffix,
                is_recovery,
                overrides,
                &mut plan,
            );
        }
        GroupByStrategy::ResourceRequirements => {
            // Default: one scheduler per resource_requirements name
            for group in &scheduler_groups {
                match process_scheduler_group(
                    group,
                    resource_requirements,
                    profile,
                    account,
                    single_allocation,
                    walltime_strategy,
                    walltime_multiplier,
                    add_actions,
                    scheduler_name_suffix,
                    is_recovery,
                    overrides,
                ) {
                    Ok((scheduler, action)) => {
                        // Record job assignments
                        for job_name in &scheduler.job_names {
                            plan.job_assignments
                                .insert(job_name.clone(), scheduler.name.clone());
                        }

                        plan.schedulers.push(scheduler);

                        if let Some(action) = action {
                            plan.actions.push(action);
                        }
                    }
                    Err(warning) => {
                        plan.warnings.push(warning);
                    }
                }
            }
        }
    }

    plan
}

/// Process a single scheduler group and return the planned scheduler and optional action.
#[allow(clippy::too_many_arguments)]
fn process_scheduler_group<RR: ResourceRequirements>(
    group: &SchedulerGroup,
    resource_requirements: &HashMap<&str, &RR>,
    profile: &HpcProfile,
    account: &str,
    single_allocation: bool,
    walltime_strategy: WalltimeStrategy,
    walltime_multiplier: f64,
    add_actions: bool,
    scheduler_name_suffix: Option<&str>,
    is_recovery: bool,
    overrides: &SchedulerOverrides,
) -> Result<(PlannedScheduler, Option<PlannedAction>), String> {
    let rr_name = &group.resource_requirements;
    let rr = resource_requirements.get(rr_name.as_str()).ok_or_else(|| {
        format!(
            "Resource requirements '{}' not found, skipping {} job(s)",
            rr_name, group.job_count
        )
    })?;

    // Parse resource requirements
    let memory_mb = parse_memory_mb(rr.memory()).map_err(|e| {
        format!(
            "Failed to parse memory '{}' for RR '{}': {}",
            rr.memory(),
            rr.name(),
            e
        )
    })?;

    let runtime_secs = duration_string_to_seconds(rr.runtime()).map_err(|e| {
        format!(
            "Failed to parse runtime '{}' for RR '{}': {}",
            rr.runtime(),
            rr.name(),
            e
        )
    })? as u64;

    let gpus = if rr.num_gpus() > 0 {
        Some(rr.num_gpus() as u32)
    } else {
        None
    };

    // Find partition: use override if provided, otherwise auto-detect
    let partition = if let Some(ref partition_name) = overrides.partition {
        profile
            .find_partition_by_name(partition_name)
            .ok_or_else(|| {
                format!(
                    "Partition '{}' not found in HPC profile for resource requirements '{}'",
                    partition_name,
                    rr.name()
                )
            })?
    } else {
        profile
            .find_best_partition(rr.num_cpus() as u32, memory_mb, runtime_secs, gpus)
            .ok_or_else(|| {
                format!(
                    "No partition found for resource requirements '{}' (CPUs: {}, Memory: {}, Runtime: {}, GPUs: {:?})",
                    rr.name(),
                    rr.num_cpus(),
                    rr.memory(),
                    rr.runtime(),
                    gpus
                )
            })?
    };

    // Calculate walltime: use override if provided, otherwise use strategy
    let walltime_secs = if let Some(override_secs) = overrides.walltime_secs {
        if override_secs > partition.max_walltime_secs {
            warn!(
                "Override walltime ({}) exceeds partition '{}' maximum ({}). Slurm may reject the job.",
                secs_to_walltime(override_secs),
                partition.name,
                secs_to_walltime(partition.max_walltime_secs)
            );
        }
        override_secs
    } else {
        calculate_walltime(
            runtime_secs,
            partition.max_walltime_secs,
            walltime_strategy,
            walltime_multiplier,
        )
    };

    // Calculate allocations using the shared helper function
    let alloc_params = AllocationParams {
        max_cpus: rr.num_cpus() as u32,
        max_memory_mb: memory_mb,
        max_runtime_secs: runtime_secs,
        max_gpus: gpus.unwrap_or(0),
        nodes_per_job: rr.num_nodes() as u32,
        job_count: group.job_count,
        allocation_walltime_secs: walltime_secs,
    };

    let num_allocations = calculate_allocations(&alloc_params, partition, single_allocation)
        .ok_or_else(|| {
            format!(
                "Invalid resource requirements for '{}': CPUs or memory is zero",
                rr.name()
            )
        })?;

    // For single allocation mode, nodes_per_alloc equals total nodes needed
    let nodes_per_alloc = if single_allocation {
        num_allocations
    } else {
        1
    };

    // Generate scheduler name
    let base_name = if group.has_dependencies {
        format!("{}_deferred", rr_name)
    } else {
        rr_name.clone()
    };
    let scheduler_name = match scheduler_name_suffix {
        Some(suffix) => format!("{}_{}", base_name, suffix),
        None => format!("{}_scheduler", base_name),
    };

    // Format memory for the scheduler (use partition's max memory to allow jobs to consume more than estimates)
    let mem_str = if partition.memory_mb >= 1024 {
        format!("{}g", partition.memory_mb / 1024)
    } else {
        format!("{}m", partition.memory_mb)
    };

    let scheduler = PlannedScheduler {
        name: scheduler_name.clone(),
        account: account.to_string(),
        partition: if overrides.partition.is_some() || partition.requires_explicit_request {
            Some(partition.name.clone())
        } else {
            None
        },
        resolved_partition: partition.name.clone(),
        mem: Some(mem_str),
        walltime: secs_to_walltime(walltime_secs),
        nodes: nodes_per_alloc,
        gres: gpus.map(|g| format!("gpu:{}", g)),
        qos: partition.default_qos.clone(),
        resource_requirements: rr_name.clone(),
        has_dependencies: group.has_dependencies,
        job_count: group.job_count,
        job_names: group.job_names.clone(),
        job_name_patterns: group.job_name_patterns.clone(),
        num_allocations,
    };

    // Create action if requested
    let action = if add_actions {
        // For jobs with dependencies, we need to specify which jobs trigger the action.
        // Use job_names (exact names) instead of job_name_patterns (regexes) because
        // after parameter expansion, each job has an exact name and using regexes
        // with individual exact-match patterns is wasteful and confusing.
        let (trigger_type, job_names, job_name_patterns) = if group.has_dependencies {
            ("on_jobs_ready", Some(group.job_names.clone()), None)
        } else {
            ("on_workflow_start", None, None)
        };

        Some(PlannedAction {
            trigger_type: trigger_type.to_string(),
            scheduler_name: scheduler_name.clone(),
            job_names,
            job_name_patterns,
            num_allocations,
            is_recovery,
        })
    } else {
        None
    };

    Ok((scheduler, action))
}

/// Helper struct to track merged partition groups
struct PartitionGroup {
    partition_name: String,
    has_dependencies: bool,
    job_count: usize,
    job_names: Vec<String>,
    job_name_patterns: Vec<String>,
    /// All RR names in this group (for naming the scheduler)
    rr_names: Vec<String>,
    /// Maximum memory in MB across all RRs
    max_memory_mb: u64,
    /// Maximum runtime in seconds across all RRs
    max_runtime_secs: u64,
    /// Maximum CPUs across all RRs
    max_cpus: i64,
    /// Maximum GPUs across all RRs
    max_gpus: i64,
    /// Maximum nodes per job across all RRs
    max_nodes: i64,
}

/// Generate a scheduler plan by grouping jobs by partition instead of by RR name.
///
/// Jobs whose resource requirements map to the same partition are merged into
/// a single scheduler. The scheduler uses the most demanding requirements
/// (max memory, max runtime, etc.) from all RRs in the group.
#[allow(clippy::too_many_arguments)]
fn generate_plan_grouped_by_partition<RR: ResourceRequirements>(
    scheduler_groups: &[SchedulerGroup],
    resource_requirements: &HashMap<&str, &RR>,
    profile: &HpcProfile,
    account: &str,
    single_allocation: bool,
    walltime_strategy: WalltimeStrategy,
    walltime_multiplier: f64,
    add_actions: bool,
    scheduler_name_suffix: Option<&str>,
    is_recovery: bool,
    overrides: &SchedulerOverrides,
    plan: &mut SchedulerPlan,
) {
    // First pass: resolve each scheduler group to its partition and build merged groups
    let mut partition_groups: HashMap<(String, bool), PartitionGroup> = HashMap::new();

    for group in scheduler_groups {
        let rr_name = &group.resource_requirements;
        let rr = match resource_requirements.get(rr_name.as_str()) {
            Some(rr) => *rr,
            None => {
                plan.warnings.push(format!(
                    "Resource requirements '{}' not found, skipping {} job(s)",
                    rr_name, group.job_count
                ));
                continue;
            }
        };

        // Parse resource requirements
        let memory_mb = match parse_memory_mb(rr.memory()) {
            Ok(m) => m,
            Err(e) => {
                plan.warnings.push(format!(
                    "Failed to parse memory '{}' for RR '{}': {}",
                    rr.memory(),
                    rr.name(),
                    e
                ));
                continue;
            }
        };

        let runtime_secs = match duration_string_to_seconds(rr.runtime()) {
            Ok(s) => s as u64,
            Err(e) => {
                plan.warnings.push(format!(
                    "Failed to parse runtime '{}' for RR '{}': {}",
                    rr.runtime(),
                    rr.name(),
                    e
                ));
                continue;
            }
        };

        let gpus = if rr.num_gpus() > 0 {
            Some(rr.num_gpus() as u32)
        } else {
            None
        };

        // Find partition: use override if provided, otherwise auto-detect
        let partition = if let Some(ref partition_name) = overrides.partition {
            match profile.find_partition_by_name(partition_name) {
                Some(p) => p,
                None => {
                    plan.warnings.push(format!(
                        "Partition '{}' not found in HPC profile for resource requirements '{}'",
                        partition_name,
                        rr.name()
                    ));
                    continue;
                }
            }
        } else {
            match profile.find_best_partition(rr.num_cpus() as u32, memory_mb, runtime_secs, gpus) {
                Some(p) => p,
                None => {
                    plan.warnings.push(format!(
                        "No partition found for resource requirements '{}' (CPUs: {}, Memory: {}, Runtime: {}, GPUs: {:?})",
                        rr.name(),
                        rr.num_cpus(),
                        rr.memory(),
                        rr.runtime(),
                        gpus
                    ));
                    continue;
                }
            }
        };

        // Group by (partition_name, has_dependencies)
        let key = (partition.name.clone(), group.has_dependencies);

        let pg = partition_groups
            .entry(key)
            .or_insert_with(|| PartitionGroup {
                partition_name: partition.name.clone(),
                has_dependencies: group.has_dependencies,
                job_count: 0,
                job_names: Vec::new(),
                job_name_patterns: Vec::new(),
                rr_names: Vec::new(),
                max_memory_mb: 0,
                max_runtime_secs: 0,
                max_cpus: 0,
                max_gpus: 0,
                max_nodes: 0,
            });

        // Merge this group into the partition group
        pg.job_count += group.job_count;
        pg.job_names.extend(group.job_names.clone());
        pg.job_name_patterns.extend(group.job_name_patterns.clone());
        pg.rr_names.push(rr_name.clone());
        pg.max_memory_mb = pg.max_memory_mb.max(memory_mb);
        pg.max_runtime_secs = pg.max_runtime_secs.max(runtime_secs);
        pg.max_cpus = pg.max_cpus.max(rr.num_cpus());
        pg.max_gpus = pg.max_gpus.max(rr.num_gpus());
        pg.max_nodes = pg.max_nodes.max(rr.num_nodes());
    }

    // Merge pass: combine deferred and non-deferred groups for the same partition
    // when their total allocations are small (reduces Slurm submissions)

    // Helper to calculate allocations for a partition group
    let calc_group_allocations = |pg: &PartitionGroup| -> Option<i64> {
        let gpus = if pg.max_gpus > 0 {
            Some(pg.max_gpus as u32)
        } else {
            None
        };
        let partition = if let Some(ref partition_name) = overrides.partition {
            profile.find_partition_by_name(partition_name)?
        } else {
            profile.find_best_partition(
                pg.max_cpus as u32,
                pg.max_memory_mb,
                pg.max_runtime_secs,
                gpus,
            )?
        };

        let alloc_walltime_secs = if let Some(override_secs) = overrides.walltime_secs {
            override_secs
        } else {
            calculate_walltime(
                pg.max_runtime_secs,
                partition.max_walltime_secs,
                walltime_strategy,
                walltime_multiplier,
            )
        };

        let params = AllocationParams {
            max_cpus: pg.max_cpus as u32,
            max_memory_mb: pg.max_memory_mb,
            max_runtime_secs: pg.max_runtime_secs,
            max_gpus: pg.max_gpus as u32,
            nodes_per_job: pg.max_nodes as u32,
            job_count: pg.job_count,
            allocation_walltime_secs: alloc_walltime_secs,
        };

        calculate_allocations(&params, partition, single_allocation)
    };

    // Collect unique partition names for merge checking
    // (we need to collect first to avoid borrowing issues during mutation)
    let partition_names: HashSet<String> = partition_groups.keys().map(|k| k.0.clone()).collect();

    // Check each partition for merge opportunities
    for partition_name in partition_names {
        let deferred_key = (partition_name.clone(), true);
        let non_deferred_key = (partition_name.clone(), false);

        // Both must exist
        let (deferred, non_deferred) = match (
            partition_groups.get(&deferred_key),
            partition_groups.get(&non_deferred_key),
        ) {
            (Some(d), Some(nd)) => (d, nd),
            _ => continue,
        };

        let deferred_allocs = calc_group_allocations(deferred).unwrap_or(i64::MAX);
        let non_deferred_allocs = calc_group_allocations(non_deferred).unwrap_or(i64::MAX);
        let total_allocs = deferred_allocs.saturating_add(non_deferred_allocs);

        // Merge if total allocations are small enough that a single scheduler makes sense.
        // Note: When both groups exist, each needs at least 1 allocation, so total >= 2.
        if total_allocs <= MERGE_THRESHOLD {
            // Merge deferred into non-deferred (so we use on_workflow_start)
            let deferred = partition_groups.remove(&deferred_key).unwrap();
            let non_deferred = partition_groups.get_mut(&non_deferred_key).unwrap();

            non_deferred.job_count += deferred.job_count;
            non_deferred.job_names.extend(deferred.job_names);
            non_deferred
                .job_name_patterns
                .extend(deferred.job_name_patterns);
            // Deduplicate rr_names to avoid redundant entries when same RR appears in both groups
            let existing: HashSet<_> = non_deferred.rr_names.iter().cloned().collect();
            for name in deferred.rr_names {
                if !existing.contains(&name) {
                    non_deferred.rr_names.push(name);
                }
            }
            non_deferred.max_memory_mb = non_deferred.max_memory_mb.max(deferred.max_memory_mb);
            non_deferred.max_runtime_secs =
                non_deferred.max_runtime_secs.max(deferred.max_runtime_secs);
            non_deferred.max_cpus = non_deferred.max_cpus.max(deferred.max_cpus);
            non_deferred.max_gpus = non_deferred.max_gpus.max(deferred.max_gpus);
            non_deferred.max_nodes = non_deferred.max_nodes.max(deferred.max_nodes);
        }
    }

    // Second pass: create schedulers for each partition group
    for pg in partition_groups.into_values() {
        // Find the partition again to get its full info
        let gpus = if pg.max_gpus > 0 {
            Some(pg.max_gpus as u32)
        } else {
            None
        };

        let partition = if let Some(ref partition_name) = overrides.partition {
            match profile.find_partition_by_name(partition_name) {
                Some(p) => p,
                None => {
                    plan.warnings.push(format!(
                        "Partition '{}' not found in HPC profile for merged group '{}'",
                        partition_name, pg.partition_name
                    ));
                    continue;
                }
            }
        } else {
            match profile.find_best_partition(
                pg.max_cpus as u32,
                pg.max_memory_mb,
                pg.max_runtime_secs,
                gpus,
            ) {
                Some(p) => p,
                None => {
                    plan.warnings.push(format!(
                        "No partition found for merged group '{}' (this shouldn't happen)",
                        pg.partition_name
                    ));
                    continue;
                }
            }
        };

        // Calculate walltime: use override if provided, otherwise use strategy
        let walltime_secs = if let Some(override_secs) = overrides.walltime_secs {
            if override_secs > partition.max_walltime_secs {
                warn!(
                    "Override walltime ({}) exceeds partition '{}' maximum ({}). Slurm may reject the job.",
                    secs_to_walltime(override_secs),
                    partition.name,
                    secs_to_walltime(partition.max_walltime_secs)
                );
            }
            override_secs
        } else {
            calculate_walltime(
                pg.max_runtime_secs,
                partition.max_walltime_secs,
                walltime_strategy,
                walltime_multiplier,
            )
        };

        // Calculate allocations using the shared helper function
        let alloc_params = AllocationParams {
            max_cpus: pg.max_cpus as u32,
            max_memory_mb: pg.max_memory_mb,
            max_runtime_secs: pg.max_runtime_secs,
            max_gpus: gpus.unwrap_or(0),
            nodes_per_job: pg.max_nodes as u32,
            job_count: pg.job_count,
            allocation_walltime_secs: walltime_secs,
        };

        let num_allocations =
            match calculate_allocations(&alloc_params, partition, single_allocation) {
                Some(n) => n,
                None => {
                    plan.warnings.push(format!(
                        "Invalid resource parameters for group '{}': CPUs or memory is zero",
                        pg.partition_name
                    ));
                    continue;
                }
            };

        // For single allocation mode, nodes_per_alloc equals total nodes needed
        let nodes_per_alloc = if single_allocation {
            num_allocations
        } else {
            1
        };

        // Generate scheduler name based on partition
        let base_name = if pg.has_dependencies {
            format!("{}_deferred", pg.partition_name)
        } else {
            pg.partition_name.clone()
        };
        let scheduler_name = match scheduler_name_suffix {
            Some(suffix) => format!("{}_{}", base_name, suffix),
            None => format!("{}_scheduler", base_name),
        };

        // Format memory for the scheduler (use partition's max memory to allow jobs to consume more than estimates)
        let mem_str = if partition.memory_mb >= 1024 {
            format!("{}g", partition.memory_mb / 1024)
        } else {
            format!("{}m", partition.memory_mb)
        };

        let scheduler = PlannedScheduler {
            name: scheduler_name.clone(),
            account: account.to_string(),
            partition: if overrides.partition.is_some() || partition.requires_explicit_request {
                Some(partition.name.clone())
            } else {
                None
            },
            resolved_partition: partition.name.clone(),
            mem: Some(mem_str),
            walltime: secs_to_walltime(walltime_secs),
            nodes: nodes_per_alloc,
            gres: gpus.map(|g| format!("gpu:{}", g)),
            qos: partition.default_qos.clone(),
            resource_requirements: pg.rr_names.join(","), // Join all RR names
            has_dependencies: pg.has_dependencies,
            job_count: pg.job_count,
            job_names: pg.job_names.clone(),
            job_name_patterns: pg.job_name_patterns.clone(),
            num_allocations,
        };

        // Record job assignments
        for job_name in &scheduler.job_names {
            plan.job_assignments
                .insert(job_name.clone(), scheduler.name.clone());
        }

        plan.schedulers.push(scheduler);

        // Create action if requested
        if add_actions {
            let (trigger_type, job_names, job_name_patterns) = if pg.has_dependencies {
                ("on_jobs_ready", Some(pg.job_names.clone()), None)
            } else {
                ("on_workflow_start", None, None)
            };

            plan.actions.push(PlannedAction {
                trigger_type: trigger_type.to_string(),
                scheduler_name,
                job_names,
                job_name_patterns,
                num_allocations,
                is_recovery,
            });
        }
    }
}

// ============================================================================
// Trait implementations for ResourceRequirementsSpec and ResourceRequirementsModel
// ============================================================================

impl ResourceRequirements for crate::client::workflow_spec::ResourceRequirementsSpec {
    fn name(&self) -> &str {
        &self.name
    }
    fn memory(&self) -> &str {
        &self.memory
    }
    fn runtime(&self) -> &str {
        &self.runtime
    }
    fn num_cpus(&self) -> i64 {
        self.num_cpus
    }
    fn num_gpus(&self) -> i64 {
        self.num_gpus
    }
    fn num_nodes(&self) -> i64 {
        self.num_nodes
    }
}

impl ResourceRequirements for crate::models::ResourceRequirementsModel {
    fn name(&self) -> &str {
        &self.name
    }
    fn memory(&self) -> &str {
        &self.memory
    }
    fn runtime(&self) -> &str {
        &self.runtime
    }
    fn num_cpus(&self) -> i64 {
        self.num_cpus
    }
    fn num_gpus(&self) -> i64 {
        self.num_gpus
    }
    fn num_nodes(&self) -> i64 {
        self.num_nodes
    }
}

// ============================================================================
// Apply plan to WorkflowSpec
// ============================================================================

use crate::client::workflow_spec::{SlurmSchedulerSpec, WorkflowActionSpec, WorkflowSpec};

/// Apply a scheduler plan to a WorkflowSpec.
///
/// This adds the planned schedulers and actions to the spec, and updates
/// job scheduler assignments.
pub fn apply_plan_to_spec(plan: &SchedulerPlan, spec: &mut WorkflowSpec) {
    // Convert planned schedulers to SlurmSchedulerSpec
    let schedulers: Vec<SlurmSchedulerSpec> = plan
        .schedulers
        .iter()
        .map(|ps| SlurmSchedulerSpec {
            name: Some(ps.name.clone()),
            account: ps.account.clone(),
            partition: ps.partition.clone(),
            mem: ps.mem.clone(),
            walltime: ps.walltime.clone(),
            nodes: ps.nodes,
            gres: ps.gres.clone(),
            ntasks_per_node: None,
            qos: ps.qos.clone(),
            tmp: None,
            extra: None,
        })
        .collect();

    // Convert planned actions to WorkflowActionSpec
    // Prefer job_names (exact matches) over job_name_patterns (regexes) when available,
    // since after parameter expansion we have exact job names and using regexes with
    // individual exact-match patterns is wasteful and confusing.
    let actions: Vec<WorkflowActionSpec> = plan
        .actions
        .iter()
        .map(|pa| WorkflowActionSpec {
            trigger_type: pa.trigger_type.clone(),
            action_type: "schedule_nodes".to_string(),
            jobs: pa.job_names.clone(),
            job_name_regexes: pa.job_name_patterns.clone(),
            commands: None,
            scheduler: Some(pa.scheduler_name.clone()),
            scheduler_type: Some("slurm".to_string()),
            num_allocations: Some(pa.num_allocations),
            start_one_worker_per_node: None,
            max_parallel_jobs: None,
            persistent: None,
        })
        .collect();

    // Update workflow spec
    spec.slurm_schedulers = Some(schedulers);

    if !actions.is_empty() {
        let mut existing_actions = spec.actions.take().unwrap_or_default();
        existing_actions.extend(actions);
        spec.actions = Some(existing_actions);
    }

    // Update job scheduler assignments
    for job in &mut spec.jobs {
        if let Some(scheduler_name) = plan.job_assignments.get(&job.name) {
            job.scheduler = Some(scheduler_name.clone());
        }
    }
}
