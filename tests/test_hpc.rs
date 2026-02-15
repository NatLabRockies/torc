//! Tests for HPC profile system and scheduler generation

use rstest::rstest;
use std::collections::HashMap;
use torc::client::commands::slurm::{
    GroupByStrategy, WalltimeStrategy, generate_schedulers_for_workflow, parse_memory_mb,
    parse_walltime_secs, secs_to_walltime,
};
use torc::client::hpc::kestrel::kestrel_profile;
use torc::client::hpc::{HpcDetection, HpcPartition, HpcProfile, HpcProfileRegistry};
use torc::client::workflow_spec::{JobSpec, ResourceRequirementsSpec, WorkflowSpec};
use torc::time_utils::duration_string_to_seconds;

// ============== Utility Function Tests ==============

#[rstest]
fn test_parse_memory_mb() {
    assert_eq!(parse_memory_mb("100g").unwrap(), 102400);
    assert_eq!(parse_memory_mb("1G").unwrap(), 1024);
    assert_eq!(parse_memory_mb("512m").unwrap(), 512);
    assert_eq!(parse_memory_mb("512M").unwrap(), 512);
    assert_eq!(parse_memory_mb("1024").unwrap(), 1024);
    assert_eq!(parse_memory_mb("1024k").unwrap(), 1);
}

#[rstest]
fn test_parse_walltime_secs() {
    assert_eq!(parse_walltime_secs("1:00:00").unwrap(), 3600);
    assert_eq!(parse_walltime_secs("4:00:00").unwrap(), 14400);
    assert_eq!(parse_walltime_secs("1-00:00:00").unwrap(), 86400);
    assert_eq!(parse_walltime_secs("2-00:00:00").unwrap(), 172800);
    assert_eq!(parse_walltime_secs("10-00:00:00").unwrap(), 864000);
    assert_eq!(parse_walltime_secs("0:30:00").unwrap(), 1800);
}

#[rstest]
fn test_duration_string_to_seconds() {
    // Test ISO 8601 duration parsing using the consolidated function from time_utils
    assert_eq!(duration_string_to_seconds("PT1H").unwrap(), 3600);
    assert_eq!(duration_string_to_seconds("PT30M").unwrap(), 1800);
    assert_eq!(duration_string_to_seconds("PT1H30M").unwrap(), 5400);
    assert_eq!(duration_string_to_seconds("P1D").unwrap(), 86400);
    assert_eq!(duration_string_to_seconds("P1DT2H").unwrap(), 93600);
    assert_eq!(duration_string_to_seconds("P0DT1M").unwrap(), 60);
    assert_eq!(duration_string_to_seconds("PT4H").unwrap(), 14400);
}

#[rstest]
fn test_secs_to_walltime() {
    assert_eq!(secs_to_walltime(3600), "01:00:00");
    assert_eq!(secs_to_walltime(14400), "04:00:00");
    assert_eq!(secs_to_walltime(86400), "1-00:00:00");
    assert_eq!(secs_to_walltime(172800), "2-00:00:00");
    assert_eq!(secs_to_walltime(93600), "1-02:00:00"); // 1 day 2 hours
}

// ============== Profile System Tests ==============

fn create_test_partition(
    name: &str,
    cpus: u32,
    memory_mb: u64,
    walltime_secs: u64,
    gpus: Option<u32>,
) -> HpcPartition {
    HpcPartition {
        name: name.to_string(),
        description: String::new(),
        cpus_per_node: cpus,
        memory_mb,
        max_walltime_secs: walltime_secs,
        max_nodes: None,
        max_nodes_per_user: None,
        min_nodes: None,
        gpus_per_node: gpus,
        gpu_type: None,
        gpu_memory_gb: None,
        local_disk_gb: None,
        shared: false,
        requires_explicit_request: false,
        default_qos: None,
        features: vec![],
    }
}

fn create_test_profile(name: &str, partitions: Vec<HpcPartition>) -> HpcProfile {
    HpcProfile {
        name: name.to_string(),
        display_name: format!("Test {}", name),
        description: String::new(),
        detection: vec![],
        default_account: None,
        partitions,
        charge_factor_cpu: 1.0,
        charge_factor_gpu: 10.0,
        metadata: HashMap::new(),
    }
}

#[rstest]
fn test_partition_can_satisfy_basic() {
    let partition = create_test_partition("standard", 104, 245760, 172800, None);

    // Should satisfy small request
    assert!(partition.can_satisfy(4, 8192, 3600, None));
    // Should satisfy request up to limits
    assert!(partition.can_satisfy(104, 245760, 172800, None));
    // Should fail if CPUs exceed
    assert!(!partition.can_satisfy(105, 8192, 3600, None));
    // Should fail if memory exceeds
    assert!(!partition.can_satisfy(4, 300000, 3600, None));
    // Should fail if walltime exceeds
    assert!(!partition.can_satisfy(4, 8192, 200000, None));
}

#[rstest]
fn test_partition_can_satisfy_gpu() {
    let partition = create_test_partition("gpu-h100", 128, 2097152, 172800, Some(4));

    // Should satisfy GPU request within limits
    assert!(partition.can_satisfy(64, 200000, 3600, Some(2)));
    // Should fail if GPUs exceed
    assert!(!partition.can_satisfy(64, 200000, 3600, Some(5)));

    // Non-GPU partition should not satisfy GPU requests
    let cpu_partition = create_test_partition("standard", 104, 245760, 172800, None);
    assert!(!cpu_partition.can_satisfy(4, 8192, 3600, Some(1)));
}

#[rstest]
fn test_env_var_detection() {
    let profile = HpcProfile {
        name: "test".to_string(),
        display_name: "Test Profile".to_string(),
        description: "Test".to_string(),
        detection: vec![HpcDetection::EnvVar {
            name: "TEST_CLUSTER".to_string(),
            value: "test".to_string(),
        }],
        default_account: None,
        partitions: vec![],
        charge_factor_cpu: 1.0,
        charge_factor_gpu: 10.0,
        metadata: HashMap::new(),
    };

    // Detection should work when env var matches
    // SAFETY: Tests run serially and we restore the var
    unsafe {
        std::env::set_var("TEST_CLUSTER", "test");
    }
    assert!(profile.detect());

    // Detection should fail when env var doesn't match
    unsafe {
        std::env::set_var("TEST_CLUSTER", "other");
    }
    assert!(!profile.detect());

    unsafe {
        std::env::remove_var("TEST_CLUSTER");
    }
}

#[rstest]
fn test_profile_registry() {
    let mut registry = HpcProfileRegistry::new();

    let profile = create_test_profile(
        "test",
        vec![create_test_partition("standard", 64, 128000, 86400, None)],
    );

    registry.register(profile);

    assert!(registry.get("test").is_some());
    assert!(registry.get("nonexistent").is_none());
}

#[rstest]
fn test_walltime_format() {
    let partition = create_test_partition("test", 64, 128000, 90061, None); // 25h 1m 1s

    let formatted = partition.max_walltime_str();
    assert!(formatted.contains("25") || formatted.contains("1-01"));
}

// ============== Kestrel Profile Tests ==============

#[rstest]
fn test_kestrel_profile_basics() {
    let profile = kestrel_profile();
    assert_eq!(profile.name, "kestrel");
    assert_eq!(profile.display_name, "NLR Kestrel");
    assert!(!profile.partitions.is_empty());
}

#[rstest]
fn test_kestrel_has_expected_partitions() {
    let profile = kestrel_profile();
    let partition_names: Vec<&str> = profile.partitions.iter().map(|p| p.name.as_str()).collect();

    // Check for key partitions
    assert!(partition_names.contains(&"debug"));
    assert!(partition_names.contains(&"short"));
    assert!(partition_names.contains(&"standard"));
    assert!(partition_names.contains(&"gpu-h100"));
}

#[rstest]
fn test_kestrel_standard_partition() {
    let profile = kestrel_profile();
    let standard = profile
        .get_partition("standard")
        .expect("Standard partition not found");

    assert_eq!(standard.cpus_per_node, 104);
    assert_eq!(standard.memory_mb, 240_000);
    assert_eq!(standard.max_walltime_secs, 172800); // 48 hours
    assert!(standard.gpus_per_node.is_none());
}

