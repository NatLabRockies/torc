//! Tests for the `torc slurm regenerate` command.
//!
//! These tests simulate failure recovery scenarios where we need to regenerate
//! Slurm schedulers for pending jobs (uninitialized, ready, blocked) after
//! some jobs have completed or failed.

mod common;

use common::{ServerProcess, run_cli_with_json, start_server};
use rstest::rstest;
use std::collections::HashMap;
use torc::client::{Configuration, apis};
use torc::models;

/// Create a workflow with jobs in various states for testing regenerate.
/// Returns (workflow_id, job_ids_by_status)
fn create_workflow_with_job_states(
    config: &Configuration,
    name: &str,
    job_configs: &[(String, models::JobStatus)],
) -> (i64, HashMap<String, i64>) {
    // Create workflow
    let user = "test_user".to_string();
    let workflow = models::WorkflowModel::new(name.to_string(), user);
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create resource requirements (using "test_rr" since "default" is reserved)
    let mut rr = models::ResourceRequirementsModel::new(workflow_id, "test_rr".to_string());
    rr.num_cpus = 4;
    rr.num_gpus = 0;
    rr.num_nodes = 1;
    rr.memory = "8g".to_string();
    rr.runtime = "PT1H".to_string();
    let rr = apis::resource_requirements_api::create_resource_requirements(config, rr)
        .expect("Failed to create resource requirements");
    let rr_id = rr.id.unwrap();

    // Create jobs
    let mut job_ids = HashMap::new();
    for (job_name, _status) in job_configs {
        let mut job = models::JobModel::new(
            workflow_id,
            job_name.clone(),
            format!("echo '{}'", job_name),
        );
        job.resource_requirements_id = Some(rr_id);
        let created_job = apis::jobs_api::create_job(config, job).expect("Failed to create job");
        job_ids.insert(job_name.clone(), created_job.id.unwrap());
    }

    // Initialize jobs - after this, jobs without dependencies will be "ready",
    // jobs with dependencies will be "blocked"
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to initialize jobs");

    (workflow_id, job_ids)
}

/// Create a multi-stage workflow with dependencies.
/// Stage 1: preprocess (no dependencies) -> Stage 2: work jobs (depend on preprocess) -> Stage 3: postprocess (depends on all work)
fn create_multi_stage_workflow(
    config: &Configuration,
    name: &str,
    num_work_jobs: usize,
) -> (i64, HashMap<String, i64>) {
    let user = "test_user".to_string();
    let workflow = models::WorkflowModel::new(name.to_string(), user);
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create resource requirements (using "test_rr" since "default" is reserved)
    let mut rr = models::ResourceRequirementsModel::new(workflow_id, "test_rr".to_string());
    rr.num_cpus = 4;
    rr.num_gpus = 0;
    rr.num_nodes = 1;
    rr.memory = "8g".to_string();
    rr.runtime = "PT1H".to_string();
    let rr = apis::resource_requirements_api::create_resource_requirements(config, rr)
        .expect("Failed to create resource requirements");
    let rr_id = rr.id.unwrap();

    // Create files for dependencies
    let prep_output = apis::files_api::create_file(
        config,
        models::FileModel::new(
            workflow_id,
            "prep_output".to_string(),
            "/tmp/prep.out".to_string(),
        ),
    )
    .expect("Failed to create file");

    let work_outputs: Vec<_> = (0..num_work_jobs)
        .map(|i| {
            apis::files_api::create_file(
                config,
                models::FileModel::new(
                    workflow_id,
                    format!("work_output_{}", i),
                    format!("/tmp/work_{}.out", i),
                ),
            )
            .expect("Failed to create file")
        })
        .collect();

    // Stage 1: preprocess
    let mut preprocess = models::JobModel::new(
        workflow_id,
        "preprocess".to_string(),
        "echo preprocess".to_string(),
    );
    preprocess.resource_requirements_id = Some(rr_id);
    preprocess.output_file_ids = Some(vec![prep_output.id.unwrap()]);
    let preprocess =
        apis::jobs_api::create_job(config, preprocess).expect("Failed to create preprocess job");

    // Stage 2: work jobs (depend on preprocess via file)
    let mut work_jobs = Vec::new();
    for (i, work_output) in work_outputs.iter().enumerate() {
        let mut work = models::JobModel::new(
            workflow_id,
            format!("work_{}", i),
            format!("echo work_{}", i),
        );
        work.resource_requirements_id = Some(rr_id);
        work.input_file_ids = Some(vec![prep_output.id.unwrap()]);
        work.output_file_ids = Some(vec![work_output.id.unwrap()]);
        let work = apis::jobs_api::create_job(config, work).expect("Failed to create work job");
        work_jobs.push(work);
    }

    // Stage 3: postprocess (depends on all work jobs via files)
    let mut postprocess = models::JobModel::new(
        workflow_id,
        "postprocess".to_string(),
        "echo postprocess".to_string(),
    );
    postprocess.resource_requirements_id = Some(rr_id);
    postprocess.input_file_ids = Some(work_outputs.iter().map(|f| f.id.unwrap()).collect());
    let postprocess =
        apis::jobs_api::create_job(config, postprocess).expect("Failed to create postprocess job");

    // Initialize workflow
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to initialize jobs");

    // Build job_ids map
    let mut job_ids = HashMap::new();
    job_ids.insert("preprocess".to_string(), preprocess.id.unwrap());
    for (i, work) in work_jobs.iter().enumerate() {
        job_ids.insert(format!("work_{}", i), work.id.unwrap());
    }
    job_ids.insert("postprocess".to_string(), postprocess.id.unwrap());

    (workflow_id, job_ids)
}

