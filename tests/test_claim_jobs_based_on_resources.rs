mod common;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::thread;

use common::{
    ServerProcess, create_custom_resources_workflow, create_dependency_chain_workflow,
    create_diverse_jobs_workflow, create_gpu_workflow, create_high_cpu_workflow,
    create_high_memory_workflow, create_many_jobs_workflow, create_maximum_resources_workflow,
    create_minimal_resources_workflow, create_multi_node_workflow,
    create_test_resource_requirements, start_server,
};
use rstest::rstest;

use torc::client::default_api;
use torc::models;

/// Test claim_jobs_based_on_resources with resource constraint limiting job allocation
/// Workflow: 4 jobs, each needing 1 CPU, 1.0 GB memory, 0 GPUs, 1 node
/// Resources: 1 CPU, 1.0 GB memory total → Can support max 1 job simultaneously
/// Expected: Returns at most 1 job (limited by both CPU and memory)
#[rstest]
fn test_prepare_jobs_minimal_resources(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create workflow with 4 jobs that match the resource requirements we'll test
    let jobs = create_minimal_resources_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // Test with ComputeNodesResources that can support exactly 1 job
    let resources = models::ComputeNodesResources::new(1, 1.0, 0, 1);

    let result =
        default_api::claim_jobs_based_on_resources(config, workflow_id, &resources, 10, None, None)
            .expect("claim_jobs_based_on_resources should succeed");

    // Should return exactly 1 job (1 CPU available ÷ 1 CPU per job = 1 job must be returned)
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        1,
        "Server must return exactly 1 job with 1 CPU available for 4 ready jobs needing 1 CPU each, got {}",
        returned_jobs.len()
    );
    let job = &returned_jobs[0];
    assert!(job.id.is_some());
    assert!(job.workflow_id == workflow_id);
    assert!(!job.name.is_empty());
    assert_eq!(
        job.status.expect("Job status should be present"),
        models::JobStatus::Pending
    );
}

/// Test claim_jobs_based_on_resources with CPU constraint limiting job allocation  
/// Workflow: 3 jobs, each needing 64 CPUs, 128.0 GB memory, 0 GPUs, 1 node
/// Resources: 64 CPUs, 128.0 GB memory total → Can support max 1 job simultaneously
/// Expected: Returns at most 1 job (limited by CPU availability)
#[rstest]
fn test_prepare_jobs_high_cpu_resources(start_server: &ServerProcess) {
    let config = &start_server.config;
    let jobs = create_high_cpu_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let resources = models::ComputeNodesResources::new(64, 128.0, 0, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::None),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Should return exactly 1 job (64 CPUs ÷ 64 CPUs per job = 1 job must be returned)
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        1,
        "Server must return exactly 1 job with 64 CPUs available for 3 ready jobs needing 64 CPUs each, got {}",
        returned_jobs.len()
    );
    let job = &returned_jobs[0];
    assert!(job.id.is_some());
    assert!(job.workflow_id == workflow_id);
    assert!(!job.name.is_empty());
    assert_eq!(
        job.status.expect("Job status should be present"),
        models::JobStatus::Pending
    );
}

/// Test claim_jobs_based_on_resources with memory constraint limiting job allocation
/// Workflow: 2 jobs, each needing 4 CPUs, 512.0 GB memory, 0 GPUs, 1 node  
/// Resources: 4 CPUs, 512.0 GB memory total → Can support max 1 job simultaneously
/// Expected: Returns at most 1 job (limited by memory availability)
#[rstest]
fn test_prepare_jobs_high_memory_resources(start_server: &ServerProcess) {
    let config = &start_server.config;
    let jobs = create_high_memory_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let resources = models::ComputeNodesResources::new(4, 512.0, 0, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        5,
        Some(models::ClaimJobsSortMethod::GpusMemoryRuntime),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Should return exactly 1 job (512GB available ÷ 512GB per job = 1 job must be returned)
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        1,
        "Server must return exactly 1 job with 512GB memory available for 2 ready jobs needing 512GB each, got {}",
        returned_jobs.len()
    );
    let job = &returned_jobs[0];
    assert!(job.id.is_some());
    assert!(job.workflow_id == workflow_id);
    assert!(!job.name.is_empty());
    assert_eq!(
        job.status.expect("Job status should be present"),
        models::JobStatus::Pending
    );
}

/// Test claim_jobs_based_on_resources with GPU constraint limiting job allocation
/// Workflow: 3 jobs, each needing 8 CPUs, 32.0 GB memory, 4 GPUs, 1 node
/// Resources: 8 CPUs, 32.0 GB memory, 4 GPUs total → Can support max 1 job simultaneously  
/// Expected: Returns at most 1 job (limited by GPU availability)
#[rstest]
fn test_prepare_jobs_gpu_resources(start_server: &ServerProcess) {
    let config = &start_server.config;
    let jobs = create_gpu_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let resources = models::ComputeNodesResources::new(8, 32.0, 4, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::GpusRuntimeMemory),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Should return exactly 1 job (4 GPUs available ÷ 4 GPUs per job = 1 job must be returned)
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        1,
        "Server must return exactly 1 job with 4 GPUs available for 3 ready jobs needing 4 GPUs each, got {}",
        returned_jobs.len()
    );
    let job = &returned_jobs[0];
    assert!(job.id.is_some());
    assert!(job.workflow_id == workflow_id);
    assert!(!job.name.is_empty());
    assert_eq!(
        job.status.expect("Job status should be present"),
        models::JobStatus::Pending
    );
}

/// Test claim_jobs_based_on_resources with node constraint limiting job allocation
/// Workflow: 2 jobs, each needing 16 CPUs, 64.0 GB memory, 0 GPUs, 4 nodes
/// Resources: 16 CPUs, 64.0 GB memory, 0 GPUs, 4 nodes total → Can support max 1 job simultaneously
/// Expected: Returns at most 1 job (limited by node availability)
#[rstest]
fn test_prepare_jobs_multi_node_resources(start_server: &ServerProcess) {
    let config = &start_server.config;
    let jobs = create_multi_node_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let resources = models::ComputeNodesResources::new(16, 64.0, 0, 4);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::None),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Should return exactly 1 job (4 nodes available ÷ 4 nodes per job = 1 job must be returned)
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        1,
        "Server must return exactly 1 job with 4 nodes available for 2 ready jobs needing 4 nodes each, got {}",
        returned_jobs.len()
    );
    let job = &returned_jobs[0];
    assert!(job.id.is_some());
    assert!(job.workflow_id == workflow_id);
    assert!(!job.name.is_empty());
    assert_eq!(
        job.status.expect("Job status should be present"),
        models::JobStatus::Pending
    );
}

