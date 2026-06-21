//! Execution Plan - Visualization of workflow execution as a DAG of events
//!
//! This module provides a high-level view of how a workflow will execute,
//! showing which jobs become ready at each event and what scheduler
//! allocations are triggered.
//!
//! The execution plan is represented as a Directed Acyclic Graph (DAG) where:
//! - Each node is an "event" (workflow start or a set of jobs completing)
//! - Edges represent dependencies between events
//! - Independent sub-workflows have separate event chains
//!
//! Built on top of WorkflowGraph for consistent dependency analysis.

use crate::client::workflow_graph::WorkflowGraph;
use crate::client::workflow_spec::{WorkflowActionSpec, WorkflowSpec};
use crate::models::{JobModel, WorkflowActionModel, WorkflowModel};
use regex::Regex;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Represents a scheduler allocation in the execution plan
#[derive(Debug, Clone, Serialize)]
pub struct SchedulerAllocation {
    pub scheduler: String,
    pub scheduler_type: String,
    pub num_allocations: i64,
    pub jobs: Vec<String>,
}

/// What triggers an execution event
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum EventTrigger {
    /// Workflow starts
    WorkflowStart,
    /// Specific jobs complete
    JobsComplete { jobs: Vec<String> },
}

/// An event in the execution plan DAG
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionEvent {
    /// Unique identifier for this event
    pub id: String,
    /// What triggers this event
    pub trigger: EventTrigger,
    /// Human-readable description of the trigger
    pub trigger_description: String,
    /// Scheduler allocations triggered by this event
    pub scheduler_allocations: Vec<SchedulerAllocation>,
    /// Jobs that become ready when this event fires
    pub jobs_becoming_ready: Vec<String>,
    /// Event IDs that must complete before this event can fire
    pub depends_on_events: Vec<String>,
    /// Event IDs that depend on this event
    pub unlocks_events: Vec<String>,
}

/// Represents the complete execution plan for a workflow as a DAG
#[derive(Debug, Serialize)]
pub struct ExecutionPlan {
    /// All events in the plan, keyed by event ID
    pub events: HashMap<String, ExecutionEvent>,
    /// Event IDs that have no dependencies (entry points)
    pub root_events: Vec<String>,
    /// Event IDs that nothing depends on (exit points)
    pub leaf_events: Vec<String>,
    /// The underlying workflow graph (if built from spec)
    #[serde(skip)]
    pub graph: Option<WorkflowGraph>,
}

// Legacy stage-based interface for backwards compatibility
/// Represents a stage in the workflow execution plan (legacy format)
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionStage {
    pub stage_number: usize,
    pub trigger_description: String,
    pub scheduler_allocations: Vec<SchedulerAllocation>,
    pub jobs_becoming_ready: Vec<String>,
}