#[rstest]
fn test_kestrel_gpu_partition() {
    let profile = kestrel_profile();
    let gpu = profile
        .get_partition("gpu-h100")
        .expect("GPU partition not found");

    assert_eq!(gpu.gpus_per_node, Some(4));
    assert!(gpu.gpu_type.is_some());
}

#[rstest]
fn test_kestrel_find_matching_partitions() {
    let profile = kestrel_profile();

    // Small CPU job should match multiple partitions
    let matches = profile.find_matching_partitions(4, 8192, 3600, None);
    assert!(!matches.is_empty());

    // GPU job should only match GPU partitions
    let gpu_matches = profile.find_matching_partitions(64, 200000, 3600, Some(2));
    assert!(!gpu_matches.is_empty());
    for partition in &gpu_matches {
        assert!(partition.gpus_per_node.is_some());
    }
}

#[rstest]
fn test_kestrel_hbw_requires_min_nodes() {
    let profile = kestrel_profile();
    let hbw = profile
        .get_partition("hbw")
        .expect("HBW partition not found");

    assert!(hbw.min_nodes.is_some());
}

// ============== Scheduler Generation Tests ==============

#[rstest]
fn test_generate_schedulers_basic() {
    let mut spec = WorkflowSpec {
        name: "test_workflow".to_string(),
        description: Some("Test workflow".to_string()),
        jobs: vec![
            JobSpec {
                name: "job1".to_string(),
                command: "echo hello".to_string(),
                resource_requirements: Some("small".to_string()),
                ..Default::default()
            },
            JobSpec {
                name: "job2".to_string(),
                command: "echo world".to_string(),
                resource_requirements: Some("medium".to_string()),
                depends_on: Some(vec!["job1".to_string()]),
                ..Default::default()
            },
        ],
        resource_requirements: Some(vec![
            ResourceRequirementsSpec {
                name: "small".to_string(),
                num_cpus: 4,
                num_gpus: 0,
                num_nodes: 1,
                memory: "8g".to_string(),
                runtime: "PT1H".to_string(),
            },
            ResourceRequirementsSpec {
                name: "medium".to_string(),
                num_cpus: 32,
                num_gpus: 0,
                num_nodes: 1,
                memory: "64g".to_string(),
                runtime: "PT4H".to_string(),
            },
        ]),
        ..Default::default()
    };

    let profile = kestrel_profile();
    let result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        false,
    )
    .unwrap();

    // Should generate 2 schedulers:
    // - small_scheduler for job1 (no dependencies, on_workflow_start)
    // - medium_deferred_scheduler for job2 (has dependencies, on_jobs_ready)
    // Schedulers are grouped by (resource_requirements, has_dependencies)
    assert_eq!(result.scheduler_count, 2);
    assert_eq!(result.action_count, 2);

    // Check that slurm_schedulers were added
    assert!(spec.slurm_schedulers.is_some());
    let schedulers = spec.slurm_schedulers.as_ref().unwrap();
    assert_eq!(schedulers.len(), 2);

    // Check scheduler names - grouped by (resource_requirement, has_deps)
    let scheduler_names: Vec<&str> = schedulers
        .iter()
        .filter_map(|s| s.name.as_deref())
        .collect();
    assert!(scheduler_names.contains(&"small_scheduler"));
    assert!(scheduler_names.contains(&"medium_deferred_scheduler"));

    // Check that jobs were assigned to correct schedulers
    // job1 (no deps) → small_scheduler
    // job2 (has deps) → medium_deferred_scheduler
    assert_eq!(spec.jobs[0].scheduler.as_ref().unwrap(), "small_scheduler");
    assert_eq!(
        spec.jobs[1].scheduler.as_ref().unwrap(),
        "medium_deferred_scheduler"
    );

    // Check that workflow actions were added
    assert!(spec.actions.is_some());
    let actions = spec.actions.as_ref().unwrap();
    assert_eq!(actions.len(), 2);

    // Jobs without dependencies use on_workflow_start
    let small_action = actions
        .iter()
        .find(|a| a.scheduler.as_deref() == Some("small_scheduler"))
        .unwrap();
    assert_eq!(small_action.trigger_type, "on_workflow_start");
    assert_eq!(small_action.action_type, "schedule_nodes");

    // Jobs with dependencies use on_jobs_ready
    let medium_action = actions
        .iter()
        .find(|a| a.scheduler.as_deref() == Some("medium_deferred_scheduler"))
        .unwrap();
    assert_eq!(medium_action.trigger_type, "on_jobs_ready");
    assert_eq!(medium_action.action_type, "schedule_nodes");
}

#[rstest]
fn test_generate_schedulers_with_gpus() {
    let mut spec = WorkflowSpec {
        name: "gpu_workflow".to_string(),
        description: Some("GPU workflow".to_string()),
        jobs: vec![JobSpec {
            name: "gpu_job".to_string(),
            command: "python train.py".to_string(),
            resource_requirements: Some("gpu_heavy".to_string()),
            ..Default::default()
        }],
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "gpu_heavy".to_string(),
            num_cpus: 64,
            num_gpus: 2,
            num_nodes: 1,
            memory: "200g".to_string(),
            runtime: "PT8H".to_string(),
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();
    let result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        false,
    )
    .unwrap();

    assert_eq!(result.scheduler_count, 1);

    let schedulers = spec.slurm_schedulers.as_ref().unwrap();
    assert_eq!(schedulers.len(), 1);

    let gpu_scheduler = &schedulers[0];
    // Per-resource-requirement scheduler naming: rr_name + "_scheduler"
    assert_eq!(gpu_scheduler.name.as_deref(), Some("gpu_heavy_scheduler"));
    assert_eq!(gpu_scheduler.account, "testaccount");
    // GPU scheduler should have gres set
    assert!(gpu_scheduler.gres.is_some());
    assert!(gpu_scheduler.gres.as_ref().unwrap().contains("gpu"));
}

#[rstest]
fn test_generate_schedulers_no_actions() {
    let mut spec = WorkflowSpec {
        name: "test_workflow".to_string(),
        description: None,
        jobs: vec![JobSpec {
            name: "job1".to_string(),
            command: "echo hello".to_string(),
            resource_requirements: Some("small".to_string()),
            ..Default::default()
        }],
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "small".to_string(),
            num_cpus: 4,
            num_gpus: 0,
            num_nodes: 1,
            memory: "8g".to_string(),
            runtime: "PT1H".to_string(),
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();
    // Pass add_actions = false
    let result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        false,
        false,
    )
    .unwrap();

    assert_eq!(result.scheduler_count, 1);
    assert_eq!(result.action_count, 0);

    // Schedulers should be added
    assert!(spec.slurm_schedulers.is_some());

    // But no actions
    assert!(spec.actions.is_none() || spec.actions.as_ref().unwrap().is_empty());
}

#[rstest]
fn test_generate_schedulers_shared_by_jobs() {
    // Jobs with the same resource requirements share a scheduler
    let mut spec = WorkflowSpec {
        name: "test_workflow".to_string(),
        description: None,
        jobs: vec![
            JobSpec {
                name: "job1".to_string(),
                command: "echo hello".to_string(),
                resource_requirements: Some("small".to_string()),
                ..Default::default()
            },
            JobSpec {
                name: "job2".to_string(),
                command: "echo world".to_string(),
                resource_requirements: Some("small".to_string()), // Same requirements
                ..Default::default()
            },
        ],
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "small".to_string(),
            num_cpus: 4,
            num_gpus: 0,
            num_nodes: 1,
            memory: "8g".to_string(),
            runtime: "PT1H".to_string(),
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();
    let result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        false,
    )
    .unwrap();

    // Only one scheduler since both jobs use the same resource requirements
    assert_eq!(result.scheduler_count, 1);

    let schedulers = spec.slurm_schedulers.as_ref().unwrap();
    assert_eq!(schedulers.len(), 1);

    // Both jobs should share the same scheduler
    assert_eq!(spec.jobs[0].scheduler.as_ref().unwrap(), "small_scheduler");
    assert_eq!(spec.jobs[1].scheduler.as_ref().unwrap(), "small_scheduler");
}

