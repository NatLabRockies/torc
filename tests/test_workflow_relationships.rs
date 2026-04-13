mod common;

use common::{
    ServerProcess, create_test_file, create_test_job, create_test_user_data, create_test_workflow,
    start_server,
};
use rstest::rstest;
use serde_json::json;
use torc::client::apis;
use torc::models;

#[rstest]
fn test_list_job_file_relationships_empty(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow with no jobs or files
    let workflow = create_test_workflow(config, "test_empty_file_rels_workflow");
    let workflow_id = workflow.id.unwrap();

    // Test list_job_file_relationships on empty workflow
    let result = apis::workflows_api::list_job_file_relationships(
        config,
        workflow_id,
        Some(0),   // offset
        Some(100), // limit
        None,
        None,
    )
    .expect("Failed to list job-file relationships");

    // Verify response structure
    assert_eq!(result.total_count, 0, "Should have no relationships");
    assert_eq!(result.count, 0, "Count should be 0");
    assert!(result.items.is_empty(), "Items array should be empty");
    assert_eq!(result.items.len(), 0, "Items array should be empty");
}

#[rstest]
fn test_list_job_file_relationships_workflow_inputs(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_file_inputs_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create workflow input files (no producer)
    let input_file1 = create_test_file(config, workflow_id, "input1.txt", "/path/to/input1.txt");
    let input_file2 = create_test_file(config, workflow_id, "input2.csv", "/path/to/input2.csv");

    // Create a job that consumes these files
    let mut job = models::JobModel::new(
        workflow_id,
        "consumer_job".to_string(),
        "cat input1.txt input2.csv".to_string(),
    );
    job.input_file_ids = Some(vec![input_file1.id.unwrap(), input_file2.id.unwrap()]);
    let job = apis::jobs_api::create_job(config, job).expect("Failed to create job");
    let job_id = job.id.unwrap();

    // List the relationships
    let result = apis::workflows_api::list_job_file_relationships(
        config,
        workflow_id,
        Some(0),
        Some(100),
        None,
        None,
    )
    .expect("Failed to list job-file relationships");

    // Verify we have 2 relationships (one for each file)
    assert_eq!(result.total_count, 2, "Should have 2 relationships");
    let items = result.items;
    assert_eq!(items.len(), 2, "Should have 2 items");

    // Verify each relationship
    for item in &items {
        // These are workflow inputs, so producer should be None
        assert!(
            item.producer_job_id.is_none(),
            "Workflow input should have no producer"
        );
        assert!(
            item.producer_job_name.is_none(),
            "Workflow input should have no producer name"
        );

        // Consumer should be the job we created
        assert_eq!(
            item.consumer_job_id,
            Some(job_id),
            "Consumer should be our job"
        );
        assert_eq!(
            item.consumer_job_name.as_ref().unwrap(),
            "consumer_job",
            "Consumer name should match"
        );

        // Verify workflow_id
        assert_eq!(item.workflow_id, workflow_id, "Workflow ID should match");
    }

    // Verify file names are present
    let file_names: Vec<&str> = items.iter().map(|item| item.file_name.as_str()).collect();
    assert!(
        file_names.contains(&"input1.txt"),
        "Should include input1.txt"
    );
    assert!(
        file_names.contains(&"input2.csv"),
        "Should include input2.csv"
    );
}

#[rstest]
fn test_list_job_file_relationships_workflow_outputs(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_file_outputs_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create workflow output files (no consumer)
    let output_file1 = create_test_file(config, workflow_id, "output1.txt", "/path/to/output1.txt");
    let output_file2 =
        create_test_file(config, workflow_id, "output2.json", "/path/to/output2.json");

    // Create a job that produces these files
    let mut job = models::JobModel::new(
        workflow_id,
        "producer_job".to_string(),
        "echo 'data' > output1.txt && echo '{}' > output2.json".to_string(),
    );
    job.output_file_ids = Some(vec![output_file1.id.unwrap(), output_file2.id.unwrap()]);
    let job = apis::jobs_api::create_job(config, job).expect("Failed to create job");
    let job_id = job.id.unwrap();

    // List the relationships
    let result = apis::workflows_api::list_job_file_relationships(
        config,
        workflow_id,
        Some(0),
        Some(100),
        None,
        None,
    )
    .expect("Failed to list job-file relationships");

    // Verify we have 2 relationships
    assert_eq!(result.total_count, 2, "Should have 2 relationships");
    let items = result.items;

    // Verify each relationship
    for item in &items {
        // Producer should be the job we created
        assert_eq!(
            item.producer_job_id,
            Some(job_id),
            "Producer should be our job"
        );
        assert_eq!(
            item.producer_job_name.as_ref().unwrap(),
            "producer_job",
            "Producer name should match"
        );

        // These are workflow outputs, so consumer should be None
        assert!(
            item.consumer_job_id.is_none(),
            "Workflow output should have no consumer"
        );
        assert!(
            item.consumer_job_name.is_none(),
            "Workflow output should have no consumer name"
        );
    }
}

