mod common;

use common::{
    ServerProcess, create_test_compute_node, create_test_file, create_test_workflow, start_server,
};
use rstest::rstest;
use torc::client::{apis, config::TorcConfig, workflow_manager::WorkflowManager};
use torc::models;
use torc::models::JobStatus;

/// Test that the server reports file IDs that should have been created by the user but do not exist.
/// These are file IDs that have been added to the workflow, are needed by at least one job,
/// and are not produced by a job.
#[rstest]
fn test_list_required_existing_files_missing_user_files(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow
    let workflow = create_test_workflow(config, "test_missing_user_files");
    let workflow_id = workflow.id.unwrap();

    // Create files that are needed by jobs but not produced by any job (user-provided files)
    let user_file1 = create_test_file(config, workflow_id, "input1", "/data/input1.txt");
    let user_file2 = create_test_file(config, workflow_id, "input2", "/data/input2.txt");
    let user_file3 = create_test_file(config, workflow_id, "input3", "/data/input3.txt");

    // Create an output file that will be produced by a job
    let output_file = create_test_file(config, workflow_id, "output", "/data/output.txt");

    // Create a job that needs the user files as input and produces an output
    let mut job = models::JobModel::new(
        workflow_id,
        "test_job".to_string(),
        "cat /data/input1.txt /data/input2.txt > /data/output.txt".to_string(),
    );
    job.input_file_ids = Some(vec![
        user_file1.id.unwrap(),
        user_file2.id.unwrap(),
        user_file3.id.unwrap(),
    ]);
    job.output_file_ids = Some(vec![output_file.id.unwrap()]);

    let _created_job = apis::jobs_api::create_job(config, job).expect("Failed to create job");

    // Initialize the workflow to set up job dependencies
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    // Call list_required_existing_files - should report all user files as missing
    let response = apis::workflows_api::list_required_existing_files(config, workflow_id)
        .expect("Failed to list required existing files");

    // Verify that the response contains the IDs of the user files that don't exist
    let missing_file_ids = response.files;

    // All user input files should be reported as missing
    assert!(missing_file_ids.contains(&user_file1.id.unwrap()));
    assert!(missing_file_ids.contains(&user_file2.id.unwrap()));
    assert!(missing_file_ids.contains(&user_file3.id.unwrap()));

    // The output file should not be in the list since it's produced by a job, not user-provided
    assert!(!missing_file_ids.contains(&output_file.id.unwrap()));

    assert_eq!(missing_file_ids.len(), 3);
}

