//! Integration tests for automatic RO-Crate entity generation.
//!
//! These tests verify that when `enable_ro_crate: true` is set on a workflow:
//! - Input files get RO-Crate entities created during initialization
//! - Output files get RO-Crate entities created when jobs complete
//! - CreateAction entities are created for job provenance

mod common;

use common::{ServerProcess, run_jobs_cli_command, start_server};
use rstest::rstest;
use std::fs;
use std::path::Path;
use torc::client::default_api;
use torc::models;

/// Create a simple workflow with enable_ro_crate enabled.
/// Returns (workflow_id, input_file_id, output_file_id, job_id)
fn create_ro_crate_enabled_workflow(
    config: &torc::client::Configuration,
    work_dir: &Path,
) -> (i64, i64, i64, i64) {
    // Create workflow with enable_ro_crate: true
    let mut workflow = models::WorkflowModel::new(
        "test_auto_ro_crate_workflow".to_string(),
        "test_user".to_string(),
    );
    workflow.enable_ro_crate = Some(true);

    let created_workflow =
        default_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Verify enable_ro_crate is set
    assert_eq!(created_workflow.enable_ro_crate, Some(true));

    // Create a compute node for job execution
    let compute_node = models::ComputeNodeModel::new(
        workflow_id,
        "test-host".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        4,                   // num_cpus
        8.0,                 // memory_gb
        0,                   // num_gpus
        1,                   // num_nodes
        "local".to_string(), // compute_node_type
        None,
    );
    default_api::create_compute_node(config, compute_node).expect("Failed to create compute node");

    // Create file paths
    let input_path = work_dir.join("input.json").to_string_lossy().to_string();
    let output_path = work_dir.join("output.json").to_string_lossy().to_string();

    // Create input file record (with st_mtime set to indicate it exists)
    let mut input_file =
        models::FileModel::new(workflow_id, "input".to_string(), input_path.clone());
    input_file.st_mtime = Some(1704067200.0); // 2024-01-01T00:00:00Z - indicates file exists

    let created_input =
        default_api::create_file(config, input_file).expect("Failed to create input file");
    let input_file_id = created_input.id.unwrap();

    // Create output file record (st_mtime is None - will be created by job)
    let output_file =
        models::FileModel::new(workflow_id, "output".to_string(), output_path.clone());
    let created_output =
        default_api::create_file(config, output_file).expect("Failed to create output file");
    let output_file_id = created_output.id.unwrap();

    // Create a job that reads input and writes output
    let mut job = models::JobModel::new(
        workflow_id,
        "process".to_string(),
        format!(
            "cat {} | sed 's/input/output/' > {}",
            input_path, output_path
        ),
    );
    // Set input and output file IDs directly on the job
    job.input_file_ids = Some(vec![input_file_id]);
    job.output_file_ids = Some(vec![output_file_id]);

    let created_job = default_api::create_job(config, job).expect("Failed to create job");
    let job_id = created_job.id.unwrap();

    (workflow_id, input_file_id, output_file_id, job_id)
}

