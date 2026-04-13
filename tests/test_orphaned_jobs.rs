mod common;

use common::{ServerProcess, create_test_workflow, start_server};
use rstest::rstest;
use serde_json::json;
use torc::client::apis;
use torc::models;

/// Helper function to create a test Slurm scheduler
fn create_test_slurm_scheduler(
    config: &torc::client::Configuration,
    workflow_id: i64,
    name: &str,
) -> models::SlurmSchedulerModel {
    let scheduler = models::SlurmSchedulerModel {
        id: None,
        workflow_id,
        name: Some(name.to_string()),
        account: "test_account".to_string(),
        gres: Some("gpu:2".to_string()),
        mem: Some("32G".to_string()),
        nodes: 2,
        ntasks_per_node: None,
        partition: Some("test_partition".to_string()),
        qos: Some("normal".to_string()),
        tmp: Some("100G".to_string()),
        walltime: "04:00:00".to_string(),
        extra: None,
    };
    apis::slurm_schedulers_api::create_slurm_scheduler(config, scheduler)
        .expect("Failed to create test Slurm scheduler")
}

/// Helper to create a compute node associated with a scheduled compute node
/// The scheduled_compute_node_id is stored in the scheduler JSON field
fn create_compute_node_with_scheduled(
    config: &torc::client::Configuration,
    workflow_id: i64,
    scheduled_compute_node_id: i64,
) -> models::ComputeNodeModel {
    // Create scheduler JSON with scheduler_id pointing to the scheduled compute node
    let scheduler_json = json!({
        "scheduler_id": scheduled_compute_node_id
    });

    let compute_node = models::ComputeNodeModel::new(
        workflow_id,
        "test-slurm-host".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        8,                   // num_cpus
        16.0,                // memory_gb
        0,                   // num_gpus
        1,                   // num_nodes
        "slurm".to_string(), // compute_node_type
        Some(scheduler_json),
    );

    apis::compute_nodes_api::create_compute_node(config, compute_node)
        .expect("Failed to create compute node with scheduled compute node")
}

/// Test that start_job sets active_compute_node_id
#[rstest]
fn test_start_job_sets_active_compute_node_id(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow
    let workflow = create_test_workflow(config, "test_start_job_active_compute_node");
    let workflow_id = workflow.id.unwrap();

    // Create a job
    let job = models::JobModel::new(workflow_id, "test_job".to_string(), "echo test".to_string());
    let created_job = apis::jobs_api::create_job(config, job).expect("Failed to create job");
    let job_id = created_job.id.unwrap();

    // Initialize jobs so it becomes ready
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to initialize jobs");

    // Get the run_id from workflow status
    let workflow_status = apis::workflows_api::get_workflow_status(config, workflow_id)
        .expect("Failed to get workflow status");
    let run_id = workflow_status.run_id;

    // Create a compute node to start the job on
    let compute_node = models::ComputeNodeModel::new(
        workflow_id,
        "test-host".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        8,
        16.0,
        0,
        1,
        "local".to_string(),
        None,
    );
    let created_compute_node = apis::compute_nodes_api::create_compute_node(config, compute_node)
        .expect("Failed to create node");
    let compute_node_id = created_compute_node.id.unwrap();

    // Claim the job (transition from Ready to Pending)
    apis::workflows_api::claim_next_jobs(config, workflow_id, Some(1))
        .expect("Failed to claim job");

    // Start the job (job_id, run_id, compute_node_id, body)
    apis::jobs_api::start_job(config, job_id, run_id, compute_node_id)
        .expect("Failed to start job");

    // List jobs filtered by active_compute_node_id
    let jobs_response = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        Some(100),
        None,
        None,
        None,
        Some(compute_node_id), // active_compute_node_id filter
    )
    .expect("Failed to list jobs");

    let jobs = jobs_response.items;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].id, Some(job_id));
    assert_eq!(jobs[0].name, "test_job");
}

