mod common;

use common::{
    ServerProcess, create_test_file, create_test_job, create_test_workflow, run_cli_with_json,
    start_server,
};
use rstest::rstest;
use serde_json::json;
use torc::client::apis;

#[rstest]
fn test_files_add_command_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_files_add_workflow");
    let workflow_id = workflow.id.unwrap();

    // Test the CLI create command with JSON output
    let args = [
        "files",
        "create",
        &workflow_id.to_string(),
        "--name",
        "test_input_file",
        "--path",
        "/data/input.txt",
    ];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to run files create command");

    assert!(json_output.get("id").is_some());
    assert_eq!(json_output.get("workflow_id").unwrap(), &json!(workflow_id));
    assert_eq!(json_output.get("name").unwrap(), &json!("test_input_file"));
    assert_eq!(json_output.get("path").unwrap(), &json!("/data/input.txt"));
    // st_mtime should be None for newly created files
    assert!(
        json_output.get("st_mtime").is_none() || json_output.get("st_mtime").unwrap().is_null()
    );
}

#[rstest]
fn test_files_add_various_paths(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_file_paths_workflow");
    let workflow_id = workflow.id.unwrap();

    // Test different path formats
    let test_cases = [
        ("absolute_unix", "/home/user/data.txt"),
        ("absolute_windows", "C:\\Users\\user\\data.txt"),
        ("relative_simple", "data/input.csv"),
        ("relative_parent", "../data/config.json"),
        ("with_spaces", "/path with spaces/file name.txt"),
        ("with_special_chars", "/data/file-name_v1.2.txt"),
        (
            "deep_nested",
            "/very/deep/nested/directory/structure/file.dat",
        ),
        ("hidden_file", "/home/user/.hidden_config"),
        ("no_extension", "/data/README"),
        ("multiple_dots", "/data/archive.tar.gz"),
    ];

    for (test_name, file_path) in &test_cases {
        let args = [
            "files",
            "create",
            &workflow_id.to_string(),
            "--name",
            test_name,
            "--path",
            file_path,
        ];

        let json_output = run_cli_with_json(&args, start_server, None)
            .unwrap_or_else(|_| panic!("Failed to create file with path: {}", file_path));

        assert_eq!(json_output.get("name").unwrap(), &json!(test_name));
        assert_eq!(json_output.get("path").unwrap(), &json!(file_path));
        assert_eq!(json_output.get("workflow_id").unwrap(), &json!(workflow_id));
    }
}

#[rstest]
fn test_files_add_different_file_types(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_file_types_workflow");
    let workflow_id = workflow.id.unwrap();

    // Test different file types that might be used in workflows
    let file_types = [
        ("data_csv", "/datasets/train.csv", "Training data"),
        (
            "data_json",
            "/config/params.json",
            "Configuration parameters",
        ),
        (
            "script_python",
            "/scripts/preprocess.py",
            "Python preprocessing script",
        ),
        ("script_bash", "/scripts/run.sh", "Shell script"),
        (
            "model_file",
            "/models/trained_model.pkl",
            "Trained ML model",
        ),
        ("log_file", "/logs/execution.log", "Execution logs"),
        ("binary_data", "/data/features.bin", "Binary feature data"),
        ("image_file", "/images/diagram.png", "Workflow diagram"),
        (
            "archive",
            "/backups/checkpoint.tar.gz",
            "Checkpoint archive",
        ),
        ("executable", "/bin/custom_tool", "Custom executable"),
    ];

    for (name, path, description) in &file_types {
        let args = [
            "files",
            "create",
            &workflow_id.to_string(),
            "--name",
            &format!("{}_{}", name, description.len()), // Make names unique
            "--path",
            path,
        ];

        let json_output = run_cli_with_json(&args, start_server, None)
            .unwrap_or_else(|_| panic!("Failed to create {} file", name));

        assert_eq!(json_output.get("path").unwrap(), &json!(path));
        assert_eq!(json_output.get("workflow_id").unwrap(), &json!(workflow_id));
    }
}