#[rstest]
fn test_generate_schedulers_errors_no_resource_requirements() {
    let mut spec = WorkflowSpec {
        name: "test_workflow".to_string(),
        description: None,
        jobs: vec![JobSpec {
            name: "job1".to_string(),
            command: "echo hello".to_string(),
            resource_requirements: Some("nonexistent".to_string()),
            ..Default::default()
        }],
        resource_requirements: None, // No resource requirements defined
        ..Default::default()
    };

    let profile = kestrel_profile();
    let result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        false,
    );

    // Should return an error when no resource requirements are defined
    match result {
        Err(e) => assert!(e.contains("resource_requirements")),
        Ok(_) => panic!("Expected error but got Ok"),
    }
}

#[rstest]
fn test_generate_schedulers_existing_schedulers_no_force() {
    use torc::client::workflow_spec::SlurmSchedulerSpec;

    let mut spec = WorkflowSpec {
        name: "test_workflow".to_string(),
        description: None,
        jobs: vec![JobSpec {
            name: "job1".to_string(),
            command: "echo hello".to_string(),
            resource_requirements: Some("small".to_string()),
            scheduler: Some("existing_scheduler".to_string()),
            ..Default::default()
        }],
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "small".to_string(),
            num_cpus: 4,
            num_gpus: 0,
            num_nodes: 1,
            memory: "8g".to_string(),
            runtime: "PT1H".to_string(),
        }]),
        slurm_schedulers: Some(vec![SlurmSchedulerSpec {
            name: Some("existing_scheduler".to_string()),
            account: "test".to_string(),
            nodes: 1,
            walltime: "01:00:00".to_string(),
            gres: None,
            mem: None,
            ntasks_per_node: None,
            partition: None,
            qos: None,
            tmp: None,
            extra: None,
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();
    // force = false should return error when slurm_schedulers already exists
    let result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        false,
    );

    match result {
        Err(e) => assert!(e.contains("already has slurm_schedulers")),
        Ok(_) => panic!("Expected error but got Ok"),
    }
}

#[rstest]
fn test_generate_schedulers_existing_schedulers_with_force() {
    use torc::client::workflow_spec::SlurmSchedulerSpec;

    let mut spec = WorkflowSpec {
        name: "test_workflow".to_string(),
        description: None,
        jobs: vec![JobSpec {
            name: "job1".to_string(),
            command: "echo hello".to_string(),
            resource_requirements: Some("small".to_string()),
            scheduler: Some("existing_scheduler".to_string()),
            ..Default::default()
        }],
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "small".to_string(),
            num_cpus: 4,
            num_gpus: 0,
            num_nodes: 1,
            memory: "8g".to_string(),
            runtime: "PT1H".to_string(),
        }]),
        slurm_schedulers: Some(vec![SlurmSchedulerSpec {
            name: Some("existing_scheduler".to_string()),
            account: "test".to_string(),
            nodes: 1,
            walltime: "01:00:00".to_string(),
            gres: None,
            mem: None,
            ntasks_per_node: None,
            partition: None,
            qos: None,
            tmp: None,
            extra: None,
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();
    // force = true should succeed even when slurm_schedulers already exists
    let result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        true,
    )
    .unwrap();

    // Job should be reassigned to scheduler based on resource requirement name
    assert_eq!(spec.jobs[0].scheduler.as_ref().unwrap(), "small_scheduler");

    // New scheduler should be generated
    assert_eq!(result.scheduler_count, 1);
}

#[rstest]
fn test_generate_schedulers_sets_correct_account() {
    let mut spec = WorkflowSpec {
        name: "test_workflow".to_string(),
        description: None,
        jobs: vec![JobSpec {
            name: "job1".to_string(),
            command: "echo hello".to_string(),
            resource_requirements: Some("small".to_string()),
            ..Default::default()
        }],
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "small".to_string(),
            num_cpus: 4,
            num_gpus: 0,
            num_nodes: 1,
            memory: "8g".to_string(),
            runtime: "PT1H".to_string(),
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();
    let _result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "my_project_account",
        false,                                 // single_allocation
        GroupByStrategy::ResourceRequirements, // group_by
        WalltimeStrategy::MaxJobRuntime,       // walltime_strategy
        1.5,                                   // walltime_multiplier
        true,                                  // add_actions
        false,                                 // overwrite
    )
    .unwrap();

    let scheduler = &spec.slurm_schedulers.as_ref().unwrap()[0];
    assert_eq!(scheduler.account, "my_project_account");
}

// ============== Walltime Strategy Tests ==============

#[rstest]
fn test_generate_schedulers_walltime_max_job_runtime_default() {
    // Test default behavior: MaxJobRuntime with 1.5x multiplier
    let mut spec = WorkflowSpec {
        name: "test_workflow".to_string(),
        description: None,
        jobs: vec![JobSpec {
            name: "job1".to_string(),
            command: "echo hello".to_string(),
            resource_requirements: Some("long_job".to_string()),
            ..Default::default()
        }],
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "long_job".to_string(),
            num_cpus: 4,
            num_gpus: 0,
            num_nodes: 1,
            memory: "8g".to_string(),
            runtime: "PT12H".to_string(), // 12 hours
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();
    let _result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime, // default strategy
        1.5,                             // default multiplier
        true,
        false,
    )
    .unwrap();

    let scheduler = &spec.slurm_schedulers.as_ref().unwrap()[0];
    // 12 hours * 1.5 = 18 hours
    assert_eq!(scheduler.walltime, "18:00:00");
}

#[rstest]
fn test_generate_schedulers_walltime_max_partition_time() {
    // Test MaxPartitionTime strategy: should use partition's max walltime
    let mut spec = WorkflowSpec {
        name: "test_workflow".to_string(),
        description: None,
        jobs: vec![JobSpec {
            name: "job1".to_string(),
            command: "echo hello".to_string(),
            resource_requirements: Some("long_job".to_string()),
            ..Default::default()
        }],
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "long_job".to_string(),
            num_cpus: 4,
            num_gpus: 0,
            num_nodes: 1,
            memory: "8g".to_string(),
            runtime: "PT12H".to_string(), // 12 hours
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();
    let _result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxPartitionTime, // use partition max
        1.5,                                // multiplier ignored for this strategy
        true,
        false,
    )
    .unwrap();

    let scheduler = &spec.slurm_schedulers.as_ref().unwrap()[0];
    // Standard partition max is 2 days
    assert_eq!(scheduler.walltime, "2-00:00:00");
}

#[rstest]
fn test_generate_schedulers_walltime_custom_multiplier() {
    // Test MaxJobRuntime with custom multiplier (2.0x)
    let mut spec = WorkflowSpec {
        name: "test_workflow".to_string(),
        description: None,
        jobs: vec![JobSpec {
            name: "job1".to_string(),
            command: "echo hello".to_string(),
            resource_requirements: Some("job_rr".to_string()),
            ..Default::default()
        }],
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "job_rr".to_string(),
            num_cpus: 4,
            num_gpus: 0,
            num_nodes: 1,
            memory: "8g".to_string(),
            runtime: "PT6H".to_string(), // 6 hours
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();
    let _result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        2.0, // 2x multiplier
        true,
        false,
    )
    .unwrap();

    let scheduler = &spec.slurm_schedulers.as_ref().unwrap()[0];
    // 6 hours * 2.0 = 12 hours
    assert_eq!(scheduler.walltime, "12:00:00");
}

#[rstest]
fn test_generate_schedulers_walltime_capped_at_partition_max() {
    // Test that walltime is capped at partition max even with high multiplier
    let mut spec = WorkflowSpec {
        name: "test_workflow".to_string(),
        description: None,
        jobs: vec![JobSpec {
            name: "job1".to_string(),
            command: "echo hello".to_string(),
            resource_requirements: Some("long_job".to_string()),
            ..Default::default()
        }],
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "long_job".to_string(),
            num_cpus: 4,
            num_gpus: 0,
            num_nodes: 1,
            memory: "8g".to_string(),
            runtime: "P1DT12H".to_string(), // 36 hours
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();
    let _result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5, // 36 hours * 1.5 = 54 hours, but partition max is 48 hours
        true,
        false,
    )
    .unwrap();

    let scheduler = &spec.slurm_schedulers.as_ref().unwrap()[0];
    // Should be capped at standard partition max (2 days = 48 hours)
    assert_eq!(scheduler.walltime, "2-00:00:00");
}

#[rstest]
fn test_generate_schedulers_walltime_uses_max_job_runtime_in_group() {
    // Test that when multiple jobs have different runtimes, the max is used
    let mut spec = WorkflowSpec {
        name: "test_workflow".to_string(),
        description: None,
        jobs: vec![
            JobSpec {
                name: "short_job".to_string(),
                command: "echo short".to_string(),
                resource_requirements: Some("shared_rr".to_string()),
                ..Default::default()
            },
            JobSpec {
                name: "long_job".to_string(),
                command: "echo long".to_string(),
                resource_requirements: Some("shared_rr".to_string()),
                ..Default::default()
            },
        ],
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "shared_rr".to_string(),
            num_cpus: 4,
            num_gpus: 0,
            num_nodes: 1,
            memory: "8g".to_string(),
            runtime: "PT8H".to_string(), // 8 hours - this is the max for all jobs using this RR
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();
    let _result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        false,
    )
    .unwrap();

    let scheduler = &spec.slurm_schedulers.as_ref().unwrap()[0];
    // 8 hours * 1.5 = 12 hours
    assert_eq!(scheduler.walltime, "12:00:00");
}

#[rstest]
fn test_generate_schedulers_walltime_zero_runtime_fallback() {
    // Test that when runtime is zero, it falls back to partition max
    let mut spec = WorkflowSpec {
        name: "test_workflow".to_string(),
        description: None,
        jobs: vec![JobSpec {
            name: "job1".to_string(),
            command: "echo hello".to_string(),
            resource_requirements: Some("zero_runtime".to_string()),
            ..Default::default()
        }],
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "zero_runtime".to_string(),
            num_cpus: 4,
            num_gpus: 0,
            num_nodes: 1,
            memory: "8g".to_string(),
            runtime: "PT0S".to_string(), // zero seconds
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();
    let _result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime, // would normally use runtime * multiplier
        1.5,
        true,
        false,
    )
    .unwrap();

    let scheduler = &spec.slurm_schedulers.as_ref().unwrap()[0];
    // With zero runtime, the "short" partition (4 hours max) is selected based on CPU/memory,
    // and walltime falls back to that partition's max
    assert_eq!(scheduler.walltime, "04:00:00");
}

#[rstest]
fn test_generate_schedulers_walltime_multiplier_one() {
    // Test with multiplier of 1.0 (exact runtime, no buffer)
    let mut spec = WorkflowSpec {
        name: "test_workflow".to_string(),
        description: None,
        jobs: vec![JobSpec {
            name: "job1".to_string(),
            command: "echo hello".to_string(),
            resource_requirements: Some("job_rr".to_string()),
            ..Default::default()
        }],
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "job_rr".to_string(),
            num_cpus: 4,
            num_gpus: 0,
            num_nodes: 1,
            memory: "8g".to_string(),
            runtime: "PT4H".to_string(), // 4 hours
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();
    let _result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.0, // exact runtime
        true,
        false,
    )
    .unwrap();

    let scheduler = &spec.slurm_schedulers.as_ref().unwrap()[0];
    // 4 hours * 1.0 = 4 hours
    assert_eq!(scheduler.walltime, "04:00:00");
}

#[rstest]
fn test_generate_schedulers_sets_memory() {
    let mut spec = WorkflowSpec {
        name: "test_workflow".to_string(),
        description: None,
        jobs: vec![JobSpec {
            name: "job1".to_string(),
            command: "echo hello".to_string(),
            resource_requirements: Some("mem_job".to_string()),
            ..Default::default()
        }],
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "mem_job".to_string(),
            num_cpus: 4,
            num_gpus: 0,
            num_nodes: 1,
            memory: "128g".to_string(),
            runtime: "PT1H".to_string(),
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();
    let _result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        false,
    )
    .unwrap();

    let scheduler = &spec.slurm_schedulers.as_ref().unwrap()[0];
    // Memory should be set to the partition's max memory, not the job's requirement.
    // This allows jobs to use more memory than their estimates.
    // Kestrel standard partition has 240,000 MB = 234g.
    assert_eq!(scheduler.mem.as_deref(), Some("234g"));
}

#[rstest]
fn test_generate_schedulers_per_resource_requirement() {
    // Schedulers are created per (resource_requirement, has_dependencies)
    // Jobs with same resource requirements but different dependency status get separate schedulers
    let mut spec = WorkflowSpec {
        name: "staged_workflow".to_string(),
        description: None,
        jobs: vec![
            JobSpec {
                name: "setup".to_string(),
                command: "echo setup".to_string(),
                resource_requirements: Some("small".to_string()),
                depends_on: None, // No dependencies
                ..Default::default()
            },
            JobSpec {
                name: "process".to_string(),
                command: "echo process".to_string(),
                resource_requirements: Some("medium".to_string()),
                depends_on: Some(vec!["setup".to_string()]), // Depends on setup
                ..Default::default()
            },
            JobSpec {
                name: "finalize".to_string(),
                command: "echo finalize".to_string(),
                resource_requirements: Some("small".to_string()), // Same as setup
                depends_on: Some(vec!["process".to_string()]),    // Depends on process
                ..Default::default()
            },
        ],
        resource_requirements: Some(vec![
            ResourceRequirementsSpec {
                name: "small".to_string(),
                num_cpus: 2,
                num_gpus: 0,
                num_nodes: 1,
                memory: "4g".to_string(),
                runtime: "PT30M".to_string(),
            },
            ResourceRequirementsSpec {
                name: "medium".to_string(),
                num_cpus: 8,
                num_gpus: 0,
                num_nodes: 1,
                memory: "16g".to_string(),
                runtime: "PT2H".to_string(),
            },
        ]),
        ..Default::default()
    };

    let profile = kestrel_profile();
    let result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        false,
    )
    .unwrap();

    // 3 schedulers:
    // - small_scheduler for setup (no deps, on_workflow_start)
    // - medium_deferred_scheduler for process (has deps, on_jobs_ready)
    // - small_deferred_scheduler for finalize (has deps, on_jobs_ready)
    // Stage-aware scheduling launches nodes when jobs become ready
    assert_eq!(result.scheduler_count, 3);
    assert_eq!(result.action_count, 3);

    let actions = spec.actions.as_ref().unwrap();
    assert_eq!(actions.len(), 3);

    // Jobs should be assigned to schedulers based on (resource_requirement, has_deps)
    assert_eq!(spec.jobs[0].scheduler.as_deref(), Some("small_scheduler")); // setup (no deps)
    assert_eq!(
        spec.jobs[1].scheduler.as_deref(),
        Some("medium_deferred_scheduler")
    ); // process (has deps)
    assert_eq!(
        spec.jobs[2].scheduler.as_deref(),
        Some("small_deferred_scheduler")
    ); // finalize (has deps)

    // Jobs without dependencies use on_workflow_start
    let small_action = actions
        .iter()
        .find(|a| a.scheduler.as_deref() == Some("small_scheduler"))
        .unwrap();
    assert_eq!(small_action.trigger_type, "on_workflow_start");

    // Jobs with dependencies use on_jobs_ready
    let medium_action = actions
        .iter()
        .find(|a| a.scheduler.as_deref() == Some("medium_deferred_scheduler"))
        .unwrap();
    assert_eq!(medium_action.trigger_type, "on_jobs_ready");

    let finalize_action = actions
        .iter()
        .find(|a| a.scheduler.as_deref() == Some("small_deferred_scheduler"))
        .unwrap();
    assert_eq!(finalize_action.trigger_type, "on_jobs_ready");
}

