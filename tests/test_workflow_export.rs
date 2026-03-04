//! Integration tests for workflow export/import functionality.
//!
//! Tests cover:
//! - Export and import of workflows with various relationship types
//! - Job-job dependencies (depends_on)
//! - Job-file dependencies (input_files, output_files)
//! - Job-user_data dependencies (input_user_data, output_user_data)
//! - Optional results and events export/import
//! - ID remapping during import
//! - Status reset on import

mod common;

use common::{ServerProcess, run_cli_with_json, start_server};
use rstest::rstest;
use serde_json::Value;
use std::fs;
use tempfile::NamedTempFile;
use torc::client::apis::default_api;
use torc::models::{FileModel, JobModel, JobStatus, UserDataModel, WorkflowModel};

/// Helper to create a test workflow with job-job, job-file, and job-user_data dependencies
fn create_test_workflow_with_dependencies(
    config: &torc::client::apis::configuration::Configuration,
    name: &str,
    user: &str,
) -> (i64, Vec<i64>, Vec<i64>, Vec<i64>) {
    // Create workflow
    let workflow = WorkflowModel::new(name.to_string(), user.to_string());
    let created_workflow =
        default_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create files
    let input_file = FileModel::new(
        workflow_id,
        "input.txt".to_string(),
        "/tmp/input.txt".to_string(),
    );
    let created_input_file =
        default_api::create_file(config, input_file).expect("Failed to create input file");
    let input_file_id = created_input_file.id.unwrap();

    let intermediate_file = FileModel::new(
        workflow_id,
        "intermediate.txt".to_string(),
        "/tmp/intermediate.txt".to_string(),
    );
    let created_intermediate_file = default_api::create_file(config, intermediate_file)
        .expect("Failed to create intermediate file");
    let intermediate_file_id = created_intermediate_file.id.unwrap();

    let output_file = FileModel::new(
        workflow_id,
        "output.txt".to_string(),
        "/tmp/output.txt".to_string(),
    );
    let created_output_file =
        default_api::create_file(config, output_file).expect("Failed to create output file");
    let output_file_id = created_output_file.id.unwrap();

    // Create user_data
    let mut config_data = UserDataModel::new(workflow_id, "config".to_string());
    config_data.data = Some(serde_json::json!({"setting": "value"}));
    let created_config_data = default_api::create_user_data(config, config_data, None, None)
        .expect("Failed to create config user_data");
    let config_data_id = created_config_data.id.unwrap();

    let mut result_data = UserDataModel::new(workflow_id, "result".to_string());
    result_data.data = Some(serde_json::json!({"result": "pending"}));
    let created_result_data = default_api::create_user_data(config, result_data, None, None)
        .expect("Failed to create result user_data");
    let result_data_id = created_result_data.id.unwrap();

    // Create jobs with dependencies
    // Job 1: reads input file, reads config, produces intermediate file
    let mut job1 = JobModel::new(
        workflow_id,
        "process_input".to_string(),
        "cat input.txt > intermediate.txt".to_string(),
    );
    job1.input_file_ids = Some(vec![input_file_id]);
    job1.output_file_ids = Some(vec![intermediate_file_id]);
    job1.input_user_data_ids = Some(vec![config_data_id]);
    let created_job1 = default_api::create_job(config, job1).expect("Failed to create job1");
    let job1_id = created_job1.id.unwrap();

    // Job 2: depends on job1, reads intermediate file, produces output file and result data
    let mut job2 = JobModel::new(
        workflow_id,
        "process_output".to_string(),
        "cat intermediate.txt > output.txt".to_string(),
    );
    job2.input_file_ids = Some(vec![intermediate_file_id]);
    job2.output_file_ids = Some(vec![output_file_id]);
    job2.output_user_data_ids = Some(vec![result_data_id]);
    let created_job2 = default_api::create_job(config, job2).expect("Failed to create job2");
    let job2_id = created_job2.id.unwrap();

    // Job 3: depends on job2 (explicit dependency), no file dependencies
    let job3 = JobModel::new(workflow_id, "finalize".to_string(), "echo done".to_string());
    let created_job3 = default_api::create_job(config, job3).expect("Failed to create job3");
    let job3_id = created_job3.id.unwrap();

    // Add explicit dependency: job3 depends on job2
    // Note: We must preserve name/command when updating to avoid overwriting with empty strings
    let mut update_job3 =
        JobModel::new(workflow_id, "finalize".to_string(), "echo done".to_string());
    update_job3.depends_on_job_ids = Some(vec![job2_id]);
    default_api::update_job(config, job3_id, update_job3)
        .expect("Failed to update job3 dependencies");

    (
        workflow_id,
        vec![job1_id, job2_id, job3_id],
        vec![input_file_id, intermediate_file_id, output_file_id],
        vec![config_data_id, result_data_id],
    )
}

