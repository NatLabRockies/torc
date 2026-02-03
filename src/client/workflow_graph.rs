//! Workflow Graph - A unified graph-based representation of workflow execution
//!
//! This module provides a directed acyclic graph (DAG) representation of workflow
//! jobs and their dependencies. It supports:
//!
//! - Dependency analysis (topological sorting, levels)
//! - Sub-graph detection (connected components)
//! - Scheduler group generation
//! - Execution plan creation
//!
//! The graph structure enables sophisticated scheduling strategies and visualization.

use regex::Regex;
use std::collections::{HashMap, HashSet, VecDeque};

use crate::client::parameter_expansion::parse_parameter_value;
use crate::client::workflow_spec::{JobSpec, WorkflowActionSpec, WorkflowSpec};
use crate::models::{JobModel, ResourceRequirementsModel};

/// A node in the workflow graph representing a job (or parameterized job template)
#[derive(Debug, Clone)]
pub struct JobNode {
    /// Job name (may contain parameter placeholders like `{index}`)
    pub name: String,
    /// Resource requirements name
    pub resource_requirements: Option<String>,
    /// Number of job instances (1 for non-parameterized, N for parameterized)
    pub instance_count: usize,
    /// Regex pattern matching all instances of this job
    pub name_pattern: String,
    /// Assigned scheduler name
    pub scheduler: Option<String>,
    /// Original job spec reference data
    pub command: String,
}

/// Represents a group of jobs that share scheduling characteristics
#[derive(Debug, Clone)]
pub struct SchedulerGroup {
    /// Resource requirements name
    pub resource_requirements: String,
    /// Whether jobs in this group have dependencies
    pub has_dependencies: bool,
    /// Total job count across all jobs in this group
    pub job_count: usize,
    /// Job name patterns for matching (regex patterns)
    pub job_name_patterns: Vec<String>,
    /// Job names in this group
    pub job_names: Vec<String>,
}

/// A connected component (independent sub-workflow) within the graph
#[derive(Debug, Clone)]
pub struct WorkflowComponent {
    /// Job names in this component
    pub jobs: HashSet<String>,
    /// Root jobs (no dependencies within the component)
    pub roots: Vec<String>,
    /// Leaf jobs (nothing depends on them within the component)
    pub leaves: Vec<String>,
}

/// The main workflow graph structure
#[derive(Debug, Clone)]
pub struct WorkflowGraph {
    /// Job nodes indexed by name
    nodes: HashMap<String, JobNode>,
    /// Forward edges: job → jobs it depends on (blockers)
    depends_on: HashMap<String, HashSet<String>>,
    /// Reverse edges: job → jobs that depend on it (dependents)
    depended_by: HashMap<String, HashSet<String>>,
    /// Cached topological levels (lazily computed)
    levels: Option<Vec<Vec<String>>>,
    /// Cached connected components (lazily computed)
    components: Option<Vec<WorkflowComponent>>,
}

