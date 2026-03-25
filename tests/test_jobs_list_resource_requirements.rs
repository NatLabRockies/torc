mod common;

use common::{ServerProcess, create_test_workflow, run_cli_with_json, start_server};
use rstest::rstest;
use serde_json::json;
use torc::client::apis;
use torc::models;

#[rstest]
fn test_list_resource_requirements_for_workflow(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_list_rr_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create resource requirements
    let mut rr1 = models::ResourceRequirementsModel::new(workflow_id, "rr1".to_string());
    rr1.num_cpus = 4;
    rr1.num_gpus = 1;
    rr1.num_nodes = 1;
    rr1.memory = "8g".to_string();
    rr1.runtime = "PT1H".to_string();
    let rr1 = apis::resource_requirements_api::create_resource_requirements(config, rr1)
        .expect("Failed to create resource requirements 1");
    let rr1_id = rr1.id.unwrap();

    let mut rr2 = models::ResourceRequirementsModel::new(workflow_id, "rr2".to_string());
    rr2.num_cpus = 8;
    rr2.num_gpus = 2;
    rr2.num_nodes = 2;
    rr2.memory = "16g".to_string();
    rr2.runtime = "PT2H".to_string();
    let rr2 = apis::resource_requirements_api::create_resource_requirements(config, rr2)
        .expect("Failed to create resource requirements 2");
    let rr2_id = rr2.id.unwrap();

    // Create jobs with resource requirements
    let mut job1 = models::JobModel::new(workflow_id, "job1".to_string(), "echo job1".to_string());
    job1.resource_requirements_id = Some(rr1_id);
    let job1 = apis::jobs_api::create_job(config, job1).expect("Failed to create job1");
    let job1_id = job1.id.unwrap();

    let mut job2 = models::JobModel::new(workflow_id, "job2".to_string(), "echo job2".to_string());
    job2.resource_requirements_id = Some(rr2_id);
    let job2 = apis::jobs_api::create_job(config, job2).expect("Failed to create job2");
    let job2_id = job2.id.unwrap();

    // Create a job without resource requirements
    let job3 = models::JobModel::new(workflow_id, "job3".to_string(), "echo job3".to_string());
    apis::jobs_api::create_job(config, job3).expect("Failed to create job3");

    // Test the CLI command with JSON output
    let args = [
        "jobs",
        "list-resource-requirements",
        &workflow_id.to_string(),
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run list-resource-requirements command");

    // Should be an array
    let jobs_array = json_output.as_array().expect("Expected JSON array");

    // Should have 2 jobs (job3 has the default resource requirements)
    assert_eq!(jobs_array.len(), 3);

    // Find job1 and job2 in the output
    let job1_output = jobs_array
        .iter()
        .find(|j| j.get("job_id").unwrap() == &json!(job1_id))
        .expect("job1 not found in output");

    let job2_output = jobs_array
        .iter()
        .find(|j| j.get("job_id").unwrap() == &json!(job2_id))
        .expect("job2 not found in output");

    // Verify job1 resource requirements
    assert_eq!(job1_output.get("job_name").unwrap(), &json!("job1"));
    assert_eq!(job1_output.get("rr_name").unwrap(), &json!("rr1"));
    assert_eq!(job1_output.get("num_cpus").unwrap(), &json!(4));
    assert_eq!(job1_output.get("num_gpus").unwrap(), &json!(1));
    assert_eq!(job1_output.get("num_nodes").unwrap(), &json!(1));
    assert_eq!(job1_output.get("memory").unwrap(), &json!("8g"));
    assert_eq!(job1_output.get("runtime").unwrap(), &json!("PT1H"));
    assert_eq!(job1_output.get("workflow_id").unwrap(), &json!(workflow_id));

    // Verify job2 resource requirements
    assert_eq!(job2_output.get("job_name").unwrap(), &json!("job2"));
    assert_eq!(job2_output.get("rr_name").unwrap(), &json!("rr2"));
    assert_eq!(job2_output.get("num_cpus").unwrap(), &json!(8));
    assert_eq!(job2_output.get("num_gpus").unwrap(), &json!(2));
    assert_eq!(job2_output.get("num_nodes").unwrap(), &json!(2));
    assert_eq!(job2_output.get("memory").unwrap(), &json!("16g"));
    assert_eq!(job2_output.get("runtime").unwrap(), &json!("PT2H"));
    assert_eq!(job2_output.get("workflow_id").unwrap(), &json!(workflow_id));

    // Verify that the 'id' field from resource requirements is not included
    assert!(job1_output.get("id").is_none());
    assert!(job2_output.get("id").is_none());
}

#[rstest]
fn test_list_resource_requirements_for_specific_job(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_list_rr_single_job");
    let workflow_id = workflow.id.unwrap();

    // Create resource requirements
    let mut rr = models::ResourceRequirementsModel::new(workflow_id, "rr_test".to_string());
    rr.num_cpus = 2;
    rr.num_gpus = 0;
    rr.num_nodes = 1;
    rr.memory = "4g".to_string();
    rr.runtime = "PT30M".to_string();
    let rr = apis::resource_requirements_api::create_resource_requirements(config, rr)
        .expect("Failed to create resource requirements");
    let rr_id = rr.id.unwrap();

    // Create job with resource requirements
    let mut job =
        models::JobModel::new(workflow_id, "test_job".to_string(), "echo test".to_string());
    job.resource_requirements_id = Some(rr_id);
    let job = apis::jobs_api::create_job(config, job).expect("Failed to create job");
    let job_id = job.id.unwrap();

    // Create another job that we won't query
    let mut other_job = models::JobModel::new(
        workflow_id,
        "other_job".to_string(),
        "echo other".to_string(),
    );
    other_job.resource_requirements_id = Some(rr_id);
    apis::jobs_api::create_job(config, other_job).expect("Failed to create other job");

    // Test the CLI command with -j flag and JSON output
    let args = [
        "jobs",
        "list-resource-requirements",
        "-j",
        &job_id.to_string(),
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run list-resource-requirements command with -j");

    // Should be an array with exactly 1 job
    let jobs_array = json_output.as_array().expect("Expected JSON array");
    assert_eq!(jobs_array.len(), 1);

    let job_output = &jobs_array[0];

    // Verify it's the correct job
    assert_eq!(job_output.get("job_id").unwrap(), &json!(job_id));
    assert_eq!(job_output.get("job_name").unwrap(), &json!("test_job"));
    assert_eq!(job_output.get("rr_name").unwrap(), &json!("rr_test"));
    assert_eq!(job_output.get("num_cpus").unwrap(), &json!(2));
    assert_eq!(job_output.get("num_gpus").unwrap(), &json!(0));
    assert_eq!(job_output.get("num_nodes").unwrap(), &json!(1));
    assert_eq!(job_output.get("memory").unwrap(), &json!("4g"));
    assert_eq!(job_output.get("runtime").unwrap(), &json!("PT30M"));
}
