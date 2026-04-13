mod common;

use common::{ServerProcess, start_server};
use rstest::rstest;
use std::fs;
use tempfile::NamedTempFile;
use torc::client::apis;
use torc::client::commands::jobs::create_jobs_from_file;
use torc::models;

#[rstest]
fn test_create_jobs_from_file_basic(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow first
    let workflow = models::WorkflowModel::new(
        "test_create_from_file_workflow".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap() as i64;

    // Create a temp file with job commands
    let job_commands = "echo 'Hello from job 1'\necho 'Hello from job 2'\necho 'Hello from job 3'";
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(temp_file.path(), job_commands).expect("Failed to write temp file");

    // Create jobs from file
    let result = create_jobs_from_file(
        config,
        workflow_id,
        temp_file.path().to_str().unwrap(),
        1,        // cpus_per_job
        "1m",     // memory_per_job
        "P0DT1M", // runtime_per_job
        "table",  // format
    );

    assert!(result.is_ok());
    let jobs_created = result.unwrap();
    assert_eq!(jobs_created, 3);

    // Verify jobs were created
    let jobs = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        Some(0),
        Some(100),
        None,
        None,
        None, // include_relationships
        None, // active_compute_node_id
    )
    .expect("Failed to list jobs");

    assert_eq!(jobs.total_count, 3);
    let job_list = &jobs.items;

    // Check job names are sequential
    assert_eq!(job_list[0].name, "job1");
    assert_eq!(job_list[1].name, "job2");
    assert_eq!(job_list[2].name, "job3");

    // Check commands
    assert_eq!(job_list[0].command, "echo 'Hello from job 1'");
    assert_eq!(job_list[1].command, "echo 'Hello from job 2'");
    assert_eq!(job_list[2].command, "echo 'Hello from job 3'");
}

#[rstest]
fn test_create_jobs_from_file_with_comments(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow first
    let workflow = models::WorkflowModel::new(
        "test_comments_workflow".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap() as i64;

    // Create a temp file with comments and empty lines
    let job_commands = r#"# This is a comment
echo 'job 1'
# Another comment

echo 'job 2'
echo 'job 3'

# Final comment"#;
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(temp_file.path(), job_commands).expect("Failed to write temp file");

    // Create jobs from file
    let result = create_jobs_from_file(
        config,
        workflow_id,
        temp_file.path().to_str().unwrap(),
        2,        // cpus_per_job
        "2g",     // memory_per_job
        "P0DT5M", // runtime_per_job
        "table",  // format
    );

    assert!(result.is_ok());
    let jobs_created = result.unwrap();
    assert_eq!(jobs_created, 3);

    // Verify jobs were created with correct resource requirements
    let jobs = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        Some(0),
        Some(100),
        None,
        None,
        None, // include_relationships
        None, // active_compute_node_id
    )
    .expect("Failed to list jobs");

    assert_eq!(jobs.total_count, 3);
    let job_list = &jobs.items;

    // Check that resource requirements were created and assigned
    let first_job = &job_list[0];
    assert!(first_job.resource_requirements_id.is_some());

    let resource_req_id = first_job.resource_requirements_id.unwrap();
    let resource_req =
        apis::resource_requirements_api::get_resource_requirements(config, resource_req_id)
            .expect("Failed to get resource requirements");

    assert_eq!(resource_req.num_cpus, 2);
    assert_eq!(resource_req.memory, "2g");
    assert_eq!(resource_req.runtime, "P0DT5M");
}

#[rstest]
fn test_create_jobs_from_file_with_existing_jobs(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow first
    let workflow = models::WorkflowModel::new(
        "test_existing_jobs_workflow".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap() as i64;

    // Create some existing jobs manually
    let _existing_job1 = apis::jobs_api::create_job(
        config,
        models::JobModel::new(
            workflow_id,
            "job1".to_string(),
            "existing command 1".to_string(),
        ),
    )
    .expect("Failed to create existing job");

    let _existing_job2 = apis::jobs_api::create_job(
        config,
        models::JobModel::new(
            workflow_id,
            "job2".to_string(),
            "existing command 2".to_string(),
        ),
    )
    .expect("Failed to create existing job");

    // Create a temp file with job commands
    let job_commands = "echo 'new job 1'\necho 'new job 2'";
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(temp_file.path(), job_commands).expect("Failed to write temp file");

    // Create jobs from file
    let result = create_jobs_from_file(
        config,
        workflow_id,
        temp_file.path().to_str().unwrap(),
        1,        // cpus_per_job
        "1m",     // memory_per_job
        "P0DT1M", // runtime_per_job
        "table",  // format
    );

    assert!(result.is_ok());
    let jobs_created = result.unwrap();
    assert_eq!(jobs_created, 2);

    // Verify total job count is now 4 (2 existing + 2 new)
    let jobs = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        Some(0),
        Some(100),
        None,
        None,
        None, // include_relationships
        None, // active_compute_node_id
    )
    .expect("Failed to list jobs");

    assert_eq!(jobs.total_count, 4);
    let job_list = &jobs.items;

    // The new jobs should be named job3 and job4 (starting from existing count + 1)
    let new_jobs: Vec<_> = job_list
        .iter()
        .filter(|job| job.command.starts_with("echo 'new job"))
        .collect();

    assert_eq!(new_jobs.len(), 2);
    assert_eq!(new_jobs[0].name, "job3");
    assert_eq!(new_jobs[1].name, "job4");
}

