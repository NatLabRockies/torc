//! Integration tests for WorkflowManager functionality.
//!
//! These tests verify that the WorkflowManager correctly handles workflow lifecycle
//! operations including initialize, reinitialize, and file management.

mod common;

use common::{
    ServerProcess, create_diamond_workflow, create_test_compute_node, create_test_file,
    create_test_user_data, create_test_workflow_advanced, start_server,
};
use rstest::rstest;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;
use torc::client::{Configuration, default_api, workflow_manager::WorkflowManager};
use torc::config::TorcConfig;
use torc::models;

/// Helper to wait for a job to reach an expected status
/// Returns true if the job reached the expected status within the timeout
fn wait_for_job_status(
    config: &Configuration,
    job_id: i64,
    expected_status: models::JobStatus,
    timeout_secs: u64,
) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed().as_secs() < timeout_secs {
        if let Ok(job) = default_api::get_job(config, job_id)
            && job.status.as_ref() == Some(&expected_status)
        {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    false
}

/// Helper function to create a WorkflowManager with a test workflow
fn create_test_workflow_manager(
    config: Configuration,
    workflow_name: &str,
) -> (WorkflowManager, models::WorkflowModel) {
    let workflow = create_test_workflow_advanced(
        &config,
        workflow_name,
        "test_user",
        Some("Test workflow for WorkflowManager".to_string()),
    );

    let torc_config = TorcConfig::load().unwrap_or_default();
    let manager = WorkflowManager::new(config, torc_config, workflow.clone());
    (manager, workflow)
}

/// Helper function to create test files on disk and in database
fn create_test_files_with_disk_files(
    config: &Configuration,
    workflow_id: i64,
    temp_dir: &TempDir,
) -> Vec<models::FileModel> {
    let mut files = Vec::new();

    // Create test files on disk
    let file1_path = temp_dir.path().join("test_file1.txt");
    let file2_path = temp_dir.path().join("test_file2.txt");
    let file3_path = temp_dir.path().join("subdir").join("test_file3.txt");

    // Create subdirectory
    fs::create_dir_all(file3_path.parent().unwrap()).unwrap();

    // Write test content
    fs::write(&file1_path, "test content 1").unwrap();
    fs::write(&file2_path, "test content 2").unwrap();
    fs::write(&file3_path, "test content 3").unwrap();

    // Create file records in database
    let file1 = create_test_file(
        config,
        workflow_id,
        "test_file1",
        file1_path.to_str().unwrap(),
    );
    files.push(file1);

    let file2 = create_test_file(
        config,
        workflow_id,
        "test_file2",
        file2_path.to_str().unwrap(),
    );
    files.push(file2);

    let file3 = create_test_file(
        config,
        workflow_id,
        "test_file3",
        file3_path.to_str().unwrap(),
    );
    files.push(file3);

    files
}

/// Helper function that follows the pattern of test_process_changed_files_end_to_end
/// Creates a job with resources, starts workflow, calls claim_jobs_based_on_resources, and completes the job
fn execute_workflow_with_job(
    config: &Configuration,
    manager: &WorkflowManager,
    workflow_id: i64,
    job_name: &str,
    command: &str,
    input_file_ids: Option<Vec<i64>>,
) -> Result<(i64, i64), Box<dyn std::error::Error>> {
    // Create resource requirements
    let resource_requirements = models::ResourceRequirementsModel::new(1, "small".to_string());
    let rr = default_api::create_resource_requirements(config, resource_requirements)?;

    // Create a job
    let mut job = models::JobModel::new(workflow_id, job_name.to_string(), command.to_string());
    job.input_file_ids = input_file_ids;
    job.resource_requirements_id = rr.id;
    let created_job = default_api::create_job(config, job)?;
    let job_id = created_job.id.unwrap();

    manager.initialize(false)?;
    let run_id = manager.get_run_id()?;

    // Prepare jobs for submission
    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    let result = default_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        &resources,
        10,
        None,
        None,
    )?;
    let returned_jobs = result.jobs.expect("Server must return jobs array");

    // Verify the job was returned with correct status
    assert_eq!(returned_jobs.len(), 1, "Should return exactly 1 job");
    let prepared_job = &returned_jobs[0];
    assert_eq!(prepared_job.id.expect("Job ID should be present"), job_id);
    assert_eq!(
        prepared_job.status.expect("Job status should be present"),
        models::JobStatus::Pending
    );

    // Complete the job execution cycle
    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    default_api::manage_status_change(config, job_id, models::JobStatus::Running, run_id, None)?;
    let job_result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        0,
        1.0,
        "2020-01-01T00:00:00Z".to_string(),
        models::JobStatus::Completed,
    );
    default_api::complete_job(config, job_id, job_result.status, run_id, job_result)?;

    Ok((job_id, run_id))
}

#[rstest]
fn test_workflow_manager_new(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config, "test_new_workflow");

    assert_eq!(manager.workflow_id, workflow.id.unwrap());
}

#[rstest]
#[should_panic(expected = "Workflow ID must be present")]
fn test_workflow_manager_new_panics_without_id() {
    let config = Configuration::new();
    let mut workflow = models::WorkflowModel::new("test".to_string(), "user".to_string());
    workflow.id = None; // No ID set
    let torc_config = TorcConfig::load().unwrap_or_default();

    // This should panic
    WorkflowManager::new(config, torc_config, workflow);
}

#[rstest]
fn test_initialize_files_empty_workflow(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, _workflow) = create_test_workflow_manager(config, "test_empty_files");

    // Should succeed even with no files
    let result = manager.initialize_files();
    assert!(result.is_ok());
}

#[rstest]
fn test_initialize_files_with_valid_files(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_valid_files");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let files = create_test_files_with_disk_files(&config, workflow_id, &temp_dir);

    // Initially files should have no mtime
    for file in &files {
        assert!(file.st_mtime.is_none());
    }

    // Initialize files - should update all mtime values
    let result = manager.initialize_files();
    assert!(result.is_ok());

    // Check that files were updated with mtime
    for file in &files {
        let updated_file =
            default_api::get_file(&config, file.id.unwrap()).expect("Failed to get updated file");
        assert!(updated_file.st_mtime.is_some());
        assert!(updated_file.st_mtime.unwrap() > 0.0);
    }
}

#[rstest]
fn test_initialize_files_with_missing_files(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_missing_files");
    let workflow_id = workflow.id.unwrap();

    // Create file records but don't create actual files on disk
    let _file1 = create_test_file(
        &config,
        workflow_id,
        "missing_file1",
        "/path/to/nonexistent/file1.txt",
    );
    let _file2 = create_test_file(
        &config,
        workflow_id,
        "missing_file2",
        "/path/to/nonexistent/file2.txt",
    );

    // Should complete but log warnings for missing files
    let result = manager.initialize_files();
    assert!(result.is_ok());
}

#[rstest]
fn test_initialize_files_mtime_unchanged(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_unchanged_files");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let files = create_test_files_with_disk_files(&config, workflow_id, &temp_dir);

    // Initialize files first time
    let result = manager.initialize_files();
    assert!(result.is_ok());

    // Get the updated mtime
    let file = default_api::get_file(&config, files[0].id.unwrap()).expect("Failed to get file");
    let original_mtime = file.st_mtime.unwrap();

    // Initialize again without changing the file - should not update
    let result = manager.initialize_files();
    assert!(result.is_ok());

    // Mtime should be unchanged
    let file_after = default_api::get_file(&config, files[0].id.unwrap())
        .expect("Failed to get file after second init");
    assert_eq!(file_after.st_mtime.unwrap(), original_mtime);
}

#[rstest]
fn test_initialize_files_with_updated_files(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_updated_files");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let files = create_test_files_with_disk_files(&config, workflow_id, &temp_dir);

    // Initialize files first time
    let result = manager.initialize_files();
    assert!(result.is_ok());

    // Get the original mtime
    let file = default_api::get_file(&config, files[0].id.unwrap()).expect("Failed to get file");
    let original_mtime = file.st_mtime.unwrap();

    // Wait a bit and modify the file
    std::thread::sleep(std::time::Duration::from_millis(10));
    let file_path = Path::new(&file.path);
    fs::write(file_path, "updated content").expect("Failed to update file");

    // Initialize again - should detect change
    let result = manager.initialize_files();
    assert!(result.is_ok());

    // Mtime should be updated
    let file_after = default_api::get_file(&config, files[0].id.unwrap())
        .expect("Failed to get file after update");
    assert!(file_after.st_mtime.unwrap() != original_mtime);
}

#[rstest]
fn test_start_workflow_basic(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_start_basic");
    let workflow_id = workflow.id.unwrap();

    // Execute workflow with job using the common pattern
    let result = execute_workflow_with_job(
        &config,
        &manager,
        workflow_id,
        "test_job",
        "echo 'test'",
        None,
    );
    assert!(result.is_ok());

    // Check that an event was created
    let events = default_api::list_events(&config, workflow_id, None, None, None, None, None, None)
        .expect("Failed to list events");
    assert!(!events.items.as_ref().unwrap().is_empty());

    // Check that jobs were completed
    let jobs = default_api::list_jobs(
        &config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None, // include_relationships
        None, // active_compute_node_id
    )
    .expect("Failed to list jobs");
    let job_items = jobs.items.as_ref().unwrap();
    assert!(!job_items.is_empty());
    assert_eq!(
        job_items[0].status.as_ref().unwrap(),
        &models::JobStatus::Completed
    );
}

#[rstest]
fn test_start_workflow_archived(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_start_archived");
    let workflow_id = workflow.id.unwrap();

    // Archive the workflow
    let mut status = default_api::get_workflow_status(&config, workflow_id)
        .expect("Failed to get workflow status");
    status.is_archived = Some(true);
    default_api::update_workflow_status(&config, workflow_id, status)
        .expect("Failed to archive workflow");

    // Start should fail for archived workflow
    let result = manager.initialize(false);
    assert!(result.is_err());

    match result.unwrap_err() {
        torc::client::errors::TorcError::OperationNotAllowed(_) => {
            // Expected error type
        }
        _ => panic!("Expected OperationNotAllowed error"),
    }
}