/// Create a workflow with jobs having different resource requirements.
fn create_workflow_with_varied_resources(
    config: &Configuration,
    name: &str,
) -> (i64, HashMap<String, i64>) {
    let user = "test_user".to_string();
    let workflow = models::WorkflowModel::new(name.to_string(), user);
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create small resource requirements (many jobs per node)
    let mut rr_small = models::ResourceRequirementsModel::new(workflow_id, "small".to_string());
    rr_small.num_cpus = 4;
    rr_small.num_gpus = 0;
    rr_small.num_nodes = 1;
    rr_small.memory = "8g".to_string();
    rr_small.runtime = "PT1H".to_string();
    let rr_small = apis::resource_requirements_api::create_resource_requirements(config, rr_small)
        .expect("Failed to create small resource requirements");

    // Create large resource requirements (one job per node)
    let mut rr_large = models::ResourceRequirementsModel::new(workflow_id, "large".to_string());
    rr_large.num_cpus = 64;
    rr_large.num_gpus = 0;
    rr_large.num_nodes = 1;
    rr_large.memory = "120g".to_string();
    rr_large.runtime = "PT4H".to_string();
    let rr_large = apis::resource_requirements_api::create_resource_requirements(config, rr_large)
        .expect("Failed to create large resource requirements");

    // Create GPU resource requirements
    let mut rr_gpu = models::ResourceRequirementsModel::new(workflow_id, "gpu".to_string());
    rr_gpu.num_cpus = 32;
    rr_gpu.num_gpus = 2;
    rr_gpu.num_nodes = 1;
    rr_gpu.memory = "64g".to_string();
    rr_gpu.runtime = "PT2H".to_string();
    let rr_gpu = apis::resource_requirements_api::create_resource_requirements(config, rr_gpu)
        .expect("Failed to create GPU resource requirements");

    // Create jobs with different resource requirements
    let mut job_ids = HashMap::new();

    // Small jobs
    for i in 0..5 {
        let mut job = models::JobModel::new(
            workflow_id,
            format!("small_job_{}", i),
            format!("echo small_{}", i),
        );
        job.resource_requirements_id = Some(rr_small.id.unwrap());
        let created = apis::jobs_api::create_job(config, job).expect("Failed to create small job");
        job_ids.insert(format!("small_job_{}", i), created.id.unwrap());
    }

    // Large jobs
    for i in 0..3 {
        let mut job = models::JobModel::new(
            workflow_id,
            format!("large_job_{}", i),
            format!("echo large_{}", i),
        );
        job.resource_requirements_id = Some(rr_large.id.unwrap());
        let created = apis::jobs_api::create_job(config, job).expect("Failed to create large job");
        job_ids.insert(format!("large_job_{}", i), created.id.unwrap());
    }

    // GPU jobs
    for i in 0..2 {
        let mut job = models::JobModel::new(
            workflow_id,
            format!("gpu_job_{}", i),
            format!("echo gpu_{}", i),
        );
        job.resource_requirements_id = Some(rr_gpu.id.unwrap());
        let created = apis::jobs_api::create_job(config, job).expect("Failed to create GPU job");
        job_ids.insert(format!("gpu_job_{}", i), created.id.unwrap());
    }

    // Initialize workflow
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to initialize jobs");

    (workflow_id, job_ids)
}