impl WorkflowGraph {
    /// Create a new empty graph
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            depends_on: HashMap::new(),
            depended_by: HashMap::new(),
            levels: None,
            components: None,
        }
    }

    /// Build a workflow graph from a workflow specification
    pub fn from_spec(spec: &WorkflowSpec) -> Result<Self, Box<dyn std::error::Error>> {
        let mut graph = Self::new();

        // First pass: add all job nodes
        for job in &spec.jobs {
            let instance_count = count_job_instances(job);
            let name_pattern =
                build_job_name_pattern(&job.name, job.parameters.is_some(), instance_count);

            let node = JobNode {
                name: job.name.clone(),
                resource_requirements: job.resource_requirements.clone(),
                instance_count,
                name_pattern,
                scheduler: job.scheduler.clone(),
                command: job.command.clone(),
            };

            graph.nodes.insert(job.name.clone(), node);
            graph.depends_on.insert(job.name.clone(), HashSet::new());
            graph.depended_by.insert(job.name.clone(), HashSet::new());
        }

        // Second pass: build dependency edges
        for job in &spec.jobs {
            let mut dependencies = HashSet::new();

            // Explicit dependencies from depends_on
            if let Some(ref deps) = job.depends_on {
                for dep in deps {
                    if graph.nodes.contains_key(dep) {
                        dependencies.insert(dep.clone());
                    }
                }
            }

            // Dependencies from depends_on_regexes
            if let Some(ref regexes) = job.depends_on_regexes {
                for regex_str in regexes {
                    let re = Regex::new(regex_str)?;
                    for other_job in &spec.jobs {
                        if re.is_match(&other_job.name) && other_job.name != job.name {
                            dependencies.insert(other_job.name.clone());
                        }
                    }
                }
            }

            // Implicit dependencies from input files
            if let Some(ref input_files) = job.input_files {
                for input_file in input_files {
                    for other_job in &spec.jobs {
                        if let Some(ref output_files) = other_job.output_files
                            && output_files.contains(input_file)
                            && other_job.name != job.name
                        {
                            dependencies.insert(other_job.name.clone());
                        }
                    }
                }
            }

            // Implicit dependencies from input user data
            if let Some(ref input_data) = job.input_user_data {
                for input_datum in input_data {
                    for other_job in &spec.jobs {
                        if let Some(ref output_data) = other_job.output_user_data
                            && output_data.contains(input_datum)
                            && other_job.name != job.name
                        {
                            dependencies.insert(other_job.name.clone());
                        }
                    }
                }
            }

            // Add edges
            for dep in &dependencies {
                graph
                    .depends_on
                    .get_mut(&job.name)
                    .unwrap()
                    .insert(dep.clone());
                graph
                    .depended_by
                    .get_mut(dep)
                    .unwrap()
                    .insert(job.name.clone());
            }
        }

        Ok(graph)
    }

    /// Build a workflow graph from database models (jobs fetched from server)
    ///
    /// This is used for recovery scenarios and execution plan visualization
    /// when we don't have access to the original workflow specification.
    pub fn from_jobs(
        jobs: &[JobModel],
        resource_requirements: &[ResourceRequirementsModel],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut graph = Self::new();

        // Build resource requirements ID -> name map
        let rr_id_to_name: HashMap<i64, String> = resource_requirements
            .iter()
            .filter_map(|rr| rr.id.map(|id| (id, rr.name.clone())))
            .collect();

        // Build job ID -> name map
        let job_id_to_name: HashMap<i64, String> = jobs
            .iter()
            .filter_map(|j| j.id.map(|id| (id, j.name.clone())))
            .collect();

        // First pass: add all job nodes
        for job in jobs {
            let rr_name = job
                .resource_requirements_id
                .and_then(|rr_id| rr_id_to_name.get(&rr_id).cloned());

            // For database jobs, we treat each job as a single instance
            // (parameterized jobs have already been expanded)
            let node = JobNode {
                name: job.name.clone(),
                resource_requirements: rr_name,
                instance_count: 1,
                name_pattern: regex::escape(&job.name), // Exact match for expanded jobs
                scheduler: None,                        // Not tracked from database models
                command: job.command.clone(),
            };

            graph.nodes.insert(job.name.clone(), node);
            graph.depends_on.insert(job.name.clone(), HashSet::new());
            graph.depended_by.insert(job.name.clone(), HashSet::new());
        }

        // Second pass: build dependency edges
        // First try explicit depends_on_job_ids
        let mut has_any_deps = false;
        for job in jobs {
            let dep_ids = job.depends_on_job_ids.clone().unwrap_or_default();
            if !dep_ids.is_empty() {
                has_any_deps = true;
            }

            for dep_id in &dep_ids {
                if let Some(dep_name) = job_id_to_name.get(dep_id)
                    && graph.nodes.contains_key(dep_name)
                {
                    graph
                        .depends_on
                        .get_mut(&job.name)
                        .unwrap()
                        .insert(dep_name.clone());
                    graph
                        .depended_by
                        .get_mut(dep_name)
                        .unwrap()
                        .insert(job.name.clone());
                }
            }
        }

        // If no explicit dependencies found, compute from file relationships
        // This handles cases where workflow hasn't been initialized yet
        if !has_any_deps {
            // Build file_id -> producing job_id map
            let mut file_producers: HashMap<i64, i64> = HashMap::new();
            for job in jobs {
                if let Some(output_ids) = &job.output_file_ids
                    && let Some(job_id) = job.id
                {
                    for file_id in output_ids {
                        file_producers.insert(*file_id, job_id);
                    }
                }
            }

            // Compute dependencies from input files
            for job in jobs {
                if let Some(input_ids) = &job.input_file_ids {
                    for file_id in input_ids {
                        if let Some(producer_id) = file_producers.get(file_id)
                            && job.id != Some(*producer_id)
                            && let Some(producer_name) = job_id_to_name.get(producer_id)
                            && graph.nodes.contains_key(producer_name)
                        {
                            graph
                                .depends_on
                                .get_mut(&job.name)
                                .unwrap()
                                .insert(producer_name.clone());
                            graph
                                .depended_by
                                .get_mut(producer_name)
                                .unwrap()
                                .insert(job.name.clone());
                        }
                    }
                }
            }
        }

        Ok(graph)
    }

    /// Get all job names in the graph
    pub fn job_names(&self) -> impl Iterator<Item = &String> {
        self.nodes.keys()
    }

    /// Get a job node by name
    pub fn get_job(&self, name: &str) -> Option<&JobNode> {
        self.nodes.get(name)
    }

    /// Get the number of jobs in the graph
    pub fn job_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get total instance count (accounting for parameterized jobs)
    pub fn total_instance_count(&self) -> usize {
        self.nodes.values().map(|n| n.instance_count).sum()
    }

    /// Check if a job has any dependencies
    pub fn has_dependencies(&self, job: &str) -> bool {
        self.depends_on
            .get(job)
            .map(|deps| !deps.is_empty())
            .unwrap_or(false)
    }

    /// Get the jobs that a job depends on (its blockers)
    pub fn dependencies_of(&self, job: &str) -> Option<&HashSet<String>> {
        self.depends_on.get(job)
    }

    /// Get the jobs that depend on a job (its dependents)
    pub fn dependents_of(&self, job: &str) -> Option<&HashSet<String>> {
        self.depended_by.get(job)
    }

    /// Get root jobs (jobs with no dependencies)
    pub fn roots(&self) -> Vec<&str> {
        self.nodes
            .keys()
            .filter(|name| {
                self.depends_on
                    .get(*name)
                    .map(|deps| deps.is_empty())
                    .unwrap_or(true)
            })
            .map(|s| s.as_str())
            .collect()
    }

    /// Get leaf jobs (jobs that nothing depends on)
    pub fn leaves(&self) -> Vec<&str> {
        self.nodes
            .keys()
            .filter(|name| {
                self.depended_by
                    .get(*name)
                    .map(|deps| deps.is_empty())
                    .unwrap_or(true)
            })
            .map(|s| s.as_str())
            .collect()
    }

    /// Compute topological levels (jobs grouped by dependency depth)
    ///
    /// Level 0 contains jobs with no dependencies.
    /// Level N contains jobs whose dependencies are all in levels < N.
    pub fn topological_levels(&mut self) -> Result<&Vec<Vec<String>>, Box<dyn std::error::Error>> {
        if let Some(ref levels) = self.levels {
            return Ok(levels);
        }

        let mut levels = Vec::new();
        let mut remaining: HashSet<String> = self.nodes.keys().cloned().collect();
        let mut processed = HashSet::new();

        while !remaining.is_empty() {
            let mut current_level = Vec::new();

            // Find all jobs whose dependencies are satisfied
            for name in &remaining {
                let deps = self.depends_on.get(name).unwrap();
                if deps.iter().all(|d| processed.contains(d)) {
                    current_level.push(name.clone());
                }
            }

            if current_level.is_empty() {
                return Err("Circular dependency detected in workflow graph".into());
            }

            // Mark these jobs as processed
            for job in &current_level {
                remaining.remove(job);
                processed.insert(job.clone());
            }

            levels.push(current_level);
        }

        self.levels = Some(levels);
        Ok(self.levels.as_ref().unwrap())
    }

    /// Find connected components (independent sub-workflows)
    ///
    /// Each component can be scheduled independently of others.
    pub fn connected_components(&mut self) -> &Vec<WorkflowComponent> {
        if let Some(ref components) = self.components {
            return components;
        }

        let mut components = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();

        for start_job in self.nodes.keys() {
            if visited.contains(start_job) {
                continue;
            }

            // BFS to find all connected jobs (treating graph as undirected)
            let mut component_jobs = HashSet::new();
            let mut queue = VecDeque::new();
            queue.push_back(start_job.clone());

            while let Some(job) = queue.pop_front() {
                if visited.contains(&job) {
                    continue;
                }
                visited.insert(job.clone());
                component_jobs.insert(job.clone());

                // Add dependencies (forward edges)
                if let Some(deps) = self.depends_on.get(&job) {
                    for dep in deps {
                        if !visited.contains(dep) {
                            queue.push_back(dep.clone());
                        }
                    }
                }

                // Add dependents (reverse edges)
                if let Some(dependents) = self.depended_by.get(&job) {
                    for dependent in dependents {
                        if !visited.contains(dependent) {
                            queue.push_back(dependent.clone());
                        }
                    }
                }
            }

            // Find roots and leaves within this component
            let roots: Vec<String> = component_jobs
                .iter()
                .filter(|name| {
                    self.depends_on
                        .get(*name)
                        .map(|deps| deps.iter().all(|d| !component_jobs.contains(d)))
                        .unwrap_or(true)
                })
                .cloned()
                .collect();

            let leaves: Vec<String> = component_jobs
                .iter()
                .filter(|name| {
                    self.depended_by
                        .get(*name)
                        .map(|deps| deps.iter().all(|d| !component_jobs.contains(d)))
                        .unwrap_or(true)
                })
                .cloned()
                .collect();

            components.push(WorkflowComponent {
                jobs: component_jobs,
                roots,
                leaves,
            });
        }

        self.components = Some(components);
        self.components.as_ref().unwrap()
    }

    /// Extract a sub-graph containing only the specified jobs
    pub fn subgraph(&self, job_names: &HashSet<String>) -> Self {
        let mut subgraph = Self::new();

        // Copy relevant nodes
        for name in job_names {
            if let Some(node) = self.nodes.get(name) {
                subgraph.nodes.insert(name.clone(), node.clone());
                subgraph.depends_on.insert(name.clone(), HashSet::new());
                subgraph.depended_by.insert(name.clone(), HashSet::new());
            }
        }

        // Copy relevant edges (only those within the subgraph)
        for name in job_names {
            if let Some(deps) = self.depends_on.get(name) {
                for dep in deps {
                    if job_names.contains(dep) {
                        subgraph
                            .depends_on
                            .get_mut(name)
                            .unwrap()
                            .insert(dep.clone());
                        subgraph
                            .depended_by
                            .get_mut(dep)
                            .unwrap()
                            .insert(name.clone());
                    }
                }
            }
        }

        subgraph
    }

    /// Generate scheduler groups based on (resource_requirements, has_dependencies)
    ///
    /// Jobs are grouped by their resource requirements and dependency status.
    /// This is used for scheduler generation.
    pub fn scheduler_groups(&self) -> Vec<SchedulerGroup> {
        // Group by (resource_requirements, has_dependencies)
        let mut groups: HashMap<(String, bool), SchedulerGroup> = HashMap::new();

        for (name, node) in &self.nodes {
            let rr_name = match &node.resource_requirements {
                Some(rr) => rr.clone(),
                None => continue, // Skip jobs without resource requirements
            };

            let has_deps = self.has_dependencies(name);
            let key = (rr_name.clone(), has_deps);

            let group = groups.entry(key).or_insert_with(|| SchedulerGroup {
                resource_requirements: rr_name,
                has_dependencies: has_deps,
                job_count: 0,
                job_name_patterns: Vec::new(),
                job_names: Vec::new(),
            });

            group.job_count += node.instance_count;
            group.job_name_patterns.push(node.name_pattern.clone());
            group.job_names.push(name.clone());
        }

        groups.into_values().collect()
    }

    /// Generate scheduler groups per connected component
    ///
    /// Returns a map of component index to scheduler groups for that component.
    pub fn scheduler_groups_by_component(
        &mut self,
    ) -> Vec<(WorkflowComponent, Vec<SchedulerGroup>)> {
        let components = self.connected_components().clone();
        let mut result = Vec::new();

        for component in components {
            let subgraph = self.subgraph(&component.jobs);
            let groups = subgraph.scheduler_groups();
            result.push((component, groups));
        }

        result
    }

    /// Find the critical path (longest path through the graph)
    ///
    /// Returns job names along the critical path and the total instance count.
    pub fn critical_path(&mut self) -> Result<(Vec<String>, usize), Box<dyn std::error::Error>> {
        // Use dynamic programming on topological order
        let levels = self.topological_levels()?.clone();

        // dist[job] = (max distance to reach this job, predecessor)
        let mut dist: HashMap<String, (usize, Option<String>)> = HashMap::new();

        for name in self.nodes.keys() {
            dist.insert(name.clone(), (0, None));
        }

        // Process in topological order
        for level in &levels {
            for job in level {
                let node = self.nodes.get(job).unwrap();
                let job_weight = node.instance_count;

                if let Some(dependents) = self.depended_by.get(job) {
                    for dependent in dependents {
                        let current_dist = dist.get(job).unwrap().0 + job_weight;
                        let dependent_dist = dist.get(dependent).unwrap().0;

                        if current_dist > dependent_dist {
                            dist.insert(dependent.clone(), (current_dist, Some(job.clone())));
                        }
                    }
                }
            }
        }

        // Find the job with maximum distance (end of critical path)
        let (end_job, (_max_dist, _)) = dist
            .iter()
            .max_by_key(|(_, (d, _))| d)
            .ok_or("Empty graph")?;

        // Backtrack to find the path
        let mut path = vec![end_job.clone()];
        let mut current = end_job.clone();

        while let Some((_, Some(prev))) = dist.get(&current) {
            path.push(prev.clone());
            current = prev.clone();
        }

        path.reverse();

        // Calculate total instance count along critical path
        let total: usize = path
            .iter()
            .filter_map(|name| self.nodes.get(name))
            .map(|n| n.instance_count)
            .sum();

        Ok((path, total))
    }

    /// Get jobs that become ready when a set of jobs complete
    pub fn jobs_unblocked_by(&self, completed_jobs: &HashSet<String>) -> Vec<String> {
        let mut unblocked = Vec::new();

        for (name, deps) in &self.depends_on {
            if completed_jobs.contains(name) {
                continue; // Already completed
            }

            // Check if all dependencies are in completed_jobs
            if deps.iter().all(|d| completed_jobs.contains(d)) && !deps.is_empty() {
                unblocked.push(name.clone());
            }
        }

        unblocked
    }

    /// Find actions that should trigger when specific jobs become ready
    pub fn matching_actions<'a>(
        &self,
        jobs_becoming_ready: &[String],
        actions: &'a [WorkflowActionSpec],
    ) -> Vec<&'a WorkflowActionSpec> {
        let mut matching = Vec::new();

        for action in actions {
            if action.trigger_type != "on_jobs_ready" {
                continue;
            }

            // Check job_name_regexes
            if let Some(ref regexes) = action.job_name_regexes {
                for regex_str in regexes {
                    if let Ok(re) = Regex::new(regex_str)
                        && jobs_becoming_ready.iter().any(|j| re.is_match(j))
                    {
                        matching.push(action);
                        break;
                    }
                }
            }

            // Check exact job names
            if let Some(ref job_names) = action.jobs
                && jobs_becoming_ready.iter().any(|j| job_names.contains(j))
                && !matching.contains(&action)
            {
                matching.push(action);
            }
        }

        matching
    }
}