#[rstest]
fn test_list_job_file_relationships_intermediate_files(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_intermediate_files_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create intermediate file
    let intermediate_file = create_test_file(
        config,
        workflow_id,
        "intermediate.dat",
        "/path/to/intermediate.dat",
    );
    let file_id = intermediate_file.id.unwrap();

    // Create producer job
    let mut producer_job = models::JobModel::new(
        workflow_id,
        "producer_job".to_string(),
        "echo 'data' > intermediate.dat".to_string(),
    );
    producer_job.output_file_ids = Some(vec![file_id]);
    let producer_job =
        apis::jobs_api::create_job(config, producer_job).expect("Failed to create producer");
    let producer_id = producer_job.id.unwrap();

    // Create consumer jobs
    let mut consumer_job1 = models::JobModel::new(
        workflow_id,
        "consumer_job1".to_string(),
        "cat intermediate.dat".to_string(),
    );
    consumer_job1.input_file_ids = Some(vec![file_id]);
    let consumer_job1 =
        apis::jobs_api::create_job(config, consumer_job1).expect("Failed to create consumer1");
    let consumer1_id = consumer_job1.id.unwrap();

    let mut consumer_job2 = models::JobModel::new(
        workflow_id,
        "consumer_job2".to_string(),
        "cat intermediate.dat".to_string(),
    );
    consumer_job2.input_file_ids = Some(vec![file_id]);
    let consumer_job2 =
        apis::jobs_api::create_job(config, consumer_job2).expect("Failed to create consumer2");
    let consumer2_id = consumer_job2.id.unwrap();

    // List the relationships
    let result = apis::workflows_api::list_job_file_relationships(
        config,
        workflow_id,
        Some(0),
        Some(100),
        None,
        None,
    )
    .expect("Failed to list job-file relationships");

    // We should have 2 relationships (one for each consumer, but each may reference the same producer)
    // The exact count depends on how the query is structured
    assert!(
        result.total_count >= 2,
        "Should have at least 2 relationships"
    );
    let items = result.items;

    // Verify we have relationships with the producer
    let producer_relationships: Vec<_> = items
        .iter()
        .filter(|item| item.producer_job_id == Some(producer_id))
        .collect();
    assert!(
        !producer_relationships.is_empty(),
        "Should have relationships with producer"
    );

    // Verify we have relationships with the consumers
    let consumer_relationships: Vec<_> = items
        .iter()
        .filter(|item| {
            item.consumer_job_id == Some(consumer1_id) || item.consumer_job_id == Some(consumer2_id)
        })
        .collect();
    assert!(
        !consumer_relationships.is_empty(),
        "Should have relationships with consumers"
    );
}

#[rstest]
fn test_list_job_file_relationships_pagination(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_file_pagination_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create 5 files
    let mut file_ids = Vec::new();
    for i in 0..5 {
        let file = create_test_file(
            config,
            workflow_id,
            &format!("file_{}.txt", i),
            &format!("/path/to/file_{}.txt", i),
        );
        file_ids.push(file.id.unwrap());
    }

    // Create a job with all files as outputs
    let mut job = models::JobModel::new(
        workflow_id,
        "multi_file_job".to_string(),
        "echo 'generating files'".to_string(),
    );
    job.output_file_ids = Some(file_ids.clone());
    let _job = apis::jobs_api::create_job(config, job).expect("Failed to create job");

    // Test pagination with limit
    let result_page1 = apis::workflows_api::list_job_file_relationships(
        config,
        workflow_id,
        Some(0),
        Some(3), // limit to 3
        None,
        None,
    )
    .expect("Failed to list first page");

    assert_eq!(result_page1.total_count, 5, "Total count should be 5");
    assert_eq!(result_page1.count, 3, "First page should have 3 items");
    assert!(result_page1.has_more, "Should have more items");

    // Get second page
    let result_page2 = apis::workflows_api::list_job_file_relationships(
        config,
        workflow_id,
        Some(3), // offset
        Some(3),
        None,
        None,
    )
    .expect("Failed to list second page");

    assert_eq!(result_page2.count, 2, "Second page should have 2 items");
    assert!(!result_page2.has_more, "Should not have more items");
}