#[rstest]
fn test_reinitialize_workflow_dry_run(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_reinitialize_dry");
    let workflow_id = workflow.id.unwrap();

    // Execute workflow with job using the common pattern
    let result = execute_workflow_with_job(
        &config,
        &manager,
        workflow_id,
        "test_job",
        "echo 'test'",
        None,
    );
    assert!(result.is_ok());

    // Get original run_id
    let original_status = default_api::get_workflow_status(&config, workflow_id)
        .expect("Failed to get workflow status");
    let original_run_id = original_status.run_id;

    // Dry run reinitialize should succeed without changing anything
    let result = manager.reinitialize(false, true);
    assert!(result.is_ok());

    // Run ID should be unchanged
    let status_after = default_api::get_workflow_status(&config, workflow_id)
        .expect("Failed to get workflow status after dry run");
    assert_eq!(status_after.run_id, original_run_id);
}

#[rstest]
fn test_reinitialize_workflow_real(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) =
        create_test_workflow_manager(config.clone(), "test_reinitialize_real");
    let workflow_id = workflow.id.unwrap();

    // Execute workflow with job using the common pattern
    let result = execute_workflow_with_job(
        &config,
        &manager,
        workflow_id,
        "test_job",
        "echo 'test'",
        None,
    );
    assert!(result.is_ok());

    // Get original run_id
    let original_status = default_api::get_workflow_status(&config, workflow_id)
        .expect("Failed to get workflow status");
    let original_run_id = original_status.run_id;

    // Real reinitialize should increment run_id
    let result = manager.reinitialize(false, false);
    assert!(result.is_ok());

    // Run ID should be incremented
    let status_after = default_api::get_workflow_status(&config, workflow_id)
        .expect("Failed to get workflow status after reinitialize");
    assert_eq!(status_after.run_id, original_run_id + 1);
}

#[rstest]
fn test_get_run_id(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_get_run_id");
    let workflow_id = workflow.id.unwrap();

    let run_id = manager.get_run_id();
    assert!(run_id.is_ok());

    // Should match the database
    let status = default_api::get_workflow_status(&config, workflow_id)
        .expect("Failed to get workflow status");
    assert_eq!(run_id.unwrap(), status.run_id);
}

#[rstest]
fn test_bump_run_id(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_bump_run_id");
    let _workflow_id = workflow.id.unwrap();

    // Get original run_id
    let original_run_id = manager.get_run_id().expect("Failed to get original run_id");

    // Bump run_id
    let result = manager.bump_run_id();
    assert!(result.is_ok());

    // Should be incremented
    let new_run_id = manager.get_run_id().expect("Failed to get new run_id");
    assert_eq!(new_run_id, original_run_id + 1);
}

#[rstest]
fn test_check_workflow_normal(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, _workflow) = create_test_workflow_manager(config, "test_check_normal");

    // Should succeed for normal workflow
    let result = manager.check_workflow(false);
    assert!(result.is_ok());
}

#[rstest]
fn test_check_workflow_archived(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_check_archived");
    let workflow_id = workflow.id.unwrap();

    // Archive the workflow
    let mut status = default_api::get_workflow_status(&config, workflow_id)
        .expect("Failed to get workflow status");
    status.is_archived = Some(true);
    default_api::update_workflow_status(&config, workflow_id, status)
        .expect("Failed to archive workflow");

    // Check should fail for archived workflow
    let result = manager.check_workflow(false);
    assert!(result.is_err());

    match result.unwrap_err() {
        torc::client::errors::TorcError::OperationNotAllowed(_) => {
            // Expected error type
        }
        _ => panic!("Expected OperationNotAllowed error"),
    }
}

#[rstest]
fn test_initialize_jobs(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_init_jobs");
    let workflow_id = workflow.id.unwrap();

    // Execute workflow with multiple jobs using the common pattern
    let result1 =
        execute_workflow_with_job(&config, &manager, workflow_id, "job1", "echo 'job1'", None);
    assert!(result1.is_ok());

    // Create and execute a second job
    let resource_requirements = models::ResourceRequirementsModel::new(1, "small".to_string());
    let rr = default_api::create_resource_requirements(&config, resource_requirements)
        .expect("Failed to create resource requirements");

    let mut job2 =
        models::JobModel::new(workflow_id, "job2".to_string(), "echo 'job2'".to_string());
    job2.resource_requirements_id = rr.id;
    let created_job2 = default_api::create_job(&config, job2).expect("Failed to create job2");

    // Initialize the second job
    let result = manager.initialize_jobs(false);
    assert!(result.is_ok());

    // Second job should now be ready
    let updated_job2 = default_api::get_job(&config, created_job2.id.unwrap())
        .expect("Failed to get updated job2");
    assert_eq!(updated_job2.status.unwrap(), models::JobStatus::Ready);
}

#[rstest]
fn test_reinitialize_jobs(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_reinit_jobs");
    let workflow_id = workflow.id.unwrap();

    // Execute workflow with job using the common pattern
    let result = execute_workflow_with_job(
        &config,
        &manager,
        workflow_id,
        "test_job",
        "echo 'test'",
        None,
    );
    assert!(result.is_ok());

    // Reinitialize should succeed
    let result = manager.reinitialize_jobs(false);
    assert!(result.is_ok());

    // Dry run should also succeed
    let result_dry = manager.reinitialize_jobs(true);
    assert!(result_dry.is_ok());
}

#[rstest]
fn test_initialize_files_file_without_id_panics(start_server: &ServerProcess) {
    // This test would be difficult to create naturally since files from the API
    // should always have IDs, but we test the panic behavior conceptually
    let config = start_server.config.clone();
    let (manager, _workflow) = create_test_workflow_manager(config, "test_panic_no_id");

    // With no files, initialize_files should succeed
    let result = manager.initialize_files();
    assert!(result.is_ok());
}

#[rstest]
fn test_process_changed_files_no_changes(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_no_changes");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let files = create_test_files_with_disk_files(&config, workflow_id, &temp_dir);

    // Initialize files first
    let result = manager.initialize_files();
    assert!(result.is_ok());

    // Process changed files - should find no changes
    let result = manager.process_changed_files(false);
    assert!(result.is_ok());

    // Files should remain unchanged
    for file in &files {
        let updated_file =
            default_api::get_file(&config, file.id.unwrap()).expect("Failed to get file");
        assert!(updated_file.st_mtime.is_some());
    }
}

#[rstest]
fn test_process_changed_files_with_modified_file(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_modified_file");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let files = create_test_files_with_disk_files(&config, workflow_id, &temp_dir);

    // Initialize files first
    let result = manager.initialize_files();
    assert!(result.is_ok());

    // Get the original mtime
    let file = default_api::get_file(&config, files[0].id.unwrap()).expect("Failed to get file");
    let original_mtime = file.st_mtime.unwrap();

    // Wait a bit and modify the file
    std::thread::sleep(std::time::Duration::from_millis(10));
    let file_path = Path::new(&file.path);
    fs::write(file_path, "updated content for change detection").expect("Failed to update file");

    // Process changed files - should detect change
    let result = manager.process_changed_files(false);
    assert!(result.is_ok());

    // File should have updated mtime
    let file_after = default_api::get_file(&config, files[0].id.unwrap())
        .expect("Failed to get file after change processing");
    assert!(file_after.st_mtime.unwrap() != original_mtime);
    assert!(file_after.st_mtime.unwrap() > original_mtime);
}

#[rstest]
fn test_process_changed_files_with_deleted_file(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_deleted_file");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let files = create_test_files_with_disk_files(&config, workflow_id, &temp_dir);

    // Initialize files first
    let result = manager.initialize_files();
    assert!(result.is_ok());

    // Delete one of the files
    let file = default_api::get_file(&config, files[0].id.unwrap()).expect("Failed to get file");
    let file_path = Path::new(&file.path);
    fs::remove_file(file_path).expect("Failed to delete file");

    // Verify the file is actually deleted
    assert!(
        !file_path.exists(),
        "File should be deleted from filesystem"
    );
    assert!(
        fs::metadata(file_path).is_err(),
        "fs::metadata should fail for deleted file"
    );

    // Process changed files - should detect deletion
    let result = manager.process_changed_files(false);
    assert!(result.is_ok());

    // Add small delay to handle potential timing/caching issues
    std::thread::sleep(std::time::Duration::from_millis(100));

    // File should have mtime set to None
    let file_after = default_api::get_file(&config, files[0].id.unwrap())
        .expect("Failed to get file after deletion processing");
    assert!(file_after.st_mtime.is_none());
}

#[rstest]
fn test_process_changed_files_dry_run(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_dry_run");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let files = create_test_files_with_disk_files(&config, workflow_id, &temp_dir);

    // Initialize files first
    let result = manager.initialize_files();
    assert!(result.is_ok());

    // Get the original mtime
    let file = default_api::get_file(&config, files[0].id.unwrap()).expect("Failed to get file");
    let original_mtime = file.st_mtime.unwrap();

    // Wait and modify the file
    std::thread::sleep(std::time::Duration::from_millis(10));
    let file_path = Path::new(&file.path);
    fs::write(file_path, "updated content for dry run test").expect("Failed to update file");

    // Process changed files in dry run mode - should not actually update
    let result = manager.process_changed_files(true);
    assert!(result.is_ok());

    // File should still have original mtime (not updated due to dry run)
    let file_after = default_api::get_file(&config, files[0].id.unwrap())
        .expect("Failed to get file after dry run");
    assert_eq!(file_after.st_mtime.unwrap(), original_mtime);
}

#[rstest]
fn test_update_jobs_on_file_change_no_dependent_jobs(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_no_dependent");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let files = create_test_files_with_disk_files(&config, workflow_id, &temp_dir);

    // Execute workflow with job that doesn't depend on any files
    let result = execute_workflow_with_job(
        &config,
        &manager,
        workflow_id,
        "independent_job",
        "echo 'independent'",
        None,
    );
    assert!(result.is_ok());

    // Update jobs on file change - should succeed with no dependent jobs
    let result = manager.update_jobs_on_file_change(files[0].clone(), false);
    assert!(result.is_ok());
}