#[rstest]
fn test_files_list_command_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow and files
    let workflow = create_test_workflow(config, "test_files_list_workflow");
    let workflow_id = workflow.id.unwrap();

    let _file1 = create_test_file(config, workflow_id, "input_data", "/data/input.csv");
    let _file2 = create_test_file(
        config,
        workflow_id,
        "output_results",
        "/results/output.json",
    );
    let _file3 = create_test_file(config, workflow_id, "config_file", "/config/settings.yaml");

    // Test the CLI list command
    let args = ["files", "list", &workflow_id.to_string(), "--limit", "10"];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to run files list command");

    // Verify JSON structure is an object with "files" field
    assert!(
        json_output.is_object(),
        "Files list should return an object"
    );
    assert!(
        json_output.get("files").is_some(),
        "Response should have 'files' field"
    );

    let files_array = json_output.get("files").unwrap().as_array().unwrap();
    assert!(files_array.len() >= 3, "Should have at least 3 files");

    // Verify each file has the expected structure
    for file in files_array {
        assert!(file.get("id").is_some());
        assert!(file.get("workflow_id").is_some());
        assert!(file.get("name").is_some());
        assert!(file.get("path").is_some());
        // st_mtime can be present or null
    }
}

#[rstest]
fn test_files_list_pagination(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_pagination_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create multiple files
    for i in 0..7 {
        let _file = create_test_file(
            config,
            workflow_id,
            &format!("file_{}", i),
            &format!("/data/file_{}.txt", i),
        );
    }

    // Test with limit
    let args = ["files", "list", &workflow_id.to_string(), "--limit", "4"];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to run paginated files list");

    let files_array = json_output.get("files").unwrap().as_array().unwrap();
    assert!(files_array.len() <= 4, "Should respect limit parameter");
    assert!(!files_array.is_empty(), "Should have at least one file");

    // Test with offset
    let args_with_offset = [
        "files",
        "list",
        &workflow_id.to_string(),
        "--limit",
        "3",
        "--offset",
        "3",
    ];

    let json_output_offset = run_cli_with_json(&args_with_offset, start_server, None)
        .expect("Failed to run files list with offset");

    let files_with_offset = json_output_offset.get("files").unwrap().as_array().unwrap();
    assert!(
        !files_with_offset.is_empty(),
        "Should have files with offset"
    );
}

#[rstest]
fn test_files_list_sorting(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_sorting_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create files with different names for sorting
    let _file_a = create_test_file(config, workflow_id, "aaa_file", "/data/aaa.txt");
    let _file_b = create_test_file(config, workflow_id, "bbb_file", "/data/bbb.txt");
    let _file_c = create_test_file(config, workflow_id, "ccc_file", "/data/ccc.txt");

    // Test sorting by name
    let args = [
        "files",
        "list",
        &workflow_id.to_string(),
        "--sort-by",
        "name",
    ];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to run sorted files list");

    let files_array = json_output.get("files").unwrap().as_array().unwrap();
    assert!(files_array.len() >= 3);

    // Test reverse sorting
    let args_reverse = [
        "files",
        "list",
        &workflow_id.to_string(),
        "--sort-by",
        "name",
        "--reverse-sort",
    ];

    let json_output_reverse = run_cli_with_json(&args_reverse, start_server, None)
        .expect("Failed to run reverse sorted files list");

    let files_array_reverse = json_output_reverse
        .get("files")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(files_array_reverse.len() >= 3);

    // Verify sorting worked - first items should be different in regular vs reverse
    if !files_array.is_empty() && !files_array_reverse.is_empty() {
        let first_regular = files_array[0].get("name").unwrap().as_str().unwrap();
        let first_reverse = files_array_reverse[0]
            .get("name")
            .unwrap()
            .as_str()
            .unwrap();

        // They should be different unless all names are the same
        if files_array.len() > 1 {
            let last_regular = files_array[files_array.len() - 1]
                .get("name")
                .unwrap()
                .as_str()
                .unwrap();
            // In alphabetical sort, first should be <= last, and in reverse it should be opposite
            assert!(first_regular <= last_regular || first_reverse >= first_regular);
        }
    }
}

#[rstest]
fn test_files_get_command_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test data
    let workflow = create_test_workflow(config, "test_files_get_workflow");
    let workflow_id = workflow.id.unwrap();
    let file = create_test_file(
        config,
        workflow_id,
        "test_get_file",
        "/path/to/important_data.csv",
    );
    let file_id = file.id.unwrap();

    // Test the CLI get command
    let args = ["files", "get", &file_id.to_string()];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to run files get command");

    // Verify JSON structure
    assert_eq!(json_output.get("id").unwrap(), &json!(file_id));
    assert_eq!(json_output.get("workflow_id").unwrap(), &json!(workflow_id));
    assert_eq!(json_output.get("name").unwrap(), &json!("test_get_file"));
    assert_eq!(
        json_output.get("path").unwrap(),
        &json!("/path/to/important_data.csv")
    );
}

