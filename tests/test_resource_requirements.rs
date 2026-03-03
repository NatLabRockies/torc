mod common;

use common::{
    ServerProcess, create_test_resource_requirements, create_test_workflow, run_cli_with_json,
    start_server,
};
use rstest::rstest;
use serde_json::json;
use torc::client::default_api;

#[rstest]
fn test_resource_requirements_add_command_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_resource_requirements_add_workflow");
    let workflow_id = workflow.id.unwrap();

    // Test the CLI create command with JSON output
    let args = [
        "resource-requirements",
        "create",
        &workflow_id.to_string(),
        "--name",
        "test_requirements",
        "--num-cpus",
        "4",
        "--num-gpus",
        "2",
        "--num-nodes",
        "1",
        "--memory",
        "8g",
        "--runtime",
        "P0DT2H",
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run resource-requirements create command");

    assert!(json_output.get("id").is_some());
    assert_eq!(json_output.get("workflow_id").unwrap(), &json!(workflow_id));
    assert_eq!(
        json_output.get("name").unwrap(),
        &json!("test_requirements")
    );
    assert_eq!(json_output.get("num_cpus").unwrap(), &json!(4));
    assert_eq!(json_output.get("num_gpus").unwrap(), &json!(2));
    assert_eq!(json_output.get("num_nodes").unwrap(), &json!(1));
    assert_eq!(json_output.get("memory").unwrap(), &json!("8g"));
    assert_eq!(json_output.get("runtime").unwrap(), &json!("P0DT2H"));
}