/// Test that complete_job clears active_compute_node_id
#[rstest]
fn test_complete_job_clears_active_compute_node_id(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow
    let workflow = create_test_workflow(config, "test_complete_job_clears_active");
    let workflow_id = workflow.id.unwrap();

    // Create a job
    let job = models::JobModel::new(
        workflow_id,
        "test_job_complete".to_string(),
        "echo test".to_string(),
    );
    let created_job = apis::jobs_api::create_job(config, job).expect("Failed to create job");
    let job_id = created_job.id.unwrap();

    // Initialize jobs
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to initialize jobs");

    // Get the run_id from workflow status
    let workflow_status = apis::workflows_api::get_workflow_status(config, workflow_id)
        .expect("Failed to get workflow status");
    let run_id = workflow_status.run_id;

    // Create compute node
    let compute_node = models::ComputeNodeModel::new(
        workflow_id,
        "test-host-complete".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        8,
        16.0,
        0,
        1,
        "local".to_string(),
        None,
    );
    let created_compute_node = apis::compute_nodes_api::create_compute_node(config, compute_node)
        .expect("Failed to create node");
    let compute_node_id = created_compute_node.id.unwrap();

    // Claim the job (transition from Ready to Pending)
    apis::workflows_api::claim_next_jobs(config, workflow_id, Some(1))
        .expect("Failed to claim job");

    // Start the job
    apis::jobs_api::start_job(config, job_id, run_id, compute_node_id)
        .expect("Failed to start job");

    // Verify job is listed with active_compute_node_id filter
    let jobs_before = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        Some(100),
        None,
        None,
        None,
        Some(compute_node_id),
    )
    .expect("Failed to list jobs before completion");
    assert_eq!(jobs_before.items.len(), 1);

    // Complete the job
    let result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        0, // return_code
        1.0,
        chrono::Utc::now().to_rfc3339(),
        models::JobStatus::Completed,
    );
    apis::jobs_api::complete_job(config, job_id, models::JobStatus::Completed, run_id, result)
        .expect("Failed to complete job");

    // Verify job is NO LONGER listed with active_compute_node_id filter
    let jobs_after = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        Some(100),
        None,
        None,
        None,
        Some(compute_node_id),
    )
    .expect("Failed to list jobs after completion");
    assert_eq!(jobs_after.items.len(), 0);
}