/// Test claim_jobs_based_on_resources with maximum resource constraint
/// Workflow: 2 jobs, each needing 128 CPUs, 1024.0 GB memory, 8 GPUs, 8 nodes
/// Resources: 128 CPUs, 1024.0 GB memory, 8 GPUs, 8 nodes total → Exactly matches 1 job's needs
/// Expected: Returns at most 1 job (resources exactly match one job's requirements)
#[rstest]
fn test_prepare_jobs_maximum_resources(start_server: &ServerProcess) {
    let config = &start_server.config;
    let jobs = create_maximum_resources_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let mut resources = models::ComputeNodesResources::new(128, 1024.0, 8, 8);
    resources.time_limit = Some("P0DT24H".to_string());

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::GpusRuntimeMemory),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Should return exactly 1 job (resources exactly match 1 job's needs)
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        1,
        "Server must return exactly 1 job when resources exactly match 1 job's requirements, got {}",
        returned_jobs.len()
    );
    let job = &returned_jobs[0];
    assert!(job.id.is_some());
    assert!(job.workflow_id == workflow_id);
    assert!(!job.name.is_empty());
    assert_eq!(
        job.status.expect("Job status should be present"),
        models::JobStatus::Pending
    );
}

/// Test claim_jobs_based_on_resources with time limit constraints
#[rstest]
fn test_prepare_jobs_with_time_limits(start_server: &ServerProcess) {
    let config = &start_server.config;
    let jobs = create_custom_resources_workflow(config, true, 4, 16.0, 0, 1);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let mut resources = models::ComputeNodesResources::new(4, 16.0, 0, 1);
    resources.time_limit = Some("P0DT1H30M".to_string());

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::GpusMemoryRuntime),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Validate response structure
    if let Some(returned_jobs) = result.jobs {
        for job in &returned_jobs {
            assert!(job.id.is_some());
            assert!(job.workflow_id == workflow_id);
            assert!(!job.name.is_empty());
            assert_eq!(
                job.status.expect("Job status should be present"),
                models::JobStatus::Pending
            );
        }
    } else {
        assert!(
            result.reason.is_some(),
            "Should provide reason when no jobs returned"
        );
    }
}

/// Test claim_jobs_based_on_resources with scheduler config ID
#[rstest]
fn test_prepare_jobs_with_scheduler_config(start_server: &ServerProcess) {
    let config = &start_server.config;
    let jobs = create_custom_resources_workflow(config, true, 8, 32.0, 2, 1);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let mut resources = models::ComputeNodesResources::new(8, 32.0, 2, 1);
    resources.scheduler_config_id = Some(1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::None),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Validate response structure
    if let Some(returned_jobs) = result.jobs {
        for job in &returned_jobs {
            assert!(job.id.is_some());
            assert!(job.workflow_id == workflow_id);
            assert!(!job.name.is_empty());
            assert_eq!(
                job.status.expect("Job status should be present"),
                models::JobStatus::Pending
            );
        }
    } else {
        assert!(
            result.reason.is_some(),
            "Should provide reason when no jobs returned"
        );
    }
}

/// Test claim_jobs_based_on_resources with different limit values
#[rstest]
#[case(1)]
#[case(5)]
#[case(100)]
fn test_prepare_jobs_different_limits(start_server: &ServerProcess, #[case] limit: i64) {
    let config = &start_server.config;
    let jobs = create_custom_resources_workflow(config, true, 4, 8.0, 0, 1);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let resources = models::ComputeNodesResources::new(4, 8.0, 0, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        limit,
        Some(models::ClaimJobsSortMethod::GpusRuntimeMemory),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Validate response structure and limit is respected
    if let Some(returned_jobs) = result.jobs {
        assert!(
            returned_jobs.len() <= limit as usize,
            "Should not return more jobs than limit"
        );
        for job in &returned_jobs {
            assert!(job.id.is_some());
            assert!(job.workflow_id == workflow_id);
            assert!(!job.name.is_empty());
            assert_eq!(
                job.status.expect("Job status should be present"),
                models::JobStatus::Pending
            );
        }
    } else {
        assert!(
            result.reason.is_some(),
            "Should provide reason when no jobs returned"
        );
    }
}

/// Test claim_jobs_based_on_resources with all sort methods
#[rstest]
#[case(models::ClaimJobsSortMethod::GpusRuntimeMemory)]
#[case(models::ClaimJobsSortMethod::GpusMemoryRuntime)]
#[case(models::ClaimJobsSortMethod::None)]
fn test_prepare_jobs_all_sort_methods(
    start_server: &ServerProcess,
    #[case] sort_method: models::ClaimJobsSortMethod,
) {
    let config = &start_server.config;
    let jobs = create_custom_resources_workflow(config, true, 4, 8.0, 1, 1);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let resources = models::ComputeNodesResources::new(4, 8.0, 1, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(sort_method),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Validate response structure
    if let Some(returned_jobs) = result.jobs {
        for job in &returned_jobs {
            assert!(job.id.is_some());
            assert!(job.workflow_id == workflow_id);
            assert!(!job.name.is_empty());
            assert_eq!(
                job.status.expect("Job status should be present"),
                models::JobStatus::Pending
            );
        }
    } else {
        assert!(
            result.reason.is_some(),
            "Should provide reason when no jobs returned"
        );
    }
}

/// Test claim_jobs_based_on_resources with fractional memory values
#[rstest]
#[case(0.5)]
#[case(2.25)]
#[case(16.75)]
fn test_prepare_jobs_fractional_memory(start_server: &ServerProcess, #[case] memory_gb: f64) {
    let config = &start_server.config;
    let jobs = create_custom_resources_workflow(config, true, 2, memory_gb, 0, 1);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let resources = models::ComputeNodesResources::new(2, memory_gb, 0, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::GpusMemoryRuntime),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Validate response structure
    if let Some(returned_jobs) = result.jobs {
        for job in &returned_jobs {
            assert!(job.id.is_some());
            assert!(job.workflow_id == workflow_id);
            assert!(!job.name.is_empty());
            assert_eq!(
                job.status.expect("Job status should be present"),
                models::JobStatus::Pending
            );
        }
    } else {
        assert!(
            result.reason.is_some(),
            "Should provide reason when no jobs returned"
        );
    }
}

/// Test claim_jobs_based_on_resources with zero GPUs explicitly
#[rstest]
fn test_prepare_jobs_zero_gpus(start_server: &ServerProcess) {
    let config = &start_server.config;
    let jobs = create_custom_resources_workflow(config, true, 8, 16.0, 0, 1);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let resources = models::ComputeNodesResources::new(8, 16.0, 0, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::GpusRuntimeMemory),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Validate response structure
    if let Some(returned_jobs) = result.jobs {
        for job in &returned_jobs {
            assert!(job.id.is_some());
            assert!(job.workflow_id == workflow_id);
            assert!(!job.name.is_empty());
            assert_eq!(
                job.status.expect("Job status should be present"),
                models::JobStatus::Pending
            );
        }
    } else {
        assert!(
            result.reason.is_some(),
            "Should provide reason when no jobs returned"
        );
    }
}

/// Test claim_jobs_based_on_resources with workflow that has no ready jobs
#[rstest]
fn test_prepare_jobs_no_ready_jobs(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create workflow without initializing jobs (so all jobs remain uninitialized)
    let jobs = create_minimal_resources_workflow(config, false);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let resources = models::ComputeNodesResources::new(4, 8.0, 0, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::None),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Should return empty jobs list with reason when no jobs are ready
    if let Some(returned_jobs) = result.jobs {
        // If jobs are returned, they should be valid
        for job in &returned_jobs {
            assert!(job.id.is_some());
            assert!(job.workflow_id == workflow_id);
            assert!(!job.name.is_empty());
            assert_eq!(
                job.status.expect("Job status should be present"),
                models::JobStatus::Pending
            );
        }
    } else {
        assert!(
            result.reason.is_some(),
            "Should provide reason when no jobs returned"
        );
    }
}

/// Test claim_jobs_based_on_resources with invalid workflow ID
#[rstest]
fn test_prepare_jobs_invalid_workflow(start_server: &ServerProcess) {
    let config = &start_server.config;
    let invalid_workflow_id = 99999i64;

    let resources = models::ComputeNodesResources::new(4, 8.0, 0, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        invalid_workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::None),
        None,
    );

    // Should return an error for invalid workflow ID
    match result {
        Ok(response) => {
            // Server might return empty response instead of error
            if let Some(returned_jobs) = response.jobs {
                assert!(
                    returned_jobs.is_empty(),
                    "Should return empty jobs for invalid workflow"
                );
            }
        }
        Err(_) => {
            // Expected: should get an error for invalid workflow ID
        }
    }
}

/// Test comprehensive resource combinations matrix
#[rstest]
#[case(1, 1.0, 0, 1)] // Minimal resources
#[case(2, 4.0, 1, 1)] // Small with GPU
#[case(8, 16.0, 0, 2)] // Multi-node CPU only
#[case(16, 32.0, 2, 1)] // High CPU/memory with GPU
#[case(32, 64.0, 4, 2)] // High resources multi-node with GPU
#[case(64, 128.0, 8, 4)] // Maximum test resources
fn test_prepare_jobs_resource_combinations(
    start_server: &ServerProcess,
    #[case] num_cpus: i64,
    #[case] memory_gb: f64,
    #[case] num_gpus: i64,
    #[case] num_nodes: i64,
) {
    let config = &start_server.config;
    let jobs =
        create_custom_resources_workflow(config, true, num_cpus, memory_gb, num_gpus, num_nodes);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let resources = models::ComputeNodesResources::new(num_cpus, memory_gb, num_gpus, num_nodes);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::GpusRuntimeMemory),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Validate response structure
    if let Some(returned_jobs) = result.jobs {
        for job in &returned_jobs {
            assert!(job.id.is_some());
            assert!(job.workflow_id == workflow_id);
            assert!(!job.name.is_empty());
            assert_eq!(
                job.status.expect("Job status should be present"),
                models::JobStatus::Pending
            );
        }
    } else {
        assert!(
            result.reason.is_some(),
            "Should provide reason when no jobs returned"
        );
    }
}