#[rstest]
fn test_export_import_basic(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow with dependencies
    let (workflow_id, _job_ids, _file_ids, _user_data_ids) =
        create_test_workflow_with_dependencies(config, "export_test", "test_user");

    // Export the workflow
    let export_file = NamedTempFile::new().expect("Failed to create temp file");
    let export_path = export_file.path().to_str().unwrap();

    let args = [
        "workflows",
        "export",
        &workflow_id.to_string(),
        "-o",
        export_path,
    ];
    run_cli_with_json(&args, start_server, Some("test_user")).expect("Failed to export workflow");

    // Verify export file exists and has content
    let export_content = fs::read_to_string(export_path).expect("Failed to read export file");
    let export_json: Value =
        serde_json::from_str(&export_content).expect("Failed to parse export JSON");

    assert_eq!(export_json["export_version"], "1.0");
    assert_eq!(export_json["workflow"]["name"], "export_test");
    assert_eq!(export_json["jobs"].as_array().unwrap().len(), 3);
    assert_eq!(export_json["files"].as_array().unwrap().len(), 3);
    assert_eq!(export_json["user_data"].as_array().unwrap().len(), 2);

    // Import the workflow with a new name
    let args = [
        "workflows",
        "import",
        export_path,
        "--name",
        "imported_workflow",
    ];
    let import_result = run_cli_with_json(&args, start_server, Some("test_user"))
        .expect("Failed to import workflow");

    let new_workflow_id = import_result["workflow_id"].as_i64().unwrap();
    assert!(new_workflow_id > 0);
    assert_ne!(new_workflow_id, workflow_id);

    // Verify the imported workflow
    let imported_workflow = default_api::get_workflow(config, new_workflow_id)
        .expect("Failed to get imported workflow");
    assert_eq!(imported_workflow.name, "imported_workflow");
    assert_eq!(imported_workflow.user, "test_user");

    // Verify jobs were imported with correct relationships
    let jobs_response = default_api::list_jobs(
        config,
        new_workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(true),
        None,
    )
    .expect("Failed to list imported jobs");
    let imported_jobs = jobs_response.items.unwrap();
    assert_eq!(imported_jobs.len(), 3);

    // Find job by name and verify its relationships
    let process_input_job = imported_jobs
        .iter()
        .find(|j| j.name == "process_input")
        .unwrap();
    assert!(process_input_job.input_file_ids.as_ref().unwrap().len() == 1);
    assert!(process_input_job.output_file_ids.as_ref().unwrap().len() == 1);
    assert!(
        process_input_job
            .input_user_data_ids
            .as_ref()
            .unwrap()
            .len()
            == 1
    );

    let finalize_job = imported_jobs.iter().find(|j| j.name == "finalize").unwrap();
    assert!(finalize_job.depends_on_job_ids.as_ref().unwrap().len() == 1);

    // Verify all jobs have uninitialized status (default behavior resets status)
    for job in &imported_jobs {
        assert_eq!(job.status, Some(JobStatus::Uninitialized));
    }

    // Verify files were imported
    let files_response = default_api::list_files(
        config,
        new_workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list imported files");
    let imported_files = files_response.items.unwrap();
    assert_eq!(imported_files.len(), 3);

    // Verify user_data was imported
    let user_data_response = default_api::list_user_data(
        config,
        new_workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list imported user_data");
    let imported_user_data = user_data_response.items.unwrap();
    assert_eq!(imported_user_data.len(), 2);
}

// Note: test_export_import_preserve_status is not implemented because the server
// does not allow updating job status through update_job (except to Disabled).
// Job status is computed by the server based on dependencies, so --preserve-status
// cannot work as designed. On import, all jobs start with Uninitialized status
// and must be re-initialized to compute their correct status based on dependencies.

#[rstest]
fn test_export_with_results(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a simple workflow
    let workflow = WorkflowModel::new("results_test".to_string(), "test_user".to_string());
    let created_workflow =
        default_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    let job = JobModel::new(
        workflow_id,
        "test_job".to_string(),
        "echo hello".to_string(),
    );
    let created_job = default_api::create_job(config, job).expect("Failed to create job");
    let job_id = created_job.id.unwrap();

    // We need a compute node to create results
    let compute_node = torc::models::ComputeNodeModel::new(
        workflow_id,
        "localhost".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        1,                   // num_cpus
        8.0,                 // memory_gb
        0,                   // num_gpus
        1,                   // num_nodes
        "local".to_string(), // compute_node_type
        None,                // scheduler
    );
    let created_compute_node = default_api::create_compute_node(config, compute_node)
        .expect("Failed to create compute node");
    let compute_node_id = created_compute_node.id.unwrap();

    // Create a result for the job
    let result = torc::models::ResultModel::new(
        job_id,
        workflow_id,
        1, // run_id
        1, // attempt_id
        compute_node_id,
        0,   // return_code
        1.5, // exec_time_minutes
        "2024-01-01T00:00:00Z".to_string(),
        JobStatus::Completed,
    );
    default_api::create_result(config, result).expect("Failed to create result");

    // Export with results
    let export_file = NamedTempFile::new().expect("Failed to create temp file");
    let export_path = export_file.path().to_str().unwrap();

    let args = [
        "workflows",
        "export",
        &workflow_id.to_string(),
        "-o",
        export_path,
        "--include-results",
    ];
    run_cli_with_json(&args, start_server, Some("test_user"))
        .expect("Failed to export workflow with results");

    // Verify export contains results and compute nodes
    let export_content = fs::read_to_string(export_path).expect("Failed to read export file");
    let export_json: Value =
        serde_json::from_str(&export_content).expect("Failed to parse export JSON");

    assert!(export_json["results"].is_array());
    let exported_results = export_json["results"].as_array().unwrap();
    assert_eq!(exported_results.len(), 1);

    assert!(export_json["compute_nodes"].is_array());
    let exported_nodes = export_json["compute_nodes"].as_array().unwrap();
    assert_eq!(exported_nodes.len(), 1);

    // Import the exported workflow and verify results round-trip
    let args = [
        "workflows",
        "import",
        export_path,
        "--name",
        "imported_with_results",
    ];
    let import_result = run_cli_with_json(&args, start_server, Some("test_user"))
        .expect("Failed to import workflow with results");

    let new_workflow_id = import_result["workflow_id"].as_i64().unwrap();
    assert_ne!(new_workflow_id, workflow_id);

    // Verify results were imported into the new workflow
    let results_response = default_api::list_results(
        config,
        new_workflow_id,
        None, // job_id
        None, // run_id
        None, // offset
        None, // limit
        None, // sort_by
        None, // reverse_sort
        None, // return_code
        None, // status
        None, // all_runs
        None, // compute_node_id
    )
    .expect("Failed to list imported results");

    let imported_results = results_response.items.unwrap();
    assert_eq!(imported_results.len(), 1, "Expected 1 result after import");

    let imported_result = &imported_results[0];
    assert_eq!(imported_result.return_code, 0);
    assert_eq!(imported_result.status, JobStatus::Completed);
    assert!((imported_result.exec_time_minutes - 1.5).abs() < 0.01);

    // Verify IDs were remapped (not the same as originals)
    assert_ne!(imported_result.workflow_id, workflow_id);
    assert_eq!(imported_result.workflow_id, new_workflow_id);
    assert_ne!(imported_result.job_id, job_id);
    assert_ne!(imported_result.compute_node_id, compute_node_id);
}

/// Test importing an old export file that has results but no compute_nodes section.
/// The import should create a placeholder compute node so results are still preserved.
#[rstest]
fn test_import_results_without_compute_nodes(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow with a job, compute node, and result
    let workflow = WorkflowModel::new("old_export_test".to_string(), "test_user".to_string());
    let created_workflow =
        default_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    let job = JobModel::new(
        workflow_id,
        "test_job".to_string(),
        "echo hello".to_string(),
    );
    let created_job = default_api::create_job(config, job).expect("Failed to create job");
    let job_id = created_job.id.unwrap();

    let compute_node = torc::models::ComputeNodeModel::new(
        workflow_id,
        "localhost".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        4,
        16.0,
        0,
        1,
        "local".to_string(),
        None,
    );
    let created_cn = default_api::create_compute_node(config, compute_node)
        .expect("Failed to create compute node");
    let cn_id = created_cn.id.unwrap();

    let result = torc::models::ResultModel::new(
        job_id,
        workflow_id,
        1,
        1,
        cn_id,
        0,
        2.5,
        "2024-06-01T12:00:00Z".to_string(),
        JobStatus::Completed,
    );
    default_api::create_result(config, result).expect("Failed to create result");

    // Export with results, then strip the compute_nodes field to simulate an old export
    let export_file = NamedTempFile::new().expect("Failed to create temp file");
    let export_path = export_file.path().to_str().unwrap();

    let args = [
        "workflows",
        "export",
        &workflow_id.to_string(),
        "-o",
        export_path,
        "--include-results",
    ];
    run_cli_with_json(&args, start_server, Some("test_user")).expect("Failed to export workflow");

    // Remove compute_nodes from the export to simulate an old format file
    let export_content = fs::read_to_string(export_path).expect("Failed to read export file");
    let mut export_json: Value =
        serde_json::from_str(&export_content).expect("Failed to parse export JSON");
    export_json.as_object_mut().unwrap().remove("compute_nodes");
    fs::write(
        export_path,
        serde_json::to_string_pretty(&export_json).unwrap(),
    )
    .expect("Failed to write modified export");

    // Import - should succeed and create results using a placeholder compute node
    let args = [
        "workflows",
        "import",
        export_path,
        "--name",
        "imported_old_format",
    ];
    let import_result = run_cli_with_json(&args, start_server, Some("test_user"))
        .expect("Failed to import workflow without compute_nodes");

    let new_workflow_id = import_result["workflow_id"].as_i64().unwrap();

    // Verify results were imported
    let results_response = default_api::list_results(
        config,
        new_workflow_id,
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
    .expect("Failed to list imported results");

    let imported_results = results_response.items.unwrap();
    assert_eq!(
        imported_results.len(),
        1,
        "Expected 1 result from old-format import"
    );
    assert_eq!(imported_results[0].return_code, 0);
    assert!((imported_results[0].exec_time_minutes - 2.5).abs() < 0.01);
}

#[rstest]
fn test_export_with_events(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a simple workflow
    let workflow = WorkflowModel::new("events_test".to_string(), "test_user".to_string());
    let created_workflow =
        default_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create an event
    let event = torc::models::EventModel::new(
        workflow_id,
        serde_json::json!({"type": "test_event", "message": "test"}),
    );
    default_api::create_event(config, event).expect("Failed to create event");

    // Export with events
    let export_file = NamedTempFile::new().expect("Failed to create temp file");
    let export_path = export_file.path().to_str().unwrap();

    let args = [
        "workflows",
        "export",
        &workflow_id.to_string(),
        "-o",
        export_path,
        "--include-events",
    ];
    run_cli_with_json(&args, start_server, Some("test_user"))
        .expect("Failed to export workflow with events");

    // Verify export contains events
    let export_content = fs::read_to_string(export_path).expect("Failed to read export file");
    let export_json: Value =
        serde_json::from_str(&export_content).expect("Failed to parse export JSON");

    assert!(export_json["events"].is_array());
    assert!(!export_json["events"].as_array().unwrap().is_empty());
}

#[rstest]
fn test_export_without_results_or_events(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow with an event
    let workflow = WorkflowModel::new("no_extras_test".to_string(), "test_user".to_string());
    let created_workflow =
        default_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create an event
    let event = torc::models::EventModel::new(workflow_id, serde_json::json!({"type": "test"}));
    default_api::create_event(config, event).expect("Failed to create event");

    // Export without --include-results or --include-events (default)
    let export_file = NamedTempFile::new().expect("Failed to create temp file");
    let export_path = export_file.path().to_str().unwrap();

    let args = [
        "workflows",
        "export",
        &workflow_id.to_string(),
        "-o",
        export_path,
    ];
    run_cli_with_json(&args, start_server, Some("test_user")).expect("Failed to export workflow");

    // Verify export does NOT contain results or events
    let export_content = fs::read_to_string(export_path).expect("Failed to read export file");
    let export_json: Value =
        serde_json::from_str(&export_content).expect("Failed to parse export JSON");

    assert!(export_json.get("results").is_none() || export_json["results"].is_null());
    assert!(export_json.get("events").is_none() || export_json["events"].is_null());
}

#[rstest]
fn test_import_id_remapping(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create first workflow
    let (workflow_id, _, _, _) =
        create_test_workflow_with_dependencies(config, "remap_test", "test_user");

    // Export
    let export_file = NamedTempFile::new().expect("Failed to create temp file");
    let export_path = export_file.path().to_str().unwrap();

    let args = [
        "workflows",
        "export",
        &workflow_id.to_string(),
        "-o",
        export_path,
    ];
    run_cli_with_json(&args, start_server, Some("test_user")).expect("Failed to export workflow");

    // Import twice to verify IDs are unique
    let args1 = ["workflows", "import", export_path, "--name", "import_1"];
    let import1 = run_cli_with_json(&args1, start_server, Some("test_user"))
        .expect("Failed to import workflow 1");

    let args2 = ["workflows", "import", export_path, "--name", "import_2"];
    let import2 = run_cli_with_json(&args2, start_server, Some("test_user"))
        .expect("Failed to import workflow 2");

    let workflow_id1 = import1["workflow_id"].as_i64().unwrap();
    let workflow_id2 = import2["workflow_id"].as_i64().unwrap();

    // Verify different workflow IDs
    assert_ne!(workflow_id1, workflow_id2);

    // Verify each workflow has its own set of jobs, files, etc.
    let jobs1 = default_api::list_jobs(
        config,
        workflow_id1,
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
    .expect("Failed to list jobs 1")
    .items
    .unwrap();

    let jobs2 = default_api::list_jobs(
        config,
        workflow_id2,
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
    .expect("Failed to list jobs 2")
    .items
    .unwrap();

    // Job IDs should all be different
    let job_ids1: Vec<i64> = jobs1.iter().map(|j| j.id.unwrap()).collect();
    let job_ids2: Vec<i64> = jobs2.iter().map(|j| j.id.unwrap()).collect();

    for id1 in &job_ids1 {
        assert!(!job_ids2.contains(id1), "Job IDs should not overlap");
    }

    // But job relationships should be internally consistent
    // (e.g., finalize job in import_2 should depend on a job in import_2, not import_1)
    let finalize2 = jobs2.iter().find(|j| j.name == "finalize").unwrap();
    if let Some(ref deps) = finalize2.depends_on_job_ids {
        for dep_id in deps {
            assert!(
                job_ids2.contains(dep_id),
                "Job dependency should reference job in same workflow"
            );
        }
    }
}

/// Test that RO-Crate entities with job ID references are correctly remapped during import.
///
/// This tests:
/// - CreateAction entity_id (e.g., #job-42-attempt-1) is remapped to new job ID
/// - File entity metadata with wasGeneratedBy is remapped to new job ID
/// - file_id references in entities are also remapped
#[rstest]
fn test_export_import_ro_crate_job_id_remapping(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow with a job and files
    let workflow = WorkflowModel::new("ro_crate_remap_test".to_string(), "test_user".to_string());
    let created_workflow =
        default_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create an output file
    let output_file = FileModel::new(
        workflow_id,
        "output.csv".to_string(),
        "data/output.csv".to_string(),
    );
    let created_file =
        default_api::create_file(config, output_file).expect("Failed to create file");
    let file_id = created_file.id.unwrap();

    // Create a job
    let job = JobModel::new(
        workflow_id,
        "process_data".to_string(),
        "python process.py".to_string(),
    );
    let created_job = default_api::create_job(config, job).expect("Failed to create job");
    let job_id = created_job.id.unwrap();

    // Create RO-Crate entities with job ID references

    // 1. CreateAction entity with job ID in entity_id
    let create_action_entity_id = format!("#job-{}-attempt-1", job_id);
    let create_action_metadata = serde_json::json!({
        "@id": create_action_entity_id,
        "@type": "CreateAction",
        "name": "process_data",
        "instrument": { "@id": format!("#workflow-{}", workflow_id) },
        "result": [{ "@id": "data/output.csv" }]
    });
    let create_action = torc::models::RoCrateEntityModel {
        id: None,
        workflow_id,
        file_id: None,
        entity_id: create_action_entity_id.clone(),
        entity_type: "CreateAction".to_string(),
        metadata: create_action_metadata.to_string(),
    };
    default_api::create_ro_crate_entity(config, create_action)
        .expect("Failed to create CreateAction entity");

    // 2. File entity with wasGeneratedBy reference to job
    let file_metadata = serde_json::json!({
        "@id": "data/output.csv",
        "@type": "File",
        "name": "output.csv",
        "encodingFormat": "text/csv",
        "wasGeneratedBy": { "@id": format!("#job-{}-attempt-1", job_id) }
    });
    let file_entity = torc::models::RoCrateEntityModel {
        id: None,
        workflow_id,
        file_id: Some(file_id),
        entity_id: "data/output.csv".to_string(),
        entity_type: "File".to_string(),
        metadata: file_metadata.to_string(),
    };
    default_api::create_ro_crate_entity(config, file_entity).expect("Failed to create File entity");

    // Export the workflow
    let export_file = NamedTempFile::new().expect("Failed to create temp file");
    let export_path = export_file.path().to_str().unwrap();

    let args = [
        "workflows",
        "export",
        &workflow_id.to_string(),
        "-o",
        export_path,
    ];
    run_cli_with_json(&args, start_server, Some("test_user")).expect("Failed to export workflow");

    // Verify export contains RO-Crate entities
    let export_content = fs::read_to_string(export_path).expect("Failed to read export file");
    let export_json: Value =
        serde_json::from_str(&export_content).expect("Failed to parse export JSON");
    assert_eq!(
        export_json["ro_crate_entities"].as_array().unwrap().len(),
        2
    );

    // Import the workflow
    let args = [
        "workflows",
        "import",
        export_path,
        "--name",
        "imported_ro_crate",
    ];
    let import_result = run_cli_with_json(&args, start_server, Some("test_user"))
        .expect("Failed to import workflow");

    let new_workflow_id = import_result["workflow_id"].as_i64().unwrap();
    assert_ne!(new_workflow_id, workflow_id);

    // Get the new job ID
    let jobs_response = default_api::list_jobs(
        config,
        new_workflow_id,
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
    .expect("Failed to list imported jobs");
    let imported_jobs = jobs_response.items.unwrap();
    assert_eq!(imported_jobs.len(), 1);
    let new_job_id = imported_jobs[0].id.unwrap();
    assert_ne!(new_job_id, job_id, "New job should have different ID");

    // Get the imported RO-Crate entities
    let entities_response =
        default_api::list_ro_crate_entities(config, new_workflow_id, None, None)
            .expect("Failed to list RO-Crate entities");
    let imported_entities = entities_response.items.unwrap();
    assert_eq!(imported_entities.len(), 2);

    // Find the CreateAction entity and verify its entity_id was remapped
    let create_action_entity = imported_entities
        .iter()
        .find(|e| e.entity_type == "CreateAction")
        .expect("CreateAction entity should exist");

    let expected_new_entity_id = format!("#job-{}-attempt-1", new_job_id);
    assert_eq!(
        create_action_entity.entity_id, expected_new_entity_id,
        "CreateAction entity_id should be remapped to new job ID"
    );

    // Verify the CreateAction metadata also has the new job ID
    let ca_metadata: Value = serde_json::from_str(&create_action_entity.metadata)
        .expect("Failed to parse CreateAction metadata");
    assert_eq!(
        ca_metadata["@id"], expected_new_entity_id,
        "CreateAction @id in metadata should be remapped"
    );

    // Find the File entity and verify its wasGeneratedBy was remapped
    let file_entity = imported_entities
        .iter()
        .find(|e| e.entity_type == "File")
        .expect("File entity should exist");

    let file_metadata: Value =
        serde_json::from_str(&file_entity.metadata).expect("Failed to parse File metadata");
    assert_eq!(
        file_metadata["wasGeneratedBy"]["@id"], expected_new_entity_id,
        "File wasGeneratedBy should reference the new job ID"
    );

    // Verify file_id was also remapped
    let new_files_response = default_api::list_files(
        config,
        new_workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list imported files");
    let new_files = new_files_response.items.unwrap();
    let new_file_id = new_files[0].id.unwrap();
    assert_ne!(new_file_id, file_id, "New file should have different ID");
    assert_eq!(
        file_entity.file_id,
        Some(new_file_id),
        "File entity file_id should be remapped"
    );
}