#[rstest]
fn test_files_update_command_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test data
    let workflow = create_test_workflow(config, "test_files_update_workflow");
    let workflow_id = workflow.id.unwrap();
    let file = create_test_file(config, workflow_id, "original_file", "/original/path.txt");
    let file_id = file.id.unwrap();

    // Test the CLI update command
    let args = [
        "files",
        "update",
        &file_id.to_string(),
        "--name",
        "updated_file_name",
        "--path",
        "/new/updated/path.txt",
    ];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to run files update command");

    // Verify the updated values
    assert_eq!(json_output.get("id").unwrap(), &json!(file_id));
    assert_eq!(
        json_output.get("name").unwrap(),
        &json!("updated_file_name")
    );
    assert_eq!(
        json_output.get("path").unwrap(),
        &json!("/new/updated/path.txt")
    );

    // Verify unchanged values
    assert_eq!(json_output.get("workflow_id").unwrap(), &json!(workflow_id));
}

#[rstest]
fn test_files_update_partial_fields(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_partial_update_workflow");
    let workflow_id = workflow.id.unwrap();
    let file = create_test_file(
        config,
        workflow_id,
        "partial_update_file",
        "/original/path.dat",
    );
    let file_id = file.id.unwrap();

    // Test updating only name
    let args = [
        "files",
        "update",
        &file_id.to_string(),
        "--name",
        "only_name_updated",
    ];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to run partial files update");

    // Only name should be updated
    assert_eq!(
        json_output.get("name").unwrap(),
        &json!("only_name_updated")
    );
    // Path should remain unchanged
    assert_eq!(
        json_output.get("path").unwrap(),
        &json!("/original/path.dat")
    );

    // Test updating only path
    let args_path = [
        "files",
        "update",
        &file_id.to_string(),
        "--path",
        "/new/path/only.dat",
    ];

    let json_output_path =
        run_cli_with_json(&args_path, start_server, None).expect("Failed to run path-only update");

    // Path should be updated, name should remain from previous update
    assert_eq!(
        json_output_path.get("name").unwrap(),
        &json!("only_name_updated")
    );
    assert_eq!(
        json_output_path.get("path").unwrap(),
        &json!("/new/path/only.dat")
    );
}

#[rstest]
fn test_files_remove_command_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test data
    let workflow = create_test_workflow(config, "test_files_remove_workflow");
    let workflow_id = workflow.id.unwrap();
    let file = create_test_file(
        config,
        workflow_id,
        "file_to_remove",
        "/temp/will_be_deleted.tmp",
    );
    let file_id = file.id.unwrap();

    // Test the CLI delete command
    let args = ["files", "delete", &file_id.to_string()];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to run files delete command");

    // Verify JSON structure shows the removed file
    assert_eq!(json_output.get("id").unwrap(), &json!(file_id));
    assert_eq!(json_output.get("workflow_id").unwrap(), &json!(workflow_id));
    assert_eq!(json_output.get("name").unwrap(), &json!("file_to_remove"));
    assert_eq!(
        json_output.get("path").unwrap(),
        &json!("/temp/will_be_deleted.tmp")
    );

    // Verify the file is actually removed by trying to get it
    let get_result = apis::files_api::get_file(config, file_id);
    assert!(get_result.is_err(), "File should be deleted");
}