/// Test that num_allocations is auto-calculated based on job count and partition capacity
#[test]
fn test_generate_schedulers_auto_calculates_allocations() {
    use torc::client::workflow_spec::{JobSpec, ResourceRequirementsSpec, WorkflowSpec};

    // Create a workflow with 10 jobs, each requiring 26 CPUs
    // On Kestrel (104 CPUs/node), 4 jobs fit per node
    // So we need 10/4 = 3 nodes (rounded up)
    let jobs: Vec<JobSpec> = (0..10)
        .map(|i| JobSpec {
            name: format!("job_{:03}", i),
            command: "echo hello".to_string(),
            resource_requirements: Some("compute".to_string()),
            ..Default::default()
        })
        .collect();

    let mut spec = WorkflowSpec {
        name: "test_workflow".to_string(),
        user: Some("testuser".to_string()),
        jobs,
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "compute".to_string(),
            num_cpus: 26, // 104 / 26 = 4 jobs per node
            num_gpus: 0,
            num_nodes: 1,
            memory: "10g".to_string(),
            runtime: "PT1H".to_string(),
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();

    // Pass None for num_allocations to trigger auto-calculation
    let result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        false,
    )
    .unwrap();

    assert_eq!(result.scheduler_count, 1);
    assert_eq!(result.action_count, 1);

    let actions = spec.actions.as_ref().unwrap();
    let action = &actions[0];

    // 10 jobs, 26 CPUs each, 1 hour runtime
    // Concurrent capacity: 104 CPUs / 26 CPUs = 4 jobs per node
    // Time slots: 4h walltime / 1h runtime = 4 sequential batches
    // Jobs per allocation: 4 concurrent × 4 time slots = 16 jobs
    // Allocations needed: ceil(10 / 16) = 1
    assert_eq!(action.num_allocations, Some(1));
}