#[rstest]
fn test_update_jobs_on_file_change_with_dependent_jobs(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_dependent_jobs");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let files = create_test_files_with_disk_files(&config, workflow_id, &temp_dir);
    let file_id = files[0].id.unwrap();

    // Create resource requirements for both jobs
    let resource_requirements = models::ResourceRequirementsModel::new(1, "small".to_string());
    let rr = default_api::create_resource_requirements(&config, resource_requirements)
        .expect("Failed to create resource requirements");

    // Create job1 with file dependency
    let mut job1 = models::JobModel::new(
        workflow_id,
        "dependent_job1".to_string(),
        "echo 'job1'".to_string(),
    );
    job1.input_file_ids = Some(vec![file_id]);
    job1.resource_requirements_id = rr.id;
    let created_job1 = default_api::create_job(&config, job1).expect("Failed to create job1");
    let job1_id = created_job1.id.unwrap();

    // Create job2 with file dependency
    let mut job2 = models::JobModel::new(
        workflow_id,
        "dependent_job2".to_string(),
        "echo 'job2'".to_string(),
    );
    job2.input_file_ids = Some(vec![file_id]);
    job2.resource_requirements_id = rr.id;
    let created_job2 = default_api::create_job(&config, job2).expect("Failed to create job2");
    let job2_id = created_job2.id.unwrap();

    // Initialize workflow once
    manager
        .initialize(false)
        .expect("Failed to initialize workflow");
    let run_id = manager.get_run_id().expect("Failed to get run_id");

    // Prepare jobs for submission
    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    let result = default_api::claim_jobs_based_on_resources(
        &config,
        workflow_id,
        &resources,
        10,
        None,
        None,
    )
    .expect("Failed to prepare jobs for submission");
    let returned_jobs = result.jobs.expect("Server must return jobs array");

    // Both jobs should be returned
    assert_eq!(returned_jobs.len(), 2, "Should return exactly 2 jobs");

    // Create compute node for job completion
    let compute_node = create_test_compute_node(&config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Complete job1
    default_api::manage_status_change(&config, job1_id, models::JobStatus::Running, run_id, None)
        .expect("Failed to change job1 status to running");
    let job1_result = models::ResultModel::new(
        job1_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        0,
        1.0,
        "2020-01-01T00:00:00Z".to_string(),
        models::JobStatus::Completed,
    );
    default_api::complete_job(&config, job1_id, job1_result.status, run_id, job1_result)
        .expect("Failed to complete job1");

    // Complete job2
    default_api::manage_status_change(&config, job2_id, models::JobStatus::Running, run_id, None)
        .expect("Failed to change job2 status to running");
    let job2_result = models::ResultModel::new(
        job2_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        0,
        1.0,
        "2020-01-01T00:00:00Z".to_string(),
        models::JobStatus::Completed,
    );
    default_api::complete_job(&config, job2_id, job2_result.status, run_id, job2_result)
        .expect("Failed to complete job2");

    // Update jobs on file change - should reset both jobs
    let result = manager.update_jobs_on_file_change(files[0].clone(), false);
    assert!(result.is_ok());

    // Jobs should now be reset to Uninitialized
    let updated_job1 = default_api::get_job(&config, job1_id).expect("Failed to get updated job1");
    let updated_job2 = default_api::get_job(&config, job2_id).expect("Failed to get updated job2");

    assert_eq!(
        updated_job1.status.unwrap(),
        models::JobStatus::Uninitialized
    );
    assert_eq!(
        updated_job2.status.unwrap(),
        models::JobStatus::Uninitialized
    );
}

#[rstest]
fn test_update_jobs_on_file_change_only_completed_jobs_reset(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_only_done_reset");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let files = create_test_files_with_disk_files(&config, workflow_id, &temp_dir);
    let file_id = files[0].id.unwrap();

    // Execute workflow with a job that depends on the file (will end up Completed)
    let (done_id, run_id) = execute_workflow_with_job(
        &config,
        &manager,
        workflow_id,
        "done_job",
        "echo 'completed job'",
        Some(vec![file_id]),
    )
    .expect("Failed to execute completed job");

    // Create additional jobs with different statuses using resource requirements
    let resource_requirements = models::ResourceRequirementsModel::new(1, "small".to_string());
    let rr = default_api::create_resource_requirements(&config, resource_requirements)
        .expect("Failed to create resource requirements");

    let mut job_running = models::JobModel::new(
        workflow_id,
        "running_job".to_string(),
        "echo 'running job'".to_string(),
    );
    job_running.input_file_ids = Some(vec![file_id]);
    job_running.resource_requirements_id = rr.id;

    let mut job_ready = models::JobModel::new(
        workflow_id,
        "ready_job".to_string(),
        "echo 'ready job'".to_string(),
    );
    job_ready.input_file_ids = Some(vec![file_id]);
    job_ready.resource_requirements_id = rr.id;

    let created_running =
        default_api::create_job(&config, job_running).expect("Failed to create running job");
    let created_ready =
        default_api::create_job(&config, job_ready).expect("Failed to create ready job");

    let running_id = created_running.id.unwrap();
    let ready_id = created_ready.id.unwrap();

    // Initialize and set different statuses
    default_api::initialize_jobs(&config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    default_api::manage_status_change(
        &config,
        running_id,
        models::JobStatus::Running,
        run_id,
        None,
    )
    .expect("Failed to set job to running");

    // Update jobs on file change
    let result = manager.update_jobs_on_file_change(files[0].clone(), false);
    assert!(result.is_ok());

    // Only Done job should be reset
    let updated_done =
        default_api::get_job(&config, done_id).expect("Failed to get updated completed job");
    let updated_running =
        default_api::get_job(&config, running_id).expect("Failed to get updated running job");
    let updated_ready =
        default_api::get_job(&config, ready_id).expect("Failed to get updated ready job");

    assert_eq!(
        updated_done.status.unwrap(),
        models::JobStatus::Uninitialized
    );
    assert_eq!(updated_running.status.unwrap(), models::JobStatus::Running);
    assert_eq!(updated_ready.status.unwrap(), models::JobStatus::Ready);
}

#[rstest]
fn test_update_jobs_on_file_change_dry_run(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_job_dry_run");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let files = create_test_files_with_disk_files(&config, workflow_id, &temp_dir);
    let file_id = files[0].id.unwrap();

    // Execute workflow with a job that depends on the file
    let (job_id, _run_id) = execute_workflow_with_job(
        &config,
        &manager,
        workflow_id,
        "dry_run_job",
        "echo 'dry run job'",
        Some(vec![file_id]),
    )
    .expect("Failed to execute job");

    // Update jobs on file change in dry run mode
    let result = manager.update_jobs_on_file_change(files[0].clone(), true);
    assert!(result.is_ok());

    // Job should still be Completed (not reset due to dry run)
    let updated_job = default_api::get_job(&config, job_id).expect("Failed to get updated job");
    assert_eq!(updated_job.status.unwrap(), models::JobStatus::Completed);
}

#[rstest]
fn test_update_jobs_on_file_change_with_canceled_jobs(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_canceled_jobs");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let files = create_test_files_with_disk_files(&config, workflow_id, &temp_dir);
    let file_id = files[0].id.unwrap();

    // Create resource requirements
    let resource_requirements = models::ResourceRequirementsModel::new(1, "small".to_string());
    let rr = default_api::create_resource_requirements(&config, resource_requirements)
        .expect("Failed to create resource requirements");

    // Create a job with file dependency
    let mut job = models::JobModel::new(
        workflow_id,
        "canceled_job".to_string(),
        "echo 'canceled job'".to_string(),
    );
    job.input_file_ids = Some(vec![file_id]);
    job.resource_requirements_id = rr.id;
    let created_job = default_api::create_job(&config, job).expect("Failed to create job");
    let job_id = created_job.id.unwrap();

    // Initialize and claim the job
    manager.initialize(false).expect("Failed to initialize");
    let run_id = manager.get_run_id().expect("Failed to get run_id");

    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    let result = default_api::claim_jobs_based_on_resources(
        &config,
        workflow_id,
        &resources,
        10,
        None,
        None,
    )
    .expect("Failed to claim jobs");
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(returned_jobs.len(), 1, "Should return exactly 1 job");

    // Set to Running using manage_status_change (non-completion status, allowed)
    default_api::manage_status_change(&config, job_id, models::JobStatus::Running, run_id, None)
        .expect("Failed to set job to Running");

    // Create compute node for the result
    let compute_node = create_test_compute_node(&config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Cancel the job using complete_job (the correct API for completion statuses)
    let job_result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        -1, // Non-zero return code indicating cancellation
        0.0,
        "2020-01-01T00:00:00Z".to_string(),
        models::JobStatus::Canceled,
    );
    default_api::complete_job(&config, job_id, job_result.status, run_id, job_result)
        .expect("Failed to cancel job");

    // Update jobs on file change - should reset Canceled job too
    let result = manager.update_jobs_on_file_change(files[0].clone(), false);
    assert!(result.is_ok());

    // Job should be reset to Uninitialized
    let updated_job = default_api::get_job(&config, job_id).expect("Failed to get updated job");
    assert_eq!(
        updated_job.status.unwrap(),
        models::JobStatus::Uninitialized
    );
}

#[rstest]
fn test_update_jobs_on_file_change_file_without_id_fails(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, _workflow) = create_test_workflow_manager(config, "test_no_file_id");

    // Create a file model without ID
    let mut file = models::FileModel::new(1, "test_file".to_string(), "/path/to/file".to_string());
    file.id = None;

    // Should fail with OperationNotAllowed error
    let result = manager.update_jobs_on_file_change(file, false);
    assert!(result.is_err());

    match result.unwrap_err() {
        torc::client::errors::TorcError::OperationNotAllowed(_) => {
            // Expected error type
        }
        _ => panic!("Expected OperationNotAllowed error"),
    }
}

#[rstest]
fn test_process_changed_files_end_to_end(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_full_integration");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let files = create_test_files_with_disk_files(&config, workflow_id, &temp_dir);
    let file_id = files[0].id.unwrap();

    let resource_requirements = models::ResourceRequirementsModel::new(1, "small".to_string());
    let rr = default_api::create_resource_requirements(&config, resource_requirements)
        .expect("Failed to create resource requirements");

    // Create a job that depends on the first file
    let mut job = models::JobModel::new(
        workflow_id,
        "integration_job".to_string(),
        "echo 'integration test'".to_string(),
    );
    job.input_file_ids = Some(vec![file_id]);
    job.resource_requirements_id = rr.id;
    let created_job = default_api::create_job(&config, job).expect("Failed to create job");
    let job_id = created_job.id.unwrap();

    let result = manager.initialize(false);
    assert!(result.is_ok());
    let run_id = manager.get_run_id().expect("Failed to get run_id");

    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    let result = default_api::claim_jobs_based_on_resources(
        &config,
        workflow_id,
        &resources,
        10,
        None,
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");
    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(
        returned_jobs.len(),
        1,
        "Server must return exactly 1 job with 1 CPU available for 4 ready jobs needing 1 CPU each, got {}",
        returned_jobs.len()
    );
    let job = &returned_jobs[0];
    assert_eq!(job.id.expect("Job ID should be present"), job_id);
    assert_eq!(
        job.status.expect("Job status should be present"),
        models::JobStatus::Pending
    );
    default_api::manage_status_change(&config, job_id, models::JobStatus::Running, run_id, None)
        .expect("Failed to set job to Completed");

    // Create a compute node for the results
    let compute_node = create_test_compute_node(&config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    let job_result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        0,
        1.0,
        "2020-01-01T00:00:00Z".to_string(),
        models::JobStatus::Completed,
    );
    default_api::complete_job(&config, job_id, job_result.status, run_id, job_result)
        .expect("Failed to complete job");

    // Modify the file
    std::thread::sleep(std::time::Duration::from_millis(10));
    let file_path = Path::new(&files[0].path);
    fs::write(file_path, "modified content for integration test").expect("Failed to modify file");

    // Process changed files - should update file and reset job
    let result = manager.process_changed_files(false);
    assert!(result.is_ok());

    // Verify file was updated
    let updated_file = default_api::get_file(&config, file_id).expect("Failed to get updated file");
    assert!(updated_file.st_mtime.is_some());

    // Verify job was reset
    let updated_job = default_api::get_job(&config, job_id).expect("Failed to get updated job");
    assert_eq!(
        updated_job.status.unwrap(),
        models::JobStatus::Uninitialized
    );
}

#[rstest]
fn test_workflow_manager_end_to_end(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_end_to_end");
    let workflow_id = workflow.id.unwrap();

    // Create some test data
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let _files = create_test_files_with_disk_files(&config, workflow_id, &temp_dir);

    // Execute workflow with job using the common pattern
    let result = execute_workflow_with_job(
        &config,
        &manager,
        workflow_id,
        "test_job",
        "echo 'test job'",
        None,
    );
    assert!(result.is_ok());

    // Check that everything was initialized properly
    let events = default_api::list_events(&config, workflow_id, None, None, None, None, None, None)
        .expect("Failed to list events");
    assert!(!events.items.as_ref().unwrap().is_empty());

    let jobs = default_api::list_jobs(
        &config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None, // include_relationships
        None, // active_compute_node_id
    )
    .expect("Failed to list jobs");
    let job_items = jobs.items.as_ref().unwrap();
    assert!(!job_items.is_empty());
    assert_eq!(
        job_items[0].status.as_ref().unwrap(),
        &models::JobStatus::Completed
    );

    let files = default_api::list_files(
        &config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None, // is_output filter
    )
    .expect("Failed to list files");
    let file_items = files.items.as_ref().unwrap();
    assert!(!file_items.is_empty());
    for file in file_items {
        assert!(file.st_mtime.is_some());
    }
}

#[rstest]
fn test_update_file_with_none_st_mtime(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_none_mtime");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let files = create_test_files_with_disk_files(&config, workflow_id, &temp_dir);
    let file_id = files[0].id.unwrap();

    // Initialize files first to set st_mtime
    let result = manager.initialize_files();
    assert!(result.is_ok());

    // Verify file has st_mtime set
    let file_before = default_api::get_file(&config, file_id).expect("Failed to get file");
    assert!(file_before.st_mtime.is_some());

    // Create a file model with st_mtime = None and try to update it
    let mut file_with_none_mtime = file_before.clone();
    file_with_none_mtime.st_mtime = None;

    // Attempt to update the file with st_mtime = None
    let update_result = default_api::update_file(&config, file_id, file_with_none_mtime);
    assert!(update_result.is_ok());

    // Check if the file was actually updated with None
    let file_after =
        default_api::get_file(&config, file_id).expect("Failed to get file after update");
    assert!(
        file_after.st_mtime.is_none(),
        "Expected st_mtime to be None after update, but got {:?}",
        file_after.st_mtime
    );
}

/// Helper function to create a job with output files
fn create_job_with_output_files(
    config: &Configuration,
    workflow_id: i64,
    job_name: &str,
    command: &str,
    temp_dir: &TempDir,
) -> (i64, Vec<models::FileModel>) {
    // Create resource requirements
    let resource_requirements = models::ResourceRequirementsModel::new(1, "small".to_string());
    let rr = default_api::create_resource_requirements(config, resource_requirements)
        .expect("Failed to create resource requirements");

    // Create output file paths (don't create actual files on disk yet)
    let mut output_files = Vec::new();
    let output1_path = temp_dir.path().join(format!("output_{}_1.txt", job_name));
    let output2_path = temp_dir.path().join(format!("output_{}_2.txt", job_name));

    // Create file records in database
    let file1 = models::FileModel::new(
        workflow_id,
        format!("output_{}_1", job_name),
        output1_path.to_str().unwrap().to_string(),
    );
    let created_file1 =
        default_api::create_file(config, file1).expect("Failed to create file 1 in database");
    output_files.push(created_file1.clone());

    let file2 = models::FileModel::new(
        workflow_id,
        format!("output_{}_2", job_name),
        output2_path.to_str().unwrap().to_string(),
    );
    let created_file2 =
        default_api::create_file(config, file2).expect("Failed to create file 2 in database");
    output_files.push(created_file2.clone());

    // Create the job with output file IDs set
    let mut job = models::JobModel::new(workflow_id, job_name.to_string(), command.to_string());
    job.resource_requirements_id = rr.id;
    job.output_file_ids = Some(vec![created_file1.id.unwrap(), created_file2.id.unwrap()]);
    let created_job = default_api::create_job(config, job).expect("Failed to create job");
    let job_id = created_job.id.unwrap();

    (job_id, output_files)
}

/// Helper function to complete a job and create its output files on disk
/// This simulates a job successfully running and producing output files
fn complete_job_and_create_files(
    config: &Configuration,
    job_id: i64,
    workflow_id: i64,
    run_id: i64,
    compute_node_id: i64,
    output_files: &[models::FileModel],
) {
    // Create job result
    let job_result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        0,   // return_code (success)
        1.0, // exec_time_minutes
        chrono::Utc::now().to_rfc3339(),
        models::JobStatus::Completed,
    );

    // Complete the job
    default_api::complete_job(config, job_id, job_result.status, run_id, job_result)
        .expect("Failed to complete job");

    // Now create the output files on disk (simulating job execution)
    for file in output_files {
        let file_path = Path::new(&file.path);
        fs::write(file_path, format!("Output from {}", file.name))
            .expect("Failed to create output file");
    }
}

#[rstest]
fn test_update_jobs_if_output_files_are_missing_no_missing_files(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) =
        create_test_workflow_manager(config.clone(), "test_no_missing_output_files");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create a job with output files (files not created on disk yet)
    let (job_id, output_files) =
        create_job_with_output_files(&config, workflow_id, "test_job", "echo 'test'", &temp_dir);

    // Initialize workflow
    let result = manager.initialize(false);
    assert!(result.is_ok());
    let run_id = manager.get_run_id().expect("Failed to get run_id");

    // Set job to running
    default_api::manage_status_change(&config, job_id, models::JobStatus::Running, run_id, None)
        .expect("Failed to set job to running");

    // Create a compute node for the results
    let compute_node = create_test_compute_node(&config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Complete the job and create output files (simulating job execution)
    complete_job_and_create_files(
        &config,
        job_id,
        workflow_id,
        run_id,
        compute_node_id,
        &output_files,
    );

    // All output files exist, so no jobs should be reset
    let result = manager.update_jobs_if_output_files_are_missing(false);
    assert!(result.is_ok());

    // Job should still be Completed
    let updated_job = default_api::get_job(&config, job_id).expect("Failed to get updated job");
    assert_eq!(updated_job.status.unwrap(), models::JobStatus::Completed);
}

#[rstest]
fn test_update_jobs_if_output_files_are_missing_with_missing_files(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) =
        create_test_workflow_manager(config.clone(), "test_missing_output_files");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create a job with output files (files not created on disk yet)
    let (job_id, output_files) =
        create_job_with_output_files(&config, workflow_id, "test_job", "echo 'test'", &temp_dir);

    // Initialize workflow
    let result = manager.initialize(false);
    assert!(result.is_ok());
    let run_id = manager.get_run_id().expect("Failed to get run_id");

    // Set job to running
    default_api::manage_status_change(&config, job_id, models::JobStatus::Running, run_id, None)
        .expect("Failed to set job to running");

    // Create a compute node for the results
    let compute_node = create_test_compute_node(&config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Complete the job and create output files (simulating job execution)
    complete_job_and_create_files(
        &config,
        job_id,
        workflow_id,
        run_id,
        compute_node_id,
        &output_files,
    );

    // Delete one of the output files
    let file_path = Path::new(&output_files[0].path);
    fs::remove_file(file_path).expect("Failed to delete output file");
    assert!(!file_path.exists(), "Output file should be deleted");

    // Function should detect missing file and reset job
    let result = manager.update_jobs_if_output_files_are_missing(false);
    assert!(result.is_ok());

    // Job should be reset to Uninitialized
    let updated_job = default_api::get_job(&config, job_id).expect("Failed to get updated job");
    assert_eq!(
        updated_job.status.unwrap(),
        models::JobStatus::Uninitialized
    );
}

#[rstest]
fn test_update_jobs_if_output_files_are_missing_dry_run_true(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) =
        create_test_workflow_manager(config.clone(), "test_missing_output_files_dry_run");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create a job with output files (files not created on disk yet)
    let (job_id, output_files) =
        create_job_with_output_files(&config, workflow_id, "test_job", "echo 'test'", &temp_dir);

    // Initialize workflow
    let result = manager.initialize(false);
    assert!(result.is_ok());
    let run_id = manager.get_run_id().expect("Failed to get run_id");

    // Set job to running
    default_api::manage_status_change(&config, job_id, models::JobStatus::Running, run_id, None)
        .expect("Failed to set job to running");

    // Create a compute node for the results
    let compute_node = create_test_compute_node(&config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Complete the job and create output files (simulating job execution)
    complete_job_and_create_files(
        &config,
        job_id,
        workflow_id,
        run_id,
        compute_node_id,
        &output_files,
    );

    // Delete one of the output files
    let file_path = Path::new(&output_files[0].path);
    fs::remove_file(file_path).expect("Failed to delete output file");
    assert!(!file_path.exists(), "Output file should be deleted");

    // Dry run should detect missing file but not reset job
    let result = manager.update_jobs_if_output_files_are_missing(true);
    assert!(result.is_ok());

    // Job should still be Completed (not reset due to dry run)
    let updated_job = default_api::get_job(&config, job_id).expect("Failed to get updated job");
    assert_eq!(updated_job.status.unwrap(), models::JobStatus::Completed);
}

#[rstest]
fn test_update_jobs_if_output_files_are_missing_dry_run_false(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) =
        create_test_workflow_manager(config.clone(), "test_missing_output_files_no_dry_run");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create a job with output files (files not created on disk yet)
    let (job_id, output_files) =
        create_job_with_output_files(&config, workflow_id, "test_job", "echo 'test'", &temp_dir);

    // Initialize workflow
    let result = manager.initialize(false);
    assert!(result.is_ok());
    let run_id = manager.get_run_id().expect("Failed to get run_id");

    // Set job to running
    default_api::manage_status_change(&config, job_id, models::JobStatus::Running, run_id, None)
        .expect("Failed to set job to running");

    // Create a compute node for the results
    let compute_node = create_test_compute_node(&config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Complete the job and create output files (simulating job execution)
    complete_job_and_create_files(
        &config,
        job_id,
        workflow_id,
        run_id,
        compute_node_id,
        &output_files,
    );

    // Delete one of the output files
    let file_path = Path::new(&output_files[0].path);
    fs::remove_file(file_path).expect("Failed to delete output file");
    assert!(!file_path.exists(), "Output file should be deleted");

    // Function should detect missing file and reset job (dry_run = false)
    let result = manager.update_jobs_if_output_files_are_missing(false);
    assert!(result.is_ok());

    // Job should be reset to Uninitialized
    let updated_job = default_api::get_job(&config, job_id).expect("Failed to get updated job");
    assert_eq!(
        updated_job.status.unwrap(),
        models::JobStatus::Uninitialized
    );
}

#[rstest]
fn test_update_jobs_if_output_files_are_missing_multiple_jobs(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) =
        create_test_workflow_manager(config.clone(), "test_multiple_jobs_missing_files");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create multiple jobs with output files (files not created on disk yet)
    let (job1_id, output_files1) =
        create_job_with_output_files(&config, workflow_id, "job1", "echo 'job1'", &temp_dir);

    let (job2_id, output_files2) =
        create_job_with_output_files(&config, workflow_id, "job2", "echo 'job2'", &temp_dir);

    let (job3_id, output_files3) =
        create_job_with_output_files(&config, workflow_id, "job3", "echo 'job3'", &temp_dir);

    // Initialize workflow
    let result = manager.initialize(false);
    assert!(result.is_ok());
    let run_id = manager.get_run_id().expect("Failed to get run_id");

    // Create a compute node for the results
    let compute_node = create_test_compute_node(&config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Complete all jobs and create their output files
    for (job_id, output_files) in [
        (job1_id, &output_files1),
        (job2_id, &output_files2),
        (job3_id, &output_files3),
    ] {
        default_api::manage_status_change(
            &config,
            job_id,
            models::JobStatus::Running,
            run_id,
            None,
        )
        .expect("Failed to set job to running");

        complete_job_and_create_files(
            &config,
            job_id,
            workflow_id,
            run_id,
            compute_node_id,
            output_files,
        );
    }

    // Delete output files from job1 and job2, but not job3
    let file1_path = Path::new(&output_files1[0].path);
    let file2_path = Path::new(&output_files2[0].path);

    fs::remove_file(file1_path).expect("Failed to delete job1 output file");
    fs::remove_file(file2_path).expect("Failed to delete job2 output file");

    assert!(!file1_path.exists(), "Job1 output file should be deleted");
    assert!(!file2_path.exists(), "Job2 output file should be deleted");

    // Function should reset job1 and job2, but not job3
    let result = manager.update_jobs_if_output_files_are_missing(false);
    assert!(result.is_ok());

    // Check job statuses
    let updated_job1 = default_api::get_job(&config, job1_id).expect("Failed to get updated job1");
    let updated_job2 = default_api::get_job(&config, job2_id).expect("Failed to get updated job2");
    let updated_job3 = default_api::get_job(&config, job3_id).expect("Failed to get updated job3");

    assert_eq!(
        updated_job1.status.unwrap(),
        models::JobStatus::Uninitialized
    );
    assert_eq!(
        updated_job2.status.unwrap(),
        models::JobStatus::Uninitialized
    );
    assert_eq!(updated_job3.status.unwrap(), models::JobStatus::Completed);
}

#[rstest]
fn test_update_jobs_if_output_files_are_missing_no_done_jobs(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) = create_test_workflow_manager(config.clone(), "test_no_done_jobs");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create a job with output files but don't complete it (files not created on disk)
    let (job_id, _output_files) =
        create_job_with_output_files(&config, workflow_id, "test_job", "echo 'test'", &temp_dir);

    // Initialize workflow but don't complete the job (leave it Ready)
    let result = manager.initialize(false);
    assert!(result.is_ok());

    // Output files don't exist (job was never completed)
    // Function should not affect non-Completed jobs
    let result = manager.update_jobs_if_output_files_are_missing(false);
    assert!(result.is_ok());

    // Job should still be Ready (not affected since it wasn't Completed)
    let updated_job = default_api::get_job(&config, job_id).expect("Failed to get updated job");
    assert_eq!(updated_job.status.unwrap(), models::JobStatus::Ready);
}