impl ExecutionPlan {
    /// Build an execution plan from a workflow specification
    pub fn from_spec(spec: &WorkflowSpec) -> Result<Self, Box<dyn std::error::Error>> {
        let graph = WorkflowGraph::from_spec(spec)?;
        let mut events: HashMap<String, ExecutionEvent> = HashMap::new();

        // Event 0: Workflow start - root jobs become ready
        let root_jobs: Vec<String> = graph.roots().iter().map(|s| s.to_string()).collect();

        // Find scheduler allocations that fire at workflow start: on_workflow_start actions, plus
        // on_jobs_ready actions gated on root jobs (root jobs are Ready immediately after init, so
        // their schedulers are submitted at submit time). The scheduler generator now ties root-job
        // schedulers to on_jobs_ready[root_jobs] for re-run safety, so both must appear here.
        let mut start_allocations = Vec::new();
        if let Some(ref actions) = spec.actions {
            for action in actions {
                if action.trigger_type == "on_workflow_start"
                    && let Some(alloc) = build_scheduler_allocation(spec, action)?
                {
                    start_allocations.push(alloc);
                }
            }
            for action in graph.matching_actions(&root_jobs, actions) {
                if let Some(alloc) = build_scheduler_allocation(spec, action)? {
                    start_allocations.push(alloc);
                }
            }
        }

        let start_event = ExecutionEvent {
            id: "start".to_string(),
            trigger: EventTrigger::WorkflowStart,
            trigger_description: "Workflow Start".to_string(),
            scheduler_allocations: start_allocations,
            jobs_becoming_ready: root_jobs.clone(),
            depends_on_events: vec![],
            unlocks_events: vec![], // Will be filled in later
        };
        events.insert("start".to_string(), start_event);

        // Build events for each distinct set of jobs that complete together
        // Key insight: jobs at the same level that share the same dependents form an event
        // But we need to track events by the jobs they unlock, not by level

        // Build a map: job -> event_id for the event where this job completes
        let mut job_completion_event: HashMap<String, String> = HashMap::new();

        // Root jobs complete as part of start event
        for job in &root_jobs {
            job_completion_event.insert(job.clone(), "start".to_string());
        }

        // Build events by analyzing what jobs become ready when other jobs complete
        // We iterate through jobs by dependency depth to ensure proper ordering
        let mut processed_jobs: HashSet<String> = root_jobs.iter().cloned().collect();

        // Keep processing until all jobs have been assigned to events
        loop {
            // Find jobs that become ready based on currently processed jobs
            let newly_ready = graph.jobs_unblocked_by(&processed_jobs);
            if newly_ready.is_empty() {
                break;
            }

            // Group newly ready jobs by their dependencies (the jobs they're waiting on)
            // Jobs with the same dependencies should be in the same event
            let mut jobs_by_deps: HashMap<Vec<String>, Vec<String>> = HashMap::new();

            for job in &newly_ready {
                if let Some(deps) = graph.dependencies_of(job) {
                    let mut dep_list: Vec<String> = deps.iter().cloned().collect();
                    dep_list.sort();
                    jobs_by_deps.entry(dep_list).or_default().push(job.clone());
                }
            }

            // Create an event for each unique dependency set
            for (deps, jobs_becoming_ready) in jobs_by_deps {
                // Generate a descriptive event ID based on the triggering jobs
                let event_id = generate_event_id(&deps);

                // Find which events the dependencies belong to
                let mut depends_on_event_ids: HashSet<String> = HashSet::new();
                for dep in &deps {
                    if let Some(evt_id) = job_completion_event.get(dep) {
                        depends_on_event_ids.insert(evt_id.clone());
                    }
                }
                let depends_on_events: Vec<String> = depends_on_event_ids.into_iter().collect();

                // Find matching scheduler actions
                let mut scheduler_allocations = Vec::new();
                if let Some(ref actions) = spec.actions {
                    let matching = graph.matching_actions(&jobs_becoming_ready, actions);
                    for action in matching {
                        if let Some(alloc) = build_scheduler_allocation(spec, action)? {
                            scheduler_allocations.push(alloc);
                        }
                    }
                }

                let trigger_description = build_trigger_description(&deps);

                let event = ExecutionEvent {
                    id: event_id.clone(),
                    trigger: EventTrigger::JobsComplete { jobs: deps.clone() },
                    trigger_description,
                    scheduler_allocations,
                    jobs_becoming_ready: jobs_becoming_ready.clone(),
                    depends_on_events,
                    unlocks_events: vec![], // Will be filled in below
                };
                events.insert(event_id.clone(), event);

                // Mark these jobs as completing in this event
                for job in &jobs_becoming_ready {
                    job_completion_event.insert(job.clone(), event_id.clone());
                }
            }

            // Add newly ready jobs to processed set
            processed_jobs.extend(newly_ready);
        }

        // Build unlocks_events by reversing the depends_on_events relationships
        let event_deps: Vec<(String, Vec<String>)> = events
            .values()
            .map(|e| (e.id.clone(), e.depends_on_events.clone()))
            .collect();

        for (event_id, deps) in event_deps {
            for dep_event_id in deps {
                if let Some(dep_event) = events.get_mut(&dep_event_id)
                    && !dep_event.unlocks_events.contains(&event_id)
                {
                    dep_event.unlocks_events.push(event_id.clone());
                }
            }
        }

        // Identify root and leaf events
        let root_events: Vec<String> = events
            .values()
            .filter(|e| e.depends_on_events.is_empty())
            .map(|e| e.id.clone())
            .collect();

        let leaf_events: Vec<String> = events
            .values()
            .filter(|e| e.unlocks_events.is_empty())
            .map(|e| e.id.clone())
            .collect();

        Ok(ExecutionPlan {
            events,
            root_events,
            leaf_events,
            graph: Some(graph),
        })
    }

