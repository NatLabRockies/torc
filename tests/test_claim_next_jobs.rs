mod common;

use common::{
    ServerProcess, create_custom_resources_workflow, create_many_jobs_workflow,
    create_minimal_resources_workflow, create_test_compute_node, create_test_resource_requirements,
    start_server,
};
use rstest::rstest;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use torc::client::apis;
use torc::models;

/// Test basic claim_next_jobs with default limit
#[rstest]
fn test_prepare_next_jobs_basic(start_server: &ServerProcess) {
    let config = &start_server.config;
    let jobs = create_minimal_resources_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let result = apis::workflows_api::claim_next_jobs(config, workflow_id, None)
        .expect("claim_next_jobs should succeed");

    // Should return jobs (default limit is 10)
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert!(
        !returned_jobs.is_empty(),
        "Should return at least some jobs"
    );
    assert!(
        returned_jobs.len() <= 10,
        "Should not exceed default limit of 10"
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

/// Test claim_next_jobs with explicit limit
#[rstest]
#[case(1)]
#[case(5)]
#[case(20)]
#[case(50)]
fn test_prepare_next_jobs_with_limit(start_server: &ServerProcess, #[case] limit: i64) {
    let config = &start_server.config;
    let jobs = create_many_jobs_workflow(config, true, 100);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let result = apis::workflows_api::claim_next_jobs(config, workflow_id, Some(limit))
        .expect("claim_next_jobs should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert!(
        returned_jobs.len() <= limit as usize,
        "Should not return more jobs than limit: returned {}, limit {}",
        returned_jobs.len(),
        limit
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

/// Test claim_next_jobs returns exactly the limit when enough jobs available
#[rstest]
fn test_prepare_next_jobs_returns_full_limit(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create workflow with 100 jobs
    let jobs = create_many_jobs_workflow(config, true, 100);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let limit = 25;
    let result = apis::workflows_api::claim_next_jobs(config, workflow_id, Some(limit))
        .expect("claim_next_jobs should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        limit as usize,
        "Should return exactly {} jobs when that many are available",
        limit
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

/// Test claim_next_jobs with limit larger than available jobs
#[rstest]
fn test_prepare_next_jobs_limit_exceeds_available(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create workflow with only 4 jobs
    let jobs = create_minimal_resources_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let limit = 50; // Much larger than 4 available jobs
    let result = apis::workflows_api::claim_next_jobs(config, workflow_id, Some(limit))
        .expect("claim_next_jobs should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        4,
        "Should return all 4 available jobs when limit exceeds available count"
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

/// Test claim_next_jobs with no ready jobs
#[rstest]
fn test_prepare_next_jobs_no_ready_jobs(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create workflow without initializing jobs (so all jobs remain uninitialized)
    let jobs = create_minimal_resources_workflow(config, false);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let result = apis::workflows_api::claim_next_jobs(config, workflow_id, Some(10))
        .expect("claim_next_jobs should succeed");

    // Should return empty jobs list when no jobs are ready
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert!(
        returned_jobs.is_empty(),
        "Should return empty array when no jobs are ready"
    );
}

/// Test claim_next_jobs with invalid workflow ID
#[rstest]
fn test_prepare_next_jobs_invalid_workflow(start_server: &ServerProcess) {
    let config = &start_server.config;
    let invalid_workflow_id = 99999i64;

    let result = apis::workflows_api::claim_next_jobs(config, invalid_workflow_id, Some(10));

    // Should return an error for invalid workflow ID
    assert!(
        result.is_err(),
        "Should return error for invalid workflow ID"
    );
}

/// Test claim_next_jobs doesn't return same jobs twice
#[rstest]
fn test_prepare_next_jobs_no_double_allocation(start_server: &ServerProcess) {
    let config = &start_server.config;
    let jobs = create_many_jobs_workflow(config, true, 50);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // First request: get 20 jobs
    let result1 = apis::workflows_api::claim_next_jobs(config, workflow_id, Some(20))
        .expect("First claim_next_jobs should succeed");

    let jobs1 = result1.jobs.expect("First call must return jobs array");
    assert_eq!(jobs1.len(), 20, "Should return 20 jobs on first call");

    // Second request: get another 20 jobs
    let result2 = apis::workflows_api::claim_next_jobs(config, workflow_id, Some(20))
        .expect("Second claim_next_jobs should succeed");

    let jobs2 = result2.jobs.expect("Second call must return jobs array");
    assert_eq!(jobs2.len(), 20, "Should return 20 more jobs on second call");

    // Verify no overlap between the two sets
    let ids1: HashSet<i64> = jobs1.iter().filter_map(|j| j.id).collect();
    let ids2: HashSet<i64> = jobs2.iter().filter_map(|j| j.id).collect();

    let overlap: Vec<_> = ids1.intersection(&ids2).collect();
    assert!(
        overlap.is_empty(),
        "Should not return the same jobs twice: {:?}",
        overlap
    );
}

/// Test claim_next_jobs marks jobs as pending
#[rstest]
fn test_prepare_next_jobs_marks_jobs_pending(start_server: &ServerProcess) {
    let config = &start_server.config;
    let jobs = create_minimal_resources_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // Get jobs
    let result = apis::workflows_api::claim_next_jobs(config, workflow_id, Some(2))
        .expect("claim_next_jobs should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(returned_jobs.len(), 2, "Should return 2 jobs");

    // Verify all returned jobs have Pending status
    for job in &returned_jobs {
        assert_eq!(
            job.status.expect("Job status should be present"),
            models::JobStatus::Pending,
            "job name='{}' should be marked as Pending",
            job.name
        );

        // Verify by fetching the job from the server
        let job_id = job.id.expect("Job should have ID");
        let fetched_job =
            apis::jobs_api::get_job(config, job_id).expect("Should be able to fetch job");
        assert_eq!(
            fetched_job.status.expect("Fetched job should have status"),
            models::JobStatus::Pending,
            "Fetched job {} should be Pending in database",
            fetched_job.name
        );
    }
}

/// Test claim_next_jobs with canceled workflow
#[rstest]
fn test_prepare_next_jobs_canceled_workflow(start_server: &ServerProcess) {
    let config = &start_server.config;
    let jobs = create_minimal_resources_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // Cancel the workflow
    apis::workflows_api::cancel_workflow(config, workflow_id)
        .expect("Should be able to cancel workflow");

    // Try to get jobs from canceled workflow
    let result = apis::workflows_api::claim_next_jobs(config, workflow_id, Some(10))
        .expect("claim_next_jobs should succeed even for canceled workflow");

    // Should return empty jobs list for canceled workflow
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert!(
        returned_jobs.is_empty(),
        "Should return empty array for canceled workflow"
    );
}

/// Test claim_next_jobs exhausts all ready jobs
#[rstest]
fn test_prepare_next_jobs_exhaust_all_jobs(start_server: &ServerProcess) {
    let config = &start_server.config;
    let num_jobs = 37; // Use odd number to test that we can exhaust all jobs
    let jobs = create_many_jobs_workflow(config, true, num_jobs);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let mut total_jobs_received = 0;
    let mut all_job_ids = HashSet::new();

    // Keep requesting jobs until none are returned
    for iteration in 0..20 {
        let result = apis::workflows_api::claim_next_jobs(config, workflow_id, Some(10))
            .expect("claim_next_jobs should succeed");

        let returned_jobs = result.jobs.expect("Server must return jobs array");

        if returned_jobs.is_empty() {
            println!("No more jobs available after {} iterations", iteration);
            break;
        }

        for job in &returned_jobs {
            let job_id = job.id.expect("Job should have ID");
            assert!(
                all_job_ids.insert(job_id),
                "job_id={} returned multiple times",
                job_id
            );
            total_jobs_received += 1;
        }
    }

    assert_eq!(
        total_jobs_received, num_jobs,
        "Should have received all {} jobs",
        num_jobs
    );
}

/// Test claim_next_jobs response structure
#[rstest]
fn test_prepare_next_jobs_response_structure(start_server: &ServerProcess) {
    let config = &start_server.config;
    let jobs = create_minimal_resources_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let result = apis::workflows_api::claim_next_jobs(config, workflow_id, Some(3))
        .expect("claim_next_jobs should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");

    for job in &returned_jobs {
        // Verify required fields
        assert!(job.id.is_some(), "Job should have ID");
        assert!(
            job.workflow_id == workflow_id,
            "Job should have correct workflow_id"
        );
        assert!(!job.name.is_empty(), "Job should have name");
        assert!(!job.command.is_empty(), "Job should have command");
        assert!(job.status.is_some(), "Job should have status");
        assert_eq!(
            job.status.unwrap(),
            models::JobStatus::Pending,
            "Job should have Pending status"
        );
        assert!(
            job.resource_requirements_id.is_some(),
            "Job should have resource_requirements_id"
        );

        // Optional fields can be None
        assert!(job.cancel_on_blocking_job_failure.is_some());
        assert!(job.supports_termination.is_some());
    }
}

/// Test claim_next_jobs with various job counts
#[rstest]
#[case(10, 5)] // More jobs than limit
#[case(5, 10)] // Fewer jobs than limit
#[case(10, 10)] // Exactly matching limit
fn test_prepare_next_jobs_various_counts(
    start_server: &ServerProcess,
    #[case] num_jobs: usize,
    #[case] limit: i64,
) {
    let config = &start_server.config;
    let jobs = create_many_jobs_workflow(config, true, num_jobs);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let result = apis::workflows_api::claim_next_jobs(config, workflow_id, Some(limit))
        .expect("claim_next_jobs should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    let expected_count = std::cmp::min(num_jobs, limit as usize);

    assert_eq!(
        returned_jobs.len(),
        expected_count,
        "Should return {} jobs (min of {} jobs available and {} limit)",
        expected_count,
        num_jobs,
        limit
    );

    for job in &returned_jobs {
        assert!(job.id.is_some());
        assert!(job.workflow_id == workflow_id);
        assert_eq!(
            job.status.expect("Job status should be present"),
            models::JobStatus::Pending
        );
    }
}

/// Test claim_next_jobs ignores resource requirements
/// This verifies that unlike claim_jobs_based_on_resources, this function
/// does not filter jobs based on resource constraints
#[rstest]
fn test_prepare_next_jobs_ignores_resources(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create jobs with very different resource requirements
    // - 2 jobs with 1 CPU, 1GB (small)
    // - 2 jobs with 64 CPUs, 512GB (large)
    let small_jobs = create_custom_resources_workflow(config, true, 1, 1.0, 0, 1);
    let workflow_id = small_jobs.values().next().unwrap().workflow_id;

    // Add large resource jobs to the same workflow
    let _large_jobs = create_custom_resources_workflow(config, true, 64, 512.0, 0, 1);

    // Request jobs without any resource filtering
    let result = apis::workflows_api::claim_next_jobs(config, workflow_id, Some(10))
        .expect("claim_next_jobs should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");

    // Should return both small and large resource jobs
    // (unlike claim_jobs_based_on_resources which would filter by resources)
    assert!(
        !returned_jobs.is_empty(),
        "Should return jobs regardless of resource requirements"
    );

    for job in &returned_jobs {
        assert!(job.id.is_some());
        assert_eq!(
            job.status.expect("Job status should be present"),
            models::JobStatus::Pending
        );
    }
}

/// Test concurrent job allocation from multiple threads to verify database locking
/// This test simulates multiple clients requesting jobs simultaneously and verifies:
/// 1. Each job is allocated to exactly one thread (no double allocation)
/// 2. All jobs are eventually allocated (no jobs are missed)
/// 3. The database locking mechanism prevents race conditions
#[rstest]
fn test_prepare_next_jobs_concurrent_allocation(start_server: &ServerProcess) {
    let config = &start_server.config;

    let num_threads = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    // Create workflow with 100 jobs
    let jobs = create_many_jobs_workflow(config, true, 100);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    // Shared state to track job allocations across threads
    // Maps job_id -> thread_id that received it
    let job_allocations: Arc<Mutex<HashMap<i64, usize>>> = Arc::new(Mutex::new(HashMap::new()));

    // Track all unique job IDs we created for validation
    let expected_job_ids: HashSet<i64> = jobs.values().filter_map(|j| j.id).collect();

    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let config_clone = config.clone();
        let job_allocations_clone = Arc::clone(&job_allocations);

        let handle = thread::spawn(move || {
            let mut thread_jobs = Vec::new();
            const MAX_ITERATIONS: usize = 50;

            // Each thread keeps requesting jobs until none are available
            for _iteration in 1..=MAX_ITERATIONS {
                // Request up to 5 jobs at a time
                let result =
                    apis::workflows_api::claim_next_jobs(&config_clone, workflow_id, Some(5));

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

                                    // Check if this job was already allocated
                                    if let Some(previous_thread) = allocations.get(&job_id) {
                                        panic!(
                                            "RACE CONDITION: Job {} was allocated to both thread {} and thread {}",
                                            job_id, previous_thread, thread_id
                                        );
                                    }

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
                thread::sleep(std::time::Duration::from_millis(5));
            }

            (thread_id, thread_jobs)
        });

        handles.push(handle);
    }

    // Collect results from all threads
    let mut thread_results: HashMap<usize, Vec<i64>> = HashMap::new();
    for handle in handles {
        let (thread_id, jobs) = handle.join().expect("Thread panicked");
        println!("Thread {} received {} jobs", thread_id, jobs.len());
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

    // Check 4: Total count matches
    let total_allocated: usize = thread_results.values().map(|jobs| jobs.len()).sum();
    assert_eq!(
        total_allocated, 100,
        "Should have allocated all 100 jobs across all threads"
    );

    println!(
        "Successfully allocated 100 jobs across {} threads with no race conditions",
        num_threads
    );
}

/// Test that concurrent requests with very small batches still prevent double allocation
#[rstest]
fn test_prepare_next_jobs_concurrent_small_batches(start_server: &ServerProcess) {
    let config = &start_server.config;

    let num_threads = 8; // More threads than typical CPU count to increase contention

    // Create workflow with 40 jobs
    let jobs = create_many_jobs_workflow(config, true, 40);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let job_allocations: Arc<Mutex<HashMap<i64, usize>>> = Arc::new(Mutex::new(HashMap::new()));
    let expected_job_ids: HashSet<i64> = jobs.values().filter_map(|j| j.id).collect();

    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let config_clone = config.clone();
        let job_allocations_clone = Arc::clone(&job_allocations);

        let handle = thread::spawn(move || {
            let mut thread_jobs = Vec::new();

            // Request only 1 job at a time to maximize contention
            for _iteration in 0..20 {
                let result = apis::workflows_api::claim_next_jobs(
                    &config_clone,
                    workflow_id,
                    Some(1), // Request just 1 job at a time
                );

                match result {
                    Ok(response) => {
                        if let Some(jobs) = response.jobs {
                            if jobs.is_empty() {
                                break;
                            }

                            for job in jobs {
                                if let Some(job_id) = job.id {
                                    thread_jobs.push(job_id);

                                    let mut allocations = job_allocations_clone.lock().unwrap();
                                    if let Some(previous_thread) = allocations.get(&job_id) {
                                        panic!(
                                            "RACE CONDITION: Job {} allocated to threads {} and {}",
                                            job_id, previous_thread, thread_id
                                        );
                                    }
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

                // Very small delay to increase likelihood of race conditions
                thread::sleep(std::time::Duration::from_micros(100));
            }

            thread_jobs
        });

        handles.push(handle);
    }

    // Collect results
    let mut all_jobs = Vec::new();
    for handle in handles {
        let thread_jobs = handle.join().expect("Thread panicked");
        all_jobs.extend(thread_jobs);
    }

    // Verify no duplicates
    let unique_jobs: HashSet<i64> = all_jobs.iter().copied().collect();
    assert_eq!(
        unique_jobs.len(),
        all_jobs.len(),
        "Found duplicate job allocations"
    );

    // Verify all jobs were allocated
    assert_eq!(
        unique_jobs.len(),
        expected_job_ids.len(),
        "Not all jobs were allocated"
    );

    println!(
        "Successfully allocated {} jobs across {} threads requesting 1 job at a time",
        all_jobs.len(),
        num_threads
    );
}

/// Test claim_next_jobs with zero limit edge case
#[rstest]
fn test_prepare_next_jobs_zero_limit(start_server: &ServerProcess) {
    let config = &start_server.config;
    let jobs = create_minimal_resources_workflow(config, true);
    let job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = job.workflow_id;

    let result = apis::workflows_api::claim_next_jobs(config, workflow_id, Some(0))
        .expect("claim_next_jobs should succeed with limit=0");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert!(
        returned_jobs.is_empty(),
        "Should return empty array when limit is 0"
    );
}

/// Test that claim_next_jobs returns jobs in priority order (higher priority first)
#[rstest]
fn test_claim_next_jobs_priority_ordering(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow
    let workflow = models::WorkflowModel::new(
        "priority_ordering_test".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create jobs with different priorities: 0, 5, 10
    let priorities = [0i64, 5, 10];
    for p in priorities {
        let mut job = models::JobModel::new(
            workflow_id,
            format!("priority_job_{p}"),
            format!("echo priority {p}"),
        );
        job.priority = Some(p);
        apis::jobs_api::create_job(config, job).expect("Failed to create job");
    }

    // Initialize jobs so they become ready
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    // Claim one job at a time and verify descending priority order
    let first = apis::workflows_api::claim_next_jobs(config, workflow_id, Some(1))
        .expect("claim_next_jobs should succeed");
    let first_jobs = first.jobs.expect("Server must return jobs array");
    assert_eq!(first_jobs.len(), 1);
    assert_eq!(
        first_jobs[0].priority,
        Some(10),
        "Highest priority job (10) should be claimed first"
    );

    let second = apis::workflows_api::claim_next_jobs(config, workflow_id, Some(1))
        .expect("claim_next_jobs should succeed");
    let second_jobs = second.jobs.expect("Server must return jobs array");
    assert_eq!(second_jobs.len(), 1);
    assert_eq!(
        second_jobs[0].priority,
        Some(5),
        "Second highest priority job (5) should be claimed second"
    );

    let third = apis::workflows_api::claim_next_jobs(config, workflow_id, Some(1))
        .expect("claim_next_jobs should succeed");
    let third_jobs = third.jobs.expect("Server must return jobs array");
    assert_eq!(third_jobs.len(), 1);
    assert_eq!(
        third_jobs[0].priority,
        Some(0),
        "Lowest priority job (0) should be claimed last"
    );
}

/// Test that claim_next_jobs returns invocation_script when set on a job
#[rstest]
fn test_claim_next_jobs_returns_invocation_script(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow
    let workflow = models::WorkflowModel::new(
        "invocation_script_test".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create a job with invocation_script set
    let invocation_script = "#!/bin/bash\nset -e\nexport MY_VAR=test\n".to_string();
    let mut job = models::JobModel::new(
        workflow_id,
        "job_with_invocation_script".to_string(),
        "echo hello".to_string(),
    );
    job.invocation_script = Some(invocation_script.clone());

    let _created_job = apis::jobs_api::create_job(config, job).expect("Failed to create job");

    // Initialize jobs
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    // Claim the job
    let result = apis::workflows_api::claim_next_jobs(config, workflow_id, Some(1))
        .expect("claim_next_jobs should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(returned_jobs.len(), 1, "Should return exactly 1 job");

    let returned_job = &returned_jobs[0];
    assert_eq!(
        returned_job.invocation_script,
        Some(invocation_script),
        "invocation_script should be returned by claim_next_jobs"
    );
}

/// Create a workflow whose jobs form `num_chains` independent linear dependency
/// chains, each `chain_depth` jobs long. After initialization, only the head of
/// each chain is `Ready`; every downstream job is `Blocked` until its single
/// predecessor completes. Returns `(workflow_id, total_job_count, run_id)`.
///
/// This shape forces the concurrent claim/complete test to interleave all three
/// server code paths: claiming `Ready` jobs (`BEGIN IMMEDIATE` write lock),
/// completing jobs, and the background unblock task promoting `Blocked` jobs to
/// `Ready` as their predecessors finish.
fn create_job_chains_workflow(
    config: &torc::client::apis::configuration::Configuration,
    num_chains: usize,
    chain_depth: usize,
) -> (i64, usize, i64) {
    let workflow = models::WorkflowModel::new(
        format!("chains_{num_chains}x{chain_depth}"),
        "test_user".to_string(),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to add workflow");
    let workflow_id = created_workflow.id.unwrap();

    let resource_req = create_test_resource_requirements(
        config,
        workflow_id,
        "chain_jobs",
        1,
        0,
        1,
        "1g",
        "P0DT1H",
    );
    let resource_id = resource_req.id.unwrap();

    for chain in 0..num_chains {
        let mut prev_id: Option<i64> = None;
        for depth in 0..chain_depth {
            let mut job = models::JobModel::new(
                workflow_id,
                format!("job_c{chain}_d{depth}"),
                format!("echo 'chain {chain} depth {depth}'"),
            );
            job.resource_requirements_id = Some(resource_id);
            if let Some(prev) = prev_id {
                job.depends_on_job_ids = Some(vec![prev]);
            }
            let created =
                apis::jobs_api::create_job(config, job).expect("Failed to create chain job");
            prev_id = created.id;
        }
    }

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    // Completion validates the provided run_id against the workflow's current
    // run_id, so capture whatever the workflow reports rather than assuming 1.
    let run_id = apis::workflows_api::get_workflow(config, workflow_id)
        .expect("Failed to get workflow")
        .run_id
        .unwrap_or(0);

    (workflow_id, num_chains * chain_depth, run_id)
}

/// Stress the realistic steady state: multiple runners concurrently claim jobs
/// AND complete jobs against the same workflow, while completions cascade through
/// dependency chains via the background unblock task.
///
/// This is the scenario `test_prepare_next_jobs_concurrent_allocation` does not
/// cover (it only claims, never completes) and `test_batch_complete_jobs_*` does
/// not cover (single caller, no concurrency). It asserts the two safety
/// properties that must hold no matter how claims, completions, and unblocks
/// interleave:
///   1. No job is ever claimed by more than one runner (no double-allocation).
///   2. No job is ever completed more than once (no double-completion).
/// and the liveness property that the workflow fully drains: every job in every
/// chain ends `Completed`, proving the background unblock task promoted each
/// `Blocked` job exactly once under contention.
#[rstest]
fn test_concurrent_claim_and_complete(start_server: &ServerProcess) {
    let config = &start_server.config;

    let num_runners = std::cmp::max(
        4,
        thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4),
    );

    // 20 chains x 5 deep = 100 jobs. Only 20 heads are Ready initially; the rest
    // must be unblocked by the background task as predecessors complete.
    let num_chains = 20;
    let chain_depth = 5;
    let (workflow_id, total_jobs, run_id) =
        create_job_chains_workflow(config, num_chains, chain_depth);

    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Shared trackers. `claimed` maps job_id -> runner that claimed it (panics on
    // a second claim). `completed` records every job_id completed (panics on a
    // second completion). `completed_count` lets runners know when to stop.
    let claimed: Arc<Mutex<HashMap<i64, usize>>> = Arc::new(Mutex::new(HashMap::new()));
    let completed: Arc<Mutex<HashMap<i64, usize>>> = Arc::new(Mutex::new(HashMap::new()));
    let completed_count = Arc::new(AtomicUsize::new(0));

    let mut handles = vec![];
    for runner_id in 0..num_runners {
        let config = config.clone();
        let claimed = Arc::clone(&claimed);
        let completed = Arc::clone(&completed);
        let completed_count = Arc::clone(&completed_count);

        let handle = thread::spawn(move || {
            // Generous wall-clock guard so a real bug fails the test instead of
            // hanging CI. 100 jobs unblocking at ~0.1s granularity drains well
            // under this bound.
            let deadline = Instant::now() + Duration::from_secs(60);

            while completed_count.load(Ordering::SeqCst) < total_jobs {
                assert!(
                    Instant::now() < deadline,
                    "runner {runner_id} timed out: only {}/{total_jobs} jobs completed",
                    completed_count.load(Ordering::SeqCst)
                );

                let response = apis::workflows_api::claim_next_jobs(&config, workflow_id, Some(4))
                    .unwrap_or_else(|e| panic!("runner {runner_id} claim failed: {e}"));
                let jobs = response.jobs.unwrap_or_default();

                if jobs.is_empty() {
                    // Nothing claimable right now: downstream jobs may still be
                    // Blocked awaiting the background unblock task. Back off and
                    // retry until the workflow drains.
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }

                for job in jobs {
                    let job_id = job.id.expect("claimed job must have an id");

                    // Property 1: a job must never be claimed twice.
                    {
                        let mut map = claimed.lock().unwrap();
                        if let Some(other) = map.insert(job_id, runner_id) {
                            panic!(
                                "DOUBLE-ALLOCATION: job {job_id} claimed by runner {other} and runner {runner_id}"
                            );
                        }
                    }

                    let result = models::ResultModel::new(
                        job_id,
                        workflow_id,
                        run_id,
                        1, // attempt_id
                        compute_node_id,
                        0,   // return_code (success)
                        0.1, // exec_time_minutes
                        chrono::Utc::now().to_rfc3339(),
                        models::JobStatus::Completed,
                    );

                    apis::jobs_api::complete_job(
                        &config,
                        job_id,
                        models::JobStatus::Completed,
                        run_id,
                        result,
                    )
                    .unwrap_or_else(|e| {
                        panic!("runner {runner_id} failed to complete job {job_id}: {e}")
                    });

                    // Property 2: a job must never be completed twice.
                    {
                        let mut map = completed.lock().unwrap();
                        if let Some(other) = map.insert(job_id, runner_id) {
                            panic!(
                                "DOUBLE-COMPLETION: job {job_id} completed by runner {other} and runner {runner_id}"
                            );
                        }
                    }
                    completed_count.fetch_add(1, Ordering::SeqCst);
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("runner thread panicked");
    }

    // Every job was claimed exactly once and completed exactly once.
    let claimed = claimed.lock().unwrap();
    let completed = completed.lock().unwrap();
    assert_eq!(
        claimed.len(),
        total_jobs,
        "expected all {total_jobs} jobs claimed exactly once, got {}",
        claimed.len()
    );
    assert_eq!(
        completed.len(),
        total_jobs,
        "expected all {total_jobs} jobs completed exactly once, got {}",
        completed.len()
    );

    // Liveness: the workflow fully drained. Every job ends Completed, which can
    // only happen if the background unblock task promoted each Blocked job to
    // Ready exactly once despite concurrent claims and completions.
    let final_completed = apis::jobs_api::list_jobs(
        &config.clone(),
        workflow_id,
        Some(models::JobStatus::Completed),
        None,
        None,
        None,
        Some(total_jobs as i64),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("list_jobs failed");
    assert_eq!(
        final_completed.items.len(),
        total_jobs,
        "expected all {total_jobs} jobs to end Completed, found {}",
        final_completed.items.len()
    );
}

/// Two runners attempt to complete the SAME job. The normal claim-based flow
/// makes this unreachable — `claim_next_jobs` hands each job to exactly one
/// runner under a `BEGIN IMMEDIATE` write lock — so this test sets the scenario
/// up directly by completing one job twice.
///
/// The invariant under test: a job can be completed only once. The first
/// completion succeeds; the second is rejected (HTTP 422 "already complete")
/// rather than silently double-recording, and no duplicate result row is
/// written.
#[rstest]
fn test_two_runners_complete_same_job_rejected(start_server: &ServerProcess) {
    let config = &start_server.config;

    // A single Ready job (1 chain x 1 deep). The helper returns the workflow's
    // current run_id, which completion validates against.
    let (workflow_id, total_jobs, run_id) = create_job_chains_workflow(config, 1, 1);
    assert_eq!(total_jobs, 1);

    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    let job_id = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("list_jobs")
    .items
    .first()
    .and_then(|j| j.id)
    .expect("workflow should have exactly one job");

    // Each "runner" submits its own result for the same job.
    let make_result = || {
        models::ResultModel::new(
            job_id,
            workflow_id,
            run_id,
            1, // attempt_id
            compute_node_id,
            0,   // return_code (success)
            0.1, // exec_time_minutes
            chrono::Utc::now().to_rfc3339(),
            models::JobStatus::Completed,
        )
    };

    // Runner A completes the job successfully.
    let first = apis::jobs_api::complete_job(
        config,
        job_id,
        models::JobStatus::Completed,
        run_id,
        make_result(),
    )
    .expect("first completion should succeed");
    assert_eq!(first.status, Some(models::JobStatus::Completed));

    // Runner B tries to complete the same, already-completed job. It must be
    // rejected, not accepted as a second completion.
    let second = apis::jobs_api::complete_job(
        config,
        job_id,
        models::JobStatus::Completed,
        run_id,
        make_result(),
    );
    let err = second.expect_err("second completion of the same job must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("already complete"),
        "expected an 'already complete' rejection, got: {msg}"
    );

    // The rejected completion must not have recorded a second result row.
    let results = apis::results_api::list_results(
        config,
        workflow_id,
        Some(job_id),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("list_results");
    assert_eq!(
        results.count, 1,
        "exactly one result should be recorded for a job completed once, found {}",
        results.count
    );
}