/// Test claim_jobs_based_on_resources with all optional fields set
#[rstest]
fn test_prepare_jobs_all_optional_fields(start_server: &ServerProcess) {
    let config = &start_server.config;
    let jobs = create_custom_resources_workflow(config, true, 16, 64.0, 4, 2);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let mut resources = models::ComputeNodesResources::new(16, 64.0, 4, 2);
    resources.id = Some(123);
    resources.time_limit = Some("P0DT24H".to_string());
    resources.scheduler_config_id = Some(456);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::GpusMemoryRuntime),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Validate response structure
    if let Some(returned_jobs) = result.jobs {
        for job in &returned_jobs {
            assert!(job.id.is_some());
            assert!(job.workflow_id == workflow_id);
            assert!(!job.name.is_empty());
            assert_eq!(
                job.status.expect("Job status should be present"),
                models::JobStatus::Pending
            );
        }
    } else {
        assert!(
            result.reason.is_some(),
            "Should provide reason when no jobs returned"
        );
    }
}

/// Test claim_jobs_based_on_resources response structure validation
#[rstest]
fn test_prepare_jobs_response_validation(start_server: &ServerProcess) {
    let config = &start_server.config;
    let jobs = create_custom_resources_workflow(config, true, 4, 8.0, 0, 1);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let resources = models::ComputeNodesResources::new(4, 8.0, 0, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::GpusRuntimeMemory),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Validate response structure
    if let Some(prepared_jobs) = &result.jobs {
        for job in prepared_jobs {
            // Each returned job should have valid fields
            assert!(job.id.is_some());
            assert!(job.workflow_id == workflow_id);
            assert!(!job.name.is_empty());
            assert!(!job.command.is_empty());
            assert_eq!(
                job.status.expect("Job status should be present"),
                models::JobStatus::Pending
            );
        }
    } else {
        assert!(
            result.reason.is_some(),
            "Should provide reason when no jobs returned"
        );
    }
}

/// Integration test demonstrating complete workflow with claim_jobs_based_on_resources
#[rstest]
fn test_prepare_jobs_integration_example(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Test different resource scenarios in sequence
    let test_scenarios = vec![
        (
            "Minimal resources",
            create_minimal_resources_workflow(config, true),
            models::ComputeNodesResources::new(1, 1.0, 0, 1),
            models::ClaimJobsSortMethod::None,
        ),
        (
            "GPU workload",
            create_gpu_workflow(config, true),
            models::ComputeNodesResources::new(8, 32.0, 2, 1),
            models::ClaimJobsSortMethod::GpusRuntimeMemory,
        ),
        (
            "High-performance computing",
            create_maximum_resources_workflow(config, true),
            {
                let mut resources = models::ComputeNodesResources::new(32, 128.0, 4, 2);
                resources.time_limit = Some("P0DT12H".to_string());
                resources.scheduler_config_id = Some(1);
                resources
            },
            models::ClaimJobsSortMethod::GpusMemoryRuntime,
        ),
    ];

    // Test each scenario
    for (description, jobs_map, resources, sort_method) in test_scenarios {
        println!("Testing scenario: {}", description);

        // Get the workflow ID from the first job
        let first_job = jobs_map
            .values()
            .next()
            .expect("Should have at least one job");
        let workflow_id = first_job.workflow_id;

        let result = default_api::claim_jobs_based_on_resources(
            config,
            workflow_id,
            &resources,
            10,
            Some(sort_method),
            None,
        )
        .expect("claim_jobs_based_on_resources should succeed");

        println!("  ✓ API call successful");
        if let Some(jobs) = result.jobs {
            println!("  ✓ Found {} jobs ready for submission", jobs.len());
            for job in &jobs {
                println!(
                    "    - Job: {} (ID: {:?}, Status: {:?})",
                    job.name, job.id, job.status
                );
                assert_eq!(
                    job.status.expect("Job status should be present"),
                    models::JobStatus::Pending
                );
            }
        } else {
            println!("  ✓ No jobs returned");
            if let Some(reason) = result.reason {
                println!("    Reason: {}", reason);
            }
        }
    }

    println!("Integration test completed successfully");
}

/// Test that claim_jobs_based_on_resources returns multiple jobs when resources allow
/// Workflow: 4 jobs, each needing 1 CPU, 1.0 GB memory, 0 GPUs, 1 node
/// Resources: 4 CPUs, 4.0 GB memory total → Can support all 4 jobs simultaneously  
/// Expected: Should return multiple jobs (at least 1, ideally all 4)
#[rstest]
fn test_prepare_jobs_multiple_jobs_returned(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create workflow with 4 jobs that each need minimal resources
    let jobs = create_minimal_resources_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // Test with resources that can support all 4 jobs (4 CPU, 4GB total)
    let resources = models::ComputeNodesResources::new(4, 4.0, 0, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::None),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Should return exactly 4 jobs (resources can support all 4 jobs)
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        4,
        "Server must return exactly 4 jobs when resources can support all 4 ready jobs, got {}",
        returned_jobs.len()
    );
    for job in &returned_jobs {
        assert!(job.id.is_some());
        assert!(job.workflow_id == workflow_id);
        assert!(!job.name.is_empty());
        assert_eq!(
            job.status.expect("Job status should be present"),
            models::JobStatus::Pending
        );
    }
}