#[rstest]
fn test_list_job_user_data_relationships_empty(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow with no jobs or user_data
    let workflow = create_test_workflow(config, "test_empty_user_data_rels_workflow");
    let workflow_id = workflow.id.unwrap();

    // Test list_job_user_data_relationships on empty workflow
    let result = apis::workflows_api::list_job_user_data_relationships(
        config,
        workflow_id,
        Some(0),
        Some(100),
        None,
        None,
    )
    .expect("Failed to list job-user_data relationships");

    // Verify response structure
    assert_eq!(result.total_count, 0, "Should have no relationships");
    assert_eq!(result.count, 0, "Count should be 0");
    assert!(result.items.is_empty(), "Items array should be empty");
    assert_eq!(result.items.len(), 0, "Items array should be empty");
}

#[rstest]
fn test_list_job_user_data_relationships_workflow_inputs(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_user_data_inputs_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create workflow input user_data (no producer)
    let input_data1 = create_test_user_data(
        config,
        workflow_id,
        "config1",
        json!({"key": "value1"}),
        false,
    );
    let input_data2 = create_test_user_data(
        config,
        workflow_id,
        "config2",
        json!({"key": "value2"}),
        false,
    );

    // Create a job that consumes these user_data
    let mut job = models::JobModel::new(
        workflow_id,
        "consumer_job".to_string(),
        "process config1 config2".to_string(),
    );
    job.input_user_data_ids = Some(vec![input_data1.id.unwrap(), input_data2.id.unwrap()]);
    let job = apis::jobs_api::create_job(config, job).expect("Failed to create job");
    let job_id = job.id.unwrap();

    // List the relationships
    let result = apis::workflows_api::list_job_user_data_relationships(
        config,
        workflow_id,
        Some(0),
        Some(100),
        None,
        None,
    )
    .expect("Failed to list job-user_data relationships");

    // Verify we have 2 relationships
    assert_eq!(result.total_count, 2, "Should have 2 relationships");
    let items = result.items;
    assert_eq!(items.len(), 2, "Should have 2 items");

    // Verify each relationship
    for item in &items {
        // These are workflow inputs, so producer should be None
        assert!(
            item.producer_job_id.is_none(),
            "Workflow input should have no producer"
        );
        assert!(
            item.producer_job_name.is_none(),
            "Workflow input should have no producer name"
        );

        // Consumer should be the job we created
        assert_eq!(
            item.consumer_job_id,
            Some(job_id),
            "Consumer should be our job"
        );
        assert_eq!(
            item.consumer_job_name.as_ref().unwrap(),
            "consumer_job",
            "Consumer name should match"
        );

        // Verify workflow_id
        assert_eq!(item.workflow_id, workflow_id, "Workflow ID should match");
    }

    // Verify user_data names are present
    let data_names: Vec<&str> = items
        .iter()
        .map(|item| item.user_data_name.as_str())
        .collect();
    assert!(data_names.contains(&"config1"), "Should include config1");
    assert!(data_names.contains(&"config2"), "Should include config2");
}

#[rstest]
fn test_list_job_user_data_relationships_workflow_outputs(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_user_data_outputs_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create workflow output user_data (no consumer)
    let output_user_data = create_test_user_data(
        config,
        workflow_id,
        "final_result",
        json!({"result": "success"}),
        false,
    );

    // Create a job that produces this user_data
    let mut job = models::JobModel::new(
        workflow_id,
        "producer_job".to_string(),
        "echo '{\"result\": \"success\"}' > final_result".to_string(),
    );
    job.output_user_data_ids = Some(vec![output_user_data.id.unwrap()]);
    let job = apis::jobs_api::create_job(config, job).expect("Failed to create job");
    let job_id = job.id.unwrap();

    // List the relationships
    let result = apis::workflows_api::list_job_user_data_relationships(
        config,
        workflow_id,
        Some(0),
        Some(100),
        None,
        None,
    )
    .expect("Failed to list job-user_data relationships");

    // Verify we have 1 relationship
    assert_eq!(result.total_count, 1, "Should have 1 relationship");
    let items = result.items;
    assert_eq!(items.len(), 1, "Should have 1 item");

    let item = &items[0];

    // Producer should be the job we created
    assert_eq!(
        item.producer_job_id,
        Some(job_id),
        "Producer should be our job"
    );
    assert_eq!(
        item.producer_job_name.as_ref().unwrap(),
        "producer_job",
        "Producer name should match"
    );

    // This is a workflow output, so consumer should be None
    assert!(
        item.consumer_job_id.is_none(),
        "Workflow output should have no consumer"
    );
    assert!(
        item.consumer_job_name.is_none(),
        "Workflow output should have no consumer name"
    );

    // Verify user_data name
    assert_eq!(
        item.user_data_name, "final_result",
        "User data name should match"
    );
}