    /// Build an execution plan from database models (workflow, jobs, actions, slurm_schedulers)
    pub fn from_database_models(
        _workflow: &WorkflowModel,
        jobs: &[JobModel],
        actions: &[WorkflowActionModel],
        slurm_schedulers: &[crate::models::SlurmSchedulerModel],
        resource_requirements: &[crate::models::ResourceRequirementsModel],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Build the workflow graph from database models
        let graph = WorkflowGraph::from_jobs(jobs, resource_requirements)?;

        // Build scheduler ID to name map
        let scheduler_id_to_name: HashMap<i64, String> = slurm_schedulers
            .iter()
            .filter_map(|s| match (s.id, &s.name) {
                (Some(id), Some(name)) => Some((id, name.clone())),
                _ => None,
            })
            .collect();
        let mut events: HashMap<String, ExecutionEvent> = HashMap::new();

        // Build job ID <-> name maps for action matching
        let _job_id_to_name: HashMap<i64, String> = jobs
            .iter()
            .filter_map(|j| j.id.map(|id| (id, j.name.clone())))
            .collect();
        let job_name_to_id: HashMap<String, i64> = jobs
            .iter()
            .filter_map(|j| j.id.map(|id| (j.name.clone(), id)))
            .collect();

        // Find root jobs using the graph
        let root_jobs: Vec<String> = graph.roots().iter().map(|s| s.to_string()).collect();

        // Build dependency graph reference for event grouping
        // (using graph's internal structure via dependencies_of)
        let dependency_graph_by_name: HashMap<String, HashSet<String>> = jobs
            .iter()
            .map(|j| {
                let deps = graph.dependencies_of(&j.name).cloned().unwrap_or_default();
                (j.name.clone(), deps)
            })
            .collect();

        // Find scheduler allocations that fire at workflow start: on_workflow_start actions, plus
        // on_jobs_ready actions gated on root jobs (Ready at init). The scheduler generator now ties
        // root-job schedulers to on_jobs_ready[root_jobs] for re-run safety, so both appear here.
        let mut start_allocations = Vec::new();
        for action in actions {
            if action.trigger_type == "on_workflow_start"
                && let Some(alloc) =
                    build_scheduler_allocation_from_db_action(action, jobs, &scheduler_id_to_name)?
            {
                start_allocations.push(alloc);
            }
        }
        let root_job_ids: HashSet<i64> = root_jobs
            .iter()
            .filter_map(|n| job_name_to_id.get(n).copied())
            .collect();
        for action in actions {
            if action.trigger_type == "on_jobs_ready" {
                let empty_vec = vec![];
                let action_job_ids = action.job_ids.as_ref().unwrap_or(&empty_vec);
                if action_job_ids.iter().any(|id| root_job_ids.contains(id))
                    && let Some(alloc) = build_scheduler_allocation_from_db_action(
                        action,
                        jobs,
                        &scheduler_id_to_name,
                    )?
                {
                    start_allocations.push(alloc);
                }
            }
        }

        let start_event = ExecutionEvent {
            id: "start".to_string(),
            trigger: EventTrigger::WorkflowStart,
            trigger_description: "Workflow Start".to_string(),
            scheduler_allocations: start_allocations,
            jobs_becoming_ready: root_jobs.clone(),
            depends_on_events: vec![],
            unlocks_events: vec![],
        };
        events.insert("start".to_string(), start_event);

        // Build a map: job -> event_id for the event where this job completes
        let mut job_completion_event: HashMap<String, String> = HashMap::new();
        for job in &root_jobs {
            job_completion_event.insert(job.clone(), "start".to_string());
        }

        // Process jobs level by level, similar to from_spec()
        let mut processed_jobs: HashSet<String> = root_jobs.iter().cloned().collect();

        loop {
            // Find jobs that become ready based on currently processed jobs
            let mut newly_ready = Vec::new();
            for job in jobs {
                if processed_jobs.contains(&job.name) {
                    continue;
                }
                if let Some(deps) = dependency_graph_by_name.get(&job.name)
                    && !deps.is_empty()
                    && deps.iter().all(|d| processed_jobs.contains(d))
                {
                    newly_ready.push(job.name.clone());
                }
            }

            if newly_ready.is_empty() {
                break;
            }

            // Group newly ready jobs by their dependencies
            let mut jobs_by_deps: HashMap<Vec<String>, Vec<String>> = HashMap::new();

            for job_name in &newly_ready {
                if let Some(deps) = dependency_graph_by_name.get(job_name) {
                    let mut dep_list: Vec<String> = deps.iter().cloned().collect();
                    dep_list.sort();
                    jobs_by_deps
                        .entry(dep_list)
                        .or_default()
                        .push(job_name.clone());
                }
            }

            // Create an event for each unique dependency set
            for (deps, jobs_becoming_ready) in jobs_by_deps {
                // Generate a descriptive event ID based on the triggering jobs
                let event_id = generate_event_id(&deps);

                // Find which events the dependencies belong to
                let mut depends_on_event_ids: HashSet<String> = HashSet::new();
                for dep in &deps {
                    if let Some(evt_id) = job_completion_event.get(dep) {
                        depends_on_event_ids.insert(evt_id.clone());
                    }
                }
                let depends_on_events: Vec<String> = depends_on_event_ids.into_iter().collect();

                // Find matching scheduler actions
                let mut scheduler_allocations = Vec::new();
                for action in actions {
                    if action.trigger_type == "on_jobs_ready" {
                        let empty_vec = vec![];
                        let action_job_ids = action.job_ids.as_ref().unwrap_or(&empty_vec);

                        let matches_any = action_job_ids.iter().any(|action_job_id| {
                            jobs_becoming_ready
                                .iter()
                                .any(|job_name| job_name_to_id.get(job_name) == Some(action_job_id))
                        });

                        if matches_any
                            && let Some(alloc) = build_scheduler_allocation_from_db_action(
                                action,
                                jobs,
                                &scheduler_id_to_name,
                            )?
                        {
                            scheduler_allocations.push(alloc);
                        }
                    }
                }

                let trigger_description = build_trigger_description(&deps);

                let event = ExecutionEvent {
                    id: event_id.clone(),
                    trigger: EventTrigger::JobsComplete { jobs: deps.clone() },
                    trigger_description,
                    scheduler_allocations,
                    jobs_becoming_ready: jobs_becoming_ready.clone(),
                    depends_on_events,
                    unlocks_events: vec![],
                };
                events.insert(event_id.clone(), event);

                // Mark these jobs as completing in this event
                for job in &jobs_becoming_ready {
                    job_completion_event.insert(job.clone(), event_id.clone());
                }
            }

            // Add newly ready jobs to processed set
            processed_jobs.extend(newly_ready);
        }

        // Build unlocks_events by reversing the depends_on_events relationships
        let event_deps: Vec<(String, Vec<String>)> = events
            .values()
            .map(|e| (e.id.clone(), e.depends_on_events.clone()))
            .collect();

        for (event_id, deps) in event_deps {
            for dep_event_id in deps {
                if let Some(dep_event) = events.get_mut(&dep_event_id)
                    && !dep_event.unlocks_events.contains(&event_id)
                {
                    dep_event.unlocks_events.push(event_id.clone());
                }
            }
        }

        // Identify root and leaf events
        let root_events: Vec<String> = events
            .values()
            .filter(|e| e.depends_on_events.is_empty())
            .map(|e| e.id.clone())
            .collect();

        let leaf_events: Vec<String> = events
            .values()
            .filter(|e| e.unlocks_events.is_empty())
            .map(|e| e.id.clone())
            .collect();

        Ok(ExecutionPlan {
            events,
            root_events,
            leaf_events,
            graph: Some(graph),
        })
    }