/// Test that claim_jobs_based_on_resources enforces resource allocation limits
/// Workflow: 4 jobs, each needing 2 CPUs, 4.0 GB memory, 0 GPUs, 1 node
/// Resources: 4 CPUs, 8.0 GB memory total → Can support max 2 jobs simultaneously
/// Expected: Returns at most 2 jobs (limited by available resources, not limit parameter)
#[rstest]
fn test_prepare_jobs_resource_allocation_limits(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create workflow with 4 jobs that each need 2 CPU, 4GB memory
    let jobs = create_custom_resources_workflow(config, true, 2, 4.0, 0, 1);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // Test with resources that can only support 2 jobs (4 CPU, 8GB total)
    let resources = models::ComputeNodesResources::new(4, 8.0, 0, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10, // Request up to 10 jobs
        Some(models::ClaimJobsSortMethod::None),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Should return exactly 2 jobs (4 CPUs ÷ 2 CPUs per job = 2 jobs must be returned)
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        2,
        "Server must return exactly 2 jobs with 4 CPUs available for jobs needing 2 CPUs each, got {}",
        returned_jobs.len()
    );
    for job in &returned_jobs {
        assert!(job.id.is_some());
        assert!(job.workflow_id == workflow_id);
        assert!(!job.name.is_empty());
        assert_eq!(
            job.status.expect("Job status should be present"),
            models::JobStatus::Pending
        );
    }
}

/// Test resource limits with GPU constraints
/// Create 3 GPU jobs, test with resources for only 1 job
#[rstest]
fn test_prepare_jobs_gpu_resource_limits(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create workflow with 3 jobs that each need 4 GPUs
    let jobs = create_gpu_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // Test with resources that have only 4 GPUs (can support 1 job)
    let resources = models::ComputeNodesResources::new(32, 128.0, 4, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10, // Request more jobs than resources can support
        Some(models::ClaimJobsSortMethod::GpusRuntimeMemory),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Should return exactly 1 job (4 GPUs available ÷ 4 GPUs per job = 1 job must be returned)
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        1,
        "Server must return exactly 1 job with 4 GPUs available for jobs needing 4 GPUs each, got {}",
        returned_jobs.len()
    );
    let job = &returned_jobs[0];
    assert!(job.id.is_some());
    assert!(job.workflow_id == workflow_id);
    assert!(!job.name.is_empty());
    assert_eq!(
        job.status.expect("Job status should be present"),
        models::JobStatus::Pending
    );
}

/// Test resource limits with memory constraints
/// Create 2 high-memory jobs, test with resources for only 1 job
#[rstest]
fn test_prepare_jobs_memory_resource_limits(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create workflow with 2 jobs that each need 512GB memory
    let jobs = create_high_memory_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // Test with resources that have only 512GB total (can support 1 job)
    let resources = models::ComputeNodesResources::new(8, 512.0, 0, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        5,
        Some(models::ClaimJobsSortMethod::GpusMemoryRuntime),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Should return exactly 1 job (512GB available ÷ 512GB per job = 1 job must be returned)
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        1,
        "Server must return exactly 1 job with 512GB memory available for jobs needing 512GB each, got {}",
        returned_jobs.len()
    );
    let job = &returned_jobs[0];
    assert!(job.id.is_some());
    assert!(job.workflow_id == workflow_id);
    assert!(!job.name.is_empty());
    assert_eq!(
        job.status.expect("Job status should be present"),
        models::JobStatus::Pending
    );
}

/// Test that limit parameter is respected when it's more restrictive than resources
/// Create 4 minimal jobs, have resources for all 4, but limit to 2
#[rstest]
fn test_prepare_jobs_limit_parameter_restriction(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create workflow with 4 jobs that each need minimal resources
    let jobs = create_minimal_resources_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // Test with resources that can support all 4 jobs, but limit to 2
    let resources = models::ComputeNodesResources::new(8, 16.0, 0, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        2, // Limit to 2 jobs even though resources could support more
        Some(models::ClaimJobsSortMethod::None),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Should return exactly 2 jobs (limited by the limit parameter, not resources)
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        2,
        "Server must return exactly 2 jobs due to limit parameter (resources could support 4), got {}",
        returned_jobs.len()
    );
    for job in &returned_jobs {
        assert!(job.id.is_some());
        assert!(job.workflow_id == workflow_id);
        assert!(!job.name.is_empty());
        assert_eq!(
            job.status.expect("Job status should be present"),
            models::JobStatus::Pending
        );
    }
}

/// Test resource limits with multi-node constraints
/// Create 2 multi-node jobs, test with resources that have insufficient nodes
#[rstest]
fn test_prepare_jobs_node_resource_limits(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create workflow with 2 jobs that each need 4 nodes
    let jobs = create_multi_node_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // Test with resources that have only 4 nodes total (can support 1 job)
    let resources = models::ComputeNodesResources::new(32, 128.0, 0, 4);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::None),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Should return exactly 1 job (4 nodes available ÷ 4 nodes per job = 1 job must be returned)
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        1,
        "Server must return exactly 1 job with 4 nodes available for jobs needing 4 nodes each, got {}",
        returned_jobs.len()
    );
    let job = &returned_jobs[0];
    assert!(job.id.is_some());
    assert!(job.workflow_id == workflow_id);
    assert!(!job.name.is_empty());
    assert_eq!(
        job.status.expect("Job status should be present"),
        models::JobStatus::Pending
    );
}

/// Test complex resource allocation with mixed constraints
/// Test CPU, memory, and GPU limits simultaneously
#[rstest]
fn test_prepare_jobs_mixed_resource_limits(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create workflow with jobs that need 8 CPU, 32GB, 4 GPU each
    let jobs = create_gpu_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // Test with mixed constraints: enough CPU for 2 jobs, enough memory for 3 jobs, enough GPU for 1 job
    // The limiting factor should be GPUs (4 available, jobs need 4 each = max 1 job)
    let resources = models::ComputeNodesResources::new(16, 96.0, 4, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::GpusRuntimeMemory),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Should return exactly 1 job (limited by GPU: 4 GPUs ÷ 4 GPUs per job = 1 job must be returned)
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        1,
        "Server must return exactly 1 job limited by GPU constraint (most restrictive resource), got {}",
        returned_jobs.len()
    );
    let job = &returned_jobs[0];
    assert!(job.id.is_some());
    assert!(job.workflow_id == workflow_id);
    assert!(!job.name.is_empty());
    assert_eq!(
        job.status.expect("Job status should be present"),
        models::JobStatus::Pending
    );
}

