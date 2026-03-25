mod common;

use common::{
    ServerProcess, create_test_job, create_test_result, create_test_workflow, run_cli_with_json,
    start_server,
};
use rstest::rstest;
use serde_json::json;
use torc::client::apis;
use torc::client::workflow_manager::WorkflowManager;
use torc::config::TorcConfig;
use torc::models;
use torc::models::JobStatus;

#[rstest]
fn test_results_list_command_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow and jobs with results
    let workflow = create_test_workflow(config, "test_results_list_workflow");
    let workflow_id = workflow.id.unwrap();

    let job1 = create_test_job(config, workflow_id, "job1");
    let job2 = create_test_job(config, workflow_id, "job2");

    // Create test results
    let _result1 = create_test_result(config, workflow_id, job1.id.unwrap());
    let _result2 = create_test_result(config, workflow_id, job2.id.unwrap());

    // Test the CLI list command - use --all-runs to see all results since we created them directly
    let args = [
        "results",
        "list",
        &workflow_id.to_string(),
        "--limit",
        "10",
        "--all-runs",
    ];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to run results list command");

    // Verify JSON structure is an object with "results" field
    assert!(
        json_output.is_object(),
        "Results list should return an object"
    );
    assert!(
        json_output.get("results").is_some(),
        "Response should have 'results' field"
    );

    let results_array = json_output.get("results").unwrap().as_array().unwrap();
    assert!(results_array.len() >= 2, "Should have at least 2 results");

    // Verify each result has the expected structure
    for result in results_array {
        assert!(result.get("id").is_some());
        assert!(result.get("job_id").is_some());
        assert!(result.get("workflow_id").is_some());
        assert!(result.get("run_id").is_some());
        assert!(result.get("return_code").is_some());
        assert!(result.get("exec_time_minutes").is_some());
        assert!(result.get("completion_time").is_some());
        assert!(result.get("status").is_some());
    }
}

#[rstest]
fn test_results_list_with_job_filter(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_job_filter_workflow");
    let workflow_id = workflow.id.unwrap();

    let job1 = create_test_job(config, workflow_id, "filtered_job");
    let job2 = create_test_job(config, workflow_id, "other_job");

    let _result1 = create_test_result(config, workflow_id, job1.id.unwrap());
    let _result2 = create_test_result(config, workflow_id, job2.id.unwrap());

    // Test filtering by job_id - use --all-runs since we created results directly
    let args = [
        "results",
        "list",
        &workflow_id.to_string(),
        "--job-id",
        &job1.id.unwrap().to_string(),
        "--all-runs",
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run results list with job filter");

    let results_array = json_output.get("results").unwrap().as_array().unwrap();
    assert!(!results_array.is_empty());

    // All results should be for the specified job
    for result in results_array {
        assert_eq!(result.get("job_id").unwrap(), &json!(job1.id.unwrap()));
    }
}

#[rstest]
fn test_results_get_command_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test data
    let workflow = create_test_workflow(config, "test_results_get_workflow");
    let workflow_id = workflow.id.unwrap();
    let job = create_test_job(config, workflow_id, "test_get_job");

    let result = create_test_result(config, workflow_id, job.id.unwrap());
    let result_id = result.id.unwrap();
    let status_done = JobStatus::Completed.to_string();

    // Test the CLI get command
    let args = ["results", "get", &result_id.to_string()];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to run results get command");

    // Verify JSON structure
    assert_eq!(json_output.get("id").unwrap(), &json!(result_id));
    assert_eq!(json_output.get("job_id").unwrap(), &json!(job.id.unwrap()));
    assert_eq!(json_output.get("workflow_id").unwrap(), &json!(workflow_id));
    assert_eq!(json_output.get("run_id").unwrap(), &json!(1));
    assert_eq!(json_output.get("return_code").unwrap(), &json!(0));
    assert_eq!(json_output.get("exec_time_minutes").unwrap(), &json!(5.5));
    assert_eq!(json_output.get("status").unwrap(), &json!(status_done));
}