/// Test auto-calculation with parameterized jobs
#[test]
fn test_generate_schedulers_auto_calculates_with_parameters() {
    // One parameterized job that expands to 100 jobs
    let mut parameters = HashMap::new();
    parameters.insert("i".to_string(), "1:100".to_string());

    let jobs = vec![JobSpec {
        name: "job_{i:03d}".to_string(),
        command: "echo hello".to_string(),
        resource_requirements: Some("small".to_string()),
        parameters: Some(parameters),
        ..Default::default()
    }];

    let mut spec = WorkflowSpec {
        name: "test_workflow".to_string(),
        user: Some("testuser".to_string()),
        jobs,
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "small".to_string(),
            num_cpus: 52, // 104 / 52 = 2 jobs per node
            num_gpus: 0,
            num_nodes: 1,
            memory: "10g".to_string(),
            runtime: "PT1H".to_string(),
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();

    // Pass None for num_allocations to trigger auto-calculation
    let result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        false,
    )
    .unwrap();

    assert_eq!(result.scheduler_count, 1);
    assert_eq!(result.action_count, 1);

    let actions = spec.actions.as_ref().unwrap();
    let action = &actions[0];

    // 100 jobs (from parameterized expansion), 52 CPUs each, 1 hour runtime
    // Concurrent capacity: 104 CPUs / 52 CPUs = 2 jobs per node
    // Time slots: 4h walltime / 1h runtime = 4 sequential batches
    // Jobs per allocation: 2 concurrent × 4 time slots = 8 jobs
    // Allocations needed: ceil(100 / 8) = 13
    assert_eq!(action.num_allocations, Some(13));
}

/// Test stage-aware scheduling: jobs with and without dependencies get separate schedulers.
/// This enables launching compute nodes only when jobs become ready.
#[test]
fn test_generate_schedulers_stage_aware_for_dependent_jobs() {
    // job1: no dependencies → scheduled at on_workflow_start
    // job2: depends on job1 → scheduled at on_jobs_ready when job1 completes
    // Both use the same resource requirements but get separate schedulers
    let jobs = vec![
        JobSpec {
            name: "job1".to_string(),
            command: "echo job1".to_string(),
            resource_requirements: Some("small".to_string()),
            ..Default::default()
        },
        JobSpec {
            name: "job2".to_string(),
            command: "echo job2".to_string(),
            resource_requirements: Some("small".to_string()),
            depends_on: Some(vec!["job1".to_string()]),
            ..Default::default()
        },
    ];

    let mut spec = WorkflowSpec {
        name: "test_workflow".to_string(),
        user: Some("testuser".to_string()),
        jobs,
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "small".to_string(),
            num_cpus: 4,
            num_gpus: 0,
            num_nodes: 1,
            memory: "8g".to_string(),
            runtime: "PT30M".to_string(),
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();

    let result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        false,
    )
    .unwrap();

    // Should generate 2 schedulers for stage-aware scheduling:
    // - small_scheduler (on_workflow_start) for job1
    // - small_deferred_scheduler (on_jobs_ready) for job2
    assert_eq!(result.scheduler_count, 2);
    assert_eq!(result.action_count, 2);

    let schedulers = spec.slurm_schedulers.as_ref().unwrap();
    assert_eq!(schedulers.len(), 2);

    // Jobs are assigned to different schedulers based on dependency status
    assert_eq!(spec.jobs[0].scheduler, Some("small_scheduler".to_string())); // no deps
    assert_eq!(
        spec.jobs[1].scheduler,
        Some("small_deferred_scheduler".to_string())
    ); // has deps

    // Verify trigger types
    let actions = spec.actions.as_ref().unwrap();
    assert_eq!(actions.len(), 2);

    let job1_action = actions
        .iter()
        .find(|a| a.scheduler.as_deref() == Some("small_scheduler"))
        .unwrap();
    assert_eq!(job1_action.trigger_type, "on_workflow_start");

    let job2_action = actions
        .iter()
        .find(|a| a.scheduler.as_deref() == Some("small_deferred_scheduler"))
        .unwrap();
    assert_eq!(job2_action.trigger_type, "on_jobs_ready");
}

/// Test that jobs-per-node calculation considers memory, not just CPUs.
/// When memory is the limiting factor, we should allocate more nodes.
#[rstest]
fn test_generate_schedulers_memory_constrained_allocation() {
    // Create 10 jobs that are memory-heavy: 8 CPUs, 120GB each
    // On Kestrel standard nodes (104 CPUs, 240GB):
    // - CPU-based: 104/8 = 13 jobs per node
    // - Memory-based: 240,000MB / 122,880MB = ~1.95 = 1 job per node
    // Memory should be the limiting factor, so we need 10 nodes for 10 jobs
    let jobs: Vec<JobSpec> = (0..10)
        .map(|i| JobSpec {
            name: format!("memory_job_{}", i),
            command: "echo heavy".to_string(),
            resource_requirements: Some("memory_heavy".to_string()),
            ..Default::default()
        })
        .collect();

    let mut spec = WorkflowSpec {
        name: "memory_test".to_string(),
        description: Some("Test memory-constrained allocation".to_string()),
        jobs,
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "memory_heavy".to_string(),
            num_cpus: 8, // Small CPU requirement
            num_gpus: 0,
            num_nodes: 1,
            memory: "120g".to_string(), // Large memory requirement (120GB = 122,880MB)
            runtime: "PT1H".to_string(),
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();
    let result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        false,
    )
    .unwrap();

    assert_eq!(result.scheduler_count, 1);
    assert_eq!(result.action_count, 1);

    // Check the action's num_allocations
    let actions = spec.actions.as_ref().unwrap();
    assert_eq!(actions.len(), 1);

    let action = &actions[0];
    // 10 jobs, 120GB memory each, 1 hour runtime
    // Concurrent by memory: 240GB / 120GB = 2 (but actually 240000MB / 122880MB = 1.95, so 1)
    // Time slots: 4h walltime / 1h runtime = 4 sequential batches
    // Jobs per allocation: 1 concurrent × 4 time slots = 4 jobs
    // Allocations needed: ceil(10 / 4) = 3
    assert_eq!(
        action.num_allocations,
        Some(3),
        "Should allocate 3 nodes for 10 memory-heavy jobs (1 concurrent × 4 time slots = 4 jobs per allocation)"
    );
}