/// Test simulating an orphaned job scenario:
/// - Create scheduled compute node (simulating Slurm job)
/// - Create compute node linked to it
/// - Start jobs on that compute node
/// - Verify jobs can be found via active_compute_node_id filter
/// - Complete jobs with failed status (simulating orphan detection)
/// - Verify compute node can be marked inactive
#[rstest]
fn test_orphaned_job_simulation(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow
    let workflow = create_test_workflow(config, "test_orphaned_job_simulation");
    let workflow_id = workflow.id.unwrap();

    // Create Slurm scheduler config
    let scheduler = create_test_slurm_scheduler(config, workflow_id, "orphan_test_scheduler");
    let scheduler_config_id = scheduler.id.unwrap();

    // Create scheduled compute node (simulating a Slurm job submission)
    let scheduled_node = models::ScheduledComputeNodesModel::new(
        workflow_id,
        12345, // scheduler_id (Slurm job ID)
        scheduler_config_id,
        "slurm".to_string(),
        "active".to_string(),
    );
    let created_scheduled = apis::scheduled_compute_nodes_api::create_scheduled_compute_node(
        config,
        scheduled_node.clone(),
    )
    .expect("Failed to create scheduled compute node");
    let scheduled_compute_node_id = created_scheduled.id.unwrap();

    // Create compute node associated with the scheduled compute node
    let compute_node =
        create_compute_node_with_scheduled(config, workflow_id, scheduled_compute_node_id);
    let compute_node_id = compute_node.id.unwrap();

    // Create multiple jobs
    let job1 = models::JobModel::new(
        workflow_id,
        "orphan_job_1".to_string(),
        "echo orphan 1".to_string(),
    );
    let job2 = models::JobModel::new(
        workflow_id,
        "orphan_job_2".to_string(),
        "echo orphan 2".to_string(),
    );

    let created_job1 = apis::jobs_api::create_job(config, job1).expect("Failed to create job1");
    let created_job2 = apis::jobs_api::create_job(config, job2).expect("Failed to create job2");
    let job1_id = created_job1.id.unwrap();
    let job2_id = created_job2.id.unwrap();

    // Initialize jobs
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to initialize jobs");

    // Get the run_id from workflow status
    let workflow_status = apis::workflows_api::get_workflow_status(config, workflow_id)
        .expect("Failed to get workflow status");
    let run_id = workflow_status.run_id;

    // Claim jobs (transition from Ready to Pending)
    apis::workflows_api::claim_next_jobs(config, workflow_id, Some(2))
        .expect("Failed to claim jobs");

    // Start both jobs on the compute node (simulating they were running when Slurm job died)
    apis::jobs_api::start_job(config, job1_id, run_id, compute_node_id)
        .expect("Failed to start job1");
    apis::jobs_api::start_job(config, job2_id, run_id, compute_node_id)
        .expect("Failed to start job2");

    // Verify both jobs are found via active_compute_node_id filter
    let orphaned_jobs = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        Some(100),
        None,
        None,
        None,
        Some(compute_node_id),
    )
    .expect("Failed to list orphaned jobs");

    let items = orphaned_jobs.items;
    assert_eq!(items.len(), 2);

    // Simulate orphan detection: fail the jobs with return code -128 (ORPHANED_JOB_RETURN_CODE)
    let orphan_return_code = -128;

    for job in &items {
        let job_id = job.id.unwrap();
        let result = models::ResultModel::new(
            job_id,
            workflow_id,
            run_id,
            1, // attempt_id
            compute_node_id,
            orphan_return_code,
            0.0,
            chrono::Utc::now().to_rfc3339(),
            models::JobStatus::Failed,
        );
        apis::jobs_api::complete_job(config, job_id, models::JobStatus::Failed, run_id, result)
            .expect("Failed to complete orphaned job");
    }

    // Verify jobs are now failed
    let failed_jobs = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        Some(models::JobStatus::Failed),
        None,
        None,
        None,
        Some(100),
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list failed jobs");
    assert_eq!(failed_jobs.items.len(), 2);

    // Verify no jobs with active_compute_node_id (they were cleared by complete_job)
    let active_jobs = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        Some(100),
        None,
        None,
        None,
        Some(compute_node_id),
    )
    .expect("Failed to list active jobs");
    assert_eq!(active_jobs.items.len(), 0);

    // Simulate marking compute node as inactive
    let mut updated_node = compute_node.clone();
    updated_node.is_active = Some(false);
    apis::compute_nodes_api::update_compute_node(config, compute_node_id, updated_node)
        .expect("Failed to mark compute node inactive");

    // Verify compute node is inactive
    let fetched_node = apis::compute_nodes_api::get_compute_node(config, compute_node_id)
        .expect("Failed to get compute node");
    assert_eq!(fetched_node.is_active, Some(false));

    // Simulate updating scheduled compute node status to complete
    let mut updated_scheduled = created_scheduled.clone();
    updated_scheduled.status = "complete".to_string();
    apis::scheduled_compute_nodes_api::update_scheduled_compute_node(
        config,
        scheduled_compute_node_id,
        updated_scheduled,
    )
    .expect("Failed to update scheduled compute node");

    // Verify scheduled compute node status
    let fetched_scheduled = apis::scheduled_compute_nodes_api::get_scheduled_compute_node(
        config,
        scheduled_compute_node_id,
    )
    .expect("Failed to get scheduled compute node");
    assert_eq!(fetched_scheduled.status, "complete");
}