/// Test that claim_jobs_based_on_resources respects job dependencies
/// Only jobs that are ready (not blocked by dependencies) should be returned
#[rstest]
fn test_prepare_jobs_with_dependencies(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create dependency chain: job1 → job2 → job3 (only job1 should be ready initially)
    let jobs = create_dependency_chain_workflow(config, true, 4, 8.0, 0, 1);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // Test with resources that could support all jobs
    let resources = models::ComputeNodesResources::new(12, 24.0, 0, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10, // Request more jobs than are ready
        Some(models::ClaimJobsSortMethod::None),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Should return exactly 1 job (chain_job_1, as job2 and job3 are blocked by dependencies)
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        1,
        "Server must return exactly 1 ready job from dependency chain, got {}",
        returned_jobs.len()
    );

    // Should be the first job in the chain
    let ready_job = &returned_jobs[0];
    assert!(ready_job.id.is_some());
    assert!(ready_job.workflow_id == workflow_id);
    assert_eq!(
        ready_job.name, "chain_job_1",
        "Ready job must be chain_job_1, got {}",
        ready_job.name
    );
    assert_eq!(
        ready_job.status.expect("Job status should be present"),
        models::JobStatus::Pending
    );
}

/// Test that limit parameter properly truncates job results when resources are abundant
/// Workflow: 100 jobs, each needing 1 CPU, 1.0 GB memory, 0 GPUs, 1 node
/// Resources: 104 CPUs, 104.0 GB memory total → Can support all 100 jobs
/// Limit: 32 jobs → Expected: Returns exactly 32 jobs (limited by limit parameter)
#[rstest]
fn test_prepare_jobs_limit_parameter_truncation(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create workflow with 100 jobs that each need 1 CPU, 1GB memory
    let jobs = create_many_jobs_workflow(config, true, 100);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // Test with resources that can support all 100 jobs (104 CPUs, 104GB total)
    let resources = models::ComputeNodesResources::new(104, 104.0, 0, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        32, // Limit to 32 jobs even though resources could support all 100
        Some(models::ClaimJobsSortMethod::None),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Should return exactly 32 jobs (limited by the limit parameter, not resources)
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        32,
        "Server must return exactly 32 jobs due to limit parameter (resources could support 100), got {}",
        returned_jobs.len()
    );

    // Validate all returned jobs are valid
    for job in &returned_jobs {
        assert!(job.id.is_some());
        assert!(job.workflow_id == workflow_id);
        assert!(!job.name.is_empty());
        assert!(
            job.name.starts_with("job_"),
            "Job name should start with 'job_', got {}",
            job.name
        );
        assert_eq!(
            job.status.expect("Job status should be present"),
            models::JobStatus::Pending
        );
    }

    // Ensure all returned jobs are unique
    let mut job_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for job in &returned_jobs {
        assert!(
            job_names.insert(job.name.clone()),
            "Duplicate job returned: {}",
            job.name
        );
    }
}

/// Test limit parameter with edge cases - limit larger than available jobs
/// Workflow: 20 jobs, each needing 1 CPU, 1.0 GB memory, 0 GPUs, 1 node  
/// Resources: 50 CPUs, 50.0 GB memory total → Can support all 20 jobs
/// Limit: 50 jobs → Expected: Returns exactly 20 jobs (limited by available jobs, not limit)
#[rstest]
fn test_prepare_jobs_limit_larger_than_available(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create workflow with 20 jobs
    let jobs = create_many_jobs_workflow(config, true, 20);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // Test with resources that can support all jobs, and limit higher than job count
    let resources = models::ComputeNodesResources::new(50, 50.0, 0, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        50, // Limit higher than the 20 jobs available
        Some(models::ClaimJobsSortMethod::None),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Should return exactly 20 jobs (all available jobs, not limited by limit parameter)
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        20,
        "Server must return exactly 20 jobs (all available) when limit is higher, got {}",
        returned_jobs.len()
    );

    // Validate all returned jobs are valid
    for job in &returned_jobs {
        assert!(job.id.is_some());
        assert!(job.workflow_id == workflow_id);
        assert!(!job.name.is_empty());
        assert_eq!(
            job.status.expect("Job status should be present"),
            models::JobStatus::Pending
        );
    }
}

/// Test limit parameter with very restrictive limit
/// Workflow: 50 jobs, each needing 1 CPU, 1.0 GB memory, 0 GPUs, 1 node
/// Resources: 100 CPUs, 100.0 GB memory total → Can support all 50 jobs
/// Limit: 1 job → Expected: Returns exactly 1 job
#[rstest]
fn test_prepare_jobs_limit_very_restrictive(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create workflow with 50 jobs
    let jobs = create_many_jobs_workflow(config, true, 50);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // Test with abundant resources but very restrictive limit
    let resources = models::ComputeNodesResources::new(100, 100.0, 0, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        1, // Very restrictive limit - only 1 job
        Some(models::ClaimJobsSortMethod::None),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    // Should return exactly 1 job (limited by the limit parameter)
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        1,
        "Server must return exactly 1 job due to restrictive limit parameter, got {}",
        returned_jobs.len()
    );

    let job = &returned_jobs[0];
    assert!(job.id.is_some());
    assert!(job.workflow_id == workflow_id);
    assert!(!job.name.is_empty());
    assert_eq!(
        job.status.expect("Job status should be present"),
        models::JobStatus::Pending
    );
}