/// Test mixed constraint: some jobs CPU-limited, some memory-limited
#[rstest]
fn test_generate_schedulers_cpu_vs_memory_constraint() {
    let mut spec = WorkflowSpec {
        name: "mixed_constraint_test".to_string(),
        description: Some("Test CPU vs memory constraints".to_string()),
        jobs: vec![
            // 4 CPU-limited jobs: 52 CPUs, 60GB each
            // On 104 CPU / 240GB node: 104/52=2 by CPU, 240000/61440=3.9 by memory -> CPU wins (2 per node)
            // 4 jobs / 2 per node = 2 allocations
            JobSpec {
                name: "cpu_job_1".to_string(),
                command: "echo cpu".to_string(),
                resource_requirements: Some("cpu_heavy".to_string()),
                ..Default::default()
            },
            JobSpec {
                name: "cpu_job_2".to_string(),
                command: "echo cpu".to_string(),
                resource_requirements: Some("cpu_heavy".to_string()),
                ..Default::default()
            },
            JobSpec {
                name: "cpu_job_3".to_string(),
                command: "echo cpu".to_string(),
                resource_requirements: Some("cpu_heavy".to_string()),
                ..Default::default()
            },
            JobSpec {
                name: "cpu_job_4".to_string(),
                command: "echo cpu".to_string(),
                resource_requirements: Some("cpu_heavy".to_string()),
                ..Default::default()
            },
        ],
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "cpu_heavy".to_string(),
            num_cpus: 52, // Half the CPUs
            num_gpus: 0,
            num_nodes: 1,
            memory: "60g".to_string(), // Only 1/4 of memory
            runtime: "PT1H".to_string(),
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();
    let _result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        false,
    )
    .unwrap();

    let actions = spec.actions.as_ref().unwrap();
    let action = &actions[0];

    // 4 jobs, 52 CPUs each, 60GB memory, 1 hour runtime
    // Concurrent by CPU: 104/52 = 2 jobs per node
    // Concurrent by memory: 240000/61440 = 3.9 = 3 jobs per node
    // Concurrent = min(2, 3) = 2 jobs per node (CPU-limited)
    // Time slots: 4h walltime / 1h runtime = 4 sequential batches
    // Jobs per allocation: 2 concurrent × 4 time slots = 8 jobs
    // Allocations needed: ceil(4 / 8) = 1
    assert_eq!(
        action.num_allocations,
        Some(1),
        "Should allocate 1 node for 4 CPU-heavy jobs (2 concurrent × 4 time slots = 8 jobs per allocation)"
    );
}

// ============== sinfo Parsing Tests ==============

use torc::client::commands::hpc::parse_sinfo_string;

/// Test parsing sinfo output from Kestrel HPC cluster
#[rstest]
fn test_parse_sinfo_string_kestrel() {
    let sinfo_output = r#"bigmem|104|2000000|2-00:00:00|(null)|10
bigmem-stdby|104|2000000|2-00:00:00|(null)|10
bigmeml|104|2000000|10-00:00:00|(null)|10
short*|104|246064|4:00:00|(null)|2112
short*|104|984256|4:00:00|(null)|64
short-stdby|104|246064|4:00:00|(null)|2112
short-stdby|104|984256|4:00:00|(null)|64
medmem|104|984256|10-00:00:00|(null)|64
medmem-stdby|104|984256|2-00:00:00|(null)|64
standard|104|246064|2-00:00:00|(null)|2112
standard|104|984256|2-00:00:00|(null)|64
standard-stdby|104|246064|2-00:00:00|(null)|2112
standard-stdby|104|984256|2-00:00:00|(null)|64
long|104|246064|10-00:00:00|(null)|1632
long|104|984256|10-00:00:00|(null)|32
hbw|104|984256|2-00:00:00|(null)|32
hbw|104|246064|2-00:00:00|(null)|480
hbw-stdby|104|984256|2-00:00:00|(null)|32
hbw-stdby|104|246064|2-00:00:00|(null)|480
hbwl|104|984256|10-00:00:00|(null)|32
hbwl|104|246064|10-00:00:00|(null)|480
debug|104|246064|1:00:00|(null)|1376
debug|104|984256|1:00:00|(null)|32
debug|104|2000000|1:00:00|(null)|10
debug-stdby|104|246064|1:00:00|(null)|1376
debug-stdby|104|984256|1:00:00|(null)|32
debug-stdby|104|2000000|1:00:00|(null)|10
debug-gpu|128|1440000|1:00:00|gpu:h100:4(S:0-3)|24
debug-gpu|128|360000|1:00:00|gpu:h100:4(S:0-3)|105
debug-gpu|128|360000|1:00:00|gpu:h100:4(S:0-1)|3
debug-gpu|128|720000|1:00:00|gpu:h100:4(S:0-3)|24
debug-gpu-stdby|128|1440000|1:00:00|gpu:h100:4(S:0-3)|24
debug-gpu-stdby|128|360000|1:00:00|gpu:h100:4(S:0-3)|105
debug-gpu-stdby|128|360000|1:00:00|gpu:h100:4(S:0-1)|3
debug-gpu-stdby|128|720000|1:00:00|gpu:h100:4(S:0-3)|24
nvme|104|246064|2-00:00:00|(null)|256
shared|104|246064|2-00:00:00|(null)|128
shared-stdby|104|246064|2-00:00:00|(null)|128
sharedl|104|246064|10-00:00:00|(null)|128
gpu-h100s|128|1440000|4:00:00|gpu:h100:4(S:0-3)|24
gpu-h100s|128|360000|4:00:00|gpu:h100:4(S:0-3)|105
gpu-h100s|128|360000|4:00:00|gpu:h100:4(S:0-1)|3
gpu-h100s|128|720000|4:00:00|gpu:h100:4(S:0-3)|24
gpu-h100s-stdby|128|1440000|4:00:00|gpu:h100:4(S:0-3)|24
gpu-h100s-stdby|128|360000|4:00:00|gpu:h100:4(S:0-3)|105
gpu-h100s-stdby|128|360000|4:00:00|gpu:h100:4(S:0-1)|3
gpu-h100s-stdby|128|720000|4:00:00|gpu:h100:4(S:0-3)|24
gpu-h100|128|1440000|2-00:00:00|gpu:h100:4(S:0-3)|24
gpu-h100|128|360000|2-00:00:00|gpu:h100:4(S:0-3)|105
gpu-h100|128|360000|2-00:00:00|gpu:h100:4(S:0-1)|3
gpu-h100|128|720000|2-00:00:00|gpu:h100:4(S:0-3)|24
gpu-h100-stdby|128|1440000|2-00:00:00|gpu:h100:4(S:0-3)|24
gpu-h100-stdby|128|360000|2-00:00:00|gpu:h100:4(S:0-3)|105
gpu-h100-stdby|128|360000|2-00:00:00|gpu:h100:4(S:0-1)|3
gpu-h100-stdby|128|720000|2-00:00:00|gpu:h100:4(S:0-3)|24
gpu-h100l|128|1440000|10-00:00:00|gpu:h100:4(S:0-3)|24
gpu-h100l|128|360000|10-00:00:00|gpu:h100:4(S:0-3)|105
gpu-h100l|128|360000|10-00:00:00|gpu:h100:4(S:0-1)|3
gpu-h100l|128|720000|10-00:00:00|gpu:h100:4(S:0-3)|24
vto|128|1440000|2-00:00:00|gpu:h100:4(S:0-3)|24
vto|128|360000|2-00:00:00|gpu:h100:4(S:0-3)|105
vto|128|360000|2-00:00:00|gpu:h100:4(S:0-1)|3
vto|128|720000|2-00:00:00|gpu:h100:4(S:0-3)|24
gpu-a100|64|246064|2-00:00:00|gpu:a100:4|2
gpu-a100|64|246064|2-00:00:00|gpu:a100:4(S:0)|4"#;

    let partitions = parse_sinfo_string(sinfo_output).unwrap();

    // Should parse all 65 lines
    assert_eq!(partitions.len(), 65);

    // Check a CPU-only partition (bigmem)
    let bigmem = partitions.iter().find(|p| p.name == "bigmem").unwrap();
    assert_eq!(bigmem.cpus, 104);
    assert_eq!(bigmem.memory_mb, 2_000_000);
    assert_eq!(bigmem.timelimit_secs, 2 * 24 * 3600); // 2 days
    assert!(bigmem.gres.is_none());

    // Check default partition (short*) - asterisk should be stripped
    let short_partitions: Vec<_> = partitions.iter().filter(|p| p.name == "short").collect();
    assert_eq!(short_partitions.len(), 2); // Two different node types
    assert_eq!(short_partitions[0].cpus, 104);
    assert_eq!(short_partitions[0].timelimit_secs, 4 * 3600); // 4 hours

    // Check a GPU partition (gpu-h100)
    let gpu_h100: Vec<_> = partitions.iter().filter(|p| p.name == "gpu-h100").collect();
    assert_eq!(gpu_h100.len(), 4); // 4 different node types
    assert_eq!(gpu_h100[0].cpus, 128);
    assert_eq!(gpu_h100[0].timelimit_secs, 2 * 24 * 3600); // 2 days
    assert!(gpu_h100[0].gres.as_ref().unwrap().contains("gpu:h100:4"));

    // Check GPU partition with A100s
    let gpu_a100: Vec<_> = partitions.iter().filter(|p| p.name == "gpu-a100").collect();
    assert_eq!(gpu_a100.len(), 2);
    assert_eq!(gpu_a100[0].cpus, 64);
    // One has simple gres, one has socket-specific
    assert!(
        gpu_a100
            .iter()
            .any(|p| p.gres.as_ref().unwrap() == "gpu:a100:4")
    );
    assert!(
        gpu_a100
            .iter()
            .any(|p| p.gres.as_ref().unwrap() == "gpu:a100:4(S:0)")
    );

    // Check long partition (10 days = 864000 seconds)
    let long_partitions: Vec<_> = partitions.iter().filter(|p| p.name == "long").collect();
    assert!(!long_partitions.is_empty());
    assert_eq!(long_partitions[0].timelimit_secs, 10 * 24 * 3600);

    // Check debug partition (1 hour)
    let debug_partitions: Vec<_> = partitions.iter().filter(|p| p.name == "debug").collect();
    assert_eq!(debug_partitions.len(), 3); // 3 different node types
    assert_eq!(debug_partitions[0].timelimit_secs, 3600); // 1 hour
}