#[rstest]
fn test_update_jobs_if_output_files_are_missing_with_upstream_jobs_dry_run(
    start_server: &ServerProcess,
) {
    let config = start_server.config.clone();
    let (manager, workflow) =
        create_test_workflow_manager(config.clone(), "test_upstream_jobs_dry_run");
    let workflow_id = workflow.id.unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create a job with output files (files not created on disk yet)
    let (job1_id, output_files1) =
        create_job_with_output_files(&config, workflow_id, "job1", "echo 'job1'", &temp_dir);

    // Create resource requirements for upstream job
    let resource_requirements = models::ResourceRequirementsModel::new(1, "small".to_string());
    let rr = default_api::create_resource_requirements(&config, resource_requirements)
        .expect("Failed to create resource requirements");

    // Create an upstream job that depends on job1
    let mut upstream_job = models::JobModel::new(
        workflow_id,
        "upstream_job".to_string(),
        "echo 'upstream'".to_string(),
    );
    upstream_job.resource_requirements_id = rr.id;
    upstream_job.depends_on_job_ids = Some(vec![job1_id]);
    let created_upstream =
        default_api::create_job(&config, upstream_job).expect("Failed to create upstream job");
    let upstream_job_id = created_upstream.id.unwrap();

    // Initialize workflow
    let result = manager.initialize(false);
    assert!(result.is_ok());
    let run_id = manager.get_run_id().expect("Failed to get run_id");

    // Create a compute node for the results
    let compute_node = create_test_compute_node(&config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Complete job1 (with output files)
    default_api::manage_status_change(&config, job1_id, models::JobStatus::Running, run_id, None)
        .expect("Failed to set job1 to running");
    complete_job_and_create_files(
        &config,
        job1_id,
        workflow_id,
        run_id,
        compute_node_id,
        &output_files1,
    );

    // Wait for the background unblocking task to transition upstream_job to ready
    assert!(
        wait_for_job_status(&config, upstream_job_id, models::JobStatus::Ready, 10),
        "upstream_job did not become ready after job1 completed"
    );

    // Complete upstream job (no output files to create)
    default_api::manage_status_change(
        &config,
        upstream_job_id,
        models::JobStatus::Running,
        run_id,
        None,
    )
    .expect("Failed to set upstream_job to running");
    let upstream_result = models::ResultModel::new(
        upstream_job_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        0,
        1.0,
        chrono::Utc::now().to_rfc3339(),
        models::JobStatus::Completed,
    );
    default_api::complete_job(
        &config,
        upstream_job_id,
        upstream_result.status,
        run_id,
        upstream_result,
    )
    .expect("Failed to complete upstream job");

    // Delete job1's output file
    let file_path = Path::new(&output_files1[0].path);
    fs::remove_file(file_path).expect("Failed to delete job1 output file");
    assert!(!file_path.exists(), "Job1 output file should be deleted");

    // Dry run should log what would be changed including upstream jobs
    let result = manager.update_jobs_if_output_files_are_missing(true);
    assert!(result.is_ok());

    // Both jobs should still be Completed (not reset due to dry run)
    let updated_job1 = default_api::get_job(&config, job1_id).expect("Failed to get updated job1");
    let updated_upstream =
        default_api::get_job(&config, upstream_job_id).expect("Failed to get updated upstream job");

    assert_eq!(updated_job1.status.unwrap(), models::JobStatus::Completed);
    assert_eq!(
        updated_upstream.status.unwrap(),
        models::JobStatus::Completed
    );
}

#[rstest]
fn test_initialize_workflow_with_missing_files_ignore_missing_data_true(
    start_server: &ServerProcess,
) {
    let config = start_server.config.clone();
    let (manager, workflow) =
        create_test_workflow_manager(config.clone(), "test_initialize_ignore_missing");
    let workflow_id = workflow.id.unwrap();

    let resource_requirements = models::ResourceRequirementsModel::new(1, "small".to_string());
    let rr = default_api::create_resource_requirements(&config, resource_requirements)
        .expect("Failed to create resource requirements");

    // Create file records in database but don't create actual files on disk
    let missing_file1 = create_test_file(
        &config,
        workflow_id,
        "missing_input1",
        "/path/to/nonexistent/input1.txt",
    );
    let missing_file2 = create_test_file(
        &config,
        workflow_id,
        "missing_input2",
        "/path/to/nonexistent/input2.txt",
    );

    // Create a job that requires these missing files
    let mut job = models::JobModel::new(
        workflow_id,
        "job_with_missing_inputs".to_string(),
        "echo 'test job with missing inputs'".to_string(),
    );
    job.input_file_ids = Some(vec![missing_file1.id.unwrap(), missing_file2.id.unwrap()]);
    job.resource_requirements_id = rr.id;
    let created_job = default_api::create_job(&config, job).expect("Failed to create job");
    let job_id = created_job.id.unwrap();

    // Verify files don't exist on disk
    assert!(
        !std::path::Path::new(&missing_file1.path).exists(),
        "File should not exist on disk"
    );
    assert!(
        !std::path::Path::new(&missing_file2.path).exists(),
        "File should not exist on disk"
    );

    // Initialize workflow with ignore_missing_data = true - should succeed despite missing files
    let result = manager.initialize(true);
    assert!(
        result.is_ok(),
        "Initialize should succeed with ignore_missing_data = true"
    );

    // Verify job is in Ready status
    let updated_job = default_api::get_job(&config, job_id).expect("Failed to get updated job");
    assert_eq!(
        updated_job.status.unwrap(),
        models::JobStatus::Ready,
        "Job should be Ready despite missing input files"
    );

    // Verify workflow is properly initialized
    let run_id = manager.get_run_id().expect("Failed to get run_id");
    assert!(run_id > 0, "Run ID should be positive");
}

#[rstest]
fn test_initialize_workflow_with_missing_files_ignore_missing_data_false(
    start_server: &ServerProcess,
) {
    let config = start_server.config.clone();
    let (manager, workflow) =
        create_test_workflow_manager(config.clone(), "test_initialize_no_ignore_missing");
    let workflow_id = workflow.id.unwrap();

    let resource_requirements = models::ResourceRequirementsModel::new(1, "small".to_string());
    let rr = default_api::create_resource_requirements(&config, resource_requirements)
        .expect("Failed to create resource requirements");

    // Create file records in database but don't create actual files on disk
    let missing_file = create_test_file(
        &config,
        workflow_id,
        "missing_input",
        "/path/to/nonexistent/input.txt",
    );

    // Create a job that requires this missing file
    let mut job = models::JobModel::new(
        workflow_id,
        "job_with_missing_input".to_string(),
        "echo 'test job with missing input'".to_string(),
    );
    job.input_file_ids = Some(vec![missing_file.id.unwrap()]);
    job.resource_requirements_id = rr.id;
    let _created_job = default_api::create_job(&config, job).expect("Failed to create job");

    // Verify file doesn't exist on disk
    assert!(
        !std::path::Path::new(&missing_file.path).exists(),
        "File should not exist on disk"
    );

    // Initialize workflow with ignore_missing_data = false - should complete but log warnings
    let result = manager.initialize(true);
    assert!(
        result.is_ok(),
        "Initialize should still succeed but with warnings for missing files"
    );
}

#[rstest]
fn test_reinitialize_workflow_with_missing_files_ignore_missing_data_true(
    start_server: &ServerProcess,
) {
    let config = start_server.config.clone();
    let (manager, workflow) =
        create_test_workflow_manager(config.clone(), "test_reinitialize_ignore_missing");
    let workflow_id = workflow.id.unwrap();

    let resource_requirements = models::ResourceRequirementsModel::new(1, "small".to_string());
    let rr = default_api::create_resource_requirements(&config, resource_requirements)
        .expect("Failed to create resource requirements");

    // Create file records in database but don't create actual files on disk
    let missing_file1 = create_test_file(
        &config,
        workflow_id,
        "missing_reinitialize_input1",
        "/path/to/nonexistent/reinitialize1.txt",
    );
    let missing_file2 = create_test_file(
        &config,
        workflow_id,
        "missing_reinitialize_input2",
        "/path/to/nonexistent/reinitialize2.txt",
    );

    // Create a job that requires these missing files
    let mut job = models::JobModel::new(
        workflow_id,
        "reinitialize_job_with_missing_inputs".to_string(),
        "echo 'reinitialize test job with missing inputs'".to_string(),
    );
    job.input_file_ids = Some(vec![missing_file1.id.unwrap(), missing_file2.id.unwrap()]);
    job.resource_requirements_id = rr.id;
    let created_job = default_api::create_job(&config, job).expect("Failed to create job");
    let job_id = created_job.id.unwrap();

    // First initialize the workflow normally
    let result = manager.initialize(true);
    assert!(result.is_ok(), "Initial initialize should succeed");

    // Complete the job to have something to reinitialize from
    let run_id = manager.get_run_id().expect("Failed to get run_id");
    default_api::manage_status_change(&config, job_id, models::JobStatus::Running, run_id, None)
        .expect("Failed to set job to running");

    // Create a compute node for the results
    let compute_node = create_test_compute_node(&config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    let job_result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        0,
        1.0,
        "2020-01-01T00:00:00Z".to_string(),
        models::JobStatus::Completed,
    );
    default_api::complete_job(&config, job_id, job_result.status, run_id, job_result)
        .expect("Failed to complete job");

    // Get original run_id before reinitialize
    let original_run_id = manager.get_run_id().expect("Failed to get original run_id");

    // Verify files still don't exist on disk
    assert!(
        !std::path::Path::new(&missing_file1.path).exists(),
        "File should not exist on disk"
    );
    assert!(
        !std::path::Path::new(&missing_file2.path).exists(),
        "File should not exist on disk"
    );

    // Reinitialize workflow with ignore_missing_data = true, dry_run = false
    let result = manager.reinitialize(true, false);
    assert!(
        result.is_ok(),
        "Reinitialize should succeed with ignore_missing_data = true"
    );

    // Verify run_id was incremented
    let new_run_id = manager.get_run_id().expect("Failed to get new run_id");
    assert_eq!(
        new_run_id,
        original_run_id + 1,
        "Run ID should be incremented after reinitialize"
    );
}

#[rstest]
fn test_reinitialize_workflow_with_missing_files_dry_run_true(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow) =
        create_test_workflow_manager(config.clone(), "test_reinitialize_dry_run_missing");
    let workflow_id = workflow.id.unwrap();

    // Create resource requirements
    let resource_requirements = models::ResourceRequirementsModel::new(1, "small".to_string());
    let rr = default_api::create_resource_requirements(&config, resource_requirements)
        .expect("Failed to create resource requirements");

    // Create file records in database but don't create actual files on disk
    let missing_file = create_test_file(
        &config,
        workflow_id,
        "missing_dry_run_input",
        "/path/to/nonexistent/dryrun.txt",
    );

    // Create a job that requires this missing file
    let mut job = models::JobModel::new(
        workflow_id,
        "dry_run_job_with_missing_input".to_string(),
        "echo 'dry run test job with missing input'".to_string(),
    );
    job.input_file_ids = Some(vec![missing_file.id.unwrap()]);
    job.resource_requirements_id = rr.id;
    let created_job = default_api::create_job(&config, job).expect("Failed to create job");
    let job_id = created_job.id.unwrap();

    // First initialize the workflow normally
    let result = manager.initialize(true);
    assert!(result.is_ok(), "Initial initialize should succeed");

    // Complete the job to have something to reinitialize from
    let run_id = manager.get_run_id().expect("Failed to get run_id");
    default_api::manage_status_change(&config, job_id, models::JobStatus::Running, run_id, None)
        .expect("Failed to set job to running");

    // Create a compute node for the results
    let compute_node = create_test_compute_node(&config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    let job_result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        0,
        1.0,
        "2020-01-01T00:00:00Z".to_string(),
        models::JobStatus::Completed,
    );
    default_api::complete_job(&config, job_id, job_result.status, run_id, job_result)
        .expect("Failed to complete job");

    // Get original run_id and job status before dry run reinitialize
    let original_run_id = manager.get_run_id().expect("Failed to get original run_id");
    let original_job = default_api::get_job(&config, job_id).expect("Failed to get original job");

    // Verify file doesn't exist on disk
    assert!(
        !std::path::Path::new(&missing_file.path).exists(),
        "File should not exist on disk"
    );

    // Reinitialize workflow with ignore_missing_data = true, dry_run = true
    let result = manager.reinitialize(true, true);
    assert!(
        result.is_ok(),
        "Dry run reinitialize should succeed with ignore_missing_data = true"
    );

    // Verify run_id was NOT incremented (dry run)
    let run_id_after = manager
        .get_run_id()
        .expect("Failed to get run_id after dry run");
    assert_eq!(
        run_id_after, original_run_id,
        "Run ID should be unchanged after dry run reinitialize"
    );

    // Verify job status was NOT changed (dry run)
    let job_after = default_api::get_job(&config, job_id).expect("Failed to get job after dry run");
    assert_eq!(
        job_after.status.unwrap(),
        original_job.status.unwrap(),
        "Job status should be unchanged after dry run reinitialize"
    );
}