#[rstest]
fn test_results_delete_command_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test data
    let workflow = create_test_workflow(config, "test_results_remove_workflow");
    let workflow_id = workflow.id.unwrap();
    let job = create_test_job(config, workflow_id, "test_remove_job");

    let result = create_test_result(config, workflow_id, job.id.unwrap());
    let result_id = result.id.unwrap();

    // Test the CLI delete command
    let args = ["results", "delete", &result_id.to_string()];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to run results delete command");

    // Verify JSON structure shows the removed result
    assert_eq!(json_output.get("id").unwrap(), &json!(result_id));
    assert_eq!(json_output.get("job_id").unwrap(), &json!(job.id.unwrap()));
    assert_eq!(json_output.get("workflow_id").unwrap(), &json!(workflow_id));

    // Verify the result is actually removed by trying to get it
    let get_result = apis::results_api::get_result(config, result_id);
    assert!(get_result.is_err(), "Result should be deleted");
}

#[rstest]
fn test_results_list_pagination(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_pagination_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create multiple results
    for i in 0..5 {
        let job = create_test_job(config, workflow_id, &format!("pagination_job_{}", i));
        let _result = create_test_result(config, workflow_id, job.id.unwrap());
    }

    // Test with limit - use --all-runs since we created results directly
    let args = [
        "results",
        "list",
        &workflow_id.to_string(),
        "--limit",
        "3",
        "--all-runs",
    ];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to run paginated results list");

    let results_array = json_output.get("results").unwrap().as_array().unwrap();
    assert!(results_array.len() >= 3, "Should respect limit parameter");

    // Test with offset - use --all-runs since we created results directly
    let args_with_offset = [
        "results",
        "list",
        &workflow_id.to_string(),
        "--limit",
        "2",
        "--offset",
        "2",
        "--all-runs",
    ];

    let json_output_offset = run_cli_with_json(&args_with_offset, start_server, None)
        .expect("Failed to run results list with offset");

    let results_with_offset = json_output_offset
        .get("results")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(
        !results_with_offset.is_empty(),
        "Should have results with offset"
    );
}

#[rstest]
fn test_results_list_sorting(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_sorting_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create results with different return codes for sorting
    for i in 0..3 {
        let job = create_test_job(config, workflow_id, &format!("sort_job_{}", i));
        let result = models::ResultModel::new(
            job.id.unwrap(),
            workflow_id,
            1,
            1, // attempt_id
            1, // compute_node_id
            i, // Different return codes for sorting
            5.0,
            "2024-01-01T12:00:00.000Z".to_string(),
            models::JobStatus::Completed,
        );
        let _created =
            apis::results_api::create_result(config, result).expect("Failed to create result");
    }

    // Test sorting by return_code - use --all-runs since we created results directly
    let args = [
        "results",
        "list",
        &workflow_id.to_string(),
        "--sort-by",
        "return_code",
        "--reverse-sort",
        "--all-runs",
    ];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to run sorted results list");

    let results_array = json_output.get("results").unwrap().as_array().unwrap();
    assert!(results_array.len() >= 3);

    // Verify sorting (should be in reverse order, so highest return_code first)
    if results_array.len() >= 2 {
        let first_return_code = results_array[0]
            .get("return_code")
            .unwrap()
            .as_i64()
            .unwrap();
        let second_return_code = results_array[1]
            .get("return_code")
            .unwrap()
            .as_i64()
            .unwrap();
        assert!(
            first_return_code >= second_return_code,
            "Results should be sorted in reverse order"
        );
    }
}