    /// Display the execution plan in a human-readable format
    pub fn display(&self) {
        println!("\n{}", "=".repeat(80));
        println!("Workflow Execution Plan (DAG)");
        println!("{}", "=".repeat(80));

        // Show connected components if we have a graph
        if let Some(ref graph) = self.graph {
            let mut graph_clone = graph.clone();
            let components = graph_clone.connected_components();
            if components.len() > 1 {
                println!(
                    "\nNote: Workflow has {} independent sub-workflows that can run in parallel",
                    components.len()
                );
            }
        }

        println!("\nEvents: {} total", self.events.len());
        println!("Root events: {}", self.root_events.join(", "));
        println!("Leaf events: {}", self.leaf_events.join(", "));

        // Display events in topological order (BFS from roots)
        let mut displayed = HashSet::new();
        let mut queue: Vec<String> = self.root_events.clone();

        while !queue.is_empty() {
            // Find events that can be displayed (all dependencies displayed)
            let mut next_queue = Vec::new();
            let mut events_to_display = Vec::new();

            for event_id in &queue {
                if displayed.contains(event_id) {
                    continue;
                }

                if let Some(event) = self.events.get(event_id) {
                    if event
                        .depends_on_events
                        .iter()
                        .all(|d| displayed.contains(d))
                    {
                        events_to_display.push(event_id.clone());
                    } else {
                        next_queue.push(event_id.clone());
                    }
                }
            }

            // Display these events
            for event_id in events_to_display {
                if let Some(event) = self.events.get(&event_id) {
                    let symbol = match &event.trigger {
                        EventTrigger::WorkflowStart => "▶",
                        EventTrigger::JobsComplete { .. } => "→",
                    };

                    println!("\n{} {}", symbol, event.trigger_description);
                    println!("{}", "-".repeat(80));

                    // Show flow information
                    if !event.depends_on_events.is_empty() {
                        let dep_descriptions: Vec<String> = event
                            .depends_on_events
                            .iter()
                            .filter_map(|id| self.events.get(id))
                            .map(|e| format!("'{}'", e.trigger_description))
                            .collect();
                        println!("  After: {}", dep_descriptions.join(", "));
                    }

                    if !event.unlocks_events.is_empty() {
                        let unlock_descriptions: Vec<String> = event
                            .unlocks_events
                            .iter()
                            .filter_map(|id| self.events.get(id))
                            .map(|e| format!("'{}'", e.trigger_description))
                            .collect();
                        println!("  Then: {}", unlock_descriptions.join(", "));
                    }

                    if !event.scheduler_allocations.is_empty() {
                        println!("\n  Scheduler Allocations:");
                        for alloc in &event.scheduler_allocations {
                            println!(
                                "    • {} ({}) - {} allocation(s)",
                                alloc.scheduler, alloc.scheduler_type, alloc.num_allocations
                            );
                            let compact = compact_job_list(&alloc.jobs);
                            println!("      For jobs: {}", compact.join(", "));
                        }
                    }

                    if !event.jobs_becoming_ready.is_empty() {
                        println!("\n  Jobs Becoming Ready:");
                        let compact = compact_job_list(&event.jobs_becoming_ready);
                        for item in compact {
                            println!("    • {}", item);
                        }
                    }

                    displayed.insert(event_id.clone());

                    // Add events this one unlocks to the next queue
                    for unlocked in &event.unlocks_events {
                        if !displayed.contains(unlocked) && !next_queue.contains(unlocked) {
                            next_queue.push(unlocked.clone());
                        }
                    }
                }
            }

            queue = next_queue;
        }

        println!("\n{}", "=".repeat(80));
        println!("Total Events: {}", self.events.len());
        println!("{}\n", "=".repeat(80));
    }