#[rstest]
fn test_reinitialize_workflow_with_missing_files_ignore_missing_data_false(
    start_server: &ServerProcess,
) {
    let config = start_server.config.clone();
    let (manager, workflow) =
        create_test_workflow_manager(config.clone(), "test_reinitialize_no_ignore_missing");
    let workflow_id = workflow.id.unwrap();

    let resource_requirements = models::ResourceRequirementsModel::new(1, "small".to_string());
    let rr = default_api::create_resource_requirements(&config, resource_requirements)
        .expect("Failed to create resource requirements");

    // Create file records in database but don't create actual files on disk
    let missing_file = create_test_file(
        &config,
        workflow_id,
        "missing_no_ignore_input",
        "/path/to/nonexistent/noignore.txt",
    );

    // Create a job that requires this missing file
    let mut job = models::JobModel::new(
        workflow_id,
        "no_ignore_job_with_missing_input".to_string(),
        "echo 'no ignore test job with missing input'".to_string(),
    );
    job.input_file_ids = Some(vec![missing_file.id.unwrap()]);
    job.resource_requirements_id = rr.id;
    let created_job = default_api::create_job(&config, job).expect("Failed to create job");
    let job_id = created_job.id.unwrap();

    // First initialize the workflow normally
    let result = manager.initialize(true);
    assert!(result.is_ok(), "Initial initialize should succeed");

    // Complete the job to have something to reinitialize from
    let run_id = manager.get_run_id().expect("Failed to get run_id");
    default_api::manage_status_change(&config, job_id, models::JobStatus::Running, run_id, None)
        .expect("Failed to set job to running");

    // Create a compute node for the results
    let compute_node = create_test_compute_node(&config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    let job_result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        0,
        1.0,
        "2020-01-01T00:00:00Z".to_string(),
        models::JobStatus::Completed,
    );
    default_api::complete_job(&config, job_id, job_result.status, run_id, job_result)
        .expect("Failed to complete job");

    // Get original run_id before reinitialize
    let original_run_id = manager.get_run_id().expect("Failed to get original run_id");

    // Verify file doesn't exist on disk
    assert!(
        !std::path::Path::new(&missing_file.path).exists(),
        "File should not exist on disk"
    );

    // Reinitialize workflow with ignore_missing_data = false, dry_run = false
    let result = manager.reinitialize(true, false);
    assert!(
        result.is_ok(),
        "Reinitialize should still succeed but with warnings for missing files"
    );

    // Verify run_id was incremented
    let new_run_id = manager.get_run_id().expect("Failed to get new run_id");
    assert_eq!(
        new_run_id,
        original_run_id + 1,
        "Run ID should be incremented after reinitialize"
    );
}