/// Test parsing empty sinfo output
#[rstest]
fn test_parse_sinfo_string_empty() {
    let result = parse_sinfo_string("").unwrap();
    assert!(result.is_empty());
}

/// Test parsing sinfo output with incomplete lines
#[rstest]
fn test_parse_sinfo_string_incomplete_lines() {
    let input = "partition|104|2000000|2-00:00:00|(null)|10\nincomplete|104\n";
    let result = parse_sinfo_string(input).unwrap();
    assert_eq!(result.len(), 1); // Only the complete line should be parsed
    assert_eq!(result[0].name, "partition");
}

/// Test that GPU constraints are considered in jobs-per-node calculation.
/// When GPUs are the limiting factor, we should allocate more nodes.
#[rstest]
fn test_generate_schedulers_gpu_constrained_allocation() {
    // Create 8 jobs that need 2 GPUs each
    // On Kestrel GPU nodes (128 CPUs, 360GB, 4 GPUs):
    // - CPU-based: 128/32 = 4 jobs per node
    // - Memory-based: 360,000MB / 92,160MB = 3.9 = 3 jobs per node
    // - GPU-based: 4/2 = 2 jobs per node
    // GPU should be the limiting factor, so we need 4 nodes for 8 jobs
    let jobs: Vec<JobSpec> = (0..8)
        .map(|i| JobSpec {
            name: format!("gpu_job_{}", i),
            command: "python train.py".to_string(),
            resource_requirements: Some("gpu_training".to_string()),
            ..Default::default()
        })
        .collect();

    let mut spec = WorkflowSpec {
        name: "gpu_test".to_string(),
        description: Some("Test GPU-constrained allocation".to_string()),
        jobs,
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "gpu_training".to_string(),
            num_cpus: 32, // 1/4 of node CPUs
            num_gpus: 2,  // Half the GPUs - this should be limiting
            num_nodes: 1,
            memory: "90g".to_string(), // ~1/4 of node memory
            runtime: "PT1H".to_string(),
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();
    let result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        false,
    )
    .unwrap();

    assert_eq!(result.scheduler_count, 1);
    assert_eq!(result.action_count, 1);

    let actions = spec.actions.as_ref().unwrap();
    let action = &actions[0];

    // 8 jobs, 2 GPUs each, 32 CPUs, 90GB memory, 1 hour runtime
    // Concurrent by GPU: 4/2 = 2 jobs per node (GPU is limiting)
    // Concurrent by CPU: 128/32 = 4 jobs per node
    // Concurrent by memory: 360000/92160 = 3.9 = 3 jobs per node
    // Concurrent = min(4, 3, 2) = 2 jobs per node
    // Time slots: 4h walltime / 1h runtime = 4 sequential batches
    // Jobs per allocation: 2 concurrent × 4 time slots = 8 jobs
    // Allocations needed: ceil(8 / 8) = 1
    assert_eq!(
        action.num_allocations,
        Some(1),
        "Should allocate 1 node for 8 GPU jobs (2 concurrent × 4 time slots = 8 jobs per allocation)"
    );
}

// ============== SlurmDefaultsSpec Tests ==============

#[rstest]
fn test_slurm_defaults_spec_serialization() {
    use torc::client::workflow_spec::SlurmDefaultsSpec;

    let mut map = HashMap::new();
    map.insert("ntasks-per-node".to_string(), serde_json::json!(4));
    map.insert("qos".to_string(), serde_json::json!("high"));
    map.insert("tmp".to_string(), serde_json::json!("100G"));
    map.insert("constraint".to_string(), serde_json::json!("cpu"));
    map.insert(
        "reservation".to_string(),
        serde_json::json!("my_reservation"),
    );
    map.insert(
        "mail-user".to_string(),
        serde_json::json!("user@example.com"),
    );
    map.insert("mail-type".to_string(), serde_json::json!("BEGIN,END,FAIL"));
    map.insert("extra".to_string(), serde_json::json!("--exclusive"));
    let defaults = SlurmDefaultsSpec(map);

    // Validate passes (no excluded params)
    assert!(defaults.validate().is_ok());

    // Serialize to JSON
    let json = serde_json::to_string(&defaults).unwrap();

    // Deserialize back
    let parsed: SlurmDefaultsSpec = serde_json::from_str(&json).unwrap();

    let string_map = parsed.to_string_map();
    assert_eq!(string_map.get("ntasks-per-node"), Some(&"4".to_string()));
    assert_eq!(string_map.get("qos"), Some(&"high".to_string()));
    assert_eq!(string_map.get("constraint"), Some(&"cpu".to_string()));
}

#[rstest]
fn test_slurm_defaults_spec_partial_fields() {
    use torc::client::workflow_spec::SlurmDefaultsSpec;

    let mut map = HashMap::new();
    map.insert("qos".to_string(), serde_json::json!("normal"));
    map.insert("mail-user".to_string(), serde_json::json!("test@test.com"));
    let defaults = SlurmDefaultsSpec(map);

    let json = serde_json::to_string(&defaults).unwrap();
    let parsed: SlurmDefaultsSpec = serde_json::from_str(&json).unwrap();

    let string_map = parsed.to_string_map();
    assert_eq!(string_map.get("qos"), Some(&"normal".to_string()));
    assert_eq!(
        string_map.get("mail-user"),
        Some(&"test@test.com".to_string())
    );
    assert!(!string_map.contains_key("constraint"));
}

#[rstest]
fn test_slurm_defaults_spec_empty() {
    use torc::client::workflow_spec::SlurmDefaultsSpec;

    let defaults = SlurmDefaultsSpec::default();
    let json = serde_json::to_string(&defaults).unwrap();

    // Empty struct should serialize to "{}"
    assert_eq!(json, "{}");

    let parsed: SlurmDefaultsSpec = serde_json::from_str(&json).unwrap();
    assert!(parsed.0.is_empty());
}