/// Helper to get the number of schedulers for a workflow
fn get_scheduler_count(config: &Configuration, workflow_id: i64) -> usize {
    let response = apis::slurm_schedulers_api::list_slurm_schedulers(
        config,
        workflow_id,
        Some(0),
        Some(100),
        None,
        None,
    )
    .expect("Failed to list schedulers");
    response.items.len()
}

// ============== Basic Regenerate Tests ==============

/// Test regenerate with all jobs in ready state (basic case)
#[rstest]
fn test_regenerate_all_jobs_ready(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow with 5 ready jobs
    let job_configs: Vec<(String, models::JobStatus)> = (0..5)
        .map(|i| (format!("job_{}", i), models::JobStatus::Ready))
        .collect();

    let (workflow_id, _job_ids) =
        create_workflow_with_job_states(config, "test_regenerate_all_ready", &job_configs);

    // Verify no schedulers exist yet
    assert_eq!(get_scheduler_count(config, workflow_id), 0);

    // Run regenerate command with kestrel profile
    let args = [
        "slurm",
        "regenerate",
        &workflow_id.to_string(),
        "--account",
        "test_account",
        "--profile",
        "kestrel",
    ];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(result.is_ok(), "Regenerate command failed: {:?}", result);

    let json = result.unwrap();
    assert!(json.get("pending_jobs").is_some());
    assert_eq!(json.get("pending_jobs").unwrap().as_i64().unwrap(), 5);

    // Verify schedulers were created
    assert!(get_scheduler_count(config, workflow_id) > 0);
}

/// Test regenerate with no pending jobs (empty workflow)
#[rstest]
fn test_regenerate_no_pending_jobs(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow with no jobs (empty workflow)
    let user = "test_user".to_string();
    let workflow = models::WorkflowModel::new("test_regenerate_no_pending".to_string(), user);
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Run regenerate command
    let args = [
        "slurm",
        "regenerate",
        &workflow_id.to_string(),
        "--account",
        "test_account",
        "--profile",
        "kestrel",
    ];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(result.is_ok(), "Regenerate command failed: {:?}", result);

    let json = result.unwrap();
    // Should report 0 pending jobs
    assert_eq!(json.get("pending_jobs").unwrap().as_i64().unwrap(), 0);
    assert!(json.get("warnings").is_some());

    // No schedulers should be created
    assert_eq!(get_scheduler_count(config, workflow_id), 0);
}

/// Test regenerate with many ready jobs (simple case with different job counts)
#[rstest]
fn test_regenerate_multiple_ready_jobs(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow with many ready jobs
    let job_configs: Vec<(String, models::JobStatus)> = (0..10)
        .map(|i| (format!("job_{}", i), models::JobStatus::Ready))
        .collect();

    let (workflow_id, _job_ids) =
        create_workflow_with_job_states(config, "test_regenerate_multiple", &job_configs);

    // Run regenerate command
    let args = [
        "slurm",
        "regenerate",
        &workflow_id.to_string(),
        "--account",
        "test_account",
        "--profile",
        "kestrel",
    ];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(result.is_ok(), "Regenerate command failed: {:?}", result);

    let json = result.unwrap();
    // Should count all 10 ready jobs
    assert_eq!(json.get("pending_jobs").unwrap().as_i64().unwrap(), 10);

    // Schedulers should be created
    assert!(get_scheduler_count(config, workflow_id) > 0);
}