/// Create a diamond workflow with enable_ro_crate enabled.
/// This tests multiple input/output files and job provenance.
fn create_diamond_ro_crate_workflow(
    config: &torc::client::Configuration,
    work_dir: &Path,
) -> (i64, Vec<i64>, Vec<i64>) {
    // Create workflow with enable_ro_crate: true
    let mut workflow = models::WorkflowModel::new(
        "test_diamond_ro_crate_workflow".to_string(),
        "test_user".to_string(),
    );
    workflow.enable_ro_crate = Some(true);

    let created_workflow =
        default_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create a compute node
    let compute_node = models::ComputeNodeModel::new(
        workflow_id,
        "test-host".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        4,
        8.0,
        0,
        1,
        "local".to_string(),
        None,
    );
    default_api::create_compute_node(config, compute_node).expect("Failed to create compute node");

    // File paths
    let f1_path = work_dir.join("f1.json").to_string_lossy().to_string();
    let f2_path = work_dir.join("f2.json").to_string_lossy().to_string();
    let f3_path = work_dir.join("f3.json").to_string_lossy().to_string();
    let f4_path = work_dir.join("f4.json").to_string_lossy().to_string();

    // Create files: f1 is input, f2/f3 are intermediate, f4 is final output
    let mut f1_model = models::FileModel::new(workflow_id, "f1".to_string(), f1_path.clone());
    f1_model.st_mtime = Some(1704067200.0); // Input file exists before workflow runs
    let f1 = default_api::create_file(config, f1_model).expect("Failed to create f1");

    let f2 = default_api::create_file(
        config,
        models::FileModel::new(workflow_id, "f2".to_string(), f2_path.clone()),
    )
    .expect("Failed to create f2");

    let f3 = default_api::create_file(
        config,
        models::FileModel::new(workflow_id, "f3".to_string(), f3_path.clone()),
    )
    .expect("Failed to create f3");

    let f4 = default_api::create_file(
        config,
        models::FileModel::new(workflow_id, "f4".to_string(), f4_path.clone()),
    )
    .expect("Failed to create f4");

    let input_file_ids = vec![f1.id.unwrap()];
    let output_file_ids = vec![f2.id.unwrap(), f3.id.unwrap(), f4.id.unwrap()];

    // Job 1: f1 -> f2, f3
    let mut job1 = models::JobModel::new(
        workflow_id,
        "split".to_string(),
        format!(
            "cat {} > {} && cat {} > {}",
            f1_path, f2_path, f1_path, f3_path
        ),
    );
    job1.input_file_ids = Some(vec![f1.id.unwrap()]);
    job1.output_file_ids = Some(vec![f2.id.unwrap(), f3.id.unwrap()]);

    let created_job1 = default_api::create_job(config, job1).expect("Failed to create job1");
    let _job1_id = created_job1.id.unwrap();

    // Job 2: f2, f3 -> f4
    let mut job2 = models::JobModel::new(
        workflow_id,
        "merge".to_string(),
        format!("cat {} {} > {}", f2_path, f3_path, f4_path),
    );
    job2.input_file_ids = Some(vec![f2.id.unwrap(), f3.id.unwrap()]);
    job2.output_file_ids = Some(vec![f4.id.unwrap()]);

    default_api::create_job(config, job2).expect("Failed to create job2");

    (workflow_id, input_file_ids, output_file_ids)
}

#[rstest]
fn test_auto_ro_crate_input_files_on_initialize(start_server: &ServerProcess) {
    let config = &start_server.config;
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    let (workflow_id, input_file_id, _output_file_id, _job_id) =
        create_ro_crate_enabled_workflow(config, work_dir);

    // Create the actual input file on disk BEFORE initialization
    let input_data = r#"{"data": "input value"}"#;
    fs::write(work_dir.join("input.json"), input_data).expect("Failed to write input.json");

    // Verify no RO-Crate entities exist yet
    let entities_before =
        default_api::list_ro_crate_entities(config, workflow_id, None, None).unwrap();
    assert_eq!(
        entities_before.items.unwrap_or_default().len(),
        0,
        "No RO-Crate entities should exist before initialization"
    );

    // Initialize the workflow - this should create RO-Crate entities for input files
    default_api::initialize_jobs(config, workflow_id, Some(false), Some(false), None)
        .expect("Failed to initialize jobs");

    // Verify RO-Crate entity was created for the input file
    let entities_after =
        default_api::list_ro_crate_entities(config, workflow_id, None, None).unwrap();
    let items = entities_after.items.unwrap();

    // Should have at least one entity (for the input file)
    assert!(
        !items.is_empty(),
        "RO-Crate entities should be created for input files after initialization"
    );

    // Find the entity for our input file
    let input_entity = items.iter().find(|e| e.file_id == Some(input_file_id));
    assert!(
        input_entity.is_some(),
        "Should have an RO-Crate entity for the input file"
    );

    let entity = input_entity.unwrap();
    assert_eq!(entity.entity_type, "File");

    // Parse and verify metadata
    let metadata: serde_json::Value =
        serde_json::from_str(&entity.metadata).expect("Failed to parse entity metadata");
    assert_eq!(metadata["@type"], "File");
    assert!(
        metadata["encodingFormat"].as_str().is_some(),
        "Should have encodingFormat"
    );
    assert!(
        metadata["dateModified"].as_str().is_some(),
        "Should have dateModified"
    );
}