/// Test ClaimJobsSortMethod::GpusRuntimeMemory sorting behavior
/// Jobs should be sorted by: GPU count (desc), then runtime (desc), then memory (desc)
#[rstest]
fn test_prepare_jobs_sort_gpus_runtime_memory(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create workflow with diverse jobs having different GPU, runtime, memory requirements
    let jobs = create_diverse_jobs_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // Use abundant resources so all jobs can potentially run (limited by job count)
    let resources = models::ComputeNodesResources::new(100, 500.0, 20, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10, // Get multiple jobs to test sorting
        Some(models::ClaimJobsSortMethod::GpusRuntimeMemory),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert!(
        !returned_jobs.is_empty(),
        "Should return at least some jobs for sorting test"
    );

    // Get resource requirements for each returned job to verify sorting
    let mut job_specs = Vec::new();
    for job in &returned_jobs {
        assert_eq!(
            job.status.expect("Job status should be present"),
            models::JobStatus::Pending
        );
        let rr_id = job
            .resource_requirements_id
            .expect("Job should have resource requirements");
        let rr = torc::client::default_api::get_resource_requirements(config, rr_id)
            .expect("Should be able to get resource requirements");

        // Parse runtime from ISO8601 format (P0DT24H -> 24 hours, P0DT30M -> 0.5 hours)
        let runtime_hours = if rr.runtime.contains("H") {
            let h_pos = rr.runtime.find("H").unwrap();
            let t_pos = rr.runtime.find("T").unwrap();
            rr.runtime[t_pos + 1..h_pos].parse::<f64>().unwrap_or(0.0)
        } else if rr.runtime.contains("M") {
            let m_pos = rr.runtime.find("M").unwrap();
            let t_pos = rr.runtime.find("T").unwrap();
            rr.runtime[t_pos + 1..m_pos].parse::<f64>().unwrap_or(0.0) / 60.0
        } else {
            0.0
        };

        // Parse memory from format like "32g" -> 32
        let memory_gb = rr.memory.trim_end_matches('g').parse::<i64>().unwrap_or(0);

        job_specs.push((job.name.clone(), rr.num_gpus, runtime_hours, memory_gb));
    }

    // Verify sorting: GPUs desc, then runtime desc, then memory desc
    for i in 1..job_specs.len() {
        let (prev_name, prev_gpus, prev_runtime, prev_memory) = &job_specs[i - 1];
        let (curr_name, curr_gpus, curr_runtime, curr_memory) = &job_specs[i];

        // Primary sort: GPUs descending
        if prev_gpus != curr_gpus {
            assert!(
                prev_gpus > curr_gpus,
                "Jobs should be sorted by GPUs (desc): job '{}' has {} GPUs, but job '{}' has {} GPUs",
                prev_name,
                prev_gpus,
                curr_name,
                curr_gpus
            );
        } else if prev_runtime != curr_runtime {
            // Secondary sort: Runtime descending (when GPUs are equal)
            assert!(
                prev_runtime >= curr_runtime,
                "Jobs with equal GPUs should be sorted by runtime (desc): job '{}' has {}h runtime, but job '{}' has {}h runtime",
                prev_name,
                prev_runtime,
                curr_name,
                curr_runtime
            );
        } else {
            // Tertiary sort: Memory descending (when GPUs and runtime are equal)
            assert!(
                prev_memory >= curr_memory,
                "Jobs with equal GPUs and runtime should be sorted by memory (desc): job '{}' has {}GB, but job '{}' has {}GB",
                prev_name,
                prev_memory,
                curr_name,
                curr_memory
            );
        }
    }
}

/// Test ClaimJobsSortMethod::GpusMemoryRuntime sorting behavior
/// Jobs should be sorted by: GPU count (desc), then memory (desc), then runtime (desc)
#[rstest]
fn test_prepare_jobs_sort_gpus_memory_runtime(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create workflow with diverse jobs having different GPU, memory, runtime requirements
    let jobs = create_diverse_jobs_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // Use abundant resources so all jobs can potentially run
    let resources = models::ComputeNodesResources::new(100, 500.0, 20, 1);

    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10, // Get multiple jobs to test sorting
        Some(models::ClaimJobsSortMethod::GpusMemoryRuntime),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert!(
        !returned_jobs.is_empty(),
        "Should return at least some jobs for sorting test"
    );

    // Get resource requirements for each returned job to verify sorting
    let mut job_specs = Vec::new();
    for job in &returned_jobs {
        assert_eq!(
            job.status.expect("Job status should be present"),
            models::JobStatus::Pending
        );
        let rr_id = job
            .resource_requirements_id
            .expect("Job should have resource requirements");
        let rr = torc::client::default_api::get_resource_requirements(config, rr_id)
            .expect("Should be able to get resource requirements");

        // Parse runtime from ISO8601 format
        let runtime_hours = if rr.runtime.contains("H") {
            let h_pos = rr.runtime.find("H").unwrap();
            let t_pos = rr.runtime.find("T").unwrap();
            rr.runtime[t_pos + 1..h_pos].parse::<f64>().unwrap_or(0.0)
        } else if rr.runtime.contains("M") {
            let m_pos = rr.runtime.find("M").unwrap();
            let t_pos = rr.runtime.find("T").unwrap();
            rr.runtime[t_pos + 1..m_pos].parse::<f64>().unwrap_or(0.0) / 60.0
        } else {
            0.0
        };

        // Parse memory from format like "32g" -> 32
        let memory_gb = rr.memory.trim_end_matches('g').parse::<i64>().unwrap_or(0);

        job_specs.push((job.name.clone(), rr.num_gpus, memory_gb, runtime_hours));
    }

    // Verify sorting: GPUs desc, then memory desc, then runtime desc
    for i in 1..job_specs.len() {
        let (prev_name, prev_gpus, prev_memory, prev_runtime) = &job_specs[i - 1];
        let (curr_name, curr_gpus, curr_memory, curr_runtime) = &job_specs[i];

        // Primary sort: GPUs descending
        if prev_gpus != curr_gpus {
            assert!(
                prev_gpus > curr_gpus,
                "Jobs should be sorted by GPUs (desc): job '{}' has {} GPUs, but job '{}' has {} GPUs",
                prev_name,
                prev_gpus,
                curr_name,
                curr_gpus
            );
        } else if prev_memory != curr_memory {
            // Secondary sort: Memory descending (when GPUs are equal)
            assert!(
                prev_memory >= curr_memory,
                "Jobs with equal GPUs should be sorted by memory (desc): job '{}' has {}GB, but job '{}' has {}GB",
                prev_name,
                prev_memory,
                curr_name,
                curr_memory
            );
        } else {
            // Tertiary sort: Runtime descending (when GPUs and memory are equal)
            assert!(
                prev_runtime >= curr_runtime,
                "Jobs with equal GPUs and memory should be sorted by runtime (desc): job '{}' has {}h runtime, but job '{}' has {}h runtime",
                prev_name,
                prev_runtime,
                curr_name,
                curr_runtime
            );
        }
    }
}

/// Test ClaimJobsSortMethod::None - no sorting should be applied
/// Jobs should be returned in their natural order (not sorted by resource requirements)
#[rstest]
fn test_prepare_jobs_sort_none(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create workflow with diverse jobs
    let jobs = create_diverse_jobs_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // Use abundant resources so all jobs can potentially run
    let resources = models::ComputeNodesResources::new(100, 500.0, 20, 1);

    let result_none = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::None),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    let returned_jobs_none = result_none.jobs.expect("Server must return jobs array");
    assert!(
        !returned_jobs_none.is_empty(),
        "Should return at least some jobs"
    );

    default_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    // Also get results with sorting to compare
    let result_sorted = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::GpusRuntimeMemory),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    let returned_jobs_sorted = result_sorted.jobs.expect("Server must return jobs array");

    // The None method should not necessarily match the sorted order
    // We can't easily test the "natural order" without knowing the server's internal ordering,
    // but we can at least verify that jobs are returned and have valid resource requirements
    for job in &returned_jobs_none {
        assert!(job.id.is_some());
        assert!(job.workflow_id == workflow_id);
        assert!(!job.name.is_empty());
        assert!(job.resource_requirements_id.is_some());
        assert_eq!(
            job.status.expect("Job status should be present"),
            models::JobStatus::Pending
        );
    }

    // Verify we get the same jobs in both cases (just potentially different order)
    let mut names_none: Vec<_> = returned_jobs_none.iter().map(|j| &j.name).collect();
    let mut names_sorted: Vec<_> = returned_jobs_sorted.iter().map(|j| &j.name).collect();
    names_none.sort();
    names_sorted.sort();

    assert_eq!(
        names_none, names_sorted,
        "Both sort methods should return the same jobs, just in different order"
    );
}