/// Test that the server reports file record IDs that should have been created by a job but do not exist.
/// These are file IDs that are produced by a job, the job has completed (JobStatus::Completed),
/// but the file IDs are not present in the list_files response.
#[rstest]
fn test_list_required_existing_files_missing_job_outputs(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow
    let workflow = create_test_workflow(config, "test_missing_job_outputs");
    let workflow_id = workflow.id.unwrap();

    // Create input files (these exist as user-provided)
    let input_file = create_test_file(config, workflow_id, "input", "/data/input.txt");

    // Create output files that should be produced by jobs
    let output_file1 = create_test_file(config, workflow_id, "output1", "/data/output1.txt");
    let output_file2 = create_test_file(config, workflow_id, "output2", "/data/output2.txt");
    let output_file3 = create_test_file(config, workflow_id, "output3", "/data/output3.txt");

    // Create first job that produces output_file1 and output_file2
    let mut job1 = models::JobModel::new(
        workflow_id,
        "producer_job1".to_string(),
        "echo 'data1' > /data/output1.txt && echo 'data2' > /data/output2.txt".to_string(),
    );
    job1.input_file_ids = Some(vec![input_file.id.unwrap()]);
    job1.output_file_ids = Some(vec![output_file1.id.unwrap(), output_file2.id.unwrap()]);

    let created_job1 = apis::jobs_api::create_job(config, job1).expect("Failed to create job1");
    let job1_id = created_job1.id.unwrap();

    // Create second job that produces output_file3
    let mut job2 = models::JobModel::new(
        workflow_id,
        "producer_job2".to_string(),
        "echo 'data3' > /data/output3.txt".to_string(),
    );
    job2.input_file_ids = Some(vec![input_file.id.unwrap()]);
    job2.output_file_ids = Some(vec![output_file3.id.unwrap()]);

    let created_job2 = apis::jobs_api::create_job(config, job2).expect("Failed to create job2");
    let job2_id = created_job2.id.unwrap();

    let torc_config = TorcConfig::default();
    let manager = WorkflowManager::new(config.clone(), torc_config, workflow);
    manager.initialize(true).expect("Failed to start workflow");

    // Create a compute node for the results
    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Mark job1 as completed (Done status) - this should have produced output_file1 and output_file2
    let job1_result = models::ResultModel::new(
        job1_id,
        workflow_id,
        1, // run_id
        1, // attempt_id
        compute_node_id,
        0,   // return_code (success)
        1.0, // exec_time_minutes
        chrono::Utc::now().to_rfc3339(),
        JobStatus::Completed,
    );

    apis::jobs_api::complete_job(
        config,
        job1_id,
        job1_result.status,
        1, // run_id
        job1_result,
    )
    .expect("Failed to complete job1");

    // Mark job2 as completed (Done status) - this should have produced output_file3
    let job2_result = models::ResultModel::new(
        job2_id,
        workflow_id,
        1, // run_id
        1, // attempt_id
        compute_node_id,
        0,   // return_code (success)
        1.0, // exec_time_minutes
        chrono::Utc::now().to_rfc3339(),
        JobStatus::Completed,
    );

    apis::jobs_api::complete_job(
        config,
        job2_id,
        job2_result.status,
        1, // run_id
        job2_result,
    )
    .expect("Failed to complete job2");

    // Verify that both jobs are marked as Done
    let job1_status = apis::jobs_api::get_job(config, job1_id).expect("Failed to get job1");
    let job2_status = apis::jobs_api::get_job(config, job2_id).expect("Failed to get job2");
    assert_eq!(job1_status.status.unwrap(), JobStatus::Completed);
    assert_eq!(job2_status.status.unwrap(), JobStatus::Completed);

    // Get the list of files that actually exist in the system
    let files_response = apis::files_api::list_files(
        config,
        workflow_id,
        None, // produced_by_job_id
        None, // offset
        None, // limit
        None, // sort_by
        None, // reverse_sort
        None, // name
        None, // path
        None, // is_output
    )
    .expect("Failed to list files");

    let _existing_file_ids: Vec<i64> = files_response.items.iter().filter_map(|f| f.id).collect();

    // For this test, we simulate that the job completed successfully but the output files
    // were not actually created (perhaps due to a job implementation bug or file system issue).
    // Since this is a test environment, the files wouldn't actually be created by the job execution,
    // so they should be reported as missing.

    // Call list_required_existing_files
    let response = apis::workflows_api::list_required_existing_files(config, workflow_id)
        .expect("Failed to list required existing files");

    let missing_file_ids = response.files;

    // The output files should be reported as missing since they were supposed to be produced
    // by completed jobs but don't actually exist
    assert!(missing_file_ids.contains(&output_file1.id.unwrap()));
    assert!(missing_file_ids.contains(&output_file2.id.unwrap()));
    assert!(missing_file_ids.contains(&output_file3.id.unwrap()));

    // The input file should not be in the missing list if it was user-provided and exists
    // (though in this test environment, it might also be missing since files aren't actually created)

    // Verify that at least our expected output files are in the missing list
    let expected_missing: Vec<i64> = vec![
        output_file1.id.unwrap(),
        output_file2.id.unwrap(),
        output_file3.id.unwrap(),
    ];

    for expected_id in expected_missing {
        assert!(
            missing_file_ids.contains(&expected_id),
            "Expected file ID {} to be in missing files list, but it wasn't. Missing IDs: {:?}",
            expected_id,
            missing_file_ids
        );
    }
}