// ============== Multi-Stage Workflow Tests ==============

/// Test regenerate with blocked jobs from multi-stage workflow
/// Jobs with unmet dependencies are blocked after initialization
#[rstest]
fn test_regenerate_with_blocked_jobs(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create multi-stage workflow - work and postprocess jobs will be blocked
    // because preprocess hasn't completed
    let (workflow_id, _job_ids) = create_multi_stage_workflow(config, "test_blocked_jobs", 5);

    // Run regenerate command - should count blocked jobs as pending
    let args = [
        "slurm",
        "regenerate",
        &workflow_id.to_string(),
        "--account",
        "test_account",
        "--profile",
        "kestrel",
    ];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(result.is_ok(), "Regenerate command failed: {:?}", result);

    let json = result.unwrap();
    // Should have: 1 ready (preprocess) + 5 blocked (work) + 1 blocked (postprocess) = 7
    let pending_jobs = json.get("pending_jobs").unwrap().as_i64().unwrap();
    assert_eq!(
        pending_jobs, 7,
        "Expected 7 pending jobs (1 ready + 6 blocked), got {}",
        pending_jobs
    );

    // Schedulers should be created
    assert!(get_scheduler_count(config, workflow_id) > 0);
}

/// Test regenerate counts both ready and blocked jobs correctly
#[rstest]
fn test_regenerate_counts_all_pending_statuses(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a larger multi-stage workflow to verify counting
    let (workflow_id, _job_ids) = create_multi_stage_workflow(config, "test_pending_count", 10);

    // Run regenerate command
    let args = [
        "slurm",
        "regenerate",
        &workflow_id.to_string(),
        "--account",
        "test_account",
        "--profile",
        "kestrel",
    ];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(result.is_ok(), "Regenerate command failed: {:?}", result);

    let json = result.unwrap();
    // Should have: 1 ready (preprocess) + 10 blocked (work) + 1 blocked (postprocess) = 12
    let pending_jobs = json.get("pending_jobs").unwrap().as_i64().unwrap();
    assert_eq!(
        pending_jobs, 12,
        "Expected 12 pending jobs, got {}",
        pending_jobs
    );
}

// ============== Resource Requirement Tests ==============

/// Test regenerate creates separate schedulers for different resource requirements
#[rstest]
fn test_regenerate_varied_resources(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow with varied resource requirements
    let (workflow_id, _job_ids) =
        create_workflow_with_varied_resources(config, "test_varied_resources");

    // Run regenerate command
    let args = [
        "slurm",
        "regenerate",
        &workflow_id.to_string(),
        "--account",
        "test_account",
        "--profile",
        "kestrel",
    ];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(result.is_ok(), "Regenerate command failed: {:?}", result);

    let json = result.unwrap();
    // Should have all 10 jobs pending
    assert_eq!(json.get("pending_jobs").unwrap().as_i64().unwrap(), 10);

    // Check schedulers_created array
    let schedulers_created = json.get("schedulers_created").unwrap().as_array().unwrap();
    // Should have created multiple schedulers for different resource types
    assert!(
        schedulers_created.len() >= 2,
        "Expected at least 2 schedulers for varied resources, got {}",
        schedulers_created.len()
    );
}