impl Default for WorkflowGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Count job instances for parameterized jobs
fn count_job_instances(job: &JobSpec) -> usize {
    if let Some(params) = &job.parameters {
        let mut count = 1usize;
        for value in params.values() {
            let param_count = parse_parameter_value(value).map(|pv| pv.len()).unwrap_or(1);
            count *= param_count;
        }
        count
    } else {
        1
    }
}

/// Build a regex pattern for matching job instances
fn build_job_name_pattern(name: &str, is_parameterized: bool, instance_count: usize) -> String {
    if is_parameterized && instance_count > 1 {
        // Convert parameterized job name like "work_{index}" to regex "^work_.*$"
        let param_regex = Regex::new(r"\{[^}]+\}").unwrap();
        let pattern = param_regex.replace_all(name, ".*").to_string();
        // Simplify consecutive .* patterns
        let pattern = pattern.replace(".*.*", ".*");
        format!("^{}$", pattern)
    } else {
        // Exact match for non-parameterized jobs
        format!("^{}$", regex::escape(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_spec() -> WorkflowSpec {
        WorkflowSpec {
            name: "test_workflow".to_string(),
            jobs: vec![
                JobSpec {
                    name: "preprocess".to_string(),
                    command: "preprocess.sh".to_string(),
                    resource_requirements: Some("small".to_string()),
                    ..Default::default()
                },
                JobSpec {
                    name: "work_{i}".to_string(),
                    command: "work.sh".to_string(),
                    resource_requirements: Some("medium".to_string()),
                    depends_on: Some(vec!["preprocess".to_string()]),
                    parameters: Some({
                        let mut m = HashMap::new();
                        m.insert("i".to_string(), "1:10".to_string());
                        m
                    }),
                    ..Default::default()
                },
                JobSpec {
                    name: "postprocess".to_string(),
                    command: "postprocess.sh".to_string(),
                    resource_requirements: Some("small".to_string()),
                    depends_on_regexes: Some(vec!["^work_.*$".to_string()]),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn test_graph_construction() {
        let spec = create_test_spec();
        let graph = WorkflowGraph::from_spec(&spec).unwrap();

        assert_eq!(graph.job_count(), 3);
        assert_eq!(graph.total_instance_count(), 12); // 1 + 10 + 1
    }

    #[test]
    fn test_roots_and_leaves() {
        let spec = create_test_spec();
        let graph = WorkflowGraph::from_spec(&spec).unwrap();

        let roots = graph.roots();
        assert_eq!(roots.len(), 1);
        assert!(roots.contains(&"preprocess"));

        let leaves = graph.leaves();
        assert_eq!(leaves.len(), 1);
        assert!(leaves.contains(&"postprocess"));
    }

    #[test]
    fn test_dependencies() {
        let spec = create_test_spec();
        let graph = WorkflowGraph::from_spec(&spec).unwrap();

        assert!(!graph.has_dependencies("preprocess"));
        assert!(graph.has_dependencies("work_{i}"));
        assert!(graph.has_dependencies("postprocess"));

        let work_deps = graph.dependencies_of("work_{i}").unwrap();
        assert!(work_deps.contains("preprocess"));

        let post_deps = graph.dependencies_of("postprocess").unwrap();
        assert!(post_deps.contains("work_{i}"));
    }

    #[test]
    fn test_topological_levels() {
        let spec = create_test_spec();
        let mut graph = WorkflowGraph::from_spec(&spec).unwrap();

        let levels = graph.topological_levels().unwrap();
        assert_eq!(levels.len(), 3);
        assert!(levels[0].contains(&"preprocess".to_string()));
        assert!(levels[1].contains(&"work_{i}".to_string()));
        assert!(levels[2].contains(&"postprocess".to_string()));
    }

    #[test]
    fn test_connected_components_single() {
        let spec = create_test_spec();
        let mut graph = WorkflowGraph::from_spec(&spec).unwrap();

        let components = graph.connected_components();
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].jobs.len(), 3);
    }

    #[test]
    fn test_connected_components_multiple() {
        // Create spec with two independent pipelines
        let spec = WorkflowSpec {
            name: "multi_pipeline".to_string(),
            jobs: vec![
                // Pipeline A
                JobSpec {
                    name: "a_start".to_string(),
                    command: "a.sh".to_string(),
                    resource_requirements: Some("small".to_string()),
                    ..Default::default()
                },
                JobSpec {
                    name: "a_end".to_string(),
                    command: "a.sh".to_string(),
                    resource_requirements: Some("small".to_string()),
                    depends_on: Some(vec!["a_start".to_string()]),
                    ..Default::default()
                },
                // Pipeline B (independent)
                JobSpec {
                    name: "b_start".to_string(),
                    command: "b.sh".to_string(),
                    resource_requirements: Some("small".to_string()),
                    ..Default::default()
                },
                JobSpec {
                    name: "b_end".to_string(),
                    command: "b.sh".to_string(),
                    resource_requirements: Some("small".to_string()),
                    depends_on: Some(vec!["b_start".to_string()]),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let mut graph = WorkflowGraph::from_spec(&spec).unwrap();
        let components = graph.connected_components();

        assert_eq!(components.len(), 2);

        // Each component should have 2 jobs
        for component in components {
            assert_eq!(component.jobs.len(), 2);
            assert_eq!(component.roots.len(), 1);
            assert_eq!(component.leaves.len(), 1);
        }
    }

    #[test]
    fn test_scheduler_groups() {
        let spec = create_test_spec();
        let graph = WorkflowGraph::from_spec(&spec).unwrap();

        let groups = graph.scheduler_groups();

        // Should have 4 groups:
        // (small, no_deps), (medium, has_deps), (small, has_deps)
        // Wait, preprocess is small/no_deps, work is medium/has_deps, postprocess is small/has_deps
        assert_eq!(groups.len(), 3);

        // Find the work group
        let work_group = groups
            .iter()
            .find(|g| g.resource_requirements == "medium")
            .unwrap();
        assert_eq!(work_group.job_count, 10); // Parameterized
        assert!(work_group.has_dependencies);
    }

    #[test]
    fn test_subgraph() {
        let spec = create_test_spec();
        let graph = WorkflowGraph::from_spec(&spec).unwrap();

        let mut subset = HashSet::new();
        subset.insert("preprocess".to_string());
        subset.insert("work_{i}".to_string());

        let subgraph = graph.subgraph(&subset);
        assert_eq!(subgraph.job_count(), 2);

        // work_{i} should still depend on preprocess
        assert!(subgraph.has_dependencies("work_{i}"));
        assert!(!subgraph.has_dependencies("preprocess"));
    }

    #[test]
    fn test_jobs_unblocked_by() {
        let spec = create_test_spec();
        let graph = WorkflowGraph::from_spec(&spec).unwrap();

        // When preprocess completes, work_{i} becomes ready
        let mut completed = HashSet::new();
        completed.insert("preprocess".to_string());

        let unblocked = graph.jobs_unblocked_by(&completed);
        assert_eq!(unblocked.len(), 1);
        assert!(unblocked.contains(&"work_{i}".to_string()));

        // When work_{i} also completes, postprocess becomes ready
        completed.insert("work_{i}".to_string());
        let unblocked = graph.jobs_unblocked_by(&completed);
        assert_eq!(unblocked.len(), 1);
        assert!(unblocked.contains(&"postprocess".to_string()));
    }
}