#[rstest]
fn test_files_long_names_and_paths(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_long_strings_workflow");
    let workflow_id = workflow.id.unwrap();

    // Test with very long file names and paths
    let long_name = "very_long_file_name_that_contains_many_characters_and_describes_exactly_what_this_file_contains_in_great_detail_for_testing_purposes";
    let long_path = "/extremely/long/path/with/many/nested/directories/that/might/be/used/in/complex/workflow/systems/where/deep/directory/structures/are/common/final_file.txt";

    let args = [
        "files",
        "create",
        &workflow_id.to_string(),
        "--name",
        long_name,
        "--path",
        long_path,
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to create file with long name and path");

    assert_eq!(json_output.get("name").unwrap(), &json!(long_name));
    assert_eq!(json_output.get("path").unwrap(), &json!(long_path));
    assert_eq!(json_output.get("workflow_id").unwrap(), &json!(workflow_id));
}

#[rstest]
fn test_files_special_characters_in_names(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_special_chars_workflow");
    let workflow_id = workflow.id.unwrap();

    // Test different special characters that might appear in file names
    let special_cases = [
        ("unicode_name", "файл_данных", "/data/unicode_файл.txt"),
        ("emoji_name", "data_📊_file", "/data/📊_results.json"),
        (
            "spaces_name",
            "file with spaces",
            "/path with spaces/file name.txt",
        ),
        ("symbols_name", "file-name_v1.2", "/data/file-name_v1.2.dat"),
        ("parentheses", "data(1)", "/backup/data(1).bak"),
        ("brackets", "config[prod]", "/config/app[prod].yaml"),
        ("ampersand", "data&results", "/output/data&results.csv"),
        ("percent", "progress_100%", "/status/progress_100%.log"),
        (
            "plus_minus",
            "delta_+5.2_-1.3",
            "/metrics/delta_+5.2_-1.3.txt",
        ),
        ("quotes", "config_'test'", "/tmp/config_'test'.json"),
    ];

    for (test_name, file_name, file_path) in &special_cases {
        let args = [
            "files",
            "create",
            &workflow_id.to_string(),
            "--name",
            file_name,
            "--path",
            file_path,
        ];

        let json_output = run_cli_with_json(&args, start_server, None).unwrap_or_else(|_| {
            panic!(
                "Failed to create file with special characters: {}",
                test_name
            )
        });

        assert_eq!(json_output.get("name").unwrap(), &json!(file_name));
        assert_eq!(json_output.get("path").unwrap(), &json!(file_path));
        assert_eq!(json_output.get("workflow_id").unwrap(), &json!(workflow_id));
    }
}

#[rstest]
fn test_files_list_required_existing_command_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_required_existing_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create files that are needed by jobs but not produced by any job (user-provided files)
    let user_file1 = create_test_file(config, workflow_id, "input1", "/data/input1.txt");
    let user_file2 = create_test_file(config, workflow_id, "input2", "/data/input2.txt");

    // Create an output file that will be produced by a job
    let output_file = create_test_file(config, workflow_id, "output", "/data/output.txt");

    // Create a job that needs the user files as input and produces an output
    let mut job = torc::models::JobModel::new(
        workflow_id,
        "test_job".to_string(),
        "cat /data/input1.txt /data/input2.txt > /data/output.txt".to_string(),
    );
    job.input_file_ids = Some(vec![user_file1.id.unwrap(), user_file2.id.unwrap()]);
    job.output_file_ids = Some(vec![output_file.id.unwrap()]);

    let _created_job =
        torc::client::apis::jobs_api::create_job(config, job).expect("Failed to create job");

    // Initialize the workflow to set up job dependencies
    torc::client::apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    // Test the CLI list-required-existing command
    let args = ["files", "list-required-existing", &workflow_id.to_string()];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run files list-required-existing command");

    // Verify JSON structure
    assert!(json_output.get("files").is_some());

    let files_array = json_output.get("files").unwrap().as_array().unwrap();

    // Should report the user files as missing (they're needed but not produced by jobs)
    let missing_file_ids: Vec<i64> = files_array.iter().map(|v| v.as_i64().unwrap()).collect();

    println!(
        "Missing file IDs: {:?} user_file1 {:?} user_file2 {:?}",
        missing_file_ids,
        user_file1.id.unwrap(),
        user_file2.id.unwrap()
    );
    assert!(missing_file_ids.contains(&user_file1.id.unwrap()));
    assert!(missing_file_ids.contains(&user_file2.id.unwrap()));

    // The output file should not be in the list since it's produced by a job, not user-provided
    assert!(!missing_file_ids.contains(&output_file.id.unwrap()));
}