/// Test the combined scenario where both user files and job output files are missing
#[rstest]
fn test_list_required_existing_files_combined_scenario(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow
    let workflow = create_test_workflow(config, "test_combined_missing_files");
    let workflow_id = workflow.id.unwrap();

    // Create user input files (should be provided by user but don't exist)
    let user_input1 = create_test_file(config, workflow_id, "user_input1", "/data/user_input1.txt");
    let user_input2 = create_test_file(config, workflow_id, "user_input2", "/data/user_input2.txt");

    // Create intermediate files (produced by job1, consumed by job2)
    let intermediate1 = create_test_file(
        config,
        workflow_id,
        "intermediate1",
        "/data/intermediate1.txt",
    );
    let intermediate2 = create_test_file(
        config,
        workflow_id,
        "intermediate2",
        "/data/intermediate2.txt",
    );

    // Create final output files
    let final_output = create_test_file(
        config,
        workflow_id,
        "final_output",
        "/data/final_output.txt",
    );

    // Create job1 that takes user inputs and produces intermediate files
    let mut job1 = models::JobModel::new(
        workflow_id,
        "stage1_job".to_string(),
        "process inputs".to_string(),
    );
    job1.input_file_ids = Some(vec![user_input1.id.unwrap(), user_input2.id.unwrap()]);
    job1.output_file_ids = Some(vec![intermediate1.id.unwrap(), intermediate2.id.unwrap()]);

    let created_job1 = apis::jobs_api::create_job(config, job1).expect("Failed to create job1");
    let job1_id = created_job1.id.unwrap();

    // Create job2 that takes intermediate files and produces final output
    let mut job2 = models::JobModel::new(
        workflow_id,
        "stage2_job".to_string(),
        "process intermediate".to_string(),
    );
    job2.input_file_ids = Some(vec![intermediate1.id.unwrap(), intermediate2.id.unwrap()]);
    job2.output_file_ids = Some(vec![final_output.id.unwrap()]);

    let created_job2 = apis::jobs_api::create_job(config, job2).expect("Failed to create job2");
    let job2_id = created_job2.id.unwrap();

    let torc_config = TorcConfig::default();
    let manager = WorkflowManager::new(config.clone(), torc_config, workflow);
    manager.initialize(true).expect("Failed to start workflow");

    // Create a compute node for the results
    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Complete job1 (it should have produced intermediate files)
    let job1_result = models::ResultModel::new(
        job1_id,
        workflow_id,
        1,
        1, // attempt_id
        compute_node_id,
        0,
        1.0,
        chrono::Utc::now().to_rfc3339(),
        JobStatus::Completed,
    );

    apis::jobs_api::complete_job(config, job1_id, job1_result.status, 1, job1_result)
        .expect("Failed to complete job1");

    // Complete job2 (it should have produced final output)
    let job2_result = models::ResultModel::new(
        job2_id,
        workflow_id,
        1,
        1, // attempt_id
        compute_node_id,
        0,
        1.0,
        chrono::Utc::now().to_rfc3339(),
        JobStatus::Completed,
    );

    apis::jobs_api::complete_job(config, job2_id, job2_result.status, 1, job2_result)
        .expect("Failed to complete job2");

    // Call list_required_existing_files
    let response = apis::workflows_api::list_required_existing_files(config, workflow_id)
        .expect("Failed to list required existing files");

    let missing_file_ids = response.files;

    // Should report missing user input files (not produced by any job)
    assert!(missing_file_ids.contains(&user_input1.id.unwrap()));
    assert!(missing_file_ids.contains(&user_input2.id.unwrap()));

    // Should report missing intermediate files (produced by completed job1)
    assert!(missing_file_ids.contains(&intermediate1.id.unwrap()));
    assert!(missing_file_ids.contains(&intermediate2.id.unwrap()));

    // Should report missing final output file (produced by completed job2)
    assert!(missing_file_ids.contains(&final_output.id.unwrap()));

    // Verify we have all expected missing files
    assert_eq!(missing_file_ids.len(), 5);
}