    /// Get the underlying workflow graph (if available)
    pub fn workflow_graph(&self) -> Option<&WorkflowGraph> {
        self.graph.as_ref()
    }

    /// Convert to legacy stage format for backwards compatibility
    /// Note: This flattens the DAG into a linear sequence, which may not
    /// accurately represent parallel subgraphs
    pub fn to_stages(&self) -> Vec<ExecutionStage> {
        let mut stages = Vec::new();
        let mut displayed = HashSet::new();
        let mut queue: Vec<String> = self.root_events.clone();
        let mut stage_number = 0;

        while !queue.is_empty() {
            let mut next_queue = Vec::new();
            let mut events_at_level = Vec::new();

            for event_id in &queue {
                if displayed.contains(event_id) {
                    continue;
                }

                if let Some(event) = self.events.get(event_id) {
                    if event
                        .depends_on_events
                        .iter()
                        .all(|d| displayed.contains(d))
                    {
                        events_at_level.push(event_id.clone());
                    } else {
                        next_queue.push(event_id.clone());
                    }
                }
            }

            // Merge events at the same level into a single stage
            if !events_at_level.is_empty() {
                let mut merged_allocations = Vec::new();
                let mut merged_jobs = Vec::new();
                let mut trigger_parts = Vec::new();

                for event_id in &events_at_level {
                    if let Some(event) = self.events.get(event_id) {
                        merged_allocations.extend(event.scheduler_allocations.clone());
                        merged_jobs.extend(event.jobs_becoming_ready.clone());
                        trigger_parts.push(event.trigger_description.clone());
                        displayed.insert(event_id.clone());

                        for unlocked in &event.unlocks_events {
                            if !displayed.contains(unlocked) && !next_queue.contains(unlocked) {
                                next_queue.push(unlocked.clone());
                            }
                        }
                    }
                }

                let trigger_description = if trigger_parts.len() == 1 {
                    trigger_parts[0].clone()
                } else {
                    trigger_parts.join(" AND ")
                };

                stages.push(ExecutionStage {
                    stage_number,
                    trigger_description,
                    scheduler_allocations: merged_allocations,
                    jobs_becoming_ready: merged_jobs,
                });
                stage_number += 1;
            }

            queue = next_queue;
        }

        stages
    }
}