#[rstest]
fn test_auto_ro_crate_output_files_on_job_completion(start_server: &ServerProcess) {
    let config = &start_server.config;
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    let (workflow_id, _input_file_id, output_file_id, job_id) =
        create_ro_crate_enabled_workflow(config, work_dir);

    // Create the input file
    let input_data = r#"{"data": "input value"}"#;
    fs::write(work_dir.join("input.json"), input_data).expect("Failed to write input.json");

    // Initialize the workflow
    default_api::initialize_jobs(config, workflow_id, Some(false), Some(false), None)
        .expect("Failed to initialize jobs");

    // Run the workflow
    let workflow_id_str = workflow_id.to_string();
    let output_dir = work_dir.to_str().unwrap();
    let cli_args = [
        workflow_id_str.as_str(),
        "--output-dir",
        output_dir,
        "--poll-interval",
        "0.1",
        "--max-parallel-jobs",
        "1",
    ];

    run_jobs_cli_command(&cli_args, start_server).expect("Failed to run jobs");

    // Verify job completed
    let job = default_api::get_job(config, job_id).expect("Failed to get job");
    assert_eq!(
        job.status,
        Some(models::JobStatus::Completed),
        "Job should be completed"
    );

    // Verify output file was created
    assert!(
        work_dir.join("output.json").exists(),
        "Output file should exist"
    );

    // Verify RO-Crate entities were created
    let entities = default_api::list_ro_crate_entities(config, workflow_id, None, None)
        .expect("Failed to list RO-Crate entities");
    let items = entities.items.unwrap();

    // Should have entities for both input and output files, plus a CreateAction
    assert!(
        items.len() >= 2,
        "Should have RO-Crate entities for input file, output file, and CreateAction. Found: {}",
        items.len()
    );

    // Find the output file entity
    let output_entity = items.iter().find(|e| e.file_id == Some(output_file_id));
    assert!(
        output_entity.is_some(),
        "Should have an RO-Crate entity for the output file"
    );

    let entity = output_entity.unwrap();
    assert_eq!(entity.entity_type, "File");

    // Parse and verify metadata includes provenance
    let metadata: serde_json::Value =
        serde_json::from_str(&entity.metadata).expect("Failed to parse entity metadata");
    assert_eq!(metadata["@type"], "File");
    assert!(
        metadata["wasGeneratedBy"].is_object(),
        "Output file entity should have wasGeneratedBy for provenance"
    );

    // Find the CreateAction entity
    let create_action = items.iter().find(|e| e.entity_type == "CreateAction");
    assert!(
        create_action.is_some(),
        "Should have a CreateAction entity for job provenance"
    );

    let action = create_action.unwrap();
    let action_metadata: serde_json::Value =
        serde_json::from_str(&action.metadata).expect("Failed to parse CreateAction metadata");
    assert_eq!(action_metadata["@type"], "CreateAction");
    assert_eq!(action_metadata["name"], "process");
    assert!(
        action_metadata["result"].is_array(),
        "CreateAction should have result array"
    );
}