/// Test regenerate with single allocation mode
#[rstest]
fn test_regenerate_single_allocation(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow with many jobs
    let job_configs: Vec<(String, models::JobStatus)> = (0..20)
        .map(|i| (format!("job_{}", i), models::JobStatus::Ready))
        .collect();

    let (workflow_id, _job_ids) =
        create_workflow_with_job_states(config, "test_single_allocation", &job_configs);

    // Run regenerate command with --single-allocation
    let args = [
        "slurm",
        "regenerate",
        &workflow_id.to_string(),
        "--account",
        "test_account",
        "--profile",
        "kestrel",
        "--single-allocation",
    ];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(result.is_ok(), "Regenerate command failed: {:?}", result);

    let json = result.unwrap();
    assert_eq!(json.get("pending_jobs").unwrap().as_i64().unwrap(), 20);

    // In single allocation mode, should create fewer, larger allocations
    let total_allocations = json.get("total_allocations").unwrap().as_i64().unwrap();
    let schedulers = json.get("schedulers_created").unwrap().as_array().unwrap();

    // With single allocation, we expect 1 scheduler with 1 (larger) allocation
    assert_eq!(
        schedulers.len(),
        1,
        "Single allocation should create 1 scheduler"
    );
    assert_eq!(
        total_allocations, 1,
        "Single allocation mode should create 1 allocation"
    );
}

// ============== Existing Scheduler Tests ==============

/// Test regenerate uses existing scheduler's account as default
#[rstest]
fn test_regenerate_uses_existing_account(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow
    let job_configs: Vec<(String, models::JobStatus)> = (0..3)
        .map(|i| (format!("job_{}", i), models::JobStatus::Ready))
        .collect();

    let (workflow_id, _job_ids) =
        create_workflow_with_job_states(config, "test_existing_account", &job_configs);

    // Create an existing scheduler with a specific account
    let scheduler = models::SlurmSchedulerModel {
        id: None,
        workflow_id,
        name: Some("existing_scheduler".to_string()),
        account: "existing_project_account".to_string(),
        partition: None,
        mem: Some("8g".to_string()),
        walltime: "01:00:00".to_string(),
        nodes: 1,
        gres: None,
        ntasks_per_node: None,
        qos: None,
        tmp: None,
        extra: None,
    };
    apis::slurm_schedulers_api::create_slurm_scheduler(config, scheduler)
        .expect("Failed to create scheduler");

    // Run regenerate without specifying account (should use existing)
    let args = [
        "slurm",
        "regenerate",
        &workflow_id.to_string(),
        "--profile",
        "kestrel",
    ];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(result.is_ok(), "Regenerate command failed: {:?}", result);

    // Verify new scheduler uses the existing account
    let response = apis::slurm_schedulers_api::list_slurm_schedulers(
        config,
        workflow_id,
        Some(0),
        Some(100),
        None,
        None,
    )
    .expect("Failed to list schedulers");

    let schedulers = response.items;
    // Should have 2 schedulers now (original + regenerated)
    assert!(schedulers.len() >= 2);

    // Find the regenerated scheduler (has "regen" in name)
    let regen_scheduler = schedulers.iter().find(|s| {
        s.name
            .as_ref()
            .map(|n| n.contains("regen"))
            .unwrap_or(false)
    });
    assert!(
        regen_scheduler.is_some(),
        "Should have regenerated scheduler"
    );
    assert_eq!(regen_scheduler.unwrap().account, "existing_project_account");
}

// ============== Edge Case Tests ==============

/// Test regenerate with jobs that use default resource requirements
/// When a job is created without specifying resource requirements, the server
/// automatically assigns the workflow's default resource requirements.
#[rstest]
fn test_regenerate_with_default_resource_requirements(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow (this also creates a "default" resource requirement automatically)
    let user = "test_user".to_string();
    let workflow = models::WorkflowModel::new("test_default_rr".to_string(), user);
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create job WITHOUT explicit resource requirements
    // The server will automatically assign the default resource requirements
    let job = models::JobModel::new(
        workflow_id,
        "job_with_default_rr".to_string(),
        "echo test".to_string(),
    );
    let created_job = apis::jobs_api::create_job(config, job).expect("Failed to create job");

    // Verify that the server assigned a resource_requirements_id
    assert!(
        created_job.resource_requirements_id.is_some(),
        "Server should have assigned default resource requirements"
    );

    // Initialize
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to initialize");

    // Run regenerate command
    let args = [
        "slurm",
        "regenerate",
        &workflow_id.to_string(),
        "--account",
        "test_account",
        "--profile",
        "kestrel",
    ];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(result.is_ok(), "Regenerate command failed: {:?}", result);

    let json = result.unwrap();

    // Should have 1 pending job (the one we created)
    assert_eq!(
        json.get("pending_jobs").unwrap().as_i64().unwrap(),
        1,
        "Should have 1 pending job"
    );

    // Should create a scheduler for the default resource requirements
    let schedulers = json.get("schedulers_created").unwrap().as_array().unwrap();
    assert_eq!(schedulers.len(), 1, "Should create 1 scheduler");

    // No warnings expected since job has resource requirements (the default)
    let warnings = json.get("warnings").unwrap().as_array().unwrap();
    assert!(
        warnings.is_empty(),
        "Should have no warnings when jobs have default resource requirements"
    );
}