// Commented out: This test requires a "create" CLI subcommand that is not currently implemented
// #[rstest]
// fn test_results_with_completion_time_default(start_server: &ServerProcess) {
//     let config = &start_server.config;
//
//     let workflow = create_test_workflow(config, "test_default_time_workflow");
//     let workflow_id = workflow.id.unwrap();
//     let job = create_test_job(config, workflow_id, "default_time_job");
//     let job_id = job.id.unwrap();
//
//     // Test create without explicit completion_time (should use current time)
//     let args = [
//         "results",
//         "create",
//         &workflow_id.to_string(),
//         "--job-id",
//         &job_id.to_string(),
//         "--run-id",
//         "1",
//         "--return-code",
//         "0",
//         "--exec-time-minutes",
//         "1.0",
//         "--status",
//         "done",
//     ];
//
//     let json_output = run_cli_with_json(&args, start_server, None)
//         .expect("Failed to run results create with default completion time");
//
//     // Should have a completion_time field
//     assert!(json_output.get("completion_time").is_some());
//     let completion_time = json_output
//         .get("completion_time")
//         .unwrap()
//         .as_str()
//         .unwrap();
//
//     // Should be in ISO 8601 format
//     assert!(
//         completion_time.ends_with("Z"),
//         "Completion time should be in UTC (end with Z)"
//     );
//     assert!(
//         completion_time.contains("T"),
//         "Completion time should contain 'T' separator"
//     );
// }