#[rstest]
fn test_slurm_defaults_spec_validates_excluded_params() {
    use torc::client::workflow_spec::SlurmDefaultsSpec;

    // Test each excluded parameter
    // Note: "account" is NOT in this list as it's now allowed in slurm_defaults
    let excluded_params = vec![
        "partition",
        "nodes",
        "walltime",
        "time",
        "mem",
        "gres",
        "name",
        "job-name",
    ];

    for param in excluded_params {
        let mut map = HashMap::new();
        map.insert(param.to_string(), serde_json::json!("test_value"));
        let defaults = SlurmDefaultsSpec(map);

        let result = defaults.validate();
        assert!(
            result.is_err(),
            "Expected error for excluded param '{}', but got Ok",
            param
        );
        assert!(
            result.unwrap_err().contains(param),
            "Error message should mention the excluded param '{}'",
            param
        );
    }
}

#[rstest]
fn test_slurm_defaults_spec_validates_excluded_params_case_insensitive() {
    use torc::client::workflow_spec::SlurmDefaultsSpec;

    // Test that excluded parameters are rejected regardless of case
    let case_variants = vec![
        ("PARTITION", "partition"),
        ("Partition", "partition"),
        ("NODES", "nodes"),
        ("Nodes", "nodes"),
        ("WallTime", "walltime"),
        ("WALLTIME", "walltime"),
        ("TIME", "time"),
        ("Time", "time"),
        ("MEM", "mem"),
        ("Mem", "mem"),
        ("GRES", "gres"),
        ("Gres", "gres"),
        ("NAME", "name"),
        ("Name", "name"),
        ("JOB-NAME", "job-name"),
        ("Job-Name", "job-name"),
    ];

    for (input_key, _expected_lower) in case_variants {
        let mut map = HashMap::new();
        map.insert(input_key.to_string(), serde_json::json!("test_value"));
        let defaults = SlurmDefaultsSpec(map);

        let result = defaults.validate();
        assert!(
            result.is_err(),
            "Expected error for case variant '{}', but got Ok",
            input_key
        );
    }
}

#[rstest]
fn test_slurm_defaults_spec_allows_arbitrary_params() {
    use torc::client::workflow_spec::SlurmDefaultsSpec;

    // Test that arbitrary sbatch params are allowed
    // Including "account" which is allowed as a workflow-level default
    let mut map = HashMap::new();
    map.insert("nice".to_string(), serde_json::json!(100));
    map.insert("exclude".to_string(), serde_json::json!("node[1-5]"));
    map.insert("comment".to_string(), serde_json::json!("My job comment"));
    map.insert("exclusive".to_string(), serde_json::json!(true));
    map.insert("requeue".to_string(), serde_json::json!(true));
    map.insert("account".to_string(), serde_json::json!("myproject"));
    let defaults = SlurmDefaultsSpec(map);

    // Validation should pass
    assert!(defaults.validate().is_ok());

    let string_map = defaults.to_string_map();
    assert_eq!(string_map.get("nice"), Some(&"100".to_string()));
    assert_eq!(string_map.get("exclude"), Some(&"node[1-5]".to_string()));
    assert_eq!(
        string_map.get("comment"),
        Some(&"My job comment".to_string())
    );
    assert_eq!(string_map.get("exclusive"), Some(&"true".to_string()));
    assert_eq!(string_map.get("account"), Some(&"myproject".to_string()));
}

#[rstest]
fn test_workflow_spec_with_slurm_defaults() {
    use torc::client::workflow_spec::SlurmDefaultsSpec;

    let mut defaults_map = HashMap::new();
    defaults_map.insert("qos".to_string(), serde_json::json!("high"));
    defaults_map.insert(
        "mail-user".to_string(),
        serde_json::json!("user@example.com"),
    );
    defaults_map.insert("mail-type".to_string(), serde_json::json!("END,FAIL"));

    let mut spec = WorkflowSpec {
        name: "test_workflow".to_string(),
        description: Some("Test workflow with slurm_defaults".to_string()),
        jobs: vec![JobSpec {
            name: "job1".to_string(),
            command: "echo hello".to_string(),
            resource_requirements: Some("small".to_string()),
            ..Default::default()
        }],
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "small".to_string(),
            num_cpus: 4,
            num_gpus: 0,
            num_nodes: 1,
            memory: "8g".to_string(),
            runtime: "PT1H".to_string(),
        }]),
        slurm_defaults: Some(SlurmDefaultsSpec(defaults_map)),
        ..Default::default()
    };

    // Verify slurm_defaults is set and validates
    assert!(spec.slurm_defaults.is_some());
    let defaults = spec.slurm_defaults.as_ref().unwrap();
    assert!(defaults.validate().is_ok());
    let string_map = defaults.to_string_map();
    assert_eq!(string_map.get("qos"), Some(&"high".to_string()));
    assert_eq!(
        string_map.get("mail-user"),
        Some(&"user@example.com".to_string())
    );

    // Verify workflow spec can still be used for scheduler generation
    let profile = kestrel_profile();
    let result = generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        false,
    )
    .unwrap();

    assert_eq!(result.scheduler_count, 1);
}

#[rstest]
fn test_slurm_defaults_yaml_parsing() {
    // Test parsing YAML with slurm_defaults (using hyphenated key names)
    let yaml = r#"
name: test_workflow
jobs:
  - name: job1
    command: echo hello
    resource_requirements: small
resource_requirements:
  - name: small
    num_cpus: 4
    num_gpus: 0
    num_nodes: 1
    memory: 8g
    runtime: PT1H
slurm_defaults:
  qos: normal
  mail-user: test@example.com
  mail-type: END
  constraint: cpu
"#;

    let spec: WorkflowSpec = serde_yaml::from_str(yaml).unwrap();

    assert!(spec.slurm_defaults.is_some());
    let defaults = spec.slurm_defaults.as_ref().unwrap();
    assert!(defaults.validate().is_ok());
    let string_map = defaults.to_string_map();
    assert_eq!(string_map.get("qos"), Some(&"normal".to_string()));
    assert_eq!(
        string_map.get("mail-user"),
        Some(&"test@example.com".to_string())
    );
    assert_eq!(string_map.get("mail-type"), Some(&"END".to_string()));
    assert_eq!(string_map.get("constraint"), Some(&"cpu".to_string()));
}

#[rstest]
fn test_slurm_defaults_json_roundtrip() {
    use torc::client::workflow_spec::SlurmDefaultsSpec;

    let mut defaults_map = HashMap::new();
    defaults_map.insert("ntasks-per-node".to_string(), serde_json::json!(8));
    defaults_map.insert("qos".to_string(), serde_json::json!("priority"));
    defaults_map.insert("reservation".to_string(), serde_json::json!("special"));
    defaults_map.insert("extra".to_string(), serde_json::json!("--nice=100"));

    let spec = WorkflowSpec {
        name: "test_workflow".to_string(),
        description: Some("Test".to_string()),
        jobs: vec![JobSpec {
            name: "job1".to_string(),
            command: "echo hello".to_string(),
            ..Default::default()
        }],
        slurm_defaults: Some(SlurmDefaultsSpec(defaults_map)),
        ..Default::default()
    };

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&spec).unwrap();

    // Deserialize back
    let parsed: WorkflowSpec = serde_json::from_str(&json).unwrap();

    assert!(parsed.slurm_defaults.is_some());
    let defaults = parsed.slurm_defaults.as_ref().unwrap();
    assert!(defaults.validate().is_ok());
    let string_map = defaults.to_string_map();
    assert_eq!(string_map.get("ntasks-per-node"), Some(&"8".to_string()));
    assert_eq!(string_map.get("qos"), Some(&"priority".to_string()));
    assert_eq!(string_map.get("reservation"), Some(&"special".to_string()));
    assert_eq!(string_map.get("extra"), Some(&"--nice=100".to_string()));
}
