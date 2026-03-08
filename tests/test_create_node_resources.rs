#![allow(clippy::too_many_arguments)]

use rstest::rstest;
use serial_test::serial;
use std::env;
use torc::client::commands::slurm::create_node_resources;
use torc::client::hpc::slurm_interface::SlurmInterface;

/// Helper function to set up Slurm environment variables for testing
fn setup_slurm_env(
    cpus_on_node: &str,
    mem_per_node: &str,
    num_nodes: &str,
    cpus_per_task: &str,
    job_gpus: Option<&str>,
) {
    unsafe {
        env::set_var("USER", "testuser");
        env::set_var("SLURM_CPUS_ON_NODE", cpus_on_node);
        env::set_var("SLURM_MEM_PER_NODE", mem_per_node);
        env::set_var("SLURM_JOB_NUM_NODES", num_nodes);
        env::set_var("SLURM_CPUS_PER_TASK", cpus_per_task);

        if let Some(gpus) = job_gpus {
            env::set_var("SLURM_JOB_GPUS", gpus);
        }
    }
}

/// Helper function to clean up Slurm environment variables after testing
fn cleanup_slurm_env(preserve_user: bool, original_user: Option<String>) {
    unsafe {
        if preserve_user {
            if let Some(user) = original_user {
                env::set_var("USER", user);
            } else {
                env::remove_var("USER");
            }
        } else {
            env::remove_var("USER");
        }

        env::remove_var("SLURM_CPUS_ON_NODE");
        env::remove_var("SLURM_MEM_PER_NODE");
        env::remove_var("SLURM_JOB_NUM_NODES");
        env::remove_var("SLURM_CPUS_PER_TASK");
        env::remove_var("SLURM_JOB_GPUS");
    }
}

#[rstest]
#[case(16, "32768", 2, 4, Some("0,1"), 123, false, 16, 32.0, 2, 2)] // Normal task, 2 nodes: per-node values (16 cpus, 32 GB, 2 gpus)
#[case(16, "16384", 1, 4, None, 456, true, 4, 4.0, 0, 1)] // Subtask, 1 node (16GB / 4 workers = 4GB)
#[case(32, "65536", 4, 8, Some("0,1,2,3"), 789, false, 32, 64.0, 4, 4)] // Large cluster, 4 nodes: per-node values (32 cpus, 64 GB, 4 gpus)
#[case(8, "8192", 1, 2, Some("0"), 101, true, 2, 2.0, 1, 1)] // Small subtask, 1 node (8GB / 4 workers = 2GB)
#[serial]
fn test_create_node_resources(
    #[case] cpus_on_node: usize,
    #[case] mem_per_node: &str,
    #[case] num_nodes: usize,
    #[case] cpus_per_task: usize,
    #[case] job_gpus: Option<&str>,
    #[case] scheduler_config_id: i64,
    #[case] is_subtask: bool,
    #[case] expected_cpus: i64,
    #[case] expected_memory: f64,
    #[case] expected_gpus: i64,
    #[case] expected_nodes: i64,
) {
    // Preserve existing USER environment variable
    let original_user = env::var("USER").ok();

    // Set up test environment
    setup_slurm_env(
        &cpus_on_node.to_string(),
        mem_per_node,
        &num_nodes.to_string(),
        &cpus_per_task.to_string(),
        job_gpus,
    );

    // Create SlurmInterface and test the function
    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");
    let resources = create_node_resources(&interface, Some(scheduler_config_id), is_subtask);

    assert_eq!(resources.num_cpus, expected_cpus, "CPU count mismatch");
    assert_eq!(
        resources.memory_gb, expected_memory,
        "Memory amount mismatch"
    );
    assert_eq!(resources.num_gpus, expected_gpus, "GPU count mismatch");
    assert_eq!(resources.num_nodes, expected_nodes, "Node count mismatch");
    assert_eq!(
        resources.scheduler_config_id,
        Some(scheduler_config_id),
        "Scheduler config ID mismatch"
    );

    // Clean up environment
    cleanup_slurm_env(true, original_user);
}

#[rstest]
#[serial]
fn test_create_node_resources_zero_values() {
    // Preserve existing USER environment variable
    let original_user = env::var("USER").ok();

    // Set up environment with zero values
    setup_slurm_env("0", "0", "0", "0", Some(""));

    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");
    let resources = create_node_resources(&interface, Some(999), false);

    // Per-node values: 0 cpus, 0 memory, 1 GPU (parsed from empty SLURM_JOB_GPUS)
    assert_eq!(
        resources.num_cpus, 0,
        "CPU count should be 0 (per-node value)"
    );
    assert_eq!(
        resources.memory_gb, 0.0,
        "Memory should be 0 (per-node value)"
    );
    assert_eq!(
        resources.num_gpus, 1,
        "GPU count should be 1 (per-node value from empty SLURM_JOB_GPUS)"
    );
    assert_eq!(resources.num_nodes, 0, "Node count should be 0");
    assert_eq!(
        resources.scheduler_config_id,
        Some(999),
        "Scheduler config ID should be set"
    );

    // Clean up environment
    cleanup_slurm_env(true, original_user);
}

#[rstest]
#[serial]
fn test_create_node_resources_gpu_parsing() {
    // Preserve existing USER environment variable
    let original_user = env::var("USER").ok();

    // Test various GPU environment variable formats
    let test_cases = vec![
        ("0", 1),       // Single GPU
        ("0,1", 2),     // Two GPUs
        ("0,1,2,3", 4), // Four GPUs
        ("", 1),        // Empty string (split still returns 1 element)
    ];

    for (gpu_string, expected_count) in test_cases {
        // Set up environment
        setup_slurm_env("8", "8192", "1", "4", Some(gpu_string));

        let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");
        let resources = create_node_resources(&interface, Some(123), false);

        assert_eq!(
            resources.num_gpus, expected_count,
            "GPU count mismatch for '{}'",
            gpu_string
        );

        // Clean up for next iteration
        cleanup_slurm_env(false, None);
    }

    // Restore original USER
    cleanup_slurm_env(true, original_user);
}