#[rstest]
fn test_list_job_user_data_relationships_intermediate_data(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_intermediate_user_data_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create intermediate user_data
    let intermediate_data = create_test_user_data(
        config,
        workflow_id,
        "temp_data",
        json!({"stage": "intermediate"}),
        true, // ephemeral
    );
    let data_id = intermediate_data.id.unwrap();

    // Create producer job
    let mut producer_job = models::JobModel::new(
        workflow_id,
        "data_generator".to_string(),
        "echo '{\"stage\": \"intermediate\"}' > temp_data".to_string(),
    );
    producer_job.output_user_data_ids = Some(vec![data_id]);
    let producer_job =
        apis::jobs_api::create_job(config, producer_job).expect("Failed to create producer");
    let producer_id = producer_job.id.unwrap();

    // Create consumer job
    let mut consumer_job = models::JobModel::new(
        workflow_id,
        "data_processor".to_string(),
        "process temp_data".to_string(),
    );
    consumer_job.input_user_data_ids = Some(vec![data_id]);
    let consumer_job =
        apis::jobs_api::create_job(config, consumer_job).expect("Failed to create consumer");
    let consumer_id = consumer_job.id.unwrap();

    // List the relationships
    let result = apis::workflows_api::list_job_user_data_relationships(
        config,
        workflow_id,
        Some(0),
        Some(100),
        None,
        None,
    )
    .expect("Failed to list job-user_data relationships");

    // We should have relationships showing both producer and consumer
    assert!(
        result.total_count >= 1,
        "Should have at least 1 relationship"
    );
    let items = result.items;

    // Verify we have a relationship with the producer
    let has_producer = items
        .iter()
        .any(|item| item.producer_job_id == Some(producer_id));
    assert!(has_producer, "Should have relationship with producer");

    // Verify we have a relationship with the consumer
    let has_consumer = items
        .iter()
        .any(|item| item.consumer_job_id == Some(consumer_id));
    assert!(has_consumer, "Should have relationship with consumer");

    // Verify user_data name
    assert!(
        items.iter().all(|item| item.user_data_name == "temp_data"),
        "All relationships should reference the same user_data"
    );
}

#[rstest]
fn test_list_job_dependencies_for_comparison(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_job_deps_comparison");
    let workflow_id = workflow.id.unwrap();

    // Create jobs with dependencies
    let job1 = create_test_job(config, workflow_id, "job1");
    let job1_id = job1.id.unwrap();

    // Create job2 blocked by job1
    let mut job2 =
        models::JobModel::new(workflow_id, "job2".to_string(), "echo 'job2'".to_string());
    job2.depends_on_job_ids = Some(vec![job1_id]);
    let job2 = apis::jobs_api::create_job(config, job2).expect("Failed to create job2");
    let job2_id = job2.id.unwrap();

    // List job dependencies
    let result = apis::workflows_api::list_job_dependencies(
        config,
        workflow_id,
        Some(0),
        Some(100),
        None,
        None,
    )
    .expect("Failed to list job dependencies");

    // Verify we have 1 dependency
    assert_eq!(result.total_count, 1, "Should have 1 dependency");
    let items = result.items;
    assert_eq!(items.len(), 1, "Should have 1 item");

    let dep = &items[0];
    assert_eq!(dep.job_id, job2_id, "Blocked job should be job2");
    assert_eq!(dep.job_name, "job2", "Blocked job name should match");
    assert_eq!(
        dep.depends_on_job_id, job1_id,
        "Blocking job should be job1"
    );
    assert_eq!(
        dep.depends_on_job_name, "job1",
        "Blocking job name should match"
    );
}