#[rstest]
fn test_files_workflow_organization(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_workflow_organization");
    let workflow_id = workflow.id.unwrap();

    // Test a realistic workflow file organization
    let workflow_files = [
        // Input data files
        ("raw_training_data", "/datasets/train.csv"),
        ("raw_validation_data", "/datasets/validation.csv"),
        ("raw_test_data", "/datasets/test.csv"),
        // Configuration files
        ("model_config", "/config/model_params.yaml"),
        ("training_config", "/config/training.json"),
        ("hyperparameters", "/config/hyperparams.json"),
        // Code files
        ("preprocessing_script", "/src/preprocess.py"),
        ("training_script", "/src/train.py"),
        ("evaluation_script", "/src/evaluate.py"),
        ("utility_functions", "/src/utils.py"),
        // Output files
        ("trained_model", "/models/final_model.pkl"),
        ("model_metrics", "/results/metrics.json"),
        ("predictions", "/results/predictions.csv"),
        ("performance_report", "/results/report.html"),
        // Log files
        ("training_log", "/logs/training.log"),
        ("error_log", "/logs/errors.log"),
    ];

    for (name, path) in &workflow_files {
        let _file = create_test_file(config, workflow_id, name, path);
    }

    // List all files and verify organization
    let args = ["files", "list", &workflow_id.to_string()];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to list workflow organization files");

    let files_array = json_output.get("files").unwrap().as_array().unwrap();
    assert_eq!(files_array.len(), workflow_files.len());

    // Verify we have files from different categories
    let paths: Vec<String> = files_array
        .iter()
        .map(|f| f.get("path").unwrap().as_str().unwrap().to_string())
        .collect();

    let has_datasets = paths.iter().any(|p| p.starts_with("/datasets"));
    let has_config = paths.iter().any(|p| p.starts_with("/config"));
    let has_src = paths.iter().any(|p| p.starts_with("/src"));
    let has_models = paths.iter().any(|p| p.starts_with("/models"));
    let has_results = paths.iter().any(|p| p.starts_with("/results"));
    let has_logs = paths.iter().any(|p| p.starts_with("/logs"));

    assert!(has_datasets, "Should have dataset files");
    assert!(has_config, "Should have config files");
    assert!(has_src, "Should have source files");
    assert!(has_models, "Should have model files");
    assert!(has_results, "Should have result files");
    assert!(has_logs, "Should have log files");
}

#[rstest]
fn test_files_error_handling(start_server: &ServerProcess) {
    // Test getting a non-existent file
    let args = ["files", "get", "999999"];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(
        result.is_err(),
        "Should fail when getting non-existent file"
    );

    // Test updating a non-existent file
    let args = ["files", "update", "999999", "--name", "should_fail"];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(
        result.is_err(),
        "Should fail when updating non-existent file"
    );

    // Test removing a non-existent file
    let args = ["files", "delete", "999999"];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(
        result.is_err(),
        "Should fail when removing non-existent file"
    );
}

#[rstest]
fn test_files_list_empty_workflow(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow with no files
    let workflow = create_test_workflow(config, "test_empty_files_workflow");
    let workflow_id = workflow.id.unwrap();

    let args = ["files", "list", &workflow_id.to_string()];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to list files for empty workflow");

    let files_array = json_output.get("files").unwrap().as_array().unwrap();
    assert!(files_array.is_empty(), "Should have no files");
}

#[rstest]
fn test_files_list_with_produced_by_job_id_filter(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_produced_by_job_id_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create a test job
    let job = create_test_job(config, workflow_id, "producer_job");
    let job_id = job.id.unwrap();

    // Create some files (note: the actual association with job might be handled differently in the backend)
    let _file1 = create_test_file(config, workflow_id, "output1", "/output/file1.txt");
    let _file2 = create_test_file(config, workflow_id, "output2", "/output/file2.txt");
    let _file3 = create_test_file(config, workflow_id, "unrelated", "/other/file3.txt");

    // Test the CLI list command with produced_by_job_id filter
    let args = [
        "files",
        "list",
        &workflow_id.to_string(),
        "--produced-by-job-id",
        &job_id.to_string(),
    ];

    // This test mainly verifies that the CLI accepts the new parameter without errors
    // The actual filtering behavior depends on the backend database relationships
    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run files list command with produced_by_job_id filter");

    // Verify the response structure is correct
    assert!(
        json_output.is_object(),
        "Files list should return an object"
    );
    assert!(
        json_output.get("files").is_some(),
        "Response should have 'files' field"
    );

    // The actual number of results depends on backend implementation of job-file relationships
    // but the command should execute without error
}