/// Test that different sort methods can return the same jobs in different orders
/// This validates that sorting actually changes the order of returned jobs
#[rstest]
fn test_prepare_jobs_different_sort_methods_different_orders(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create workflow with diverse jobs that will sort differently
    let jobs = create_diverse_jobs_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // Use abundant resources so all jobs can run
    let resources = models::ComputeNodesResources::new(100, 500.0, 20, 1);

    // Get results with GpusRuntimeMemory sorting
    let result1 = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::GpusRuntimeMemory),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    default_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    // Get results with GpusMemoryRuntime sorting
    let result2 = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::GpusMemoryRuntime),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    default_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    let jobs1 = result1.jobs.expect("Server must return jobs array");
    let jobs2 = result2.jobs.expect("Server must return jobs array");

    assert!(
        !jobs1.is_empty(),
        "Should return jobs for first sort method"
    );
    assert!(
        !jobs2.is_empty(),
        "Should return jobs for second sort method"
    );

    // Both should return the same job set
    let names1: Vec<_> = jobs1.iter().map(|j| &j.name).collect();
    let names2: Vec<_> = jobs2.iter().map(|j| &j.name).collect();
    let names1_sorted = {
        let mut temp = names1.clone();
        temp.sort();
        temp
    };
    let names2_sorted = {
        let mut temp = names2.clone();
        temp.sort();
        temp
    };

    assert_eq!(
        names1_sorted, names2_sorted,
        "Both sort methods should return the same jobs"
    );

    // The order should potentially be different (unless jobs happen to sort the same way)
    // We can't guarantee they'll be different, but we can at least verify the sorting logic works
    for (i, job) in jobs1.iter().enumerate() {
        assert!(job.id.is_some());
        assert!(job.workflow_id == workflow_id);
        assert!(!job.name.is_empty());
        // Job names should follow pattern diverse_job_N
        assert!(
            job.name.starts_with("diverse_job_"),
            "Job {} should have name starting with 'diverse_job_', got {}",
            i,
            job.name
        );
        assert_eq!(
            job.status.expect("Job status should be present"),
            models::JobStatus::Pending
        );
    }
}

/// Test concurrent job allocation to verify database locking and mutual exclusion
/// This test simulates multiple clients requesting jobs simultaneously and verifies:
/// 1. Each job is allocated to exactly one client (no double allocation)
/// 2. All jobs are eventually allocated (no jobs are missed)
/// The test creates N threads (one per CPU core) that concurrently request jobs,
/// ensuring the server's database locking mechanism correctly prevents race conditions
#[rstest]
fn test_prepare_jobs_concurrent_allocation(start_server: &ServerProcess) {
    let config = &start_server.config;

    let num_threads = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    // Create workflow with 100 jobs
    let jobs = create_many_jobs_workflow(config, true, 100);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // Resources that can support all jobs
    let resources = models::ComputeNodesResources::new(200, 200.0, 0, 1);

    // Shared state to track job allocations across threads
    // Maps job_id -> thread_id that received it
    let job_allocations: Arc<Mutex<HashMap<i64, usize>>> = Arc::new(Mutex::new(HashMap::new()));

    // Track all unique job IDs we created for validation
    let expected_job_ids: HashSet<i64> = jobs.values().filter_map(|j| j.id).collect();

    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let config_clone = config.clone();
        let resources_clone = resources.clone();
        let job_allocations_clone = Arc::clone(&job_allocations);

        let handle = thread::spawn(move || {
            let mut thread_jobs = Vec::new();
            const MAX_ITERATIONS: usize = 50;

            // Each thread keeps requesting jobs until none are available
            for _iteration in 1..=MAX_ITERATIONS {
                // Request up to 10 jobs at a time
                let result = default_api::claim_jobs_based_on_resources(
                    &config_clone,
                    workflow_id,
                    &resources_clone,
                    10,
                    Some(models::ClaimJobsSortMethod::None),
                    None,
                );

                match result {
                    Ok(response) => {
                        if let Some(jobs) = response.jobs {
                            if jobs.is_empty() {
                                // No more jobs available
                                break;
                            }

                            for job in jobs {
                                if let Some(job_id) = job.id {
                                    thread_jobs.push(job_id);

                                    // Update shared allocation map
                                    let mut allocations = job_allocations_clone.lock().unwrap();
                                    allocations.insert(job_id, thread_id);
                                }
                            }
                        } else {
                            break;
                        }
                    }
                    Err(e) => {
                        panic!("Thread {} error: {}", thread_id, e);
                    }
                }

                // Small delay to allow other threads to interleave
                thread::sleep(std::time::Duration::from_millis(10));
            }

            (thread_id, thread_jobs)
        });

        handles.push(handle);
    }

    // Collect results from all threads
    let mut thread_results: HashMap<usize, Vec<i64>> = HashMap::new();
    for handle in handles {
        let (thread_id, jobs) = handle.join().expect("Thread panicked");
        thread_results.insert(thread_id, jobs);
    }

    let allocations = job_allocations.lock().unwrap();

    // Check 1: All expected jobs were allocated
    let allocated_job_ids: HashSet<i64> = allocations.keys().copied().collect();
    let missing_jobs: Vec<_> = expected_job_ids.difference(&allocated_job_ids).collect();
    assert!(
        missing_jobs.is_empty(),
        "Missing {} jobs that were never allocated: {:?}",
        missing_jobs.len(),
        missing_jobs
    );

    // Check 2: No unexpected jobs were allocated
    let unexpected_jobs: Vec<_> = allocated_job_ids.difference(&expected_job_ids).collect();
    assert!(
        unexpected_jobs.is_empty(),
        "Found {} unexpected job IDs that weren't created: {:?}",
        unexpected_jobs.len(),
        unexpected_jobs
    );

    // Check 3: Each job appears exactly once (no double allocation)
    let mut job_counts: HashMap<i64, usize> = HashMap::new();
    for jobs in thread_results.values() {
        for job_id in jobs {
            *job_counts.entry(*job_id).or_insert(0) += 1;
        }
    }

    let duplicates: Vec<_> = job_counts.iter().filter(|(_, count)| **count > 1).collect();

    assert!(
        duplicates.is_empty(),
        "Found {} jobs allocated to multiple threads: {:?}",
        duplicates.len(),
        duplicates
    );
}