/// Test that list_jobs returns empty when filtering by non-existent active_compute_node_id
#[rstest]
fn test_list_jobs_no_active_compute_node(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow with some jobs
    let workflow = create_test_workflow(config, "test_no_active_compute_node");
    let workflow_id = workflow.id.unwrap();

    let job = models::JobModel::new(
        workflow_id,
        "inactive_job".to_string(),
        "echo test".to_string(),
    );
    apis::jobs_api::create_job(config, job).expect("Failed to create job");

    // Initialize jobs
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to initialize jobs");

    // Query with a compute_node_id that no jobs are running on
    let jobs = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        Some(100),
        None,
        None,
        None,
        Some(99999), // Non-existent compute_node_id
    )
    .expect("Failed to list jobs");

    assert_eq!(jobs.items.len(), 0);
}

/// Test multiple compute nodes with different jobs running
#[rstest]
fn test_multiple_compute_nodes_job_tracking(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow
    let workflow = create_test_workflow(config, "test_multi_compute_node_tracking");
    let workflow_id = workflow.id.unwrap();

    // Create Slurm scheduler
    let scheduler = create_test_slurm_scheduler(config, workflow_id, "multi_scheduler");
    let scheduler_config_id = scheduler.id.unwrap();

    // Create two scheduled compute nodes (two Slurm jobs)
    let scheduled1 = models::ScheduledComputeNodesModel::new(
        workflow_id,
        11111,
        scheduler_config_id,
        "slurm".to_string(),
        "active".to_string(),
    );
    let scheduled2 = models::ScheduledComputeNodesModel::new(
        workflow_id,
        22222,
        scheduler_config_id,
        "slurm".to_string(),
        "active".to_string(),
    );

    let created_scheduled1 =
        apis::scheduled_compute_nodes_api::create_scheduled_compute_node(config, scheduled1)
            .expect("Failed to create scheduled1");
    let created_scheduled2 =
        apis::scheduled_compute_nodes_api::create_scheduled_compute_node(config, scheduled2)
            .expect("Failed to create scheduled2");

    // Create compute nodes linked to each scheduled node
    let compute_node1 =
        create_compute_node_with_scheduled(config, workflow_id, created_scheduled1.id.unwrap());
    let compute_node2 =
        create_compute_node_with_scheduled(config, workflow_id, created_scheduled2.id.unwrap());
    let cn1_id = compute_node1.id.unwrap();
    let cn2_id = compute_node2.id.unwrap();

    // Create 4 jobs
    let mut jobs = Vec::new();
    for i in 1..=4 {
        let job = models::JobModel::new(
            workflow_id,
            format!("multi_job_{}", i),
            format!("echo multi {}", i),
        );
        let created = apis::jobs_api::create_job(config, job).expect("Failed to create job");
        jobs.push(created);
    }

    // Initialize
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to initialize jobs");

    // Get the run_id from workflow status
    let workflow_status = apis::workflows_api::get_workflow_status(config, workflow_id)
        .expect("Failed to get workflow status");
    let run_id = workflow_status.run_id;

    // Claim all jobs (transition from Ready to Pending)
    apis::workflows_api::claim_next_jobs(config, workflow_id, Some(4))
        .expect("Failed to claim jobs");

    // Start jobs 1 and 2 on compute_node1
    apis::jobs_api::start_job(config, jobs[0].id.unwrap(), run_id, cn1_id)
        .expect("Failed to start job 1");
    apis::jobs_api::start_job(config, jobs[1].id.unwrap(), run_id, cn1_id)
        .expect("Failed to start job 2");

    // Start jobs 3 and 4 on compute_node2
    apis::jobs_api::start_job(config, jobs[2].id.unwrap(), run_id, cn2_id)
        .expect("Failed to start job 3");
    apis::jobs_api::start_job(config, jobs[3].id.unwrap(), run_id, cn2_id)
        .expect("Failed to start job 4");

    // Verify jobs on compute_node1
    let cn1_jobs = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        Some(100),
        None,
        None,
        None,
        Some(cn1_id),
    )
    .expect("Failed to list cn1 jobs");
    let cn1_items = cn1_jobs.items;
    assert_eq!(cn1_items.len(), 2);
    let cn1_names: Vec<&str> = cn1_items.iter().map(|j| j.name.as_str()).collect();
    assert!(cn1_names.contains(&"multi_job_1"));
    assert!(cn1_names.contains(&"multi_job_2"));

    // Verify jobs on compute_node2
    let cn2_jobs = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        Some(100),
        None,
        None,
        None,
        Some(cn2_id),
    )
    .expect("Failed to list cn2 jobs");
    let cn2_items = cn2_jobs.items;
    assert_eq!(cn2_items.len(), 2);
    let cn2_names: Vec<&str> = cn2_items.iter().map(|j| j.name.as_str()).collect();
    assert!(cn2_names.contains(&"multi_job_3"));
    assert!(cn2_names.contains(&"multi_job_4"));

    // Simulate Slurm job 11111 dying - fail jobs on compute_node1
    for job in &cn1_items {
        let job_id = job.id.unwrap();
        let result = models::ResultModel::new(
            job_id,
            workflow_id,
            run_id,
            1, // attempt_id
            cn1_id,
            -128, // ORPHANED_JOB_RETURN_CODE
            0.0,
            chrono::Utc::now().to_rfc3339(),
            models::JobStatus::Failed,
        );
        apis::jobs_api::complete_job(config, job_id, models::JobStatus::Failed, run_id, result)
            .expect("Failed to complete orphaned job");
    }

    // Verify compute_node1 now has no active jobs
    let cn1_after = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        Some(100),
        None,
        None,
        None,
        Some(cn1_id),
    )
    .expect("Failed to list cn1 jobs after");
    assert_eq!(cn1_after.items.len(), 0);

    // Verify compute_node2 still has its jobs running
    let cn2_after = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        Some(100),
        None,
        None,
        None,
        Some(cn2_id),
    )
    .expect("Failed to list cn2 jobs after");
    assert_eq!(cn2_after.items.len(), 2);
}

