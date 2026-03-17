mod common;

use std::fs;
use std::path::PathBuf;

use common::{ServerProcess, start_server};
use rstest::rstest;
use tempfile::NamedTempFile;
use torc::client::default_api;
use torc::client::workflow_spec::{
    FileSpec, JobSpec, ResourceRequirementsSpec, SlurmSchedulerSpec, UserDataSpec, WorkflowSpec,
};

#[test]
fn test_job_specification_new() {
    let job = JobSpec::new("test_job".to_string(), "echo hello".to_string());

    assert_eq!(job.name, "test_job");
    assert_eq!(job.command, "echo hello");
    assert_eq!(job.invocation_script, None);
    assert_eq!(job.cancel_on_blocking_job_failure, Some(false));
    assert_eq!(job.supports_termination, Some(false));
    assert_eq!(job.resource_requirements, None);
    assert_eq!(job.depends_on, None);
    assert_eq!(job.input_files, None);
    assert_eq!(job.output_files, None);
    assert_eq!(job.input_user_data, None);
    assert_eq!(job.output_user_data, None);
    assert_eq!(job.scheduler, None);
}

#[test]
fn test_job_specification_all_fields() {
    let mut job = JobSpec::new("complex_job".to_string(), "python script.py".to_string());

    job.invocation_script = Some("#!/bin/bash\nset -e\n".to_string());
    job.cancel_on_blocking_job_failure = Some(true);
    job.supports_termination = Some(true);
    job.resource_requirements = Some("large_job".to_string());
    job.depends_on = Some(vec!["job1".to_string(), "job2".to_string()]);
    job.input_files = Some(vec!["input.csv".to_string()]);
    job.output_files = Some(vec!["output.json".to_string()]);
    job.input_user_data = Some(vec!["config".to_string()]);
    job.output_user_data = Some(vec!["results".to_string()]);
    job.scheduler = Some("gpu_scheduler".to_string());

    assert_eq!(job.name, "complex_job");
    assert_eq!(job.command, "python script.py");
    assert_eq!(
        job.invocation_script,
        Some("#!/bin/bash\nset -e\n".to_string())
    );
    assert_eq!(job.cancel_on_blocking_job_failure, Some(true));
    assert_eq!(job.supports_termination, Some(true));
    assert_eq!(job.resource_requirements, Some("large_job".to_string()));
    assert_eq!(
        job.depends_on,
        Some(vec!["job1".to_string(), "job2".to_string()])
    );
    assert_eq!(job.input_files, Some(vec!["input.csv".to_string()]));
    assert_eq!(job.output_files, Some(vec!["output.json".to_string()]));
    assert_eq!(job.input_user_data, Some(vec!["config".to_string()]));
    assert_eq!(job.output_user_data, Some(vec!["results".to_string()]));
    assert_eq!(job.scheduler, Some("gpu_scheduler".to_string()));
}

#[test]
fn test_workflow_specification_new() {
    let jobs = vec![
        JobSpec::new("job1".to_string(), "echo hello".to_string()),
        JobSpec::new("job2".to_string(), "echo world".to_string()),
    ];

    let workflow = WorkflowSpec::new(
        "test_workflow".to_string(),
        "test_user".to_string(),
        Some("Test workflow description".to_string()),
        jobs.clone(),
    );

    assert_eq!(workflow.name, "test_workflow");
    assert_eq!(workflow.user, Some("test_user".to_string()));
    assert_eq!(
        workflow.description,
        Some("Test workflow description".to_string())
    );
    assert_eq!(workflow.jobs.len(), 2);
    assert_eq!(workflow.jobs[0].name, "job1");
    assert_eq!(workflow.jobs[1].name, "job2");
    assert_eq!(workflow.files, None);
    assert_eq!(workflow.user_data, None);
    assert_eq!(workflow.resource_requirements, None);
    assert_eq!(workflow.slurm_schedulers, None);
}

#[test]
fn test_workflow_specification_minimal_serialization() {
    let jobs = vec![JobSpec::new("simple_job".to_string(), "ls".to_string())];
    let workflow = WorkflowSpec::new(
        "minimal_workflow".to_string(),
        "user".to_string(),
        Some("Minimal test".to_string()),
        jobs,
    );

    let json = serde_json::to_string_pretty(&workflow).expect("Failed to serialize");
    let deserialized: WorkflowSpec = serde_json::from_str(&json).expect("Failed to deserialize");

    assert_eq!(workflow, deserialized);
}

#[test]
fn test_workflow_specification_complete_serialization() {
    // Create files
    let files = vec![
        FileSpec::new("input.txt".to_string(), "/data/input.txt".to_string()),
        FileSpec::new("output.txt".to_string(), "/data/output.txt".to_string()),
    ];

    // Create user data
    let user_data = vec![
        UserDataSpec {
            is_ephemeral: Some(true),
            name: Some("config".to_string()),
            data: Some(serde_json::json!({"key": "value"})),
        },
        UserDataSpec {
            is_ephemeral: Some(false),
            name: Some("results".to_string()),
            data: Some(serde_json::json!({"count": 42})),
        },
    ];

    // Create resource requirements
    let resource_requirements = vec![
        ResourceRequirementsSpec {
            name: "small_job".to_string(),
            num_cpus: 1,
            num_gpus: 0,
            num_nodes: 1,

            memory: "2g".to_string(),
            runtime: "PT30M".to_string(),
        },
        ResourceRequirementsSpec {
            name: "large_job".to_string(),
            num_cpus: 8,
            num_gpus: 2,
            num_nodes: 2,

            memory: "64g".to_string(),
            runtime: "PT4H".to_string(),
        },
    ];

    // Create slurm schedulers
    let slurm_schedulers = vec![
        SlurmSchedulerSpec {
            name: Some("default".to_string()),
            account: "project1".to_string(),
            gres: None,
            mem: Some("8G".to_string()),
            nodes: 1,
            ntasks_per_node: Some(1),
            partition: Some("general".to_string()),
            qos: Some("normal".to_string()),
            tmp: Some("10G".to_string()),
            walltime: "01:00:00".to_string(),
            extra: None,
        },
        SlurmSchedulerSpec {
            name: Some("gpu".to_string()),
            account: "gpu_project".to_string(),
            gres: Some("gpu:2".to_string()),
            mem: Some("32G".to_string()),
            nodes: 1,
            ntasks_per_node: Some(2),
            partition: Some("gpu".to_string()),
            qos: Some("high".to_string()),
            tmp: Some("50G".to_string()),
            walltime: "04:00:00".to_string(),
            extra: Some("--constraint=v100".to_string()),
        },
    ];

    // Create complex jobs
    let mut job1 = JobSpec::new("preprocess".to_string(), "python preprocess.py".to_string());
    job1.invocation_script = Some("#!/bin/bash\nexport PYTHONPATH=/opt/tools\n".to_string());
    job1.supports_termination = Some(true);
    job1.resource_requirements = Some("small_job".to_string());
    job1.input_files = Some(vec!["input.txt".to_string()]);
    job1.output_files = Some(vec!["output.txt".to_string()]);
    job1.input_user_data = Some(vec!["config".to_string()]);
    job1.output_user_data = Some(vec!["results".to_string()]);
    job1.scheduler = Some("default".to_string());

    let mut job2 = JobSpec::new("analyze".to_string(), "python analyze.py".to_string());
    job2.cancel_on_blocking_job_failure = Some(true);
    job2.supports_termination = Some(true);
    job2.resource_requirements = Some("large_job".to_string());
    job2.depends_on = Some(vec!["preprocess".to_string()]);
    job2.input_files = Some(vec!["output.txt".to_string()]);
    job2.input_user_data = Some(vec!["results".to_string()]);
    job2.scheduler = Some("gpu".to_string());

    let jobs = vec![job1, job2];

    let mut workflow = WorkflowSpec::new(
        "complex_workflow".to_string(),
        "data_scientist".to_string(),
        Some("Complex data processing workflow".to_string()),
        jobs,
    );

    workflow.files = Some(files);
    workflow.user_data = Some(user_data);
    workflow.resource_requirements = Some(resource_requirements);
    workflow.slurm_schedulers = Some(slurm_schedulers);

    let json = serde_json::to_string_pretty(&workflow).expect("Failed to serialize");
    let deserialized: WorkflowSpec = serde_json::from_str(&json).expect("Failed to deserialize");

    assert_eq!(workflow, deserialized);
    assert_eq!(deserialized.files.as_ref().unwrap().len(), 2);
    assert_eq!(deserialized.user_data.as_ref().unwrap().len(), 2);
    assert_eq!(
        deserialized.resource_requirements.as_ref().unwrap().len(),
        2
    );
    assert_eq!(deserialized.slurm_schedulers.as_ref().unwrap().len(), 2);
    assert_eq!(deserialized.jobs.len(), 2);
}