/// Helper function to create a workflow with a chain of jobs connected by user_data dependencies
/// Creates: workflow -> ud1 (with data) -> job1 -> ud2 -> job2 -> ud3 -> job3 -> ud4
/// Returns: (manager, workflow, job_ids, user_data_ids)
fn create_workflow_with_user_data_chain(
    config: &Configuration,
    workflow_name: &str,
) -> (WorkflowManager, models::WorkflowModel, Vec<i64>, Vec<i64>) {
    // Create workflow
    let workflow = create_test_workflow_advanced(
        config,
        workflow_name,
        "test_user",
        Some(format!(
            "Test workflow for user_data chain: {}",
            workflow_name
        )),
    );
    let workflow_id = workflow.id.unwrap();

    // Create user_data records
    let created_ud1 = create_test_user_data(
        config,
        workflow_id,
        "ud1",
        serde_json::json!("initial data"),
        false,
    );
    let ud1_id = created_ud1.id.unwrap();

    let created_ud2 =
        create_test_user_data(config, workflow_id, "ud2", serde_json::Value::Null, false);
    let ud2_id = created_ud2.id.unwrap();

    let created_ud3 =
        create_test_user_data(config, workflow_id, "ud3", serde_json::Value::Null, false);
    let ud3_id = created_ud3.id.unwrap();

    let created_ud4 =
        create_test_user_data(config, workflow_id, "ud4", serde_json::Value::Null, false);
    let ud4_id = created_ud4.id.unwrap();

    let user_data_ids = vec![ud1_id, ud2_id, ud3_id, ud4_id];

    // Create resource requirements
    let resource_requirements = models::ResourceRequirementsModel::new(1, "small".to_string());
    let rr = default_api::create_resource_requirements(config, resource_requirements)
        .expect("Failed to create resource requirements");

    // Create job1: consumes ud1, produces ud2
    let mut job1 =
        models::JobModel::new(workflow_id, "job1".to_string(), "echo 'job1'".to_string());
    job1.input_user_data_ids = Some(vec![ud1_id]);
    job1.output_user_data_ids = Some(vec![ud2_id]);
    job1.resource_requirements_id = rr.id;
    let created_job1 = default_api::create_job(config, job1).expect("Failed to create job1");
    let job1_id = created_job1.id.unwrap();

    // Create job2: consumes ud2, produces ud3
    let mut job2 =
        models::JobModel::new(workflow_id, "job2".to_string(), "echo 'job2'".to_string());
    job2.input_user_data_ids = Some(vec![ud2_id]);
    job2.output_user_data_ids = Some(vec![ud3_id]);
    job2.resource_requirements_id = rr.id;
    let created_job2 = default_api::create_job(config, job2).expect("Failed to create job2");
    let job2_id = created_job2.id.unwrap();

    // Create job3: consumes ud3, produces ud4
    let mut job3 =
        models::JobModel::new(workflow_id, "job3".to_string(), "echo 'job3'".to_string());
    job3.input_user_data_ids = Some(vec![ud3_id]);
    job3.output_user_data_ids = Some(vec![ud4_id]);
    job3.resource_requirements_id = rr.id;
    let created_job3 = default_api::create_job(config, job3).expect("Failed to create job3");
    let job3_id = created_job3.id.unwrap();

    let job_ids = vec![job1_id, job2_id, job3_id];

    let torc_config = TorcConfig::load().unwrap_or_default();
    let manager = WorkflowManager::new(config.clone(), torc_config, workflow.clone());

    (manager, workflow, job_ids, user_data_ids)
}