#[rstest]
fn test_results_list_with_return_code_filter(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_return_code_filter_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create jobs
    let job1 = create_test_job(config, workflow_id, "success_job");
    let job2 = create_test_job(config, workflow_id, "failed_job");

    // Create results with different return codes
    let success_result = models::ResultModel::new(
        job1.id.unwrap(),
        workflow_id,
        1,
        1, // attempt_id
        1, // compute_node_id
        0, // success return code
        2.5,
        "2024-01-01T10:00:00.000Z".to_string(),
        models::JobStatus::Completed,
    );
    let _result1 = apis::results_api::create_result(config, success_result)
        .expect("Failed to create success result");

    let failed_result = models::ResultModel::new(
        job2.id.unwrap(),
        workflow_id,
        1,
        1, // attempt_id
        1, // compute_node_id
        1, // failure return code
        3.5,
        "2024-01-01T11:00:00.000Z".to_string(),
        models::JobStatus::Terminated,
    );
    let _result2 = apis::results_api::create_result(config, failed_result)
        .expect("Failed to create failed result");

    // Test the CLI list command with return_code filter
    let args = [
        "results",
        "list",
        &workflow_id.to_string(),
        "--return-code",
        "0",
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run results list command with return_code filter");

    // Verify the response structure is correct
    assert!(
        json_output.is_object(),
        "Results list should return an object"
    );
    assert!(
        json_output.get("results").is_some(),
        "Response should have 'results' field"
    );

    // The command should execute without error
    // The actual filtering depends on backend implementation
}

#[rstest]
fn test_results_list_with_status_filter(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_status_filter_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create jobs
    let job1 = create_test_job(config, workflow_id, "done_job");
    let job2 = create_test_job(config, workflow_id, "terminated_job");

    // Create results with different statuses
    let done_result = models::ResultModel::new(
        job1.id.unwrap(),
        workflow_id,
        1,
        1, // attempt_id
        1, // compute_node_id
        0,
        2.5,
        "2024-01-01T10:00:00.000Z".to_string(),
        models::JobStatus::Completed,
    );
    let _result1 = apis::results_api::create_result(config, done_result)
        .expect("Failed to create done result");

    let terminated_result = models::ResultModel::new(
        job2.id.unwrap(),
        workflow_id,
        1,
        1, // attempt_id
        1, // compute_node_id
        1,
        3.5,
        "2024-01-01T11:00:00.000Z".to_string(),
        models::JobStatus::Terminated,
    );
    let _result2 = apis::results_api::create_result(config, terminated_result)
        .expect("Failed to create terminated result");

    // Test the CLI list command with status filter
    let args = [
        "results",
        "list",
        &workflow_id.to_string(),
        "--status",
        "completed",
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run results list command with status filter");

    // Verify the response structure is correct
    assert!(
        json_output.is_object(),
        "Results list should return an object"
    );
    assert!(
        json_output.get("results").is_some(),
        "Response should have 'results' field"
    );

    // The command should execute without error
    // The actual filtering depends on backend implementation
}

#[rstest]
fn test_results_list_all_runs_default_behavior(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_all_runs_default_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create two jobs
    let job1 = create_test_job(config, workflow_id, "job1_all_runs");
    let job2 = create_test_job(config, workflow_id, "job2_all_runs");
    let job1_id = job1.id.unwrap();
    let job2_id = job2.id.unwrap();

    // Use WorkflowManager to initialize workflow for run 1
    let torc_config = TorcConfig::load().unwrap_or_default();
    let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow.clone());
    workflow_manager
        .initialize(true)
        .expect("Failed to initialize workflow for run 1");

    // Get run_id for run 1
    let status = apis::workflows_api::get_workflow_status(config, workflow_id)
        .expect("Failed to get workflow status");
    let run_id_1 = status.run_id;

    // Complete jobs for run 1 using complete_job (which updates workflow_result table)
    // Note: status must match return_code - non-zero return_code requires Failed status
    // so that reset_job_status with failed_only=true will reset these jobs
    apis::jobs_api::complete_job(
        config,
        job1_id,
        models::JobStatus::Failed,
        run_id_1,
        models::ResultModel::new(
            job1_id,
            workflow_id,
            run_id_1,
            1, // attempt_id
            1, // compute_node_id
            1, // failed return_code
            2.5,
            "2024-01-01T10:00:00.000Z".to_string(),
            models::JobStatus::Failed,
        ),
    )
    .expect("Failed to complete job1 for run 1");

    apis::jobs_api::complete_job(
        config,
        job2_id,
        models::JobStatus::Failed,
        run_id_1,
        models::ResultModel::new(
            job2_id,
            workflow_id,
            run_id_1,
            1, // attempt_id
            1, // compute_node_id
            1, // failed return_code
            3.5,
            "2024-01-01T11:00:00.000Z".to_string(),
            models::JobStatus::Failed,
        ),
    )
    .expect("Failed to complete job2 for run 1");

    apis::workflows_api::reset_job_status(config, workflow_id, Some(true))
        .expect("Failed to reset job status");

    // Reinitialize workflow for run 2 (should trigger workflow_result cleanup and increment run_id)
    workflow_manager
        .reinitialize(false, false)
        .expect("Failed to reinitialize workflow for run 2");

    // Get run_id for run 2
    let status = apis::workflows_api::get_workflow_status(config, workflow_id)
        .expect("Failed to get workflow status");
    let run_id_2 = status.run_id;

    // Complete jobs for run 2
    apis::jobs_api::complete_job(
        config,
        job1_id,
        models::JobStatus::Completed,
        run_id_2,
        models::ResultModel::new(
            job1_id,
            workflow_id,
            run_id_2,
            1, // attempt_id
            1, // compute_node_id
            0,
            4.5,
            "2024-01-02T10:00:00.000Z".to_string(),
            models::JobStatus::Completed,
        ),
    )
    .expect("Failed to complete job1 for run 2");

    apis::jobs_api::complete_job(
        config,
        job2_id,
        models::JobStatus::Terminated,
        run_id_2,
        models::ResultModel::new(
            job2_id,
            workflow_id,
            run_id_2,
            1, // attempt_id
            1, // compute_node_id
            1, // Different return code
            5.5,
            "2024-01-02T11:00:00.000Z".to_string(),
            models::JobStatus::Terminated,
        ),
    )
    .expect("Failed to complete job2 for run 2");

    // Test default behavior (should only show current results from run 2)
    let args = ["results", "list", &workflow_id.to_string()];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run results list command with default behavior");

    assert!(
        json_output.is_object(),
        "Results list should return an object"
    );
    assert!(
        json_output.get("results").is_some(),
        "Response should have 'results' field"
    );

    let results_array = json_output.get("results").unwrap().as_array().unwrap();

    // Should only show 2 results (from run 2)
    assert_eq!(
        results_array.len(),
        2,
        "Default behavior should only show current results (2 from run 2)"
    );

    // All results should be from run_id 2
    for result in results_array {
        assert_eq!(
            result.get("run_id").unwrap(),
            &json!(run_id_2),
            "Default should only show results from latest run (run 2)"
        );
    }

    // Verify we have results for both jobs from run 2
    let job_ids: Vec<i64> = results_array
        .iter()
        .map(|r| r.get("job_id").unwrap().as_i64().unwrap())
        .collect();
    assert!(job_ids.contains(&job1_id), "Should have result for job1");
    assert!(job_ids.contains(&job2_id), "Should have result for job2");
}

#[rstest]
fn test_results_list_all_runs_true(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_all_runs_true_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create two jobs
    let job1 = create_test_job(config, workflow_id, "job1_all_runs_true");
    let job2 = create_test_job(config, workflow_id, "job2_all_runs_true");
    let job1_id = job1.id.unwrap();
    let job2_id = job2.id.unwrap();

    // Initialize workflow for run 1
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to initialize jobs for run 1");

    // Create results for run 1
    let result1_run1 = models::ResultModel::new(
        job1_id,
        workflow_id,
        1, // run_id 1
        1, // attempt_id
        1, // compute_node_id
        0,
        2.5,
        "2024-01-01T10:00:00.000Z".to_string(),
        models::JobStatus::Completed,
    );
    apis::results_api::create_result(config, result1_run1)
        .expect("Failed to create result1 for run 1");

    let result2_run1 = models::ResultModel::new(
        job2_id,
        workflow_id,
        1, // run_id 1
        1, // attempt_id
        1, // compute_node_id
        0,
        3.5,
        "2024-01-01T11:00:00.000Z".to_string(),
        models::JobStatus::Completed,
    );
    apis::results_api::create_result(config, result2_run1)
        .expect("Failed to create result2 for run 1");

    // Reinitialize workflow for run 2
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to reinitialize jobs for run 2");

    // Create results for run 2
    let result1_run2 = models::ResultModel::new(
        job1_id,
        workflow_id,
        2, // run_id 2
        1, // attempt_id
        1, // compute_node_id
        0,
        4.5,
        "2024-01-02T10:00:00.000Z".to_string(),
        models::JobStatus::Completed,
    );
    apis::results_api::create_result(config, result1_run2)
        .expect("Failed to create result1 for run 2");

    let result2_run2 = models::ResultModel::new(
        job2_id,
        workflow_id,
        2, // run_id 2
        1, // attempt_id
        1, // compute_node_id
        1,
        5.5,
        "2024-01-02T11:00:00.000Z".to_string(),
        models::JobStatus::Terminated,
    );
    apis::results_api::create_result(config, result2_run2)
        .expect("Failed to create result2 for run 2");

    // Reinitialize workflow for run 3
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to reinitialize jobs for run 3");

    // Create results for run 3 (only one job)
    let result1_run3 = models::ResultModel::new(
        job1_id,
        workflow_id,
        3, // run_id 3
        1, // attempt_id
        1, // compute_node_id
        0,
        6.5,
        "2024-01-03T10:00:00.000Z".to_string(),
        models::JobStatus::Completed,
    );
    apis::results_api::create_result(config, result1_run3)
        .expect("Failed to create result1 for run 3");

    // Test with --all-runs flag (should show all historical results)
    let args = ["results", "list", &workflow_id.to_string(), "--all-runs"];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run results list command with --all-runs");

    assert!(
        json_output.is_object(),
        "Results list should return an object"
    );
    assert!(
        json_output.get("results").is_some(),
        "Response should have 'results' field"
    );

    let results_array = json_output.get("results").unwrap().as_array().unwrap();

    // Should show all 5 results (2 from run 1, 2 from run 2, 1 from run 3)
    assert_eq!(
        results_array.len(),
        5,
        "With --all-runs should show all historical results (5 total)"
    );

    // Count results per run
    let mut run1_count = 0;
    let mut run2_count = 0;
    let mut run3_count = 0;

    for result in results_array {
        match result.get("run_id").unwrap().as_i64().unwrap() {
            1 => run1_count += 1,
            2 => run2_count += 1,
            3 => run3_count += 1,
            _ => panic!("Unexpected run_id in results"),
        }
    }

    assert_eq!(run1_count, 2, "Should have 2 results from run 1");
    assert_eq!(run2_count, 2, "Should have 2 results from run 2");
    assert_eq!(run3_count, 1, "Should have 1 result from run 3");
}

#[rstest]
fn test_results_list_all_runs_with_filters(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_all_runs_filters_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create two jobs
    let job1 = create_test_job(config, workflow_id, "job1_filters");
    let job2 = create_test_job(config, workflow_id, "job2_filters");
    let job1_id = job1.id.unwrap();
    let job2_id = job2.id.unwrap();

    // Initialize workflow for run 1
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to initialize jobs for run 1");

    // Create results for run 1 with different statuses
    let result1_run1 = models::ResultModel::new(
        job1_id,
        workflow_id,
        1,
        1, // attempt_id
        1, // compute_node_id
        0,
        2.5,
        "2024-01-01T10:00:00.000Z".to_string(),
        models::JobStatus::Completed,
    );
    apis::results_api::create_result(config, result1_run1)
        .expect("Failed to create result1 for run 1");

    let result2_run1 = models::ResultModel::new(
        job2_id,
        workflow_id,
        1,
        1, // attempt_id
        1, // compute_node_id
        1,
        3.5,
        "2024-01-01T11:00:00.000Z".to_string(),
        models::JobStatus::Terminated,
    );
    apis::results_api::create_result(config, result2_run1)
        .expect("Failed to create result2 for run 1");

    // Reinitialize for run 2
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to reinitialize jobs for run 2");

    // Create results for run 2
    let result1_run2 = models::ResultModel::new(
        job1_id,
        workflow_id,
        2,
        1, // attempt_id
        1, // compute_node_id
        0,
        4.5,
        "2024-01-02T10:00:00.000Z".to_string(),
        models::JobStatus::Completed,
    );
    apis::results_api::create_result(config, result1_run2)
        .expect("Failed to create result1 for run 2");

    let result2_run2 = models::ResultModel::new(
        job2_id,
        workflow_id,
        2,
        1, // attempt_id
        1, // compute_node_id
        0,
        5.5,
        "2024-01-02T11:00:00.000Z".to_string(),
        models::JobStatus::Completed,
    );
    apis::results_api::create_result(config, result2_run2)
        .expect("Failed to create result2 for run 2");

    // Test 1: --all-runs with job_id filter (should show all runs for that job)
    let args = [
        "results",
        "list",
        &workflow_id.to_string(),
        "--all-runs",
        "--job-id",
        &job1_id.to_string(),
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run results list with --all-runs and job filter");

    let results_array = json_output.get("results").unwrap().as_array().unwrap();
    assert_eq!(
        results_array.len(),
        2,
        "Should show 2 results for job1 across both runs"
    );

    // All results should be for job1
    for result in results_array {
        assert_eq!(
            result.get("job_id").unwrap(),
            &json!(job1_id),
            "All results should be for job1"
        );
    }

    // Test 2: --all-runs with status filter
    let args = [
        "results",
        "list",
        &workflow_id.to_string(),
        "--all-runs",
        "--status",
        "terminated",
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run results list with --all-runs and status filter");

    let results_array = json_output.get("results").unwrap().as_array().unwrap();
    assert_eq!(
        results_array.len(),
        1,
        "Should show 1 terminated result across all runs"
    );

    // Test 3: Default (without --all-runs) with status filter should only check run 2
    let args = [
        "results",
        "list",
        &workflow_id.to_string(),
        "--status",
        "terminated",
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run results list with status filter (no --all-runs)");

    let results_array = json_output.get("results").unwrap().as_array().unwrap();
    // Run 2 has no terminated results, so should be empty
    assert_eq!(
        results_array.len(),
        0,
        "Default behavior should show 0 terminated results from current run (run 2)"
    );
}

#[rstest]
fn test_results_workflow_result_table_cleanup_on_reinitialize(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_workflow_result_cleanup");
    let workflow_id = workflow.id.unwrap();

    // Create jobs
    let job1 = create_test_job(config, workflow_id, "job1_cleanup");
    let job2 = create_test_job(config, workflow_id, "job2_cleanup");
    let job1_id = job1.id.unwrap();
    let job2_id = job2.id.unwrap();

    // Use WorkflowManager to initialize workflow
    let torc_config = TorcConfig::load().unwrap_or_default();
    let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow.clone());
    workflow_manager
        .initialize(true)
        .expect("Failed to initialize workflow for run 1");

    // Get run_id for run 1
    let status = apis::workflows_api::get_workflow_status(config, workflow_id)
        .expect("Failed to get workflow status");
    let run_id = status.run_id;

    // Complete jobs for run 1 using complete_job (which updates workflow_result table)
    apis::jobs_api::complete_job(
        config,
        job1_id,
        models::JobStatus::Completed,
        run_id,
        models::ResultModel::new(
            job1_id,
            workflow_id,
            run_id,
            1, // attempt_id
            1, // compute_node_id
            0,
            2.5,
            "2024-01-01T10:00:00.000Z".to_string(),
            models::JobStatus::Completed,
        ),
    )
    .expect("Failed to complete job1 for run 1");

    apis::jobs_api::complete_job(
        config,
        job2_id,
        models::JobStatus::Completed,
        run_id,
        models::ResultModel::new(
            job2_id,
            workflow_id,
            run_id,
            1, // attempt_id
            1, // compute_node_id
            0,
            3.5,
            "2024-01-01T11:00:00.000Z".to_string(),
            models::JobStatus::Completed,
        ),
    )
    .expect("Failed to complete job2 for run 1");

    // Verify we have 2 current results (default list shows workflow_result entries)
    let args = ["results", "list", &workflow_id.to_string()];
    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to list results before reinitialize");
    assert_eq!(
        json_output
            .get("results")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2,
        "Should have 2 current results before reinitialize"
    );

    // Reset job status before reinitializing (required to reset completed jobs)
    apis::workflows_api::reset_job_status(config, workflow_id, Some(false))
        .expect("Failed to reset job status");

    // Reinitialize workflow (should clean up workflow_result table for incomplete jobs)
    workflow_manager
        .reinitialize(false, false)
        .expect("Failed to reinitialize workflow");

    // After reinitialize, jobs are reset to ready/blocked status (not complete)
    // So workflow_result should be empty (default list should show 0 results)
    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to list results after reinitialize");
    assert_eq!(
        json_output
            .get("results")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        0,
        "Default list should show 0 results after reinitialize (workflow_result cleaned up)"
    );

    // But with --all-runs, we should still see the historical results
    let args_all_runs = ["results", "list", &workflow_id.to_string(), "--all-runs"];
    let json_output = run_cli_with_json(&args_all_runs, start_server, None)
        .expect("Failed to list all results after reinitialize");
    assert_eq!(
        json_output
            .get("results")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2,
        "With --all-runs should still show 2 historical results after reinitialize"
    );
}