#[test]
fn test_from_json_file() {
    let workflow_data = serde_json::json!({
        "name": "file_test_workflow",
        "user": "file_user",
        "description": "Test reading from file",
        "jobs": [
            {
                "name": "test_job",
                "command": "echo hello",
                "invocation_script": null,
                "cancel_on_blocking_job_failure": false,
                "supports_termination": false,
                "resource_requirements": null,
                "depends_on": null,
                "input_files": null,
                "output_files": null,
                "input_user_data": null,
                "output_user_data": null,
                "scheduler": null
            }
        ],
        "files": null,
        "user_data": null,
        "resource_requirements": null,
        "slurm_schedulers": null
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let workflow =
        WorkflowSpec::from_spec_file(temp_file.path()).expect("Failed to read from JSON file");

    assert_eq!(workflow.name, "file_test_workflow");
    assert_eq!(workflow.user, Some("file_user".to_string()));
    assert_eq!(
        workflow.description,
        Some("Test reading from file".to_string())
    );
    assert_eq!(workflow.jobs.len(), 1);
    assert_eq!(workflow.jobs[0].name, "test_job");
}

#[test]
fn test_from_json_file_invalid_json() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(temp_file.path(), "{ invalid json }").expect("Failed to write temp file");

    let result = WorkflowSpec::from_spec_file(temp_file.path());
    assert!(result.is_err());
}

#[test]
fn test_from_json_file_missing_required_fields() {
    let workflow_data = serde_json::json!({
        "name": "incomplete_workflow",
        "user": "test_user"
        // Missing description and jobs
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::from_spec_file(temp_file.path());
    assert!(result.is_err());
}

#[test]
fn test_empty_jobs_list() {
    let workflow_data = serde_json::json!({
        "name": "empty_workflow",
        "user": "test_user",
        "description": "Workflow with no jobs",
        "jobs": [],
        "files": null,
        "user_data": null,
        "resource_requirements": null,
        "slurm_schedulers": null
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let workflow =
        WorkflowSpec::from_spec_file(temp_file.path()).expect("Failed to read from JSON file");

    assert_eq!(workflow.jobs.len(), 0);
}

#[test]
fn test_job_with_all_optional_fields_none() {
    let job_data = serde_json::json!({
        "name": "minimal_job",
        "command": "echo test",
        "invocation_script": null,
        "cancel_on_blocking_job_failure": false,
        "supports_termination": false,
        "resource_requirements": null,
        "depends_on": null,
        "input_files": null,
        "output_files": null,
        "input_user_data": null,
        "output_user_data": null,
        "scheduler": null
    });

    let job: JobSpec = serde_json::from_value(job_data).expect("Failed to deserialize job");

    assert_eq!(job.name, "minimal_job");
    assert_eq!(job.command, "echo test");
    assert_eq!(job.invocation_script, None);
    assert_eq!(job.cancel_on_blocking_job_failure, Some(false));
    assert_eq!(job.supports_termination, Some(false));
    assert_eq!(job.resource_requirements, None);
    assert_eq!(job.depends_on, None);
    assert_eq!(job.input_files, None);
    assert_eq!(job.output_files, None);
    assert_eq!(job.input_user_data, None);
    assert_eq!(job.output_user_data, None);
    assert_eq!(job.scheduler, None);
}

#[test]
fn test_job_with_empty_arrays() {
    let job_data = serde_json::json!({
        "name": "empty_arrays_job",
        "command": "echo test",
        "invocation_script": null,
        "cancel_on_blocking_job_failure": false,
        "supports_termination": false,
        "resource_requirements": null,
        "depends_on": [],
        "input_files": [],
        "output_files": [],
        "input_user_data": [],
        "output_user_data": [],
        "scheduler": null
    });

    let job: JobSpec = serde_json::from_value(job_data).expect("Failed to deserialize job");

    assert_eq!(job.depends_on, Some(vec![]));
    assert_eq!(job.input_files, Some(vec![]));
    assert_eq!(job.output_files, Some(vec![]));
    assert_eq!(job.input_user_data, Some(vec![]));
    assert_eq!(job.output_user_data, Some(vec![]));
}

#[test]
fn test_workflow_with_complex_dependencies() {
    let jobs = vec![
        {
            let mut job = JobSpec::new("job_a".to_string(), "echo a".to_string());
            job.output_files = Some(vec!["file_a".to_string()]);
            job.output_user_data = Some(vec!["data_a".to_string()]);
            job
        },
        {
            let mut job = JobSpec::new("job_b".to_string(), "echo b".to_string());
            job.output_files = Some(vec!["file_b".to_string()]);
            job.output_user_data = Some(vec!["data_b".to_string()]);
            job
        },
        {
            let mut job = JobSpec::new("job_c".to_string(), "echo c".to_string());
            job.depends_on = Some(vec!["job_a".to_string(), "job_b".to_string()]);
            job.input_files = Some(vec!["file_a".to_string(), "file_b".to_string()]);
            job.input_user_data = Some(vec!["data_a".to_string(), "data_b".to_string()]);
            job.output_files = Some(vec!["file_c".to_string()]);
            job
        },
    ];

    let workflow = WorkflowSpec::new(
        "dependency_test".to_string(),
        "test_user".to_string(),
        Some("Test complex dependencies".to_string()),
        jobs,
    );

    // Serialize and deserialize to ensure structure is preserved
    let json = serde_json::to_string_pretty(&workflow).expect("Failed to serialize");
    let deserialized: WorkflowSpec = serde_json::from_str(&json).expect("Failed to deserialize");

    assert_eq!(deserialized.jobs.len(), 3);

    // Check job_c dependencies
    let job_c = &deserialized.jobs[2];
    assert_eq!(job_c.name, "job_c");
    assert_eq!(
        job_c.depends_on,
        Some(vec!["job_a".to_string(), "job_b".to_string()])
    );
    assert_eq!(
        job_c.input_files,
        Some(vec!["file_a".to_string(), "file_b".to_string()])
    );
    assert_eq!(
        job_c.input_user_data,
        Some(vec!["data_a".to_string(), "data_b".to_string()])
    );
}

#[rstest]
fn test_create_workflow_from_json_file_minimal(start_server: &ServerProcess) {
    let workflow_data = serde_json::json!({
        "name": "integration_test_workflow",
        "user": "integration_user",
        "description": "Integration test workflow",
        "jobs": [
            {
                "name": "simple_job",
                "command": "echo 'Hello World'",
                "invocation_script": null,
                "cancel_on_blocking_job_failure": false,
                "supports_termination": false,
                "resource_requirements": null,
                "depends_on": null,
                "input_files": null,
                "output_files": null,
                "input_user_data": null,
                "output_user_data": null,
                "scheduler": null
            }
        ],
        "files": null,
        "user_data": null,
        "resource_requirements": null,
        "slurm_schedulers": null
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        workflow_data["user"].as_str().unwrap(),
        false,
        false,
    )
    .expect("Failed to create workflow from spec file");

    assert!(workflow_id > 0);

    // Verify workflow was created by fetching it
    let created_workflow = default_api::get_workflow(&start_server.config, workflow_id)
        .expect("Failed to get created workflow");

    assert_eq!(created_workflow.name, "integration_test_workflow");
    assert_eq!(created_workflow.user, "integration_user");
    assert_eq!(
        created_workflow.description,
        Some("Integration test workflow".to_string())
    );
}

#[rstest]
fn test_create_workflow_from_json_file_with_files(start_server: &ServerProcess) {
    let workflow_data = serde_json::json!({
        "name": "workflow_with_files",
        "user": "file_user",
        "description": "Workflow with file dependencies",
        "jobs": [
            {
                "name": "file_job",
                "command": "cat input.txt > output.txt",
                "invocation_script": null,
                "cancel_on_blocking_job_failure": false,
                "supports_termination": false,
                "resource_requirements": null,
                "depends_on": null,
                "input_files": ["input_file"],
                "output_files": ["output_file"],
                "input_user_data": null,
                "output_user_data": null,
                "scheduler": null
            }
        ],
        "files": [
            {
                "name": "input_file",
                "path": "/data/input.txt"
            },
            {
                "name": "output_file",
                "path": "/data/output.txt"
            }
        ],
        "user_data": null,
        "resource_requirements": null,
        "slurm_schedulers": null
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        workflow_data["user"].as_str().unwrap(),
        false,
        false,
    )
    .expect("Failed to create workflow from spec file");

    assert!(workflow_id > 0);
}

#[rstest]
fn test_create_workflow_from_json_file_with_dependencies(start_server: &ServerProcess) {
    let workflow_data = serde_json::json!({
        "name": "workflow_with_deps",
        "user": "deps_user",
        "description": "Workflow with job dependencies",
        "jobs": [
            {
                "name": "first_job",
                "command": "echo 'First job'",
                "invocation_script": null,
                "cancel_on_blocking_job_failure": false,
                "supports_termination": false,
                "resource_requirements": null,
                "depends_on": null,
                "input_files": null,
                "output_files": null,
                "input_user_data": null,
                "output_user_data": null,
                "scheduler": null
            },
            {
                "name": "second_job",
                "command": "echo 'Second job'",
                "invocation_script": null,
                "cancel_on_blocking_job_failure": true,
                "supports_termination": false,
                "resource_requirements": null,
                "depends_on": ["first_job"],
                "input_files": null,
                "output_files": null,
                "input_user_data": null,
                "output_user_data": null,
                "scheduler": null
            }
        ],
        "files": null,
        "user_data": null,
        "resource_requirements": null,
        "slurm_schedulers": null
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        workflow_data["user"].as_str().unwrap(),
        false,
        false,
    )
    .expect("Failed to create workflow from spec file");

    assert!(workflow_id > 0);
}

#[rstest]
fn test_create_workflow_from_json_file_duplicate_file_names(start_server: &ServerProcess) {
    let workflow_data = serde_json::json!({
        "name": "duplicate_files_workflow",
        "user": "error_user",
        "description": "Workflow with duplicate file names",
        "jobs": [
            {
                "name": "test_job",
                "command": "echo test",
                "invocation_script": null,
                "cancel_on_blocking_job_failure": false,
                "supports_termination": false,
                "resource_requirements": null,
                "depends_on": null,
                "input_files": null,
                "output_files": null,
                "input_user_data": null,
                "output_user_data": null,
                "scheduler": null
            }
        ],
        "files": [
            {
                "name": "duplicate_name",
                "path": "/data/file1.txt"
            },
            {
                "name": "duplicate_name",
                "path": "/data/file2.txt"
            }
        ],
        "user_data": null,
        "resource_requirements": null,
        "slurm_schedulers": null
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        workflow_data["user"].as_str().unwrap(),
        false,
        false,
    );

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Duplicate file name")
    );
}

#[rstest]
fn test_create_workflow_from_json_file_missing_file_reference(start_server: &ServerProcess) {
    let workflow_data = serde_json::json!({
        "name": "missing_file_workflow",
        "user": "error_user",
        "description": "Workflow with missing file reference",
        "jobs": [
            {
                "name": "test_job",
                "command": "echo test",
                "invocation_script": null,
                "cancel_on_blocking_job_failure": false,
                "supports_termination": false,
                "resource_requirements": null,
                "depends_on": null,
                "input_files": ["nonexistent_file"],
                "output_files": null,
                "input_user_data": null,
                "output_user_data": null,
                "scheduler": null
            }
        ],
        "files": null,
        "user_data": null,
        "resource_requirements": null,
        "slurm_schedulers": null
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        workflow_data["user"].as_str().unwrap(),
        false,
        false,
    );

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("not found for job")
    );
}

#[rstest]
fn test_create_workflow_from_json_file_missing_job_dependency(start_server: &ServerProcess) {
    let workflow_data = serde_json::json!({
        "name": "missing_dep_workflow",
        "user": "error_user",
        "description": "Workflow with missing job dependency",
        "jobs": [
            {
                "name": "dependent_job",
                "command": "echo test",
                "invocation_script": null,
                "cancel_on_blocking_job_failure": false,
                "supports_termination": false,
                "resource_requirements": null,
                "depends_on": ["nonexistent_job"],
                "input_files": null,
                "output_files": null,
                "input_user_data": null,
                "output_user_data": null,
                "scheduler": null
            }
        ],
        "files": null,
        "user_data": null,
        "resource_requirements": null,
        "slurm_schedulers": null
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        workflow_data["user"].as_str().unwrap(),
        false,
        false,
    );

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("not found for job")
    );
}

#[rstest]
fn test_create_workflow_from_json5_file(start_server: &ServerProcess) {
    let workflow_data = r#"{
        // JSON5 format with comments
        "name": "json5_test_workflow",
        "user": "json5_user",
        "description": "Test workflow using JSON5 format",
        "jobs": [
            {
                "name": "json5_job",
                "command": "echo 'JSON5 Hello World'",
                "invocation_script": null,
                "cancel_on_blocking_job_failure": false,
                "supports_termination": false,
                "resource_requirements": null,
                "depends_on": null,
                "input_files": null,
                "output_files": null,
                "input_user_data": null,
                "output_user_data": null,
                "scheduler": null
            }
        ],
        "files": null,
        "user_data": null,
        "resource_requirements": null,
        "slurm_schedulers": null
    }"#;

    let temp_file = NamedTempFile::with_suffix(".json5").expect("Failed to create temp file");
    fs::write(temp_file.path(), workflow_data).expect("Failed to write temp file");

    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "json5_user",
        false,
        false,
    )
    .expect("Failed to create workflow from JSON5 file");

    assert!(workflow_id > 0);
}

#[rstest]
fn test_create_workflow_from_yaml_file(start_server: &ServerProcess) {
    let workflow_data = r#"
# YAML format with comments
name: yaml_test_workflow
user: yaml_user
description: Test workflow using YAML format
jobs:
  - name: yaml_job
    command: echo 'YAML Hello World'
    invocation_script: null
    cancel_on_blocking_job_failure: false
    supports_termination: false
    resource_requirements: null
    depends_on: null
    input_files: null
    output_files: null
    input_user_data: null
    output_user_data: null
    scheduler: null
files: null
user_data: null
resource_requirements: null
slurm_schedulers: null
"#;

    let temp_file = NamedTempFile::with_suffix(".yaml").expect("Failed to create temp file");
    fs::write(temp_file.path(), workflow_data).expect("Failed to write temp file");

    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "yaml_user",
        false,
        false,
    )
    .expect("Failed to create workflow from YAML file");

    assert!(workflow_id > 0);
}

#[rstest]
fn test_create_workflow_from_yaml_file_with_user(start_server: &ServerProcess) {
    let workflow_data = r#"
# YAML format with comments
name: yaml_test_workflow
user: yaml_user
description: Test workflow using YAML format
jobs:
  - name: yaml_job
    command: echo 'YAML Hello World'
    invocation_script: null
    cancel_on_blocking_job_failure: false
    supports_termination: false
    resource_requirements: null
    depends_on: null
    input_files: null
    output_files: null
    input_user_data: null
    output_user_data: null
    scheduler: null
files: null
user_data: null
resource_requirements: null
slurm_schedulers: null
"#;

    let temp_file = NamedTempFile::with_suffix(".yaml").expect("Failed to create temp file");
    fs::write(temp_file.path(), workflow_data).expect("Failed to write temp file");

    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "yaml_user",
        false,
        false,
    )
    .expect("Failed to create workflow from YAML file");

    assert!(workflow_id > 0);
}

#[rstest]
fn test_create_workflow_from_spec_auto_detect_json(start_server: &ServerProcess) {
    let workflow_data = serde_json::json!({
        "name": "auto_detect_json_workflow",
        "user": "auto_user",
        "description": "Test auto-detection of JSON format",
        "jobs": [
            {
                "name": "auto_job",
                "command": "echo 'Auto-detected JSON'",
                "invocation_script": null,
                "cancel_on_blocking_job_failure": false,
                "supports_termination": false,
                "resource_requirements": null,
                "depends_on": null,
                "input_files": null,
                "output_files": null,
                "input_user_data": null,
                "output_user_data": null,
                "scheduler": null
            }
        ],
        "files": null,
        "user_data": null,
        "resource_requirements": null,
        "slurm_schedulers": null
    });

    // Create file without extension to test auto-detection
    let temp_file = NamedTempFile::with_suffix(".spec").expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "auto_user",
        false,
        false,
    )
    .expect("Failed to create workflow from spec file with auto-detection");

    assert!(workflow_id > 0);
}

#[rstest]
fn test_from_spec_file_json5_format() {
    let json5_content = r#"{
        // JSON5 test with comments
        "name": "test_workflow",
        "user": "test_user", 
        "description": "JSON5 test",
        "jobs": [],
        "files": null,
        "user_data": null,
        "resource_requirements": null,
        "slurm_schedulers": null
    }"#;

    let temp_file = NamedTempFile::with_suffix(".json5").expect("Failed to create temp file");
    fs::write(temp_file.path(), json5_content).expect("Failed to write temp file");

    let spec =
        WorkflowSpec::from_spec_file(temp_file.path()).expect("Failed to parse JSON5 spec file");

    assert_eq!(spec.name, "test_workflow");
    assert_eq!(spec.user, Some("test_user".to_string()));
    assert_eq!(spec.description, Some("JSON5 test".to_string()));
}

#[rstest]
fn test_from_spec_file_yaml_format() {
    let yaml_content = r#"
# YAML test with comments
name: test_workflow
user: test_user
description: YAML test
jobs: []
files: null
user_data: null
resource_requirements: null
slurm_schedulers: null
"#;

    let temp_file = NamedTempFile::with_suffix(".yaml").expect("Failed to create temp file");
    fs::write(temp_file.path(), yaml_content).expect("Failed to write temp file");

    let spec =
        WorkflowSpec::from_spec_file(temp_file.path()).expect("Failed to parse YAML spec file");

    assert_eq!(spec.name, "test_workflow");
    assert_eq!(spec.user, Some("test_user".to_string()));
    assert_eq!(spec.description, Some("YAML test".to_string()));
}

#[test]
fn test_workflow_specification_with_all_resource_types() {
    // Create a workflow that uses all possible resource types
    let files = vec![FileSpec::new(
        "script.py".to_string(),
        "/scripts/script.py".to_string(),
    )];

    let user_data = vec![UserDataSpec {
        is_ephemeral: Some(false),
        name: Some("config_data".to_string()),
        data: Some(serde_json::json!({"param": "value"})),
    }];

    let resource_requirements = vec![ResourceRequirementsSpec {
        name: "test_resources".to_string(),
        num_cpus: 4,
        num_gpus: 1,
        num_nodes: 1,

        memory: "8g".to_string(),
        runtime: "PT1H".to_string(),
    }];

    let slurm_schedulers = vec![SlurmSchedulerSpec {
        name: Some("test_scheduler".to_string()),
        account: "test_account".to_string(),
        gres: Some("gpu:1".to_string()),
        mem: Some("16G".to_string()),
        nodes: 1,
        ntasks_per_node: Some(1),
        partition: Some("test".to_string()),
        qos: Some("normal".to_string()),
        tmp: Some("20G".to_string()),
        walltime: "02:00:00".to_string(),
        extra: Some("--test-flag".to_string()),
    }];

    let mut job = JobSpec::new(
        "comprehensive_job".to_string(),
        "python script.py".to_string(),
    );
    job.invocation_script = Some("#!/bin/bash\nset -euo pipefail\n".to_string());
    job.cancel_on_blocking_job_failure = Some(true);
    job.supports_termination = Some(true);
    job.resource_requirements = Some("test_resources".to_string());
    job.input_files = Some(vec!["script.py".to_string()]);
    job.input_user_data = Some(vec!["config_data".to_string()]);
    job.scheduler = Some("test_scheduler".to_string());

    let mut workflow = WorkflowSpec::new(
        "comprehensive_workflow".to_string(),
        "comprehensive_user".to_string(),
        Some("Uses all resource types".to_string()),
        vec![job],
    );

    workflow.files = Some(files);
    workflow.user_data = Some(user_data);
    workflow.resource_requirements = Some(resource_requirements);
    workflow.slurm_schedulers = Some(slurm_schedulers);

    // Test serialization roundtrip
    let json = serde_json::to_string_pretty(&workflow).expect("Failed to serialize");
    let deserialized: WorkflowSpec = serde_json::from_str(&json).expect("Failed to deserialize");

    assert_eq!(workflow, deserialized);

    // Verify all resource types are present
    assert!(deserialized.files.is_some());
    assert!(deserialized.user_data.is_some());
    assert!(deserialized.resource_requirements.is_some());
    assert!(deserialized.slurm_schedulers.is_some());

    // Verify job references all resource types
    let job = &deserialized.jobs[0];
    assert!(job.invocation_script.is_some());
    assert_eq!(job.cancel_on_blocking_job_failure, Some(true));
    assert_eq!(job.supports_termination, Some(true));
    assert!(job.resource_requirements.is_some());
    assert!(job.input_files.is_some());
    assert!(job.input_user_data.is_some());
    assert!(job.scheduler.is_some());
}

#[test]
fn test_job_specification_boolean_permutations() {
    // Test all combinations of boolean fields
    let bool_combinations = vec![(false, false), (false, true), (true, false), (true, true)];

    for (cancel_on_failure, supports_termination) in bool_combinations {
        let mut job = JobSpec::new("bool_test".to_string(), "echo test".to_string());
        job.cancel_on_blocking_job_failure = Some(cancel_on_failure);
        job.supports_termination = Some(supports_termination);

        let json = serde_json::to_string(&job).expect("Failed to serialize job");
        let deserialized: JobSpec = serde_json::from_str(&json).expect("Failed to deserialize job");

        assert_eq!(
            deserialized.cancel_on_blocking_job_failure,
            Some(cancel_on_failure)
        );
        assert_eq!(
            deserialized.supports_termination,
            Some(supports_termination)
        );
    }
}

#[test]
fn test_workflow_with_large_number_of_jobs() {
    // Test workflow with many jobs to ensure batching works
    let mut jobs = Vec::new();
    for i in 0..2500 {
        // More than 2 batches of 1000
        jobs.push(JobSpec::new(
            format!("job_{}", i),
            format!("echo 'Job {}'", i),
        ));
    }

    let workflow = WorkflowSpec::new(
        "large_workflow".to_string(),
        "batch_user".to_string(),
        Some("Workflow with many jobs".to_string()),
        jobs,
    );

    assert_eq!(workflow.jobs.len(), 2500);

    // Test serialization
    let json = serde_json::to_string(&workflow).expect("Failed to serialize large workflow");
    let deserialized: WorkflowSpec =
        serde_json::from_str(&json).expect("Failed to deserialize large workflow");

    assert_eq!(deserialized.jobs.len(), 2500);
    assert_eq!(deserialized.jobs[0].name, "job_0");
    assert_eq!(deserialized.jobs[2499].name, "job_2499");
}

#[test]
fn test_workflow_specification_default_values() {
    // Test that Default trait works correctly
    let default_workflow = WorkflowSpec::default();

    assert_eq!(default_workflow.name, "");
    assert_eq!(default_workflow.user, None);
    assert_eq!(default_workflow.description, None);
    assert_eq!(default_workflow.jobs.len(), 0);
    assert_eq!(default_workflow.files, None);
    assert_eq!(default_workflow.user_data, None);
    assert_eq!(default_workflow.resource_requirements, None);
    assert_eq!(default_workflow.slurm_schedulers, None);
}

#[test]
fn test_job_specification_default_values() {
    // Test that Default trait works correctly for JobSpec
    let default_job = JobSpec::new("test_job".to_string(), "echo hello".to_string());

    assert_eq!(default_job.name, "test_job");
    assert_eq!(default_job.command, "echo hello");
    assert_eq!(default_job.invocation_script, None);
    assert_eq!(default_job.cancel_on_blocking_job_failure, Some(false));
    assert_eq!(default_job.supports_termination, Some(false));
    assert_eq!(default_job.resource_requirements, None);
    assert_eq!(default_job.depends_on, None);
    assert_eq!(default_job.input_files, None);
    assert_eq!(default_job.output_files, None);
    assert_eq!(default_job.input_user_data, None);
    assert_eq!(default_job.output_user_data, None);
    assert_eq!(default_job.scheduler, None);
}

#[test]
fn test_specification_structs_serialization() {
    // Test that the new specification structs serialize/deserialize correctly
    let file_spec = FileSpec::new(
        "test_file.txt".to_string(),
        "/path/to/test_file.txt".to_string(),
    );

    let user_data_spec = UserDataSpec {
        is_ephemeral: Some(true),
        name: Some("test_data".to_string()),
        data: Some(serde_json::json!({"key": "value"})),
    };

    let resource_spec = ResourceRequirementsSpec {
        name: "test_resource".to_string(),
        num_cpus: 4,
        num_gpus: 1,
        num_nodes: 2,

        memory: "8g".to_string(),
        runtime: "PT2H".to_string(),
    };

    let scheduler_spec = SlurmSchedulerSpec {
        name: Some("test_scheduler".to_string()),
        account: "test_account".to_string(),
        gres: Some("gpu:1".to_string()),
        mem: Some("16G".to_string()),
        nodes: 2,
        ntasks_per_node: Some(4),
        partition: Some("gpu".to_string()),
        qos: Some("high".to_string()),
        tmp: Some("50G".to_string()),
        walltime: "04:00:00".to_string(),
        extra: Some("--test-flag".to_string()),
    };

    // Test serialization roundtrip
    let file_json = serde_json::to_string(&file_spec).expect("Failed to serialize FileSpec");
    let file_deserialized: FileSpec =
        serde_json::from_str(&file_json).expect("Failed to deserialize FileSpec");
    assert_eq!(file_spec, file_deserialized);

    let user_data_json =
        serde_json::to_string(&user_data_spec).expect("Failed to serialize UserDataSpec");
    let user_data_deserialized: UserDataSpec =
        serde_json::from_str(&user_data_json).expect("Failed to deserialize UserDataSpec");
    assert_eq!(user_data_spec, user_data_deserialized);

    let resource_json = serde_json::to_string(&resource_spec)
        .expect("Failed to serialize ResourceRequirementsSpec");
    let resource_deserialized: ResourceRequirementsSpec = serde_json::from_str(&resource_json)
        .expect("Failed to deserialize ResourceRequirementsSpec");
    assert_eq!(resource_spec, resource_deserialized);

    let scheduler_json =
        serde_json::to_string(&scheduler_spec).expect("Failed to serialize SlurmSchedulerSpec");
    let scheduler_deserialized: SlurmSchedulerSpec =
        serde_json::from_str(&scheduler_json).expect("Failed to deserialize SlurmSchedulerSpec");
    assert_eq!(scheduler_spec, scheduler_deserialized);
}

#[test]
fn test_workflow_specification_with_new_structs() {
    // Test that a complete workflow specification works with the new specification structs
    let files = vec![
        FileSpec::new("input.dat".to_string(), "/data/input.dat".to_string()),
        FileSpec::new("output.dat".to_string(), "/data/output.dat".to_string()),
    ];

    let user_data = vec![UserDataSpec {
        is_ephemeral: Some(false),
        name: Some("config".to_string()),
        data: Some(serde_json::json!({"batch_size": 100})),
    }];

    let resource_requirements = vec![ResourceRequirementsSpec {
        name: "medium_job".to_string(),
        num_cpus: 4,
        num_gpus: 0,
        num_nodes: 1,

        memory: "16g".to_string(),
        runtime: "PT1H30M".to_string(),
    }];

    let slurm_schedulers = vec![SlurmSchedulerSpec {
        name: Some("cpu_scheduler".to_string()),
        account: "research".to_string(),
        gres: None,
        mem: Some("32G".to_string()),
        nodes: 1,
        ntasks_per_node: Some(4),
        partition: Some("cpu".to_string()),
        qos: Some("normal".to_string()),
        tmp: Some("10G".to_string()),
        walltime: "02:00:00".to_string(),
        extra: None,
    }];

    let mut job = JobSpec::new("process_data".to_string(), "python process.py".to_string());
    job.input_files = Some(vec!["input.dat".to_string()]);
    job.output_files = Some(vec!["output.dat".to_string()]);
    job.input_user_data = Some(vec!["config".to_string()]);
    job.resource_requirements = Some("medium_job".to_string());
    job.scheduler = Some("cpu_scheduler".to_string());

    let mut workflow = WorkflowSpec::new(
        "data_processing".to_string(),
        "scientist".to_string(),
        Some("Process scientific data".to_string()),
        vec![job],
    );

    workflow.files = Some(files);
    workflow.user_data = Some(user_data);
    workflow.resource_requirements = Some(resource_requirements);
    workflow.slurm_schedulers = Some(slurm_schedulers);

    // Test serialization roundtrip
    let json = serde_json::to_string_pretty(&workflow).expect("Failed to serialize workflow");
    let deserialized: WorkflowSpec =
        serde_json::from_str(&json).expect("Failed to deserialize workflow");

    assert_eq!(workflow, deserialized);
    assert_eq!(deserialized.files.as_ref().unwrap().len(), 2);
    assert_eq!(deserialized.user_data.as_ref().unwrap().len(), 1);
    assert_eq!(
        deserialized.resource_requirements.as_ref().unwrap().len(),
        1
    );
    assert_eq!(deserialized.slurm_schedulers.as_ref().unwrap().len(), 1);

    // Verify that the JSON doesn't contain workflow_id or id fields
    assert!(!json.contains("workflow_id"));
    assert!(!json.contains("\"id\""));
    assert!(!json.contains("st_mtime"));
}

#[test]
fn test_json_field_name_compatibility() {
    // Test that JSON field names match exactly what's expected
    let job = JobSpec {
        name: "test".to_string(),
        command: "echo".to_string(),
        invocation_script: Some("script".to_string()),
        cancel_on_blocking_job_failure: Some(true),
        supports_termination: Some(false),
        resource_requirements: Some("req".to_string()),
        depends_on: Some(vec!["dep".to_string()]),
        depends_on_regexes: None,
        input_files: Some(vec!["in.txt".to_string()]),
        input_file_regexes: None,
        output_files: Some(vec!["out.txt".to_string()]),
        output_file_regexes: None,
        input_user_data: Some(vec!["in_data".to_string()]),
        input_user_data_regexes: None,
        output_user_data: Some(vec!["out_data".to_string()]),
        output_user_data_regexes: None,
        scheduler: Some("sched".to_string()),
        parameters: None,
        parameter_mode: None,
        use_parameters: None,
        failure_handler: None,
        stdio: None,
        priority: None,
    };

    let json = serde_json::to_value(&job).expect("Failed to serialize to JSON value");

    // Check that all expected fields are present with correct names
    assert!(json.get("name").is_some());
    assert!(json.get("command").is_some());
    assert!(json.get("invocation_script").is_some());
    assert!(json.get("cancel_on_blocking_job_failure").is_some());
    assert!(json.get("supports_termination").is_some());
    assert!(json.get("resource_requirements").is_some());
    assert!(json.get("depends_on").is_some());
    assert!(json.get("input_files").is_some());
    assert!(json.get("output_files").is_some());
    assert!(json.get("input_user_data").is_some());
    assert!(json.get("output_user_data").is_some());
    assert!(json.get("scheduler").is_some());
}

#[rstest]
fn test_create_workflow_rollback_on_error(start_server: &ServerProcess) {
    // Test that workflow is properly cleaned up when creation fails
    let workflow_data = serde_json::json!({
        "name": "rollback_test_workflow",
        "user": "rollback_user",
        "description": "Should be rolled back",
        "jobs": [
            {
                "name": "failing_job",
                "command": "echo test",
                "invocation_script": null,
                "cancel_on_blocking_job_failure": false,
                "supports_termination": false,
                "resource_requirements": "nonexistent_resource", // This should cause failure
                "depends_on": null,
                "input_files": null,
                "output_files": null,
                "input_user_data": null,
                "output_user_data": null,
                "scheduler": null
            }
        ],
        "files": null,
        "user_data": null,
        "resource_requirements": null, // Missing the required resource
        "slurm_schedulers": null
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "rollback_user",
        false,
        false,
    );

    // Should fail due to missing resource requirements
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("not found for job")
    );

    // Verify no workflow with this name exists (confirming rollback)
    let workflows = default_api::list_workflows(
        &start_server.config,
        None,
        None,
        None,
        Some(100),
        Some("rollback_test_workflow"),
        Some("rollback_user"),
        None,
        None,
    )
    .expect("Failed to list workflows");

    assert_eq!(workflows.items.unwrap_or_default().len(), 0);
}

#[rstest]
fn test_create_workflow_with_regex_job_dependencies(start_server: &ServerProcess) {
    use tempfile::TempDir;

    // Create temp directory for files
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let input_path = temp_dir.path().join("input.txt");
    fs::write(&input_path, "test input").expect("Failed to write input file");

    let workflow_data = serde_json::json!({
        "name": "regex_job_deps_workflow",
        "user": "regex_user",
        "description": "Test workflow with regex job dependencies",
        "jobs": [
            {
                "name": "preprocess",
                "command": "echo 'preprocess'",
            },
            {
                "name": "work_1",
                "command": "echo 'work 1'",
                "depends_on": ["preprocess"],
            },
            {
                "name": "work_2",
                "command": "echo 'work 2'",
                "depends_on": ["preprocess"],
            },
            {
                "name": "work_3",
                "command": "echo 'work 3'",
                "depends_on": ["preprocess"],
            },
            {
                "name": "postprocess",
                "command": "echo 'postprocess'",
                "depends_on_regexes": ["work_.*"],
            }
        ],
        "files": null,
        "user_data": null,
        "resource_requirements": null,
        "slurm_schedulers": null
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "regex_user",
        false,
        false,
    )
    .expect("Failed to create workflow with regex job dependencies");

    assert!(workflow_id > 0);

    // Verify that postprocess job has dependencies on all work_* jobs
    let jobs = default_api::list_jobs(
        &start_server.config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(true), // include_relationships - needed for tests that check dependencies/files
        None,       // active_compute_node_id
    )
    .expect("Failed to list jobs");

    let job_items = jobs.items.unwrap();
    let postprocess_job = job_items
        .iter()
        .find(|j| j.name == "postprocess")
        .expect("Postprocess job not found");

    let deps = postprocess_job
        .depends_on_job_ids
        .as_ref()
        .expect("No dependencies found");
    assert_eq!(
        deps.len(),
        3,
        "Expected 3 dependencies (work_1, work_2, work_3)"
    );
}

#[rstest]
fn test_create_workflow_with_regex_file_dependencies(start_server: &ServerProcess) {
    use tempfile::TempDir;

    // Create temp directory and files
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file1 = temp_dir.path().join("data_1.txt");
    let file2 = temp_dir.path().join("data_2.txt");
    let file3 = temp_dir.path().join("data_3.txt");
    fs::write(&file1, "data 1").expect("Failed to write file1");
    fs::write(&file2, "data 2").expect("Failed to write file2");
    fs::write(&file3, "data 3").expect("Failed to write file3");

    let workflow_data = serde_json::json!({
        "name": "regex_file_deps_workflow",
        "user": "regex_user",
        "description": "Test workflow with regex file dependencies",
        "jobs": [
            {
                "name": "aggregate",
                "command": "echo 'aggregate all data files'",
                "input_file_regexes": [r"data_\d+"],
            }
        ],
        "files": [
            {
                "name": "data_1",
                "path": file1.to_str().unwrap(),
            },
            {
                "name": "data_2",
                "path": file2.to_str().unwrap(),
            },
            {
                "name": "data_3",
                "path": file3.to_str().unwrap(),
            }
        ],
        "user_data": null,
        "resource_requirements": null,
        "slurm_schedulers": null
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "regex_user",
        false,
        false,
    )
    .expect("Failed to create workflow with regex file dependencies");

    assert!(workflow_id > 0);

    // Verify that aggregate job has all 3 data files as inputs
    let jobs = default_api::list_jobs(
        &start_server.config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(true), // include_relationships - needed for tests that check dependencies/files
        None,       // active_compute_node_id
    )
    .expect("Failed to list jobs");

    let job_items = jobs.items.unwrap();
    let aggregate_job = job_items
        .iter()
        .find(|j| j.name == "aggregate")
        .expect("Aggregate job not found");

    let input_files = aggregate_job
        .input_file_ids
        .as_ref()
        .expect("No input files found");
    assert_eq!(
        input_files.len(),
        3,
        "Expected 3 input files (data_1.txt, data_2.txt, data_3.txt)"
    );
}

#[rstest]
fn test_create_workflow_with_regex_user_data_dependencies(start_server: &ServerProcess) {
    let workflow_data = serde_json::json!({
        "name": "regex_user_data_deps_workflow",
        "user": "regex_user",
        "description": "Test workflow with regex user data dependencies",
        "jobs": [
            {
                "name": "process_all_configs",
                "command": "echo 'process all config data'",
                "input_user_data_regexes": ["config_.*"],
            }
        ],
        "files": null,
        "user_data": [
            {
                "name": "config_dev",
                "data": {"env": "development"},
            },
            {
                "name": "config_test",
                "data": {"env": "test"},
            },
            {
                "name": "config_prod",
                "data": {"env": "production"},
            },
            {
                "name": "other_data",
                "data": {"type": "other"},
            }
        ],
        "resource_requirements": null,
        "slurm_schedulers": null
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "regex_user",
        false,
        false,
    )
    .expect("Failed to create workflow with regex user data dependencies");

    assert!(workflow_id > 0);

    // Verify that process_all_configs job has only the config_* user data (not other_data)
    let jobs = default_api::list_jobs(
        &start_server.config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(true), // include_relationships - needed for tests that check dependencies/files
        None,       // active_compute_node_id
    )
    .expect("Failed to list jobs");

    let job_items = jobs.items.unwrap();
    let process_job = job_items
        .iter()
        .find(|j| j.name == "process_all_configs")
        .expect("Process job not found");

    let input_user_data = process_job
        .input_user_data_ids
        .as_ref()
        .expect("No input user data found");
    assert_eq!(
        input_user_data.len(),
        3,
        "Expected 3 user data items (config_dev, config_test, config_prod, but not other_data)"
    );
}

#[rstest]
fn test_create_workflow_with_mixed_exact_and_regex_dependencies(start_server: &ServerProcess) {
    let workflow_data = serde_json::json!({
        "name": "mixed_deps_workflow",
        "user": "regex_user",
        "description": "Test workflow with both exact and regex dependencies",
        "jobs": [
            {
                "name": "init",
                "command": "echo 'init'",
            },
            {
                "name": "process_1",
                "command": "echo 'process 1'",
                "depends_on": ["init"],
            },
            {
                "name": "process_2",
                "command": "echo 'process 2'",
                "depends_on": ["init"],
            },
            {
                "name": "special",
                "command": "echo 'special'",
                "depends_on": ["init"],
            },
            {
                "name": "finalize",
                "command": "echo 'finalize'",
                "depends_on": ["special"],
                "depends_on_regexes": ["process_.*"],
            }
        ],
        "files": null,
        "user_data": null,
        "resource_requirements": null,
        "slurm_schedulers": null
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "regex_user",
        false,
        false,
    )
    .expect("Failed to create workflow with mixed dependencies");

    assert!(workflow_id > 0);

    // Verify that finalize job has dependencies on special + process_1 + process_2
    let jobs = default_api::list_jobs(
        &start_server.config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(true), // include_relationships - needed for tests that check dependencies/files
        None,       // active_compute_node_id
    )
    .expect("Failed to list jobs");

    let job_items = jobs.items.unwrap();
    let finalize_job = job_items
        .iter()
        .find(|j| j.name == "finalize")
        .expect("Finalize job not found");

    let deps = finalize_job
        .depends_on_job_ids
        .as_ref()
        .expect("No dependencies found");
    assert_eq!(
        deps.len(),
        3,
        "Expected 3 dependencies (special, process_1, process_2)"
    );
}

#[rstest]
fn test_create_workflows_from_all_example_files(start_server: &ServerProcess) {
    // Define the subdirectories to check
    let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
    let subdirs = vec!["yaml", "json", "kdl"];

    let mut all_spec_files = Vec::new();

    // Iterate over each subdirectory and collect workflow spec files
    for subdir in &subdirs {
        let subdir_path = examples_dir.join(subdir);

        // Skip if subdirectory doesn't exist
        if !subdir_path.exists() {
            eprintln!(
                "Warning: Subdirectory {:?} does not exist, skipping",
                subdir
            );
            continue;
        }

        let spec_files: Vec<PathBuf> = fs::read_dir(&subdir_path)
            .unwrap_or_else(|e| panic!("Failed to read {} directory: {}", subdir, e))
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.is_file() {
                    let extension = path.extension()?.to_str()?;
                    if matches!(extension, "json" | "json5" | "yaml" | "kdl" | "yml") {
                        return Some(path);
                    }
                }
                None
            })
            .collect();

        eprintln!(
            "Found {} workflow spec files in examples/{}/",
            spec_files.len(),
            subdir
        );

        all_spec_files.extend(spec_files);
    }

    eprintln!(
        "\nTotal: {} workflow spec files across all subdirectories\n",
        all_spec_files.len()
    );
    assert!(
        !all_spec_files.is_empty(),
        "No workflow spec files found in examples subdirectories"
    );

    // Test each spec file
    for spec_file in all_spec_files {
        eprintln!(
            "Testing workflow spec: {:?}",
            spec_file.strip_prefix(&examples_dir).unwrap_or(&spec_file)
        );

        // Read the spec to get the user field
        let spec = WorkflowSpec::from_spec_file(&spec_file)
            .unwrap_or_else(|e| panic!("Failed to read spec from {:?}: {}", spec_file, e));

        let user = spec.user.unwrap_or_else(|| "test_user".to_string());

        // Create the workflow
        let workflow_id = WorkflowSpec::create_workflow_from_spec(
            &start_server.config,
            &spec_file,
            &user,
            false,
            false,
        )
        .unwrap_or_else(|e| panic!("Failed to create workflow from {:?}: {}", spec_file, e));

        assert!(workflow_id > 0, "Invalid workflow ID for {:?}", spec_file);

        // Verify the workflow was created by fetching it
        let created_workflow = default_api::get_workflow(&start_server.config, workflow_id)
            .unwrap_or_else(|e| {
                panic!("Failed to get created workflow from {:?}: {}", spec_file, e)
            });

        assert_eq!(
            created_workflow.id,
            Some(workflow_id),
            "Workflow ID mismatch for {:?}",
            spec_file
        );
        assert_eq!(
            created_workflow.user, user,
            "Workflow user mismatch for {:?}",
            spec_file
        );

        eprintln!(
            "  ✓ Successfully created and verified workflow '{}' (ID: {})",
            created_workflow.name, workflow_id
        );

        default_api::delete_workflow(&start_server.config, workflow_id, None)
            .expect("Warning: Failed to delete workflow");
    }
}

// =============================================================================
// Scheduler Node Validation Tests
// =============================================================================

/// Test that validation fails when a job requests a different multi-node count
/// than the scheduler provides (e.g., job wants 3 nodes, scheduler allocates 2)
#[rstest]
fn test_scheduler_node_validation_fails_with_mismatched_nodes(start_server: &ServerProcess) {
    let workflow_data = serde_json::json!({
        "name": "multi_node_mismatch_workflow",
        "description": "Workflow with mismatched scheduler nodes",
        "jobs": [
            {
                "name": "job1",
                "command": "echo hello",
                "resource_requirements": "three_node_req",
                "scheduler": "multi_node_scheduler"
            }
        ],
        "files": null,
        "user_data": null,
        "resource_requirements": [
            {
                "name": "three_node_req",
                "num_cpus": 1,
                "num_nodes": 3,
                "memory": "1g",
                "runtime": "PT1H"
            }
        ],
        "slurm_schedulers": [
            {
                "name": "multi_node_scheduler",
                "account": "test_account",
                "nodes": 2,
                "walltime": "01:00:00"
            }
        ],
        "actions": [
            {
                "trigger_type": "on_workflow_start",
                "action_type": "schedule_nodes",
                "scheduler": "multi_node_scheduler",
                "scheduler_type": "slurm"
            }
        ]
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "test_user",
        false,
        false, // skip_checks = false
    );

    // Should fail: job requests 3 nodes but scheduler only allocates 2
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Scheduler node validation failed"),
        "Expected scheduler node validation error, got: {}",
        err_msg
    );
    assert!(
        err_msg.contains("multi_node_scheduler"),
        "Error should mention the scheduler name: {}",
        err_msg
    );
}

/// Test that single-node jobs in a multi-node allocation are valid (Pattern 1)
#[rstest]
fn test_scheduler_node_validation_passes_single_node_jobs_in_multi_node_allocation(
    start_server: &ServerProcess,
) {
    let workflow_data = serde_json::json!({
        "name": "single_node_in_multi_alloc",
        "description": "Single-node jobs in a multi-node allocation",
        "jobs": [
            {
                "name": "job1",
                "command": "echo hello",
                "resource_requirements": "single_node_req",
                "scheduler": "multi_node_scheduler"
            }
        ],
        "files": null,
        "user_data": null,
        "resource_requirements": [
            {
                "name": "single_node_req",
                "num_cpus": 1,
                "num_nodes": 1,
                "memory": "1g",
                "runtime": "PT1H"
            }
        ],
        "slurm_schedulers": [
            {
                "name": "multi_node_scheduler",
                "account": "test_account",
                "nodes": 2,
                "walltime": "01:00:00"
            }
        ],
        "actions": [
            {
                "trigger_type": "on_workflow_start",
                "action_type": "schedule_nodes",
                "scheduler": "multi_node_scheduler",
                "scheduler_type": "slurm"
            }
        ]
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "test_user",
        false,
        false,
    );

    assert!(
        result.is_ok(),
        "Single-node jobs in a multi-node allocation should be valid, got: {:?}",
        result.err()
    );

    if let Ok(workflow_id) = result {
        let _ = default_api::delete_workflow(&start_server.config, workflow_id, None);
    }
}

/// Test that start_one_worker_per_node is accepted for direct-mode workflows.
#[rstest]
fn test_scheduler_node_validation_passes_with_start_one_worker_per_node(
    start_server: &ServerProcess,
) {
    let workflow_data = serde_json::json!({
        "name": "multi_node_with_workers_workflow",
        "description": "Workflow with multi-node scheduler and start_one_worker_per_node",
        "jobs": [
            {
                "name": "job1",
                "command": "echo hello",
                "resource_requirements": "single_node_req",
                "scheduler": "multi_node_scheduler"
            }
        ],
        "files": null,
        "user_data": null,
        "resource_requirements": [
            {
                "name": "single_node_req",
                "num_cpus": 1,
                "num_nodes": 1,
                "memory": "1g",
                "runtime": "PT1H"
            }
        ],
        "slurm_schedulers": [
            {
                "name": "multi_node_scheduler",
                "account": "test_account",
                "nodes": 2,
                "walltime": "01:00:00"
            }
        ],
        "execution_config": {
            "mode": "direct"
        },
        "actions": [
            {
                "trigger_type": "on_workflow_start",
                "action_type": "schedule_nodes",
                "scheduler": "multi_node_scheduler",
                "scheduler_type": "slurm",
                "start_one_worker_per_node": true
            }
        ]
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "test_user",
        false,
        false, // skip_checks = false
    );

    // Should succeed for direct mode workflows.
    assert!(
        result.is_ok(),
        "Expected success with start_one_worker_per_node in spec, got: {:?}",
        result.err()
    );

    // Clean up
    if let Ok(workflow_id) = result {
        let _ = default_api::delete_workflow(&start_server.config, workflow_id, None);
    }
}

#[rstest]
fn test_scheduler_node_validation_fails_with_start_one_worker_per_node_in_slurm_mode(
    start_server: &ServerProcess,
) {
    let workflow_data = serde_json::json!({
        "name": "multi_node_with_workers_slurm_mode",
        "description": "Workflow with start_one_worker_per_node in slurm execution mode",
        "jobs": [
            {
                "name": "job1",
                "command": "echo hello",
                "resource_requirements": "single_node_req",
                "scheduler": "multi_node_scheduler"
            }
        ],
        "resource_requirements": [
            {
                "name": "single_node_req",
                "num_cpus": 1,
                "num_nodes": 1,
                "memory": "1g",
                "runtime": "PT1H"
            }
        ],
        "slurm_schedulers": [
            {
                "name": "multi_node_scheduler",
                "account": "test_account",
                "nodes": 2,
                "walltime": "01:00:00"
            }
        ],
        "execution_config": {
            "mode": "slurm"
        },
        "actions": [
            {
                "trigger_type": "on_workflow_start",
                "action_type": "schedule_nodes",
                "scheduler": "multi_node_scheduler",
                "scheduler_type": "slurm",
                "start_one_worker_per_node": true
            }
        ]
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "test_user",
        false,
        false,
    );

    assert!(result.is_err(), "Expected workflow creation to fail");
    let err = result.err().unwrap().to_string();
    assert!(
        err.contains("start_one_worker_per_node requires execution_config.mode to be 'direct'"),
        "Unexpected error: {}",
        err
    );
}

/// Test that start_one_worker_per_node is rejected when execution_config.mode is auto (default).
#[rstest]
fn test_scheduler_node_validation_fails_with_start_one_worker_per_node_in_auto_mode(
    start_server: &ServerProcess,
) {
    let workflow_data = serde_json::json!({
        "name": "multi_node_with_workers_auto_mode",
        "description": "Workflow with start_one_worker_per_node in auto execution mode",
        "jobs": [
            {
                "name": "job1",
                "command": "echo hello",
                "resource_requirements": "single_node_req",
                "scheduler": "multi_node_scheduler"
            }
        ],
        "resource_requirements": [
            {
                "name": "single_node_req",
                "num_cpus": 1,
                "num_nodes": 1,
                "memory": "1g",
                "runtime": "PT1H"
            }
        ],
        "slurm_schedulers": [
            {
                "name": "multi_node_scheduler",
                "account": "test_account",
                "nodes": 2,
                "walltime": "01:00:00"
            }
        ],
        "actions": [
            {
                "trigger_type": "on_workflow_start",
                "action_type": "schedule_nodes",
                "scheduler": "multi_node_scheduler",
                "scheduler_type": "slurm",
                "start_one_worker_per_node": true
            }
        ]
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "test_user",
        false,
        false,
    );

    assert!(result.is_err(), "Expected workflow creation to fail");
    let err = result.err().unwrap().to_string();
    assert!(
        err.contains("start_one_worker_per_node requires execution_config.mode to be 'direct'"),
        "Unexpected error: {}",
        err
    );
}

/// Test that validation passes when job num_nodes matches scheduler nodes
#[rstest]
fn test_scheduler_node_validation_passes_with_matching_nodes(start_server: &ServerProcess) {
    let workflow_data = serde_json::json!({
        "name": "matching_nodes_workflow",
        "description": "Workflow with matching job and scheduler nodes",
        "jobs": [
            {
                "name": "job1",
                "command": "echo hello",
                "resource_requirements": "multi_node_req",
                "scheduler": "multi_node_scheduler"
            }
        ],
        "files": null,
        "user_data": null,
        "resource_requirements": [
            {
                "name": "multi_node_req",
                "num_cpus": 1,
                "num_nodes": 2,
                "memory": "1g",
                "runtime": "PT1H"
            }
        ],
        "slurm_schedulers": [
            {
                "name": "multi_node_scheduler",
                "account": "test_account",
                "nodes": 2,
                "walltime": "01:00:00"
            }
        ],
        "actions": [
            {
                "trigger_type": "on_workflow_start",
                "action_type": "schedule_nodes",
                "scheduler": "multi_node_scheduler",
                "scheduler_type": "slurm"
            }
        ]
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "test_user",
        false,
        false, // skip_checks = false
    );

    // Should succeed because job num_nodes matches scheduler nodes
    assert!(
        result.is_ok(),
        "Expected success with matching nodes, got: {:?}",
        result.err()
    );

    // Clean up
    if let Ok(workflow_id) = result {
        let _ = default_api::delete_workflow(&start_server.config, workflow_id, None);
    }
}

/// Test that skip_checks=true bypasses the validation
#[rstest]
fn test_scheduler_node_validation_skipped_with_skip_checks(start_server: &ServerProcess) {
    let workflow_data = serde_json::json!({
        "name": "skip_checks_workflow",
        "description": "Workflow that would fail validation but uses skip_checks",
        "jobs": [
            {
                "name": "job1",
                "command": "echo hello",
                "resource_requirements": "three_node_req",
                "scheduler": "multi_node_scheduler"
            }
        ],
        "files": null,
        "user_data": null,
        "resource_requirements": [
            {
                "name": "three_node_req",
                "num_cpus": 1,
                "num_nodes": 3,
                "memory": "1g",
                "runtime": "PT1H"
            }
        ],
        "slurm_schedulers": [
            {
                "name": "multi_node_scheduler",
                "account": "test_account",
                "nodes": 2,
                "walltime": "01:00:00"
            }
        ],
        "actions": [
            {
                "trigger_type": "on_workflow_start",
                "action_type": "schedule_nodes",
                "scheduler": "multi_node_scheduler",
                "scheduler_type": "slurm"
            }
        ]
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "test_user",
        false,
        true, // skip_checks = true
    );

    // Should succeed because skip_checks is true
    assert!(
        result.is_ok(),
        "Expected success with skip_checks=true, got: {:?}",
        result.err()
    );

    // Clean up
    if let Ok(workflow_id) = result {
        let _ = default_api::delete_workflow(&start_server.config, workflow_id, None);
    }
}

/// Test that single-node schedulers pass validation without any special requirements
#[rstest]
fn test_scheduler_node_validation_passes_with_single_node_scheduler(start_server: &ServerProcess) {
    let workflow_data = serde_json::json!({
        "name": "single_node_scheduler_workflow",
        "description": "Workflow with single-node scheduler (nodes=1)",
        "jobs": [
            {
                "name": "job1",
                "command": "echo hello",
                "scheduler": "single_node_scheduler"
            }
        ],
        "files": null,
        "user_data": null,
        "resource_requirements": null,
        "slurm_schedulers": [
            {
                "name": "single_node_scheduler",
                "account": "test_account",
                "nodes": 1,
                "walltime": "01:00:00"
            }
        ],
        "actions": [
            {
                "trigger_type": "on_workflow_start",
                "action_type": "schedule_nodes",
                "scheduler": "single_node_scheduler",
                "scheduler_type": "slurm"
            }
        ]
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "test_user",
        false,
        false, // skip_checks = false
    );

    // Should succeed because scheduler only has 1 node
    assert!(
        result.is_ok(),
        "Expected success with single-node scheduler, got: {:?}",
        result.err()
    );

    // Clean up
    if let Ok(workflow_id) = result {
        let _ = default_api::delete_workflow(&start_server.config, workflow_id, None);
    }
}

// =============================================================================
// Tests for validate_spec (dry-run functionality)
// =============================================================================

/// Test that validate_spec returns correct summary for a simple workflow
#[test]
fn test_validate_spec_basic_workflow() {
    let workflow_data = serde_json::json!({
        "name": "simple_workflow",
        "description": "A simple test workflow",
        "jobs": [
            {"name": "job1", "command": "echo hello"},
            {"name": "job2", "command": "echo world", "depends_on": ["job1"]}
        ]
    });

    let temp_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::validate_spec(temp_file.path());

    assert!(result.valid, "Expected validation to pass");
    assert!(result.errors.is_empty(), "Expected no errors");
    assert_eq!(result.summary.workflow_name, "simple_workflow");
    assert_eq!(
        result.summary.workflow_description,
        Some("A simple test workflow".to_string())
    );
    assert_eq!(result.summary.job_count, 2);
    assert_eq!(result.summary.job_count_before_expansion, 2);
    assert!(!result.summary.has_schedule_nodes_action);
}

/// Test that validate_spec correctly reports parameterized job expansion
#[test]
fn test_validate_spec_with_parameterization() {
    let workflow_data = serde_json::json!({
        "name": "parameterized_workflow",
        "description": "Workflow with parameterized jobs",
        "jobs": [
            {
                "name": "job_{i:03d}",
                "command": "echo Job {i}",
                "parameters": {
                    "i": "1:10"
                }
            }
        ]
    });

    let temp_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::validate_spec(temp_file.path());

    assert!(result.valid, "Expected validation to pass");
    assert!(result.errors.is_empty(), "Expected no errors");
    assert_eq!(result.summary.job_count, 10, "Should have 10 expanded jobs");
    assert_eq!(
        result.summary.job_count_before_expansion, 1,
        "Should have 1 job before expansion"
    );
    // Verify job names are expanded correctly
    assert!(result.summary.job_names.contains(&"job_001".to_string()));
    assert!(result.summary.job_names.contains(&"job_010".to_string()));
}

/// Test that validate_spec returns errors for invalid workflow
#[test]
fn test_validate_spec_with_invalid_actions() {
    let workflow_data = serde_json::json!({
        "name": "invalid_workflow",
        "jobs": [
            {"name": "job1", "command": "echo hello"}
        ],
        "actions": [
            {
                "trigger_type": "on_workflow_start",
                "action_type": "schedule_nodes"
                // Missing scheduler and scheduler_type - should fail validation
            }
        ]
    });

    let temp_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::validate_spec(temp_file.path());

    assert!(!result.valid, "Expected validation to fail");
    assert!(!result.errors.is_empty(), "Expected errors");
    // Should contain error about missing scheduler
    let error_text = result.errors.join(" ");
    assert!(
        error_text.contains("scheduler"),
        "Expected error about scheduler, got: {}",
        error_text
    );
}

/// Test that validate_spec returns errors for scheduler node mismatch
/// (job requests 3 nodes but scheduler only allocates 2)
#[test]
fn test_validate_spec_with_scheduler_error() {
    let workflow_data = serde_json::json!({
        "name": "scheduler_error_workflow",
        "jobs": [
            {
                "name": "job1",
                "command": "echo hello",
                "resource_requirements": "three_node_req",
                "scheduler": "multi_node_scheduler"
            }
        ],
        "resource_requirements": [
            {
                "name": "three_node_req",
                "num_nodes": 3,
                "num_cpus": 1,
                "memory": "1g"
            }
        ],
        "slurm_schedulers": [
            {
                "name": "multi_node_scheduler",
                "account": "test_account",
                "nodes": 2,
                "walltime": "01:00:00"
            }
        ],
        "actions": [
            {
                "trigger_type": "on_workflow_start",
                "action_type": "schedule_nodes",
                "scheduler": "multi_node_scheduler",
                "scheduler_type": "slurm"
            }
        ]
    });

    let temp_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::validate_spec(temp_file.path());

    // Should fail validation with errors (matches create behavior with skip_checks=false)
    assert!(!result.valid, "Expected validation to fail");
    assert!(!result.errors.is_empty(), "Expected errors");
    // Should error about scheduler node mismatch
    let error_text = result.errors.join(" ");
    assert!(
        error_text.contains("nodes"),
        "Expected error about nodes, got: {}",
        error_text
    );
}

/// Test that validate_spec reports file parse errors
#[test]
fn test_validate_spec_with_invalid_file() {
    let temp_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp file");
    fs::write(temp_file.path(), "not valid json {{{").expect("Failed to write temp file");

    let result = WorkflowSpec::validate_spec(temp_file.path());

    assert!(!result.valid, "Expected validation to fail");
    assert!(!result.errors.is_empty(), "Expected errors");
    let error_text = result.errors.join(" ");
    assert!(
        error_text.contains("parse"),
        "Expected parse error, got: {}",
        error_text
    );
}

/// Test that validate_spec reports file not found error
#[test]
fn test_validate_spec_with_missing_file() {
    let result = WorkflowSpec::validate_spec("/nonexistent/path/to/workflow.json");

    assert!(!result.valid, "Expected validation to fail");
    assert!(!result.errors.is_empty(), "Expected errors");
    let error_text = result.errors.join(" ");
    assert!(
        error_text.contains("parse") || error_text.contains("file"),
        "Expected file error, got: {}",
        error_text
    );
}

/// Test that validate_spec returns correct summary for workflow with all components
#[test]
fn test_validate_spec_complete_workflow() {
    let workflow_data = serde_json::json!({
        "name": "complete_workflow",
        "description": "A complete workflow with all components",
        "jobs": [
            {"name": "job1", "command": "echo hello", "resource_requirements": "small"},
            {"name": "job2", "command": "echo world", "depends_on": ["job1"]}
        ],
        "files": [
            {"name": "input_file", "path": "/tmp/input.txt"},
            {"name": "output_file", "path": "/tmp/output.txt"}
        ],
        "user_data": [
            {"name": "config", "data": {"key": "value"}}
        ],
        "resource_requirements": [
            {"name": "small", "num_cpus": 1, "num_nodes": 1, "memory": "1g"}
        ],
        "slurm_schedulers": [
            {"name": "test_scheduler", "account": "test", "nodes": 1, "walltime": "00:30:00"}
        ],
        "actions": [
            {
                "trigger_type": "on_workflow_start",
                "action_type": "schedule_nodes",
                "scheduler": "test_scheduler",
                "scheduler_type": "slurm"
            }
        ]
    });

    let temp_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::validate_spec(temp_file.path());

    assert!(result.valid, "Expected validation to pass");
    assert_eq!(result.summary.job_count, 2);
    assert_eq!(result.summary.file_count, 2);
    assert_eq!(result.summary.user_data_count, 1);
    assert_eq!(result.summary.resource_requirements_count, 1);
    assert_eq!(result.summary.slurm_scheduler_count, 1);
    assert_eq!(result.summary.action_count, 1);
    assert!(result.summary.has_schedule_nodes_action);
    assert_eq!(
        result.summary.scheduler_names,
        vec!["test_scheduler".to_string()]
    );
}

/// Test that validate_spec detects duplicate job names
#[test]
fn test_validate_spec_duplicate_job_names() {
    let workflow_data = serde_json::json!({
        "name": "duplicate_job_names_workflow",
        "jobs": [
            {"name": "job1", "command": "echo hello"},
            {"name": "job1", "command": "echo world"}
        ]
    });

    let temp_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::validate_spec(temp_file.path());

    assert!(!result.valid, "Expected validation to fail");
    let error_text = result.errors.join(" ");
    assert!(
        error_text.contains("Duplicate job name") && error_text.contains("job1"),
        "Expected duplicate job name error, got: {}",
        error_text
    );
}

/// Test that validate_spec detects duplicate file names
#[test]
fn test_validate_spec_duplicate_file_names() {
    let workflow_data = serde_json::json!({
        "name": "duplicate_file_names_workflow",
        "jobs": [
            {"name": "job1", "command": "echo hello"}
        ],
        "files": [
            {"name": "file1", "path": "/tmp/file1.txt"},
            {"name": "file1", "path": "/tmp/file2.txt"}
        ]
    });

    let temp_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::validate_spec(temp_file.path());

    assert!(!result.valid, "Expected validation to fail");
    let error_text = result.errors.join(" ");
    assert!(
        error_text.contains("Duplicate file name") && error_text.contains("file1"),
        "Expected duplicate file name error, got: {}",
        error_text
    );
}

/// Test that validate_spec detects non-existent job dependencies
#[test]
fn test_validate_spec_nonexistent_dependency() {
    let workflow_data = serde_json::json!({
        "name": "nonexistent_dependency_workflow",
        "jobs": [
            {"name": "job1", "command": "echo hello", "depends_on": ["nonexistent_job"]}
        ]
    });

    let temp_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::validate_spec(temp_file.path());

    assert!(!result.valid, "Expected validation to fail");
    let error_text = result.errors.join(" ");
    assert!(
        error_text.contains("depends_on non-existent job")
            && error_text.contains("nonexistent_job"),
        "Expected non-existent dependency error, got: {}",
        error_text
    );
}

/// Test that validate_spec detects non-existent resource_requirements reference
#[test]
fn test_validate_spec_nonexistent_resource_requirements() {
    let workflow_data = serde_json::json!({
        "name": "nonexistent_rr_workflow",
        "jobs": [
            {"name": "job1", "command": "echo hello", "resource_requirements": "nonexistent_rr"}
        ]
    });

    let temp_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::validate_spec(temp_file.path());

    assert!(!result.valid, "Expected validation to fail");
    let error_text = result.errors.join(" ");
    assert!(
        error_text.contains("non-existent resource_requirements")
            && error_text.contains("nonexistent_rr"),
        "Expected non-existent resource_requirements error, got: {}",
        error_text
    );
}

/// Test that validate_spec detects non-existent scheduler reference
#[test]
fn test_validate_spec_nonexistent_scheduler() {
    let workflow_data = serde_json::json!({
        "name": "nonexistent_scheduler_workflow",
        "jobs": [
            {"name": "job1", "command": "echo hello", "scheduler": "nonexistent_scheduler"}
        ]
    });

    let temp_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::validate_spec(temp_file.path());

    assert!(!result.valid, "Expected validation to fail");
    let error_text = result.errors.join(" ");
    assert!(
        error_text.contains("non-existent scheduler")
            && error_text.contains("nonexistent_scheduler"),
        "Expected non-existent scheduler error, got: {}",
        error_text
    );
}

/// Test that validate_spec detects non-existent input file reference
#[test]
fn test_validate_spec_nonexistent_input_file() {
    let workflow_data = serde_json::json!({
        "name": "nonexistent_file_workflow",
        "jobs": [
            {"name": "job1", "command": "echo hello", "input_files": ["nonexistent_file"]}
        ]
    });

    let temp_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::validate_spec(temp_file.path());

    assert!(!result.valid, "Expected validation to fail");
    let error_text = result.errors.join(" ");
    assert!(
        error_text.contains("non-existent file") && error_text.contains("nonexistent_file"),
        "Expected non-existent file error, got: {}",
        error_text
    );
}

/// Test that validate_spec detects circular dependencies
#[test]
fn test_validate_spec_circular_dependency() {
    let workflow_data = serde_json::json!({
        "name": "circular_dependency_workflow",
        "jobs": [
            {"name": "job1", "command": "echo hello", "depends_on": ["job2"]},
            {"name": "job2", "command": "echo world", "depends_on": ["job1"]}
        ]
    });

    let temp_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::validate_spec(temp_file.path());

    assert!(!result.valid, "Expected validation to fail");
    let error_text = result.errors.join(" ");
    assert!(
        error_text.contains("Circular dependency"),
        "Expected circular dependency error, got: {}",
        error_text
    );
}

/// Test that validate_spec detects invalid regex in depends_on_regexes
#[test]
fn test_validate_spec_invalid_regex() {
    let workflow_data = serde_json::json!({
        "name": "invalid_regex_workflow",
        "jobs": [
            {"name": "job1", "command": "echo hello"},
            {"name": "job2", "command": "echo world", "depends_on_regexes": ["[invalid("]}
        ]
    });

    let temp_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::validate_spec(temp_file.path());

    assert!(!result.valid, "Expected validation to fail");
    let error_text = result.errors.join(" ");
    assert!(
        error_text.contains("invalid") && error_text.contains("depends_on_regexes"),
        "Expected invalid regex error, got: {}",
        error_text
    );
}

/// Test that validate_spec detects action referencing non-existent job
#[test]
fn test_validate_spec_action_nonexistent_job() {
    let workflow_data = serde_json::json!({
        "name": "action_nonexistent_job_workflow",
        "jobs": [
            {"name": "job1", "command": "echo hello"}
        ],
        "actions": [
            {
                "trigger_type": "on_jobs_complete",
                "action_type": "run_commands",
                "jobs": ["nonexistent_job"],
                "commands": ["echo done"]
            }
        ]
    });

    let temp_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::validate_spec(temp_file.path());

    assert!(!result.valid, "Expected validation to fail");
    let error_text = result.errors.join(" ");
    assert!(
        error_text.contains("non-existent job") && error_text.contains("nonexistent_job"),
        "Expected action non-existent job error, got: {}",
        error_text
    );
}

/// Test that validate_spec detects action referencing non-existent scheduler
#[test]
fn test_validate_spec_action_nonexistent_scheduler() {
    let workflow_data = serde_json::json!({
        "name": "action_nonexistent_scheduler_workflow",
        "jobs": [
            {"name": "job1", "command": "echo hello"}
        ],
        "actions": [
            {
                "trigger_type": "on_workflow_start",
                "action_type": "schedule_nodes",
                "scheduler": "nonexistent_scheduler",
                "scheduler_type": "slurm"
            }
        ]
    });

    let temp_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::validate_spec(temp_file.path());

    assert!(!result.valid, "Expected validation to fail");
    let error_text = result.errors.join(" ");
    assert!(
        error_text.contains("non-existent") && error_text.contains("scheduler"),
        "Expected action non-existent scheduler error, got: {}",
        error_text
    );
}

/// Test that validate_spec warns about heterogeneous schedulers without jobs_sort_method
#[test]
fn test_validate_spec_heterogeneous_schedulers_warning() {
    let workflow_data = serde_json::json!({
        "name": "heterogeneous_schedulers_workflow",
        "jobs": [
            {"name": "job1", "command": "echo hello"},
            {"name": "job2", "command": "echo world"}
        ],
        "slurm_schedulers": [
            {
                "name": "small_scheduler",
                "account": "test",
                "mem": "4G",
                "walltime": "01:00:00",
                "nodes": 1
            },
            {
                "name": "big_scheduler",
                "account": "test",
                "mem": "128G",
                "walltime": "04:00:00",
                "nodes": 1
            }
        ],
        "actions": [
            {
                "trigger_type": "on_workflow_start",
                "action_type": "schedule_nodes",
                "scheduler": "small_scheduler",
                "scheduler_type": "slurm"
            },
            {
                "trigger_type": "on_workflow_start",
                "action_type": "schedule_nodes",
                "scheduler": "big_scheduler",
                "scheduler_type": "slurm"
            }
        ]
    });

    let temp_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::validate_spec(temp_file.path());

    // Should be valid but with warnings
    assert!(result.valid, "Expected validation to pass");
    assert!(!result.warnings.is_empty(), "Expected warnings");
    let warning_text = result.warnings.join(" ");
    assert!(
        warning_text.contains("jobs_sort_method"),
        "Expected warning about jobs_sort_method, got: {}",
        warning_text
    );
}

/// Test that validate_spec does NOT warn when jobs_sort_method is set
#[test]
fn test_validate_spec_no_warning_with_sort_method() {
    let workflow_data = serde_json::json!({
        "name": "heterogeneous_with_sort_workflow",
        "jobs_sort_method": "gpus_runtime_memory",
        "jobs": [
            {"name": "job1", "command": "echo hello"},
            {"name": "job2", "command": "echo world"}
        ],
        "slurm_schedulers": [
            {
                "name": "small_scheduler",
                "account": "test",
                "mem": "4G",
                "walltime": "01:00:00",
                "nodes": 1
            },
            {
                "name": "big_scheduler",
                "account": "test",
                "mem": "128G",
                "walltime": "04:00:00",
                "nodes": 1
            }
        ],
        "actions": [
            {
                "trigger_type": "on_workflow_start",
                "action_type": "schedule_nodes",
                "scheduler": "small_scheduler",
                "scheduler_type": "slurm"
            },
            {
                "trigger_type": "on_workflow_start",
                "action_type": "schedule_nodes",
                "scheduler": "big_scheduler",
                "scheduler_type": "slurm"
            }
        ]
    });

    let temp_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::validate_spec(temp_file.path());

    assert!(result.valid, "Expected validation to pass");
    assert!(
        result.warnings.is_empty(),
        "Expected no warnings when jobs_sort_method is set, got: {:?}",
        result.warnings
    );
}

/// Test that validate_spec does NOT warn when all jobs have explicit scheduler assignments
#[test]
fn test_validate_spec_no_warning_with_scheduler_assignments() {
    let workflow_data = serde_json::json!({
        "name": "heterogeneous_with_assignments_workflow",
        "jobs": [
            {"name": "job1", "command": "echo hello", "scheduler": "small_scheduler"},
            {"name": "job2", "command": "echo world", "scheduler": "big_scheduler"}
        ],
        "slurm_schedulers": [
            {
                "name": "small_scheduler",
                "account": "test",
                "mem": "4G",
                "walltime": "01:00:00",
                "nodes": 1
            },
            {
                "name": "big_scheduler",
                "account": "test",
                "mem": "128G",
                "walltime": "04:00:00",
                "nodes": 1
            }
        ],
        "actions": [
            {
                "trigger_type": "on_workflow_start",
                "action_type": "schedule_nodes",
                "scheduler": "small_scheduler",
                "scheduler_type": "slurm"
            },
            {
                "trigger_type": "on_workflow_start",
                "action_type": "schedule_nodes",
                "scheduler": "big_scheduler",
                "scheduler_type": "slurm"
            }
        ]
    });

    let temp_file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::validate_spec(temp_file.path());

    assert!(result.valid, "Expected validation to pass");
    assert!(
        result.warnings.is_empty(),
        "Expected no warnings when all jobs have scheduler assignments, got: {:?}",
        result.warnings
    );
}

// =============================================================================
// Subgraph Workflow Tests
// =============================================================================

/// Test that the subgraph workflow examples parse correctly in all formats
#[test]
fn test_subgraph_workflow_parses_in_all_formats() {
    let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/subgraphs");

    // Test each format with slurm schedulers
    let formats = vec![
        "subgraphs_workflow.json",
        "subgraphs_workflow.json5",
        "subgraphs_workflow.yaml",
        "subgraphs_workflow.kdl",
    ];

    for format in formats {
        let spec_file = examples_dir.join(format);
        if !spec_file.exists() {
            eprintln!("Skipping {} (file not found)", format);
            continue;
        }

        let mut spec = WorkflowSpec::from_spec_file(&spec_file)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", format, e));

        assert_eq!(
            spec.name, "two_subgraph_pipeline",
            "Workflow name mismatch for {}",
            format
        );

        // Expand parameters to get the full job list
        spec.expand_parameters()
            .unwrap_or_else(|e| panic!("Failed to expand parameters for {}: {}", format, e));

        assert_eq!(spec.jobs.len(), 15, "Expected 15 jobs for {}", format);
        assert!(
            spec.slurm_schedulers.is_some(),
            "Expected slurm_schedulers for {}",
            format
        );
        assert!(spec.actions.is_some(), "Expected actions for {}", format);

        eprintln!(
            "✓ {} parses correctly with {} jobs",
            format,
            spec.jobs.len()
        );
    }
}

/// Test that the no_slurm versions parse correctly
#[test]
fn test_subgraph_workflow_no_slurm_parses_in_all_formats() {
    let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/subgraphs");

    let formats = vec![
        "subgraphs_workflow_no_slurm.json",
        "subgraphs_workflow_no_slurm.json5",
        "subgraphs_workflow_no_slurm.yaml",
        "subgraphs_workflow_no_slurm.kdl",
    ];

    for format in formats {
        let spec_file = examples_dir.join(format);
        if !spec_file.exists() {
            eprintln!("Skipping {} (file not found)", format);
            continue;
        }

        let mut spec = WorkflowSpec::from_spec_file(&spec_file)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", format, e));

        assert_eq!(
            spec.name, "two_subgraph_pipeline",
            "Workflow name mismatch for {}",
            format
        );

        // Expand parameters to get the full job list
        spec.expand_parameters()
            .unwrap_or_else(|e| panic!("Failed to expand parameters for {}: {}", format, e));

        assert_eq!(spec.jobs.len(), 15, "Expected 15 jobs for {}", format);
        assert!(
            spec.slurm_schedulers.is_none(),
            "Expected no slurm_schedulers for {}",
            format
        );
        assert!(spec.actions.is_none(), "Expected no actions for {}", format);

        eprintln!(
            "✓ {} parses correctly with {} jobs (no slurm)",
            format,
            spec.jobs.len()
        );
    }
}

/// Test that execution plans have 4 stages for both slurm and no_slurm versions
#[test]
fn test_subgraph_workflow_execution_plan_has_4_stages() {
    use torc::client::execution_plan::ExecutionPlan;

    let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/subgraphs");

    // Test with slurm schedulers
    let slurm_spec_file = examples_dir.join("subgraphs_workflow.yaml");
    if slurm_spec_file.exists() {
        let mut spec =
            WorkflowSpec::from_spec_file(&slurm_spec_file).expect("Failed to parse slurm workflow");
        spec.expand_parameters()
            .expect("Failed to expand parameters");

        let plan = ExecutionPlan::from_spec(&spec).expect("Failed to build execution plan");

        // With the DAG structure, we have 6 events for the subgraph workflow:
        // 1. start event (prep_a, prep_b)
        // 2. prep_a completes -> work_a_1..5
        // 3. prep_b completes -> work_b_1..5
        // 4. work_a_* complete -> post_a
        // 5. work_b_* complete -> post_b
        // 6. post_a, post_b complete -> final
        assert_eq!(
            plan.events.len(),
            6,
            "Expected 6 events for slurm workflow (DAG structure), got {}",
            plan.events.len()
        );

        // Verify there's exactly one root event (start)
        assert_eq!(plan.root_events.len(), 1, "Should have 1 root event");

        // Verify the start event has workflow start trigger
        let start_event = plan.events.get(&plan.root_events[0]).unwrap();
        assert!(
            start_event.trigger_description.contains("Workflow Start"),
            "Root event should be workflow start"
        );

        eprintln!(
            "✓ Slurm workflow has {} events (DAG structure)",
            plan.events.len()
        );
    }

    // Test without slurm schedulers
    let no_slurm_spec_file = examples_dir.join("subgraphs_workflow_no_slurm.yaml");
    if no_slurm_spec_file.exists() {
        let mut spec = WorkflowSpec::from_spec_file(&no_slurm_spec_file)
            .expect("Failed to parse no_slurm workflow");
        spec.expand_parameters()
            .expect("Failed to expand parameters");

        let plan = ExecutionPlan::from_spec(&spec).expect("Failed to build execution plan");

        assert_eq!(
            plan.events.len(),
            6,
            "Expected 6 events for no_slurm workflow (DAG structure), got {}",
            plan.events.len()
        );

        eprintln!(
            "✓ No-slurm workflow has {} events (DAG structure)",
            plan.events.len()
        );
    }
}

/// Test that slurm and no_slurm versions produce the same execution plan structure
#[test]
fn test_subgraph_workflow_slurm_and_no_slurm_have_same_events() {
    use torc::client::execution_plan::ExecutionPlan;

    let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/subgraphs");

    let slurm_spec_file = examples_dir.join("subgraphs_workflow.yaml");
    let no_slurm_spec_file = examples_dir.join("subgraphs_workflow_no_slurm.yaml");

    if !slurm_spec_file.exists() || !no_slurm_spec_file.exists() {
        eprintln!("Skipping test - example files not found");
        return;
    }

    // Parse and expand both specs
    let mut slurm_spec =
        WorkflowSpec::from_spec_file(&slurm_spec_file).expect("Failed to parse slurm spec");
    slurm_spec
        .expand_parameters()
        .expect("Failed to expand slurm parameters");

    let mut no_slurm_spec =
        WorkflowSpec::from_spec_file(&no_slurm_spec_file).expect("Failed to parse no_slurm spec");
    no_slurm_spec
        .expand_parameters()
        .expect("Failed to expand no_slurm parameters");

    // Build execution plans
    let slurm_plan =
        ExecutionPlan::from_spec(&slurm_spec).expect("Failed to build slurm execution plan");
    let no_slurm_plan =
        ExecutionPlan::from_spec(&no_slurm_spec).expect("Failed to build no_slurm execution plan");

    // Verify same number of events
    assert_eq!(
        slurm_plan.events.len(),
        no_slurm_plan.events.len(),
        "Slurm and no_slurm workflows should have the same number of events"
    );

    // Collect all jobs becoming ready from both plans
    let mut slurm_all_jobs: Vec<String> = slurm_plan
        .events
        .values()
        .flat_map(|e| e.jobs_becoming_ready.clone())
        .collect();
    slurm_all_jobs.sort();

    let mut no_slurm_all_jobs: Vec<String> = no_slurm_plan
        .events
        .values()
        .flat_map(|e| e.jobs_becoming_ready.clone())
        .collect();
    no_slurm_all_jobs.sort();

    assert_eq!(
        slurm_all_jobs, no_slurm_all_jobs,
        "Both plans should make the same jobs ready"
    );

    eprintln!(
        "✓ Both versions have {} events with {} total jobs",
        slurm_plan.events.len(),
        slurm_all_jobs.len()
    );
}

/// Test that all format pairs (slurm vs no_slurm) produce identical execution plans
#[test]
fn test_subgraph_workflow_all_formats_produce_same_execution_plan() {
    use torc::client::execution_plan::ExecutionPlan;

    let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/subgraphs");

    // Compare JSON vs YAML (slurm versions)
    let json_file = examples_dir.join("subgraphs_workflow.json");
    let yaml_file = examples_dir.join("subgraphs_workflow.yaml");

    if json_file.exists() && yaml_file.exists() {
        let mut json_spec = WorkflowSpec::from_spec_file(&json_file).expect("Failed to parse JSON");
        json_spec
            .expand_parameters()
            .expect("Failed to expand JSON parameters");

        let mut yaml_spec = WorkflowSpec::from_spec_file(&yaml_file).expect("Failed to parse YAML");
        yaml_spec
            .expand_parameters()
            .expect("Failed to expand YAML parameters");

        let json_plan = ExecutionPlan::from_spec(&json_spec).expect("Failed to build JSON plan");
        let yaml_plan = ExecutionPlan::from_spec(&yaml_spec).expect("Failed to build YAML plan");

        assert_eq!(
            json_plan.events.len(),
            yaml_plan.events.len(),
            "JSON and YAML should have same number of events"
        );

        // Verify total job counts match
        let json_job_count: usize = json_plan
            .events
            .values()
            .map(|e| e.jobs_becoming_ready.len())
            .sum();
        let yaml_job_count: usize = yaml_plan
            .events
            .values()
            .map(|e| e.jobs_becoming_ready.len())
            .sum();

        assert_eq!(
            json_job_count, yaml_job_count,
            "Total job counts should match between JSON and YAML"
        );

        eprintln!("✓ JSON and YAML produce identical execution plans");
    }

    // Compare no_slurm versions
    let json_no_slurm = examples_dir.join("subgraphs_workflow_no_slurm.json");
    let yaml_no_slurm = examples_dir.join("subgraphs_workflow_no_slurm.yaml");

    if json_no_slurm.exists() && yaml_no_slurm.exists() {
        let mut json_spec =
            WorkflowSpec::from_spec_file(&json_no_slurm).expect("Failed to parse JSON no_slurm");
        json_spec
            .expand_parameters()
            .expect("Failed to expand parameters");

        let mut yaml_spec =
            WorkflowSpec::from_spec_file(&yaml_no_slurm).expect("Failed to parse YAML no_slurm");
        yaml_spec
            .expand_parameters()
            .expect("Failed to expand parameters");

        let json_plan = ExecutionPlan::from_spec(&json_spec).expect("Failed to build plan");
        let yaml_plan = ExecutionPlan::from_spec(&yaml_spec).expect("Failed to build plan");

        assert_eq!(
            json_plan.events.len(),
            yaml_plan.events.len(),
            "No_slurm JSON and YAML should have same number of events"
        );

        eprintln!("✓ No_slurm JSON and YAML produce identical execution plans");
    }
}

/// Test subgraph workflow job structure
#[test]
fn test_subgraph_workflow_job_structure() {
    let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/subgraphs");
    let spec_file = examples_dir.join("subgraphs_workflow_no_slurm.yaml");

    if !spec_file.exists() {
        eprintln!("Skipping test - example file not found");
        return;
    }

    let mut spec = WorkflowSpec::from_spec_file(&spec_file).expect("Failed to parse spec");
    spec.expand_parameters()
        .expect("Failed to expand parameters");

    // Verify job counts after expansion
    // prep_a, prep_b = 2
    // work_a_1..5, work_b_1..5 = 10
    // post_a, post_b = 2
    // final = 1
    // Total = 15
    assert_eq!(spec.jobs.len(), 15, "Expected 15 jobs after expansion");

    // Verify prep jobs have no dependencies
    let prep_a = spec.jobs.iter().find(|j| j.name == "prep_a");
    assert!(prep_a.is_some(), "prep_a job not found");
    assert!(
        prep_a.unwrap().depends_on.is_none()
            || prep_a.unwrap().depends_on.as_ref().unwrap().is_empty(),
        "prep_a should have no explicit dependencies"
    );

    // Verify work jobs have input_files
    let work_a_1 = spec.jobs.iter().find(|j| j.name == "work_a_1");
    assert!(work_a_1.is_some(), "work_a_1 job not found");
    assert!(
        work_a_1.unwrap().input_files.is_some(),
        "work_a_1 should have input_files"
    );

    // Verify post jobs have input_files from work jobs
    let post_a = spec.jobs.iter().find(|j| j.name == "post_a");
    assert!(post_a.is_some(), "post_a job not found");
    let post_a_inputs = post_a.unwrap().input_files.as_ref().unwrap();
    assert_eq!(
        post_a_inputs.len(),
        5,
        "post_a should have 5 input files from work_a jobs"
    );

    // Verify final job has input_files from both post jobs
    let final_job = spec.jobs.iter().find(|j| j.name == "final");
    assert!(final_job.is_some(), "final job not found");
    let final_inputs = final_job.unwrap().input_files.as_ref().unwrap();
    assert_eq!(
        final_inputs.len(),
        2,
        "final should have 2 input files (post_a_out, post_b_out)"
    );

    eprintln!("✓ Job structure verified: 15 jobs with correct dependencies");
}

/// Test that subgraph workflow resource requirements are preserved
#[test]
fn test_subgraph_workflow_resource_requirements() {
    let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/subgraphs");
    let spec_file = examples_dir.join("subgraphs_workflow_no_slurm.yaml");

    if !spec_file.exists() {
        eprintln!("Skipping test - example file not found");
        return;
    }

    let spec = WorkflowSpec::from_spec_file(&spec_file).expect("Failed to parse spec");

    let resource_reqs = spec
        .resource_requirements
        .as_ref()
        .expect("Missing resource_requirements");
    assert_eq!(
        resource_reqs.len(),
        5,
        "Expected 5 resource requirement definitions"
    );

    // Verify specific resource requirements
    let small = resource_reqs.iter().find(|r| r.name == "small");
    assert!(small.is_some(), "small resource requirement not found");
    assert_eq!(small.unwrap().num_cpus, 1);

    let work_large = resource_reqs.iter().find(|r| r.name == "work_large");
    assert!(
        work_large.is_some(),
        "work_large resource requirement not found"
    );
    assert_eq!(work_large.unwrap().num_cpus, 8);
    assert_eq!(work_large.unwrap().memory, "32g");

    let work_gpu = resource_reqs.iter().find(|r| r.name == "work_gpu");
    assert!(
        work_gpu.is_some(),
        "work_gpu resource requirement not found"
    );
    assert_eq!(work_gpu.unwrap().num_gpus, 1);

    eprintln!("✓ Resource requirements verified");
}

/// Integration test: create subgraph workflows on server
#[rstest]
fn test_create_subgraph_workflows_from_examples(start_server: &ServerProcess) {
    let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/subgraphs");

    // Test both slurm and no_slurm YAML versions
    let test_files = vec![
        ("subgraphs_workflow.yaml", true),           // has schedulers
        ("subgraphs_workflow_no_slurm.yaml", false), // no schedulers
    ];

    for (filename, has_schedulers) in test_files {
        let spec_file = examples_dir.join(filename);
        if !spec_file.exists() {
            eprintln!("Skipping {} (file not found)", filename);
            continue;
        }

        let workflow_id = WorkflowSpec::create_workflow_from_spec(
            &start_server.config,
            &spec_file,
            "test_user",
            false,
            true, // skip_checks - we don't have a real Slurm environment
        )
        .unwrap_or_else(|e| panic!("Failed to create workflow from {}: {}", filename, e));

        assert!(workflow_id > 0, "Invalid workflow ID for {}", filename);

        // Verify the workflow was created
        let workflow = default_api::get_workflow(&start_server.config, workflow_id)
            .expect("Failed to get workflow");
        assert_eq!(workflow.name, "two_subgraph_pipeline");

        // Verify job count
        let jobs = default_api::list_jobs(
            &start_server.config,
            workflow_id,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None, // active_compute_node_id
        )
        .expect("Failed to list jobs");

        let job_count = jobs.items.as_ref().map(|j| j.len()).unwrap_or(0);
        assert_eq!(
            job_count, 15,
            "Expected 15 jobs for {}, got {}",
            filename, job_count
        );

        // Verify schedulers if present
        if has_schedulers {
            let response = default_api::list_slurm_schedulers(
                &start_server.config,
                workflow_id,
                Some(0),  // offset
                Some(50), // limit
                None,     // sort_by
                None,     // reverse_sort
                None,     // name filter
                None,     // account filter
                None,     // gres filter
                None,     // mem filter
                None,     // nodes filter
                None,     // partition filter
                None,     // qos filter
                None,     // tmp filter
                None,     // walltime filter
            )
            .expect("Failed to list schedulers");
            let sched_count = response.items.unwrap_or_default().len();
            assert!(
                sched_count > 0,
                "Expected schedulers for {}, got {}",
                filename,
                sched_count
            );
            eprintln!(
                "✓ {} created with {} jobs and {} schedulers",
                filename, job_count, sched_count
            );
        } else {
            eprintln!(
                "✓ {} created with {} jobs (no schedulers)",
                filename, job_count
            );
        }

        // Clean up
        let _ = default_api::delete_workflow(&start_server.config, workflow_id, None);
    }
}

/// Test that generate_schedulers_for_workflow assigns correct trigger types
/// Jobs without dependencies get on_workflow_start, jobs with dependencies get on_jobs_ready
#[test]
fn test_subgraph_workflow_generated_actions_have_correct_triggers() {
    use torc::client::commands::slurm::{
        GroupByStrategy, WalltimeStrategy, generate_schedulers_for_workflow,
    };
    use torc::client::hpc::kestrel::kestrel_profile;

    let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/subgraphs");
    let no_slurm_spec_file = examples_dir.join("subgraphs_workflow_no_slurm.yaml");

    if !no_slurm_spec_file.exists() {
        eprintln!("Skipping test - example file not found");
        return;
    }

    // Parse the no_slurm spec
    let mut spec =
        WorkflowSpec::from_spec_file(&no_slurm_spec_file).expect("Failed to parse no_slurm spec");

    // Generate schedulers
    let profile = kestrel_profile();
    let result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,                                 // single_allocation
        GroupByStrategy::ResourceRequirements, // group_by
        WalltimeStrategy::MaxJobRuntime,       // walltime_strategy
        1.5,                                   // walltime_multiplier
        true,                                  // add_actions
        false,                                 // overwrite
    )
    .expect("Failed to generate schedulers");

    eprintln!(
        "Generated {} schedulers and {} actions",
        result.scheduler_count, result.action_count
    );

    let actions = spec
        .actions
        .as_ref()
        .expect("Should have generated actions");

    // Build map of scheduler -> trigger_type
    let scheduler_triggers: std::collections::HashMap<String, String> = actions
        .iter()
        .filter_map(|a| a.scheduler.clone().map(|s| (s, a.trigger_type.clone())))
        .collect();

    // Verify each job has the correct trigger type based on dependencies
    // prep_a, prep_b: no dependencies -> on_workflow_start
    // work_*: depend on prep_* outputs -> on_jobs_ready
    // post_*: depend on work_* outputs -> on_jobs_ready
    // final: depends on post_* outputs -> on_jobs_ready
    for job in &spec.jobs {
        let sched = job
            .scheduler
            .as_ref()
            .unwrap_or_else(|| panic!("Job {} should have scheduler assigned", job.name));
        let trigger = scheduler_triggers
            .get(sched)
            .unwrap_or_else(|| panic!("Scheduler {} should have action", sched));

        let expected_trigger = if job.name == "prep_a" || job.name == "prep_b" {
            "on_workflow_start"
        } else {
            "on_jobs_ready"
        };

        assert_eq!(
            trigger, expected_trigger,
            "Job {} (scheduler {}) should have trigger {}, got {}",
            job.name, sched, expected_trigger, trigger
        );
    }

    eprintln!("✓ All jobs have correct trigger types");
}

/// Test that execution plan from database correctly computes dependencies from file relationships
#[test]
fn test_subgraph_workflow_execution_plan_from_database() {
    use torc::client::execution_plan::ExecutionPlan;

    let start_server = common::start_server();

    let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/subgraphs");
    let spec_file = examples_dir.join("subgraphs_workflow.yaml");

    if !spec_file.exists() {
        eprintln!("Skipping test - example file not found");
        return;
    }

    // Create workflow on server
    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        &spec_file,
        "test_user",
        false,
        true, // skip_checks
    )
    .expect("Failed to create workflow");

    // Fetch workflow, jobs (with relationships), and actions from server
    let workflow = default_api::get_workflow(&start_server.config, workflow_id)
        .expect("Failed to get workflow");

    let jobs = default_api::list_jobs(
        &start_server.config,
        workflow_id,
        None,
        None,
        None,
        None,
        Some(10000),
        None,
        None,
        Some(true), // include_relationships - this is key!
        None,       // active_compute_node_id
    )
    .expect("Failed to list jobs")
    .items
    .unwrap_or_default();

    let actions = default_api::get_workflow_actions(&start_server.config, workflow_id)
        .expect("Failed to get actions");

    let slurm_schedulers = default_api::list_slurm_schedulers(
        &start_server.config,
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
        None,
    )
    .map(|r| r.items.unwrap_or_default())
    .unwrap_or_default();

    let resource_requirements = default_api::list_resource_requirements(
        &start_server.config,
        workflow_id,
        None, // job_id
        None, // offset
        None, // limit
        None, // sort_by
        None, // reverse_sort
        None, // name
        None, // memory
        None, // num_cpus
        None, // num_gpus
        None, // num_nodes
        None, // runtime
    )
    .map(|r| r.items.unwrap_or_default())
    .unwrap_or_default();

    // Build execution plan from database models
    let plan = ExecutionPlan::from_database_models(
        &workflow,
        &jobs,
        &actions,
        &slurm_schedulers,
        &resource_requirements,
    )
    .expect("Failed to build execution plan from database");

    // With the DAG structure, we have 6 events:
    // 1. start event (prep_a, prep_b)
    // 2. prep_a completes -> work_a_1..5
    // 3. prep_b completes -> work_b_1..5
    // 4. work_a_* complete -> post_a
    // 5. work_b_* complete -> post_b
    // 6. post_a, post_b complete -> final
    assert_eq!(
        plan.events.len(),
        6,
        "Expected 6 events from database (DAG structure), got {}",
        plan.events.len()
    );

    // Find the start event
    let start_event = plan
        .events
        .get(&plan.root_events[0])
        .expect("Start event not found");

    // Verify start event has 2 jobs (prep_a, prep_b)
    assert_eq!(
        start_event.jobs_becoming_ready.len(),
        2,
        "Start event should have 2 jobs, got {} - {:?}",
        start_event.jobs_becoming_ready.len(),
        start_event.jobs_becoming_ready
    );
    assert!(
        start_event
            .jobs_becoming_ready
            .contains(&"prep_a".to_string()),
        "Start event should contain prep_a"
    );
    assert!(
        start_event
            .jobs_becoming_ready
            .contains(&"prep_b".to_string()),
        "Start event should contain prep_b"
    );

    // Collect all jobs becoming ready across all events
    let all_jobs: Vec<String> = plan
        .events
        .values()
        .flat_map(|e| e.jobs_becoming_ready.clone())
        .collect();

    // Should have 15 total jobs
    assert_eq!(
        all_jobs.len(),
        15,
        "Total jobs across all events should be 15, got {}",
        all_jobs.len()
    );

    // Verify final job is in a leaf event
    let leaf_event = plan
        .events
        .get(&plan.leaf_events[0])
        .expect("Leaf event not found");
    assert!(
        leaf_event
            .jobs_becoming_ready
            .contains(&"final".to_string()),
        "Leaf event should contain final job"
    );

    // Clean up
    let _ = default_api::delete_workflow(&start_server.config, workflow_id, None);

    eprintln!("✓ Execution plan from database has correct 6 events (DAG structure)");
}

/// Test that execution plan from spec matches execution plan from database
#[test]
fn test_subgraph_workflow_execution_plan_spec_vs_database() {
    use torc::client::execution_plan::ExecutionPlan;

    let start_server = common::start_server();

    let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/subgraphs");
    let spec_file = examples_dir.join("subgraphs_workflow.yaml");

    if !spec_file.exists() {
        eprintln!("Skipping test - example file not found");
        return;
    }

    // Build execution plan directly from spec
    let mut spec = WorkflowSpec::from_spec_file(&spec_file).expect("Failed to parse spec");
    spec.expand_parameters()
        .expect("Failed to expand parameters");
    let spec_plan = ExecutionPlan::from_spec(&spec).expect("Failed to build plan from spec");

    // Create workflow on server
    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        &spec_file,
        "test_user",
        false,
        true, // skip_checks
    )
    .expect("Failed to create workflow");

    // Fetch workflow, jobs (with relationships), and actions from server
    let workflow = default_api::get_workflow(&start_server.config, workflow_id)
        .expect("Failed to get workflow");

    let jobs = default_api::list_jobs(
        &start_server.config,
        workflow_id,
        None,
        None,
        None,
        None,
        Some(10000),
        None,
        None,
        Some(true), // include_relationships
        None,       // active_compute_node_id
    )
    .expect("Failed to list jobs")
    .items
    .unwrap_or_default();

    let actions = default_api::get_workflow_actions(&start_server.config, workflow_id)
        .expect("Failed to get actions");

    let slurm_schedulers = default_api::list_slurm_schedulers(
        &start_server.config,
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
        None,
    )
    .map(|r| r.items.unwrap_or_default())
    .unwrap_or_default();

    let resource_requirements = default_api::list_resource_requirements(
        &start_server.config,
        workflow_id,
        None, // job_id
        None, // offset
        None, // limit
        None, // sort_by
        None, // reverse_sort
        None, // name
        None, // memory
        None, // num_cpus
        None, // num_gpus
        None, // num_nodes
        None, // runtime
    )
    .map(|r| r.items.unwrap_or_default())
    .unwrap_or_default();

    // Build execution plan from database models
    let db_plan = ExecutionPlan::from_database_models(
        &workflow,
        &jobs,
        &actions,
        &slurm_schedulers,
        &resource_requirements,
    )
    .expect("Failed to build plan from database");

    // Compare event counts
    assert_eq!(
        spec_plan.events.len(),
        db_plan.events.len(),
        "Spec and database should have same number of events"
    );

    // Collect all jobs from both plans and compare
    let mut spec_all_jobs: Vec<String> = spec_plan
        .events
        .values()
        .flat_map(|e| e.jobs_becoming_ready.clone())
        .collect();
    spec_all_jobs.sort();

    let mut db_all_jobs: Vec<String> = db_plan
        .events
        .values()
        .flat_map(|e| e.jobs_becoming_ready.clone())
        .collect();
    db_all_jobs.sort();

    assert_eq!(
        spec_all_jobs,
        db_all_jobs,
        "Spec and database should have same total jobs.\nSpec: {:?}\nDB: {:?}",
        spec_all_jobs.len(),
        db_all_jobs.len()
    );

    eprintln!(
        "✓ Both plans have {} events with {} total jobs",
        spec_plan.events.len(),
        spec_all_jobs.len()
    );

    // Clean up
    let _ = default_api::delete_workflow(&start_server.config, workflow_id, None);

    eprintln!("✓ Execution plan from spec matches execution plan from database");
}