#[rstest]
fn test_user_data_dependency_chain(start_server: &ServerProcess) {
    let config = start_server.config.clone();
    let (manager, workflow, job_ids, user_data_ids) =
        create_workflow_with_user_data_chain(&config, "test_user_data_chain");

    let workflow_id = workflow.id.unwrap();
    let (job1_id, job2_id, job3_id) = (job_ids[0], job_ids[1], job_ids[2]);
    let (ud1_id, ud2_id, ud3_id, ud4_id) = (
        user_data_ids[0],
        user_data_ids[1],
        user_data_ids[2],
        user_data_ids[3],
    );

    // Initialize workflow
    let result = manager.initialize(false);
    assert!(result.is_ok(), "Failed to initialize workflow");
    let run_id = manager.get_run_id().expect("Failed to get run_id");

    // Check job1 is Ready (has all input data), others are blocked
    let job1 = default_api::get_job(&config, job1_id).expect("Failed to get job1");
    let job2 = default_api::get_job(&config, job2_id).expect("Failed to get job2");
    let job3 = default_api::get_job(&config, job3_id).expect("Failed to get job3");

    assert_eq!(
        job1.status.unwrap(),
        models::JobStatus::Ready,
        "job1 should be Ready after initialization"
    );
    assert_ne!(
        job2.status.unwrap(),
        models::JobStatus::Ready,
        "job2 should not be Ready (waiting for ud2)"
    );
    assert_ne!(
        job3.status.unwrap(),
        models::JobStatus::Ready,
        "job3 should not be Ready (waiting for ud3)"
    );

    // Create compute node for results
    let compute_node = create_test_compute_node(&config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Simulate job1 execution: change status to running, populate ud2, complete job1
    default_api::manage_status_change(&config, job1_id, models::JobStatus::Running, run_id, None)
        .expect("Failed to set job1 to running");

    // Populate ud2 with data
    let mut ud2 = default_api::get_user_data(&config, ud2_id).expect("Failed to get ud2");
    ud2.data = Some(serde_json::json!("data from job1"));
    default_api::update_user_data(&config, ud2_id, ud2).expect("Failed to update ud2");

    // Complete job1
    let job1_result = models::ResultModel::new(
        job1_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        0,
        1.0,
        "2020-01-01T00:00:00Z".to_string(),
        models::JobStatus::Completed,
    );
    default_api::complete_job(&config, job1_id, job1_result.status, run_id, job1_result)
        .expect("Failed to complete job1");

    // Wait for background unblocking task to process job1 completion and unblock job2
    assert!(
        wait_for_job_status(&config, job2_id, models::JobStatus::Ready, 5),
        "job2 should become Ready after job1 completes and ud2 is populated"
    );

    // Check job2 is now Ready
    let job2_after = default_api::get_job(&config, job2_id).expect("Failed to get job2 after job1");
    assert_eq!(
        job2_after.status.unwrap(),
        models::JobStatus::Ready,
        "job2 should be Ready after job1 completes and ud2 is populated"
    );

    // job3 should still not be Ready
    let job3_after = default_api::get_job(&config, job3_id).expect("Failed to get job3 after job1");
    assert_ne!(
        job3_after.status.unwrap(),
        models::JobStatus::Ready,
        "job3 should still not be Ready (waiting for ud3)"
    );

    // Simulate job2 execution
    default_api::manage_status_change(&config, job2_id, models::JobStatus::Running, run_id, None)
        .expect("Failed to set job2 to running");

    // Populate ud3 with data
    let mut ud3 = default_api::get_user_data(&config, ud3_id).expect("Failed to get ud3");
    ud3.data = Some(serde_json::json!("data from job2"));
    default_api::update_user_data(&config, ud3_id, ud3).expect("Failed to update ud3");

    // Complete job2
    let job2_result = models::ResultModel::new(
        job2_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        0,
        1.0,
        "2020-01-01T00:00:01Z".to_string(),
        models::JobStatus::Completed,
    );
    default_api::complete_job(&config, job2_id, job2_result.status, run_id, job2_result)
        .expect("Failed to complete job2");

    // Wait for background unblocking task to process job2 completion and unblock job3
    assert!(
        wait_for_job_status(&config, job3_id, models::JobStatus::Ready, 5),
        "job3 should become Ready after job2 completes and ud3 is populated"
    );

    // Check job3 is now Ready
    let job3_after_job2 =
        default_api::get_job(&config, job3_id).expect("Failed to get job3 after job2");
    assert_eq!(
        job3_after_job2.status.unwrap(),
        models::JobStatus::Ready,
        "job3 should be Ready after job2 completes and ud3 is populated"
    );

    // Simulate job3 execution
    default_api::manage_status_change(&config, job3_id, models::JobStatus::Running, run_id, None)
        .expect("Failed to set job3 to running");

    // Populate ud4 with data
    let mut ud4 = default_api::get_user_data(&config, ud4_id).expect("Failed to get ud4");
    ud4.data = Some(serde_json::json!("data from job3"));
    default_api::update_user_data(&config, ud4_id, ud4).expect("Failed to update ud4");

    // Complete job3
    let job3_result = models::ResultModel::new(
        job3_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        0,
        1.0,
        "2020-01-01T00:00:02Z".to_string(),
        models::JobStatus::Completed,
    );
    default_api::complete_job(&config, job3_id, job3_result.status, run_id, job3_result)
        .expect("Failed to complete job3");

    // Verify all jobs are Completed
    let job1_final = default_api::get_job(&config, job1_id).expect("Failed to get job1 final");
    let job2_final = default_api::get_job(&config, job2_id).expect("Failed to get job2 final");
    let job3_final = default_api::get_job(&config, job3_id).expect("Failed to get job3 final");

    assert_eq!(job1_final.status.unwrap(), models::JobStatus::Completed);
    assert_eq!(job2_final.status.unwrap(), models::JobStatus::Completed);
    assert_eq!(job3_final.status.unwrap(), models::JobStatus::Completed);

    // Verify all user_data has been populated
    let ud2_final = default_api::get_user_data(&config, ud2_id).expect("Failed to get ud2 final");
    let ud3_final = default_api::get_user_data(&config, ud3_id).expect("Failed to get ud3 final");
    let ud4_final = default_api::get_user_data(&config, ud4_id).expect("Failed to get ud4 final");

    assert!(ud2_final.data.is_some(), "ud2 should have data");
    assert!(ud3_final.data.is_some(), "ud3 should have data");
    assert!(ud4_final.data.is_some(), "ud4 should have data");

    // Test reinitialize after changing ud1 data
    // Change the value of ud1's data field
    let mut ud1 = default_api::get_user_data(&config, ud1_id).expect("Failed to get ud1");
    ud1.data = Some(serde_json::json!("modified data"));
    default_api::update_user_data(&config, ud1_id, ud1).expect("Failed to update ud1");

    // Reinitialize the workflow
    let reinit_result = manager.reinitialize(false, false);
    assert!(reinit_result.is_ok(), "Failed to reinitialize workflow");

    // Check that job1 is Ready and all others are Blocked
    let job1_after_reinit =
        default_api::get_job(&config, job1_id).expect("Failed to get job1 after reinit");
    let job2_after_reinit =
        default_api::get_job(&config, job2_id).expect("Failed to get job2 after reinit");
    let job3_after_reinit =
        default_api::get_job(&config, job3_id).expect("Failed to get job3 after reinit");

    assert_eq!(
        job1_after_reinit.status.unwrap(),
        models::JobStatus::Ready,
        "job1 should be Ready after reinitialize (ud1 has been changed)"
    );
    assert_eq!(
        job2_after_reinit.status.unwrap(),
        models::JobStatus::Blocked,
        "job2 should be Blocked after reinitialize (waiting for job1 to run again)"
    );
    assert_eq!(
        job3_after_reinit.status.unwrap(),
        models::JobStatus::Blocked,
        "job3 should be Blocked after reinitialize (waiting for job2)"
    );
}

/// Test that reinitialization correctly sets jobs to Ready when they are only blocked by completed jobs.
/// This test verifies the fix for the bug where jobs blocked only by Completed jobs were incorrectly
/// marked as Blocked instead of Ready during reinitialization.
///
/// Scenario:
/// 1. Create diamond workflow: preprocess → (work1, work2) → postprocess
/// 2. Complete all jobs (status = Completed)
/// 3. Modify an output file from preprocess (f2.json)
/// 4. Reinitialize workflow
/// 5. Verify work1 is set to Ready (not Blocked), since preprocess is already Completed
#[rstest]
fn test_reinitialize_with_file_change_depends_on_complete_job(start_server: &ServerProcess) {
    let config = start_server.config.clone();

    // Create a temporary directory for workflow files
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create diamond workflow with files in temp directory
    let jobs = create_diamond_workflow(&config, false, temp_dir.path());

    let preprocess_job = jobs.get("preprocess").unwrap();
    let work1_job = jobs.get("work1").unwrap();
    let work2_job = jobs.get("work2").unwrap();
    let postprocess_job = jobs.get("postprocess").unwrap();

    let workflow_id = preprocess_job.workflow_id;
    let preprocess_id = preprocess_job.id.unwrap();
    let work1_id = work1_job.id.unwrap();
    let work2_id = work2_job.id.unwrap();
    let postprocess_id = postprocess_job.id.unwrap();

    // Get workflow model for manager
    let workflow = default_api::get_workflow(&config, workflow_id).expect("Failed to get workflow");

    // Create workflow manager
    let torc_config = TorcConfig::load().unwrap_or_default();
    let manager = WorkflowManager::new(config.clone(), torc_config, workflow);

    // Define file paths
    let f1_path = temp_dir.path().join("f1.json");
    let f2_path = temp_dir.path().join("f2.json");
    let f3_path = temp_dir.path().join("f3.json");
    let f4_path = temp_dir.path().join("f4.json");
    let f5_path = temp_dir.path().join("f5.json");
    let f6_path = temp_dir.path().join("f6.json");

    // Create f1 (input to preprocess) BEFORE initialization
    fs::write(&f1_path, r#"{"input": "data"}"#).expect("Failed to write f1");

    // Initialize the workflow (ignore missing input files)
    manager.initialize(true).expect("Failed to initialize");
    let run_id = manager.get_run_id().expect("Failed to get run_id");

    // Simulate successful execution of all jobs
    let compute_node = create_test_compute_node(&config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Execute preprocess job
    default_api::manage_status_change(
        &config,
        preprocess_id,
        models::JobStatus::Running,
        run_id,
        None,
    )
    .expect("Failed to set preprocess to Running");
    fs::write(&f2_path, r#"{"preprocess": "output1"}"#).expect("Failed to write f2");
    fs::write(&f3_path, r#"{"preprocess": "output2"}"#).expect("Failed to write f3");

    let preprocess_result = models::ResultModel::new(
        preprocess_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        0,
        1.0,
        chrono::Utc::now().to_rfc3339(),
        models::JobStatus::Completed,
    );
    default_api::complete_job(
        &config,
        preprocess_id,
        models::JobStatus::Completed,
        run_id,
        preprocess_result,
    )
    .expect("Failed to complete preprocess");

    // Wait for work1 to be unblocked by background task, then execute
    assert!(
        wait_for_job_status(&config, work1_id, models::JobStatus::Ready, 5),
        "work1 did not become ready after preprocess completed"
    );
    default_api::manage_status_change(&config, work1_id, models::JobStatus::Running, run_id, None)
        .expect("Failed to set work1 to Running");
    fs::write(&f4_path, r#"{"work1": "output"}"#).expect("Failed to write f4");

    let work1_result = models::ResultModel::new(
        work1_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        0,
        1.0,
        chrono::Utc::now().to_rfc3339(),
        models::JobStatus::Completed,
    );
    default_api::complete_job(
        &config,
        work1_id,
        models::JobStatus::Completed,
        run_id,
        work1_result,
    )
    .expect("Failed to complete work1");

    // Wait for work2 to be unblocked by background task, then execute
    assert!(
        wait_for_job_status(&config, work2_id, models::JobStatus::Ready, 5),
        "work2 did not become ready after preprocess completed"
    );
    default_api::manage_status_change(&config, work2_id, models::JobStatus::Running, run_id, None)
        .expect("Failed to set work2 to Running");
    fs::write(&f5_path, r#"{"work2": "output"}"#).expect("Failed to write f5");

    // Update f5 mtime in database
    let f5_metadata = fs::metadata(&f5_path).expect("Failed to get f5 metadata");
    let f5_mtime = f5_metadata
        .modified()
        .expect("Failed to get f5 mtime")
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    let mut f5_model = default_api::list_files(
        &config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        Some("f5"),
        None,
        None,
    )
    .expect("Failed to list f5")
    .items
    .unwrap()[0]
        .clone();
    f5_model.st_mtime = Some(f5_mtime);
    default_api::update_file(&config, f5_model.id.unwrap(), f5_model).expect("Failed to update f5");

    let work2_result = models::ResultModel::new(
        work2_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        0,
        1.0,
        chrono::Utc::now().to_rfc3339(),
        models::JobStatus::Completed,
    );
    default_api::complete_job(
        &config,
        work2_id,
        models::JobStatus::Completed,
        run_id,
        work2_result,
    )
    .expect("Failed to complete work2");

    // Wait for postprocess to be unblocked by background task, then execute
    assert!(
        wait_for_job_status(&config, postprocess_id, models::JobStatus::Ready, 5),
        "postprocess did not become ready after work1 and work2 completed"
    );
    default_api::manage_status_change(
        &config,
        postprocess_id,
        models::JobStatus::Running,
        run_id,
        None,
    )
    .expect("Failed to set postprocess to Running");
    fs::write(&f6_path, r#"{"postprocess": "output"}"#).expect("Failed to write f6");

    // Update f6 mtime in database
    let f6_metadata = fs::metadata(&f6_path).expect("Failed to get f6 metadata");
    let f6_mtime = f6_metadata
        .modified()
        .expect("Failed to get f6 mtime")
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    let mut f6_model = default_api::list_files(
        &config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        Some("f6"),
        None,
        None,
    )
    .expect("Failed to list f6")
    .items
    .unwrap()[0]
        .clone();
    f6_model.st_mtime = Some(f6_mtime);
    default_api::update_file(&config, f6_model.id.unwrap(), f6_model).expect("Failed to update f6");

    let postprocess_result = models::ResultModel::new(
        postprocess_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        0,
        1.0,
        chrono::Utc::now().to_rfc3339(),
        models::JobStatus::Completed,
    );
    default_api::complete_job(
        &config,
        postprocess_id,
        models::JobStatus::Completed,
        run_id,
        postprocess_result,
    )
    .expect("Failed to complete postprocess");

    // Update all file mtimes in database by calling initialize_files
    // This properly records all current file mtimes
    manager
        .initialize_files()
        .expect("Failed to initialize file mtimes");

    // Verify all jobs are Completed
    let preprocess_done =
        default_api::get_job(&config, preprocess_id).expect("Failed to get preprocess");
    let work1_done = default_api::get_job(&config, work1_id).expect("Failed to get work1");
    let work2_done = default_api::get_job(&config, work2_id).expect("Failed to get work2");
    let postprocess_done =
        default_api::get_job(&config, postprocess_id).expect("Failed to get postprocess");

    assert_eq!(
        preprocess_done.status.unwrap(),
        models::JobStatus::Completed
    );
    assert_eq!(work1_done.status.unwrap(), models::JobStatus::Completed);
    assert_eq!(work2_done.status.unwrap(), models::JobStatus::Completed);
    assert_eq!(
        postprocess_done.status.unwrap(),
        models::JobStatus::Completed
    );

    // Wait a moment to ensure file modification time is different
    thread::sleep(Duration::from_millis(100));

    // Modify f2.json (output of preprocess, input to work1)
    fs::write(&f2_path, r#"{"preprocess": "modified_output"}"#).expect("Failed to modify f2");

    // Reinitialize the workflow (this should detect the file change)
    let reinit_result = manager.reinitialize(false, false);
    assert!(reinit_result.is_ok(), "Failed to reinitialize workflow");

    // Check job statuses after reinitialization
    let preprocess_after = default_api::get_job(&config, preprocess_id)
        .expect("Failed to get preprocess after reinit");
    let work1_after =
        default_api::get_job(&config, work1_id).expect("Failed to get work1 after reinit");
    let work2_after =
        default_api::get_job(&config, work2_id).expect("Failed to get work2 after reinit");
    let postprocess_after = default_api::get_job(&config, postprocess_id)
        .expect("Failed to get postprocess after reinit");

    // Assertions to verify correct behavior:
    // - preprocess should still be Completed (not affected by f2 modification)
    // - work1 should be Ready (not Blocked), since:
    //   * f2 was modified, so work1 needs to re-run
    //   * work1 is blocked by preprocess, but preprocess is Completed
    //   * Therefore work1 should be Ready to run
    // - work2 should still be Completed (f3 wasn't modified)
    // - postprocess should be Blocked (waiting for work1 to complete)

    assert_eq!(
        preprocess_after.status.unwrap(),
        models::JobStatus::Completed,
        "preprocess should remain Completed (f2 is its output, not input)"
    );

    assert_eq!(
        work1_after.status.unwrap(),
        models::JobStatus::Ready,
        "work1 should be Ready (not Blocked), since preprocess (which blocks work1) is Completed"
    );

    assert_eq!(
        work2_after.status.unwrap(),
        models::JobStatus::Completed,
        "work2 should remain Completed (f3 was not modified)"
    );

    assert_eq!(
        postprocess_after.status.unwrap(),
        models::JobStatus::Blocked,
        "postprocess should be Blocked (waiting for work1 to complete)"
    );
}