#[rstest]
fn test_resource_requirements_add_with_defaults(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_defaults_workflow");
    let workflow_id = workflow.id.unwrap();

    // Test with minimal arguments (should use defaults)
    let args = [
        "resource-requirements",
        "create",
        &workflow_id.to_string(),
        "--name",
        "minimal_requirements",
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run resource-requirements create with defaults");

    assert_eq!(
        json_output.get("name").unwrap(),
        &json!("minimal_requirements")
    );
    assert_eq!(json_output.get("num_cpus").unwrap(), &json!(1));
    assert_eq!(json_output.get("num_gpus").unwrap(), &json!(0));
    assert_eq!(json_output.get("num_nodes").unwrap(), &json!(1));
    assert_eq!(json_output.get("memory").unwrap(), &json!("1m"));
    assert_eq!(json_output.get("runtime").unwrap(), &json!("PT1M"));
}

#[rstest]
fn test_resource_requirements_add_high_performance(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_high_perf_workflow");
    let workflow_id = workflow.id.unwrap();

    // Test with high-performance requirements
    let args = [
        "resource-requirements",
        "create",
        &workflow_id.to_string(),
        "--name",
        "hpc_requirements",
        "--num-cpus",
        "64",
        "--num-gpus",
        "8",
        "--num-nodes",
        "4",
        "--memory",
        "256g",
        "--runtime",
        "P1DT0H",
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run resource-requirements create with high performance specs");

    assert_eq!(json_output.get("name").unwrap(), &json!("hpc_requirements"));
    assert_eq!(json_output.get("num_cpus").unwrap(), &json!(64));
    assert_eq!(json_output.get("num_gpus").unwrap(), &json!(8));
    assert_eq!(json_output.get("num_nodes").unwrap(), &json!(4));
    assert_eq!(json_output.get("memory").unwrap(), &json!("256g"));
    assert_eq!(json_output.get("runtime").unwrap(), &json!("P1DT0H"));
}

#[rstest]
fn test_resource_requirements_list_command_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow and resource requirements
    let workflow = create_test_workflow(config, "test_resource_requirements_list_workflow");
    let workflow_id = workflow.id.unwrap();

    let _req1 = create_test_resource_requirements(
        config,
        workflow_id,
        "cpu_intensive",
        8,
        0,
        1,
        "16g",
        "P0DT4H",
    );
    let _req2 = create_test_resource_requirements(
        config,
        workflow_id,
        "gpu_compute",
        4,
        2,
        1,
        "32g",
        "P0DT8H",
    );

    // Test the CLI list command
    let args = [
        "resource-requirements",
        "list",
        &workflow_id.to_string(),
        "--limit",
        "10",
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run resource-requirements list command");

    // Verify JSON structure is an object with "resource_requirements" field
    assert!(
        json_output.is_object(),
        "Resource requirements list should return an object"
    );
    assert!(
        json_output.get("resource_requirements").is_some(),
        "Response should have 'resource_requirements' field"
    );

    let requirements_array = json_output
        .get("resource_requirements")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(
        requirements_array.len() >= 2,
        "Should have at least 2 resource requirements"
    );

    // Verify each requirement has the expected structure
    for req in requirements_array {
        assert!(req.get("id").is_some());
        assert!(req.get("workflow_id").is_some());
        assert!(req.get("name").is_some());
        assert!(req.get("num_cpus").is_some());
        assert!(req.get("num_gpus").is_some());
        assert!(req.get("num_nodes").is_some());
        assert!(req.get("memory").is_some());
        assert!(req.get("runtime").is_some());
    }
}

#[rstest]
fn test_resource_requirements_list_pagination(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_pagination_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create multiple resource requirements
    for i in 0..5 {
        let _req = create_test_resource_requirements(
            config,
            workflow_id,
            &format!("req_{}", i),
            i + 1,
            0,
            1,
            &format!("{}g", i + 1),
            "P0DT1H",
        );
    }

    // Test with limit
    let args = [
        "resource-requirements",
        "list",
        &workflow_id.to_string(),
        "--limit",
        "3",
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run paginated resource-requirements list");

    let requirements_array = json_output
        .get("resource_requirements")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(
        requirements_array.len() <= 3,
        "Should respect limit parameter"
    );
    assert!(
        !requirements_array.is_empty(),
        "Should have at least one requirement"
    );

    // Test with offset
    let args_with_offset = [
        "resource-requirements",
        "list",
        &workflow_id.to_string(),
        "--limit",
        "2",
        "--offset",
        "2",
    ];

    let json_output_offset = run_cli_with_json(&args_with_offset, start_server, None)
        .expect("Failed to run resource-requirements list with offset");

    let requirements_with_offset = json_output_offset
        .get("resource_requirements")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(
        !requirements_with_offset.is_empty(),
        "Should have requirements with offset"
    );
}

#[rstest]
fn test_resource_requirements_list_sorting(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_sorting_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create requirements with different CPU counts for sorting
    let _req_low =
        create_test_resource_requirements(config, workflow_id, "low_cpu", 2, 0, 1, "4g", "P0DT1H");
    let _req_medium = create_test_resource_requirements(
        config,
        workflow_id,
        "medium_cpu",
        8,
        0,
        1,
        "16g",
        "P0DT2H",
    );
    let _req_high = create_test_resource_requirements(
        config,
        workflow_id,
        "high_cpu",
        16,
        0,
        1,
        "32g",
        "P0DT4H",
    );

    // Test sorting by num_cpus
    let args = [
        "resource-requirements",
        "list",
        &workflow_id.to_string(),
        "--sort-by",
        "num_cpus",
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run sorted resource-requirements list");

    let requirements_array = json_output
        .get("resource_requirements")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(requirements_array.len() >= 3);

    // Test reverse sorting
    let args_reverse = [
        "resource-requirements",
        "list",
        &workflow_id.to_string(),
        "--sort-by",
        "num_cpus",
        "--reverse-sort",
    ];

    let json_output_reverse = run_cli_with_json(&args_reverse, start_server, None)
        .expect("Failed to run reverse sorted resource-requirements list");

    let requirements_array_reverse = json_output_reverse
        .get("resource_requirements")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(requirements_array_reverse.len() >= 3);

    // With reverse sort, highest CPU count should come first
    if requirements_array_reverse.len() >= 2 {
        let first_cpus = requirements_array_reverse[0]
            .get("num_cpus")
            .unwrap()
            .as_i64()
            .unwrap();
        let second_cpus = requirements_array_reverse[1]
            .get("num_cpus")
            .unwrap()
            .as_i64()
            .unwrap();
        assert!(
            first_cpus >= second_cpus,
            "With reverse sort, higher CPU counts should come first"
        );
    }
}

#[rstest]
fn test_resource_requirements_get_command_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test data
    let workflow = create_test_workflow(config, "test_resource_requirements_get_workflow");
    let workflow_id = workflow.id.unwrap();
    let req = create_test_resource_requirements(
        config,
        workflow_id,
        "test_get_req",
        12,
        4,
        2,
        "64g",
        "P0DT12H",
    );
    let req_id = req.id.unwrap();

    // Test the CLI get command
    let args = ["resource-requirements", "get", &req_id.to_string()];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run resource-requirements get command");

    // Verify JSON structure
    assert_eq!(json_output.get("id").unwrap(), &json!(req_id));
    assert_eq!(json_output.get("workflow_id").unwrap(), &json!(workflow_id));
    assert_eq!(json_output.get("name").unwrap(), &json!("test_get_req"));
    assert_eq!(json_output.get("num_cpus").unwrap(), &json!(12));
    assert_eq!(json_output.get("num_gpus").unwrap(), &json!(4));
    assert_eq!(json_output.get("num_nodes").unwrap(), &json!(2));
    assert_eq!(json_output.get("memory").unwrap(), &json!("64g"));
    assert_eq!(json_output.get("runtime").unwrap(), &json!("P0DT12H"));
}

#[rstest]
fn test_resource_requirements_update_command_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test data
    let workflow = create_test_workflow(config, "test_resource_requirements_update_workflow");
    let workflow_id = workflow.id.unwrap();
    let req = create_test_resource_requirements(
        config,
        workflow_id,
        "test_update_req",
        4,
        1,
        1,
        "8g",
        "P0DT2H",
    );
    let req_id = req.id.unwrap();

    // Test the CLI update command
    let args = [
        "resource-requirements",
        "update",
        &req_id.to_string(),
        "--name",
        "updated_requirements",
        "--num-cpus",
        "8",
        "--num-gpus",
        "2",
        "--memory",
        "16g",
        "--runtime",
        "P0DT4H",
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run resource-requirements update command");

    // Verify the updated values
    assert_eq!(json_output.get("id").unwrap(), &json!(req_id));
    assert_eq!(
        json_output.get("name").unwrap(),
        &json!("updated_requirements")
    );
    assert_eq!(json_output.get("num_cpus").unwrap(), &json!(8));
    assert_eq!(json_output.get("num_gpus").unwrap(), &json!(2));
    assert_eq!(json_output.get("memory").unwrap(), &json!("16g"));
    assert_eq!(json_output.get("runtime").unwrap(), &json!("P0DT4H"));

    // Verify unchanged values
    assert_eq!(json_output.get("workflow_id").unwrap(), &json!(workflow_id));
    assert_eq!(json_output.get("num_nodes").unwrap(), &json!(1)); // Should remain unchanged
}

#[rstest]
fn test_resource_requirements_update_partial_fields(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_partial_update_workflow");
    let workflow_id = workflow.id.unwrap();
    let req = create_test_resource_requirements(
        config,
        workflow_id,
        "partial_update_req",
        4,
        0,
        1,
        "8g",
        "P0DT2H",
    );
    let req_id = req.id.unwrap();

    // Test updating only specific fields
    let args = [
        "resource-requirements",
        "update",
        &req_id.to_string(),
        "--num-cpus",
        "16",
        "--memory",
        "32g",
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run partial resource-requirements update");

    // Only specified fields should be updated
    assert_eq!(json_output.get("num_cpus").unwrap(), &json!(16));
    assert_eq!(json_output.get("memory").unwrap(), &json!("32g"));

    // Other fields should remain unchanged
    assert_eq!(
        json_output.get("name").unwrap(),
        &json!("partial_update_req")
    );
    assert_eq!(json_output.get("num_gpus").unwrap(), &json!(0));
    assert_eq!(json_output.get("num_nodes").unwrap(), &json!(1));
    assert_eq!(json_output.get("runtime").unwrap(), &json!("P0DT2H"));
}

#[rstest]
fn test_resource_requirements_remove_command_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test data
    let workflow = create_test_workflow(config, "test_resource_requirements_remove_workflow");
    let workflow_id = workflow.id.unwrap();
    let req = create_test_resource_requirements(
        config,
        workflow_id,
        "test_remove_req",
        4,
        1,
        1,
        "8g",
        "P0DT2H",
    );
    let req_id = req.id.unwrap();

    // Test the CLI delete command
    let args = ["resource-requirements", "delete", &req_id.to_string()];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run resource-requirements delete command");

    // Verify JSON structure shows the removed requirement
    assert_eq!(json_output.get("id").unwrap(), &json!(req_id));
    assert_eq!(json_output.get("workflow_id").unwrap(), &json!(workflow_id));
    assert_eq!(json_output.get("name").unwrap(), &json!("test_remove_req"));

    // Verify the requirement is actually removed by trying to get it
    let get_result = default_api::get_resource_requirements(config, req_id);
    assert!(
        get_result.is_err(),
        "Resource requirement should be deleted"
    );
}

#[rstest]
fn test_resource_requirements_memory_formats(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_memory_formats_workflow");
    let workflow_id = workflow.id.unwrap();

    // Test different memory format specifications
    let memory_formats = ["1m", "512k", "4g", "2t", "1024m", "8192k"];

    for (i, memory_format) in memory_formats.iter().enumerate() {
        let args = [
            "resource-requirements",
            "create",
            &workflow_id.to_string(),
            "--name",
            &format!("memory_test_{}", i),
            "--memory",
            memory_format,
        ];

        let json_output = run_cli_with_json(&args, start_server, None).unwrap_or_else(|_| {
            panic!(
                "Failed to create requirement with memory format {}",
                memory_format
            )
        });

        assert_eq!(json_output.get("memory").unwrap(), &json!(memory_format));
    }
}

#[rstest]
fn test_resource_requirements_runtime_formats(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_runtime_formats_workflow");
    let workflow_id = workflow.id.unwrap();

    // Test different ISO 8601 duration formats
    let runtime_formats = [
        "P0DT1M",    // 1 minute
        "P0DT30M",   // 30 minutes
        "P0DT1H",    // 1 hour
        "P0DT2H30M", // 2 hours 30 minutes
        "P1DT0H",    // 1 day
        "P7DT12H",   // 7 days 12 hours
    ];

    for (i, runtime_format) in runtime_formats.iter().enumerate() {
        let args = [
            "resource-requirements",
            "create",
            &workflow_id.to_string(),
            "--name",
            &format!("runtime_test_{}", i),
            "--runtime",
            runtime_format,
        ];

        let json_output = run_cli_with_json(&args, start_server, None).unwrap_or_else(|_| {
            panic!(
                "Failed to create requirement with runtime format {}",
                runtime_format
            )
        });

        assert_eq!(json_output.get("runtime").unwrap(), &json!(runtime_format));
    }
}

#[rstest]
fn test_resource_requirements_extreme_values(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_extreme_values_workflow");
    let workflow_id = workflow.id.unwrap();

    // Test with extreme resource values
    let test_cases = [
        ("zero_resources", 0, 0, 1, "0", "P0DT0M"),
        ("single_core", 1, 0, 1, "1g", "P0DT15M"),
        ("high_cpu", 128, 0, 1, "512g", "P30DT0H"),
        ("gpu_intensive", 4, 16, 2, "256g", "P7DT0H"),
        ("multi_node", 32, 8, 16, "1t", "P1DT0H"),
    ];

    for (name, cpus, gpus, nodes, memory, runtime) in &test_cases {
        let args = [
            "resource-requirements",
            "create",
            &workflow_id.to_string(),
            "--name",
            name,
            "--num-cpus",
            &cpus.to_string(),
            "--num-gpus",
            &gpus.to_string(),
            "--num-nodes",
            &nodes.to_string(),
            "--memory",
            memory,
            "--runtime",
            runtime,
        ];

        let json_output = run_cli_with_json(&args, start_server, None).unwrap_or_else(|_| {
            panic!(
                "Failed to create requirement with extreme values for {}",
                name
            )
        });

        assert_eq!(json_output.get("name").unwrap(), &json!(name));
        assert_eq!(json_output.get("num_cpus").unwrap(), &json!(cpus));
        assert_eq!(json_output.get("num_gpus").unwrap(), &json!(gpus));
        assert_eq!(json_output.get("num_nodes").unwrap(), &json!(nodes));
        assert_eq!(json_output.get("memory").unwrap(), &json!(memory));
        assert_eq!(json_output.get("runtime").unwrap(), &json!(runtime));
    }
}

#[rstest]
fn test_resource_requirements_update_all_fields(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_update_all_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create initial requirement
    let req = create_test_resource_requirements(
        config,
        workflow_id,
        "original_req",
        1,
        0,
        1,
        "1g",
        "P0DT1M",
    );
    let req_id = req.id.unwrap();

    // Update all fields
    let args = [
        "resource-requirements",
        "update",
        &req_id.to_string(),
        "--name",
        "completely_updated_req",
        "--num-cpus",
        "64",
        "--num-gpus",
        "8",
        "--num-nodes",
        "4",
        "--memory",
        "512g",
        "--runtime",
        "P3DT0H",
    ];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to update all fields");

    // Verify all fields were updated
    assert_eq!(
        json_output.get("name").unwrap(),
        &json!("completely_updated_req")
    );
    assert_eq!(json_output.get("num_cpus").unwrap(), &json!(64));
    assert_eq!(json_output.get("num_gpus").unwrap(), &json!(8));
    assert_eq!(json_output.get("num_nodes").unwrap(), &json!(4));
    assert_eq!(json_output.get("memory").unwrap(), &json!("512g"));
    assert_eq!(json_output.get("runtime").unwrap(), &json!("P3DT0H"));

    // Verify workflow_id and id remain unchanged
    assert_eq!(json_output.get("id").unwrap(), &json!(req_id));
    assert_eq!(json_output.get("workflow_id").unwrap(), &json!(workflow_id));
}

#[rstest]
fn test_resource_requirements_error_handling(start_server: &ServerProcess) {
    // Test getting a non-existent resource requirement
    let args = ["resource-requirements", "get", "999999"];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(
        result.is_err(),
        "Should fail when getting non-existent resource requirement"
    );

    // Test updating a non-existent resource requirement
    let args = [
        "resource-requirements",
        "update",
        "999999",
        "--name",
        "should_fail",
    ];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(
        result.is_err(),
        "Should fail when updating non-existent resource requirement"
    );

    // Test removing a non-existent resource requirement
    let args = ["resource-requirements", "delete", "999999"];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(
        result.is_err(),
        "Should fail when removing non-existent resource requirement"
    );
}

#[rstest]
fn test_resource_requirements_variations(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_name_variations_workflow");
    let workflow_id = workflow.id.unwrap();

    // Test different name patterns
    let test_names = [
        "simple_name",
        "name-with-dashes",
        "name_with_underscores",
        "Name With Spaces",
        "UPPERCASE_NAME",
        "MixedCaseNamE",
        "name123numbers",
        "name.with.dots",
        "very_long_name_that_contains_many_characters_to_test_length_limits",
    ];

    for name in &test_names {
        let args = [
            "resource-requirements",
            "create",
            &workflow_id.to_string(),
            "--name",
            name,
        ];

        let json_output = run_cli_with_json(&args, start_server, None)
            .unwrap_or_else(|_| panic!("Failed to create requirement with name: {}", name));

        assert_eq!(json_output.get("name").unwrap(), &json!(name));
    }
}

#[rstest]
fn test_resource_requirements_list_empty_workflow(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow with no resource requirements
    let workflow = create_test_workflow(config, "test_empty_requirements_workflow");
    let workflow_id = workflow.id.unwrap();

    let args = ["resource-requirements", "list", &workflow_id.to_string()];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to list resource requirements for empty workflow");

    let requirements_array = json_output
        .get("resource_requirements")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(
        requirements_array.len() == 1,
        "Should return array of length 1 for workflow with no resource requirements"
    );
}

#[rstest]
fn test_resource_requirements_mixed_workloads(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_mixed_workloads_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create requirements for different workload types
    let workloads = [
        ("web_server", 2, 0, 1, "4g", "P0DT24H"),
        ("database", 8, 0, 1, "32g", "P0DT12H"),
        ("ml_training", 16, 4, 1, "128g", "P2DT0H"),
        ("batch_processing", 32, 0, 2, "64g", "P0DT6H"),
        ("gpu_inference", 4, 2, 1, "16g", "P0DT1H"),
    ];

    for (name, cpus, gpus, nodes, memory, runtime) in &workloads {
        let _req = create_test_resource_requirements(
            config,
            workflow_id,
            name,
            *cpus,
            *gpus,
            *nodes,
            memory,
            runtime,
        );
    }

    // List all requirements and verify variety
    let args = ["resource-requirements", "list", &workflow_id.to_string()];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to list mixed workload requirements");

    let requirements_array = json_output
        .get("resource_requirements")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(requirements_array.len(), 5 + 1);

    // Verify we have different resource configurations
    let cpu_counts: Vec<i64> = requirements_array
        .iter()
        .map(|req| req.get("num_cpus").unwrap().as_i64().unwrap())
        .collect();

    let gpu_counts: Vec<i64> = requirements_array
        .iter()
        .map(|req| req.get("num_gpus").unwrap().as_i64().unwrap())
        .collect();

    // Should have variety in CPU and GPU counts
    let unique_cpus: std::collections::HashSet<_> = cpu_counts.iter().collect();
    let unique_gpus: std::collections::HashSet<_> = gpu_counts.iter().collect();

    assert!(
        unique_cpus.len() > 1,
        "Should have variety in CPU requirements"
    );
    assert!(
        unique_gpus.len() > 1,
        "Should have variety in GPU requirements"
    );
}