#[rstest]
fn test_auto_ro_crate_disabled_by_default(start_server: &ServerProcess) {
    let config = &start_server.config;
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    // Create workflow WITHOUT enable_ro_crate (should be None/false by default)
    let workflow = models::WorkflowModel::new(
        "test_ro_crate_disabled_workflow".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        default_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Verify enable_ro_crate is not set
    assert!(
        created_workflow.enable_ro_crate.is_none()
            || created_workflow.enable_ro_crate == Some(false),
        "enable_ro_crate should be None or false by default"
    );

    // Create compute node
    let compute_node = models::ComputeNodeModel::new(
        workflow_id,
        "test-host".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        4,
        8.0,
        0,
        1,
        "local".to_string(),
        None,
    );
    default_api::create_compute_node(config, compute_node).unwrap();

    // Create a file
    let input_path = work_dir.join("input.txt").to_string_lossy().to_string();
    let file = models::FileModel::new(workflow_id, "input".to_string(), input_path.clone());
    default_api::create_file(config, file).unwrap();

    // Create the actual file
    fs::write(work_dir.join("input.txt"), "test data").unwrap();

    // Initialize the workflow
    default_api::initialize_jobs(config, workflow_id, Some(false), Some(false), None).unwrap();

    // Verify NO RO-Crate entities were created
    let entities = default_api::list_ro_crate_entities(config, workflow_id, None, None).unwrap();
    assert_eq!(
        entities.items.unwrap_or_default().len(),
        0,
        "No RO-Crate entities should be created when enable_ro_crate is not set"
    );
}

#[rstest]
fn test_auto_ro_crate_diamond_workflow(start_server: &ServerProcess) {
    let config = &start_server.config;
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    let (workflow_id, input_file_ids, output_file_ids) =
        create_diamond_ro_crate_workflow(config, work_dir);

    // Create the input file (f1)
    let input_data = r#"{"data": "initial input"}"#;
    fs::write(work_dir.join("f1.json"), input_data).expect("Failed to write f1.json");

    // Initialize the workflow
    default_api::initialize_jobs(config, workflow_id, Some(false), Some(false), None)
        .expect("Failed to initialize jobs");

    // Verify input file entity was created
    let entities_after_init =
        default_api::list_ro_crate_entities(config, workflow_id, None, None).unwrap();
    let items = entities_after_init.items.unwrap();

    let input_entity = items.iter().find(|e| e.file_id == Some(input_file_ids[0]));
    assert!(
        input_entity.is_some(),
        "Should have RO-Crate entity for input file f1"
    );

    // Run the workflow
    let workflow_id_str = workflow_id.to_string();
    let output_dir = work_dir.to_str().unwrap();
    let cli_args = [
        workflow_id_str.as_str(),
        "--output-dir",
        output_dir,
        "--poll-interval",
        "0.1",
        "--max-parallel-jobs",
        "2",
    ];

    run_jobs_cli_command(&cli_args, start_server).expect("Failed to run jobs");

    // Verify all jobs completed
    let jobs = default_api::list_jobs(
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
    )
    .expect("Failed to list jobs");

    for job in jobs.items.unwrap() {
        assert_eq!(
            job.status,
            Some(models::JobStatus::Completed),
            "Job {} should be completed",
            job.name
        );
    }

    // Verify all output files exist
    assert!(work_dir.join("f2.json").exists(), "f2.json should exist");
    assert!(work_dir.join("f3.json").exists(), "f3.json should exist");
    assert!(work_dir.join("f4.json").exists(), "f4.json should exist");

    // Verify RO-Crate entities were created for output files
    let final_entities =
        default_api::list_ro_crate_entities(config, workflow_id, None, None).unwrap();
    let final_items = final_entities.items.unwrap();

    // Should have entities for:
    // - 1 input file (f1)
    // - 3 output files (f2, f3, f4)
    // - 2 CreateAction entities (one for each job)
    // Note: f2 and f3 are outputs of job1 but inputs of job2, so they get entity from job1's output
    assert!(
        final_items.len() >= 4,
        "Should have multiple RO-Crate entities. Found: {}",
        final_items.len()
    );

    // Verify output file entities exist
    for output_file_id in &output_file_ids {
        let output_entity = final_items
            .iter()
            .find(|e| e.file_id == Some(*output_file_id));
        assert!(
            output_entity.is_some(),
            "Should have RO-Crate entity for output file_id={}",
            output_file_id
        );
    }

    // Verify CreateAction entities exist
    let create_actions: Vec<_> = final_items
        .iter()
        .filter(|e| e.entity_type == "CreateAction")
        .collect();
    assert!(
        create_actions.len() >= 2,
        "Should have CreateAction entities for each job. Found: {}",
        create_actions.len()
    );

    // Verify CreateAction metadata
    for action in create_actions {
        let metadata: serde_json::Value =
            serde_json::from_str(&action.metadata).expect("Failed to parse CreateAction metadata");
        assert_eq!(metadata["@type"], "CreateAction");
        assert!(
            metadata["name"].as_str().is_some(),
            "CreateAction should have name"
        );
        assert!(
            metadata["instrument"].is_object(),
            "CreateAction should have instrument"
        );
        assert!(
            metadata["result"].is_array(),
            "CreateAction should have result array"
        );
    }
}