#[rstest]
fn test_create_jobs_from_file_name_conflicts(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow first
    let workflow = models::WorkflowModel::new(
        "test_name_conflicts_workflow".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap() as i64;

    // Create an existing job that will conflict with the expected naming
    let _existing_job = apis::jobs_api::create_job(
        config,
        models::JobModel::new(
            workflow_id,
            "job1".to_string(),
            "existing command".to_string(),
        ),
    )
    .expect("Failed to create existing job");

    // Create a temp file with job commands
    let job_commands = "echo 'conflicting job'";
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(temp_file.path(), job_commands).expect("Failed to write temp file");

    // Create jobs from file
    let result = create_jobs_from_file(
        config,
        workflow_id,
        temp_file.path().to_str().unwrap(),
        1,        // cpus_per_job
        "1m",     // memory_per_job
        "P0DT1M", // runtime_per_job
        "table",  // format
    );

    assert!(result.is_ok());
    let jobs_created = result.unwrap();
    assert_eq!(jobs_created, 1);

    // Verify the new job got a unique name (should be job2 since job1 exists)
    let jobs = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        Some(0),
        Some(100),
        None,
        None,
        None, // include_relationships
        None, // active_compute_node_id
    )
    .expect("Failed to list jobs");

    assert_eq!(jobs.total_count, 2);
    let job_list = &jobs.items;

    let new_job = job_list
        .iter()
        .find(|job| job.command == "echo 'conflicting job'")
        .expect("New job not found");

    assert_eq!(new_job.name, "job2");
}

#[rstest]
fn test_create_jobs_from_file_empty_file(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow first
    let workflow = models::WorkflowModel::new(
        "test_empty_file_workflow".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap() as i64;

    // Create an empty temp file
    let job_commands = "";
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(temp_file.path(), job_commands).expect("Failed to write temp file");

    // Create jobs from file - should fail
    let result = create_jobs_from_file(
        config,
        workflow_id,
        temp_file.path().to_str().unwrap(),
        1,        // cpus_per_job
        "1m",     // memory_per_job
        "P0DT1M", // runtime_per_job
        "table",  // format
    );

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("No valid commands found")
    );
}

#[rstest]
fn test_create_jobs_from_file_only_comments(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow first
    let workflow = models::WorkflowModel::new(
        "test_only_comments_workflow".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap() as i64;

    // Create a temp file with only comments
    let job_commands = r#"# Comment 1
# Comment 2
# Comment 3"#;
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(temp_file.path(), job_commands).expect("Failed to write temp file");

    // Create jobs from file - should fail
    let result = create_jobs_from_file(
        config,
        workflow_id,
        temp_file.path().to_str().unwrap(),
        1,        // cpus_per_job
        "1m",     // memory_per_job
        "P0DT1M", // runtime_per_job
        "table",  // format
    );

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("No valid commands found")
    );
}

#[rstest]
fn test_create_jobs_from_file_nonexistent_file(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow first
    let workflow = models::WorkflowModel::new(
        "test_nonexistent_workflow".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap() as i64;

    // Try to create jobs from a non-existent file
    let result = create_jobs_from_file(
        config,
        workflow_id,
        "/tmp/nonexistent_file_12345.txt",
        1,        // cpus_per_job
        "1m",     // memory_per_job
        "P0DT1M", // runtime_per_job
        "table",  // format
    );

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("File does not exist")
    );
}

#[rstest]
fn test_create_jobs_from_file_complex_commands(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow first
    let workflow = models::WorkflowModel::new(
        "test_complex_commands_workflow".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap() as i64;

    // Create a temp file with complex commands
    let job_commands = r#"python -c "print('Hello World')"
ls -la /tmp
find /home -name "*.txt" -type f
wget https://example.com/data.csv -O output.csv
ffmpeg -i input.mp4 -vcodec libx264 output.mp4"#;
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(temp_file.path(), job_commands).expect("Failed to write temp file");

    // Create jobs from file
    let result = create_jobs_from_file(
        config,
        workflow_id,
        temp_file.path().to_str().unwrap(),
        4,         // cpus_per_job
        "8g",      // memory_per_job
        "P0DT30M", // runtime_per_job
        "table",   // format
    );

    assert!(result.is_ok());
    let jobs_created = result.unwrap();
    assert_eq!(jobs_created, 5);

    // Verify jobs were created with complex commands
    let jobs = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        Some(0),
        Some(100),
        None,
        None,
        None, // include_relationships
        None, // active_compute_node_id
    )
    .expect("Failed to list jobs");

    assert_eq!(jobs.total_count, 5);
    let job_list = &jobs.items;

    // Check that complex commands were preserved
    assert_eq!(job_list[0].command, r#"python -c "print('Hello World')""#);
    assert_eq!(job_list[1].command, "ls -la /tmp");
    assert_eq!(job_list[2].command, r#"find /home -name "*.txt" -type f"#);
    assert_eq!(
        job_list[3].command,
        "wget https://example.com/data.csv -O output.csv"
    );
    assert_eq!(
        job_list[4].command,
        "ffmpeg -i input.mp4 -vcodec libx264 output.mp4"
    );
}