/// Test that claim_jobs_based_on_resources returns invocation_script when set on a job
#[rstest]
fn test_claim_jobs_based_on_resources_returns_invocation_script(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow
    let workflow = models::WorkflowModel::new(
        "invocation_script_resource_test".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        default_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create resource requirements
    let resource_req = create_test_resource_requirements(
        config,
        workflow_id,
        "test_rr",
        1,        // num_cpus
        0,        // num_gpus
        1,        // num_nodes
        "1g",     // memory
        "P0DT1H", // runtime
    );

    // Create a job with invocation_script set
    let invocation_script = "#!/bin/bash\nset -e\nexport MY_VAR=test\n".to_string();
    let mut job = models::JobModel::new(
        workflow_id,
        "job_with_invocation_script".to_string(),
        "echo hello".to_string(),
    );
    job.invocation_script = Some(invocation_script.clone());
    job.resource_requirements_id = Some(resource_req.id.unwrap());

    let _created_job = default_api::create_job(config, job).expect("Failed to create job");

    // Initialize jobs
    default_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    // Claim the job with sufficient resources
    let resources = models::ComputeNodesResources::new(1, 1.0, 0, 1);
    let result =
        default_api::claim_jobs_based_on_resources(config, workflow_id, &resources, 10, None, None)
            .expect("claim_jobs_based_on_resources should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(returned_jobs.len(), 1, "Should return exactly 1 job");

    let returned_job = &returned_jobs[0];
    assert_eq!(
        returned_job.invocation_script,
        Some(invocation_script),
        "invocation_script should be returned by claim_jobs_based_on_resources"
    );
}

/// Test that multi-node jobs (step_nodes > 1) reserve whole nodes exclusively,
/// preventing single-node jobs from being scheduled on those nodes.
///
/// Allocation: 4 nodes × 16 CPUs per node (64 total CPUs).
/// Jobs: 1 multi-node job (step_nodes=2) + 3 single-node jobs (8 CPUs each).
///
/// The multi-node job reserves 2 whole nodes, leaving 2 nodes (32 CPUs) for
/// single-node jobs. All 3 single-node jobs fit (3 × 8 = 24 ≤ 32).
/// Expected: 4 jobs returned (1 multi-node + 3 single-node).
#[rstest]
fn test_multi_node_reserves_whole_nodes(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = models::WorkflowModel::new("multi_node_test".to_string(), "user".to_string());
    let created_workflow =
        default_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Multi-node resource requirements: step_nodes=2
    let mut multi_rr =
        models::ResourceRequirementsModel::new(workflow_id, "multi_node_rr".to_string());
    multi_rr.num_cpus = 16;
    multi_rr.num_gpus = 0;
    multi_rr.num_nodes = 2;
    multi_rr.step_nodes = Some(2);
    multi_rr.memory = "32g".to_string();
    multi_rr.runtime = "PT1H".to_string();
    let multi_rr = default_api::create_resource_requirements(config, multi_rr)
        .expect("Failed to create multi-node RR");

    // Single-node resource requirements
    let single_rr = create_test_resource_requirements(
        config,
        workflow_id,
        "single_node_rr",
        8,
        0,
        1,
        "16g",
        "PT1H",
    );

    // Create 1 multi-node job
    let mut job = models::JobModel::new(workflow_id, "mpi_job".to_string(), "echo mpi".to_string());
    job.resource_requirements_id = Some(multi_rr.id.unwrap());
    default_api::create_job(config, job).expect("Failed to create job");

    // Create 3 single-node jobs
    for i in 0..3 {
        let mut job = models::JobModel::new(
            workflow_id,
            format!("single_job_{}", i),
            format!("echo single {}", i),
        );
        job.resource_requirements_id = Some(single_rr.id.unwrap());
        default_api::create_job(config, job).expect("Failed to create job");
    }

    default_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    // 4 nodes × 16 CPUs per node: multi-node takes 2 nodes, 3 single-node jobs fit on remaining 2
    let resources = models::ComputeNodesResources::new(16, 32.0, 0, 4);
    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::GpusRuntimeMemory),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        4,
        "Expected 4 jobs (1 multi-node + 3 single-node), got {}",
        returned_jobs.len()
    );
}

/// Test that multi-node jobs cannot be placed when single-node jobs would
/// no longer fit after the exclusive node reservation.
///
/// Allocation: 2 nodes × 16 CPUs per node (32 total CPUs).
/// Jobs: 2 single-node jobs (8 CPUs each) + 1 multi-node job (step_nodes=2).
///
/// The 2 single-node jobs consume shared resources. The multi-node job needs
/// both nodes free, but the single-node jobs are using them. The server
/// processes jobs in the order returned by SQL (sorted by gpus/runtime/memory
/// descending), so the multi-node job should be selected first, then the
/// single-node jobs should fit. All 3 should be returned... unless the
/// multi-node job takes both nodes, leaving 0 shared nodes.
///
/// With 2 nodes and step_nodes=2: the multi-node job takes both nodes.
/// No shared nodes remain → single-node jobs cannot fit.
/// Expected: 1 job returned (the multi-node job only).
#[rstest]
fn test_multi_node_blocks_single_node_when_all_nodes_used(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = models::WorkflowModel::new("mn_blocks_sn_test".to_string(), "user".to_string());
    let created_workflow =
        default_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Multi-node RR: needs all 2 nodes
    let mut multi_rr =
        models::ResourceRequirementsModel::new(workflow_id, "multi_2node".to_string());
    multi_rr.num_cpus = 16;
    multi_rr.num_gpus = 0;
    multi_rr.num_nodes = 2;
    multi_rr.step_nodes = Some(2);
    multi_rr.memory = "32g".to_string();
    multi_rr.runtime = "PT2H".to_string(); // higher runtime so sorted first
    let multi_rr = default_api::create_resource_requirements(config, multi_rr)
        .expect("Failed to create multi-node RR");

    // Single-node RR
    let single_rr = create_test_resource_requirements(
        config,
        workflow_id,
        "single_1node",
        8,
        0,
        1,
        "16g",
        "PT1H",
    );

    // Create jobs: 1 multi-node + 2 single-node
    let mut job =
        models::JobModel::new(workflow_id, "mpi_full".to_string(), "echo mpi".to_string());
    job.resource_requirements_id = Some(multi_rr.id.unwrap());
    default_api::create_job(config, job).expect("Failed to create job");

    for i in 0..2 {
        let mut job = models::JobModel::new(
            workflow_id,
            format!("small_job_{}", i),
            format!("echo {}", i),
        );
        job.resource_requirements_id = Some(single_rr.id.unwrap());
        default_api::create_job(config, job).expect("Failed to create job");
    }

    default_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    // 2 nodes × 16 CPUs per node
    let resources = models::ComputeNodesResources::new(16, 32.0, 0, 2);
    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::GpusRuntimeMemory),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        1,
        "Expected 1 job (multi-node uses all 2 nodes, no room for single-node), got {}",
        returned_jobs.len()
    );
    assert_eq!(returned_jobs[0].name, "mpi_full");
}

/// Test that step_nodes determines multi-node reservation.
///
/// A job with step_nodes=2 reserves 2 whole nodes, so only 2 such jobs
/// fit on a 4-node allocation.
#[rstest]
fn test_step_nodes_reserves_whole_nodes(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = models::WorkflowModel::new("step_nodes_test".to_string(), "user".to_string());
    let created_workflow =
        default_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    // RR with step_nodes=2 (num_nodes must be >= step_nodes)
    let mut rr = models::ResourceRequirementsModel::new(workflow_id, "step2_rr".to_string());
    rr.num_cpus = 8;
    rr.num_gpus = 0;
    rr.num_nodes = 2;
    rr.step_nodes = Some(2);
    rr.memory = "16g".to_string();
    rr.runtime = "PT1H".to_string();
    let rr = default_api::create_resource_requirements(config, rr).expect("Failed to create RR");

    // Create 2 jobs with this RR
    for i in 0..2 {
        let mut job = models::JobModel::new(
            workflow_id,
            format!("step_job_{}", i),
            format!("echo {}", i),
        );
        job.resource_requirements_id = Some(rr.id.unwrap());
        default_api::create_job(config, job).expect("Failed to create job");
    }

    default_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    // 4 nodes: each job reserves 2 nodes, so only 2 jobs can fit
    let resources = models::ComputeNodesResources::new(16, 32.0, 0, 4);
    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        Some(models::ClaimJobsSortMethod::None),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        2,
        "Expected 2 jobs (each reserves 2 of 4 nodes via step_nodes), got {}",
        returned_jobs.len()
    );
}