/// Build trigger description for a set of completing jobs
fn build_trigger_description(level_jobs: &[String]) -> String {
    if level_jobs.len() == 1 {
        format!("When job '{}' completes", level_jobs[0])
    } else if level_jobs.len() <= 3 {
        format!(
            "When jobs {} complete",
            level_jobs
                .iter()
                .map(|j| format!("'{}'", j))
                .collect::<Vec<_>>()
                .join(", ")
        )
    } else {
        format!(
            "When {} jobs complete ({}...)",
            level_jobs.len(),
            level_jobs
                .iter()
                .take(2)
                .map(|j| format!("'{}'", j))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// Generate a descriptive event ID based on the jobs that trigger it
fn generate_event_id(trigger_jobs: &[String]) -> String {
    if trigger_jobs.is_empty() {
        return "start".to_string();
    }

    // For a single job, use "after_<job_name>"
    if trigger_jobs.len() == 1 {
        return format!("after_{}", trigger_jobs[0]);
    }

    // For multiple jobs, try to find a common prefix
    let first = &trigger_jobs[0];
    let mut common_prefix_len = first.len();

    for job in trigger_jobs.iter().skip(1) {
        let matching = first
            .chars()
            .zip(job.chars())
            .take_while(|(a, b)| a == b)
            .count();
        common_prefix_len = common_prefix_len.min(matching);
    }

    // Use common prefix if it's meaningful (at least 2 chars, not just "work" or similar)
    if common_prefix_len >= 3 {
        let prefix = &first[..common_prefix_len];
        // Clean up trailing underscores or numbers
        let clean_prefix = prefix.trim_end_matches(|c: char| c == '_' || c.is_ascii_digit());
        if clean_prefix.len() >= 3 {
            return format!("after_{}_jobs", clean_prefix);
        }
    }

    // Fall back to listing first job and count
    format!(
        "after_{}_and_{}_more",
        trigger_jobs[0],
        trigger_jobs.len() - 1
    )
}

/// Get job names that match an action's job_names or job_name_regexes
fn get_matching_jobs(
    spec: &WorkflowSpec,
    action: &WorkflowActionSpec,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut matched = Vec::new();

    // Match exact names
    if let Some(ref names) = action.jobs {
        matched.extend(names.clone());
    }

    // Match regex patterns
    if let Some(ref regexes) = action.job_name_regexes {
        for regex_str in regexes {
            let re = Regex::new(regex_str)?;
            for job in &spec.jobs {
                if re.is_match(&job.name) && !matched.contains(&job.name) {
                    matched.push(job.name.clone());
                }
            }
        }
    }

    Ok(matched)
}

/// Build a scheduler allocation from an action
fn build_scheduler_allocation(
    spec: &WorkflowSpec,
    action: &WorkflowActionSpec,
) -> Result<Option<SchedulerAllocation>, Box<dyn std::error::Error>> {
    if action.action_type != "schedule_nodes" {
        return Ok(None);
    }

    let scheduler = action
        .scheduler
        .as_ref()
        .ok_or("schedule_nodes action missing scheduler")?
        .clone();

    let scheduler_type = action
        .scheduler_type
        .as_ref()
        .ok_or("schedule_nodes action missing scheduler_type")?
        .clone();

    let num_allocations = action.num_allocations.unwrap_or(1);

    let jobs = get_matching_jobs(spec, action)?;

    Ok(Some(SchedulerAllocation {
        scheduler,
        scheduler_type,
        num_allocations,
        jobs,
    }))
}

/// Build a scheduler allocation from a database action model
fn build_scheduler_allocation_from_db_action(
    action: &WorkflowActionModel,
    workflow_jobs: &[JobModel],
    scheduler_id_to_name: &HashMap<i64, String>,
) -> Result<Option<SchedulerAllocation>, Box<dyn std::error::Error>> {
    if action.action_type != "schedule_nodes" {
        return Ok(None);
    }

    // action_config is already a serde_json::Value
    let config = &action.action_config;

    let scheduler_type = config["scheduler_type"]
        .as_str()
        .ok_or("Action config missing scheduler_type")?
        .to_string();

    let num_allocations = config["num_allocations"].as_i64().unwrap_or(1);

    // Get job names from job IDs
    let empty_vec = vec![];
    let action_job_ids = action.job_ids.as_ref().unwrap_or(&empty_vec);
    let mut jobs = Vec::new();
    for job_id in action_job_ids {
        if let Some(job) = workflow_jobs.iter().find(|j| j.id == Some(*job_id)) {
            jobs.push(job.name.clone());
        }
    }

    // Look up the scheduler name from the ID
    let scheduler = if let Some(scheduler_id) = config["scheduler_id"].as_i64() {
        scheduler_id_to_name
            .get(&scheduler_id)
            .cloned()
            .unwrap_or_else(|| format!("{}_scheduler", scheduler_type))
    } else {
        format!("{}_scheduler", scheduler_type)
    };

    Ok(Some(SchedulerAllocation {
        scheduler,
        scheduler_type,
        num_allocations,
        jobs,
    }))
}

/// Compact a list of job names by detecting and collapsing sequential patterns
/// e.g., ["job_001", "job_002", "job_003"] -> ["job_{001-003}"]
fn compact_job_list(jobs: &[String]) -> Vec<String> {
    if jobs.is_empty() {
        return vec![];
    }

    if jobs.len() <= 3 {
        return jobs.to_vec();
    }

    // Try to detect pattern: prefix + numeric suffix
    let pattern_re = Regex::new(r"^(.+?)(\d+)$").unwrap();

    #[derive(Debug)]
    struct JobGroup {
        prefix: String,
        numbers: Vec<(usize, String)>, // (numeric_value, original_string)
    }

    let mut groups: HashMap<String, JobGroup> = HashMap::new();
    let mut non_pattern_jobs = Vec::new();

    for job in jobs {
        if let Some(caps) = pattern_re.captures(job) {
            let prefix = caps.get(1).unwrap().as_str().to_string();
            let num_str = caps.get(2).unwrap().as_str();
            let num_val = num_str.parse::<usize>().unwrap();

            groups
                .entry(prefix.clone())
                .or_insert_with(|| JobGroup {
                    prefix: prefix.clone(),
                    numbers: Vec::new(),
                })
                .numbers
                .push((num_val, num_str.to_string()));
        } else {
            non_pattern_jobs.push(job.clone());
        }
    }

    let mut result = Vec::new();

    // Process each group
    for (_, mut group) in groups {
        group.numbers.sort_by_key(|&(n, _)| n);

        if group.numbers.len() <= 3 {
            // Too few to compact, just add them individually
            for (_, num_str) in &group.numbers {
                result.push(format!("{}{}", group.prefix, num_str));
            }
        } else {
            // Check if they're sequential
            let is_sequential = group.numbers.windows(2).all(|w| w[1].0 == w[0].0 + 1);

            if is_sequential {
                // Compact as range
                let first_num = &group.numbers.first().unwrap().1;
                let last_num = &group.numbers.last().unwrap().1;
                result.push(format!("{}{{{}..{}}}", group.prefix, first_num, last_num));
            } else {
                // Not sequential, check for multiple sequential runs
                let mut runs = Vec::new();
                let mut current_run = vec![group.numbers[0].clone()];

                for i in 1..group.numbers.len() {
                    if group.numbers[i].0 == current_run.last().unwrap().0 + 1 {
                        current_run.push(group.numbers[i].clone());
                    } else {
                        runs.push(current_run);
                        current_run = vec![group.numbers[i].clone()];
                    }
                }
                runs.push(current_run);

                // Compact runs that have 4+ elements
                for run in runs {
                    if run.len() >= 4 {
                        let first_num = &run.first().unwrap().1;
                        let last_num = &run.last().unwrap().1;
                        result.push(format!("{}{{{}..{}}}", group.prefix, first_num, last_num));
                    } else {
                        for (_, num_str) in &run {
                            result.push(format!("{}{}", group.prefix, num_str));
                        }
                    }
                }
            }
        }
    }

    // Add non-pattern jobs
    result.extend(non_pattern_jobs);

    result
}