/// Test reset_job_status clears active_compute_node_id
#[rstest]
fn test_reset_job_clears_active_compute_node_id(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow
    let workflow = create_test_workflow(config, "test_reset_clears_active");
    let workflow_id = workflow.id.unwrap();

    // Create and start a job
    let job = models::JobModel::new(
        workflow_id,
        "reset_test_job".to_string(),
        "echo reset test".to_string(),
    );
    let created_job = apis::jobs_api::create_job(config, job).expect("Failed to create job");
    let job_id = created_job.id.unwrap();

    // Initialize
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to initialize jobs");

    // Get the run_id from workflow status
    let workflow_status = apis::workflows_api::get_workflow_status(config, workflow_id)
        .expect("Failed to get workflow status");
    let run_id = workflow_status.run_id;

    // Create compute node and start job
    let compute_node = models::ComputeNodeModel::new(
        workflow_id,
        "reset-host".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        8,
        16.0,
        0,
        1,
        "local".to_string(),
        None,
    );
    let created_node = apis::compute_nodes_api::create_compute_node(config, compute_node)
        .expect("Failed to create node");
    let compute_node_id = created_node.id.unwrap();

    // Claim the job (transition from Ready to Pending)
    apis::workflows_api::claim_next_jobs(config, workflow_id, Some(1))
        .expect("Failed to claim job");

    apis::jobs_api::start_job(config, job_id, run_id, compute_node_id)
        .expect("Failed to start job");

    // Verify job has active_compute_node_id
    let before_reset = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        Some(100),
        None,
        None,
        None,
        Some(compute_node_id),
    )
    .expect("Failed to list before reset");
    assert_eq!(before_reset.items.len(), 1);

    // Reset job status
    apis::workflows_api::reset_job_status(config, workflow_id, None)
        .expect("Failed to reset job status");

    // Verify active_compute_node_id is cleared
    let after_reset = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        Some(100),
        None,
        None,
        None,
        Some(compute_node_id),
    )
    .expect("Failed to list after reset");
    assert_eq!(after_reset.items.len(), 0);
}