/// Test regenerate with non-existent workflow ID
/// The command should fail with a 404 error for non-existent workflows
#[rstest]
fn test_regenerate_nonexistent_workflow(start_server: &ServerProcess) {
    let args = [
        "slurm",
        "regenerate",
        "999999", // Non-existent workflow ID
        "--account",
        "test_account",
        "--profile",
        "kestrel",
    ];

    let result = run_cli_with_json(&args, start_server, None);
    match result {
        Ok(json) => panic!(
            "Expected error for non-existent workflow, but command succeeded: {:?}",
            json
        ),
        Err(err) => {
            let err_str = err.to_string();
            assert!(
                err_str.contains("404") || err_str.contains("not found"),
                "Expected 404/not-found error, got: {}",
                err_str
            );
        }
    }
}

/// Test regenerate with blocked jobs (should include them in pending count)
#[rstest]
fn test_regenerate_includes_blocked_jobs(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create multi-stage workflow (work jobs will be blocked initially)
    let (workflow_id, _job_ids) = create_multi_stage_workflow(config, "test_includes_blocked", 5);

    // Don't complete preprocess - work jobs should remain blocked

    // Run regenerate command
    let args = [
        "slurm",
        "regenerate",
        &workflow_id.to_string(),
        "--account",
        "test_account",
        "--profile",
        "kestrel",
    ];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(result.is_ok(), "Regenerate command failed: {:?}", result);

    let json = result.unwrap();
    // Should count: 1 ready (preprocess) + 5 blocked (work) + 1 blocked (postprocess) = 7
    let pending_jobs = json.get("pending_jobs").unwrap().as_i64().unwrap();
    assert_eq!(
        pending_jobs, 7,
        "Expected 7 pending jobs (1 ready + 6 blocked), got {}",
        pending_jobs
    );
}

// ============== Output Format Tests ==============

/// Test regenerate JSON output structure
#[rstest]
fn test_regenerate_json_output_structure(start_server: &ServerProcess) {
    let config = &start_server.config;

    let job_configs: Vec<(String, models::JobStatus)> = (0..5)
        .map(|i| (format!("job_{}", i), models::JobStatus::Ready))
        .collect();

    let (workflow_id, _job_ids) =
        create_workflow_with_job_states(config, "test_json_output", &job_configs);

    let args = [
        "slurm",
        "regenerate",
        &workflow_id.to_string(),
        "--account",
        "test_account",
        "--profile",
        "kestrel",
    ];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(result.is_ok(), "Regenerate command failed: {:?}", result);

    let json = result.unwrap();

    // Verify required fields exist
    assert!(json.get("workflow_id").is_some());
    assert!(json.get("pending_jobs").is_some());
    assert!(json.get("schedulers_created").is_some());
    assert!(json.get("total_allocations").is_some());
    assert!(json.get("warnings").is_some());
    assert!(json.get("submitted").is_some());

    // Verify types
    assert!(json.get("workflow_id").unwrap().is_i64());
    assert!(json.get("pending_jobs").unwrap().is_i64());
    assert!(json.get("schedulers_created").unwrap().is_array());
    assert!(json.get("total_allocations").unwrap().is_i64());
    assert!(json.get("warnings").unwrap().is_array());
    assert!(json.get("submitted").unwrap().is_boolean());
}
