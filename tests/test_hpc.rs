//! Tests for HPC profile system and scheduler generation

use rstest::rstest;
use serial_test::serial;
use std::collections::HashMap;

/// RAII guard to restore environment variables after a test
struct EnvGuard {
    vars: HashMap<String, Option<String>>,
}

impl EnvGuard {
    fn new(overrides: HashMap<&str, &str>) -> Self {
        let mut vars = HashMap::new();
        for (key, value) in overrides {
            vars.insert(key.to_string(), std::env::var(key).ok());
            unsafe {
                std::env::set_var(key, value);
            }
        }
        Self { vars }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in &self.vars {
            unsafe {
                match value {
                    Some(v) => std::env::set_var(key, v),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
}
use torc::client::commands::slurm::{
    GroupByStrategy, WalltimeStrategy, generate_schedulers_for_workflow, parse_memory_mb,
    parse_walltime_secs, secs_to_walltime,
};
use torc::client::hpc::kestrel::kestrel_profile;
use torc::client::hpc::{HpcDetection, HpcPartition, HpcProfile, HpcProfileRegistry};
use torc::client::scheduler_plan::{SchedulerOverrides, generate_scheduler_plan};
use torc::client::workflow_graph::WorkflowGraph;
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
    assert_eq!(standard.memory_mb, 246_064);
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
    // Kestrel standard partition has 246,064 MB = 240g.
    assert_eq!(scheduler.mem.as_deref(), Some("240g"));
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
    // Allocation walltime: 1h × 1.5 multiplier = 1.5h
    // Time slots: floor(1.5h / 1h) = 1 sequential batch
    // Jobs per allocation: 4 concurrent × 1 time slot = 4 jobs
    // Allocations needed: ceil(10 / 4) = 3
    assert_eq!(action.num_allocations, Some(3));
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
    // Allocation walltime: 1h × 1.5 multiplier = 1.5h
    // Time slots: floor(1.5h / 1h) = 1 sequential batch
    // Jobs per allocation: 2 concurrent × 1 time slot = 2 jobs
    // Allocations needed: ceil(100 / 2) = 50
    assert_eq!(action.num_allocations, Some(50));
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
    // On Kestrel standard nodes (104 CPUs, 246,064MB):
    // - CPU-based: 104/8 = 13 jobs per node
    // - Memory-based: 246,064MB / 122,880MB = 2 jobs per node
    // Memory should be the limiting factor
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
    // Concurrent by memory: 246,064MB / 122,880MB = 2 jobs per node
    // Allocation walltime: 1h × 1.5 multiplier = 1.5h
    // Time slots: floor(1.5h / 1h) = 1 sequential batch
    // Jobs per allocation: 2 concurrent × 1 time slot = 2 jobs
    // Allocations needed: ceil(10 / 2) = 5
    assert_eq!(
        action.num_allocations,
        Some(5),
        "Should allocate 5 nodes for 10 memory-heavy jobs (2 concurrent × 1 time slot = 2 jobs per allocation)"
    );
}

/// Test that long-running whole-node jobs each get their own allocation.
///
/// Regression test: previously, time_slots was calculated from partition max walltime
/// instead of the actual allocation walltime. With MaxJobRuntime strategy and 1.5x multiplier,
/// a 20h job gets a 30h allocation walltime. But the bug used the 48h partition max,
/// giving time_slots = 48h/20h = 2, which halved the allocation count (5 instead of 10).
#[rstest]
fn test_generate_schedulers_whole_node_long_runtime() {
    // 10 jobs each consuming an entire node: 12 CPUs, 160GB memory, 20 hours runtime.
    // On Kestrel standard partition (104 CPUs, 246,064MB, 48h max walltime):
    // - Only 1 job fits per node (160GB = 163,840MB, 246,064/163,840 = 1)
    // - Allocation walltime: 20h × 1.5 = 30h (MaxJobRuntime strategy)
    // - Time slots: floor(30h / 20h) = 1
    // - Each allocation can run exactly 1 job, so we need 10 allocations.
    let jobs: Vec<JobSpec> = (0..10)
        .map(|i| JobSpec {
            name: format!("draw_{}_extreme_week", i + 1),
            command: format!("julia outagesim.jl {}", i + 1),
            resource_requirements: Some("whole_node".to_string()),
            ..Default::default()
        })
        .collect();

    let mut spec = WorkflowSpec {
        name: "nodal_test".to_string(),
        description: Some("Long-running whole-node jobs".to_string()),
        jobs,
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "whole_node".to_string(),
            num_cpus: 12,
            num_gpus: 0,
            num_nodes: 1,

            memory: "160g".to_string(),
            runtime: "PT20H".to_string(),
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

    // 10 jobs, 12 CPUs, 160GB memory, 20h runtime
    // Concurrent by memory: 246,064MB / 163,840MB = 1 job per node
    // Concurrent by CPU: 104 / 12 = 8 jobs per node
    // Concurrent = min(8, 1) = 1 job per node (memory-limited)
    // Allocation walltime: 20h × 1.5 = 30h
    // Time slots: floor(30h / 20h) = 1
    // Jobs per allocation: 1 concurrent × 1 time slot = 1 job
    // Allocations needed: ceil(10 / 1) = 10
    assert_eq!(
        action.num_allocations,
        Some(10),
        "Each whole-node job should get its own allocation (10 jobs = 10 allocations)"
    );

    // Verify walltime is based on job runtime, not partition max
    let schedulers = spec.slurm_schedulers.as_ref().unwrap();
    assert_eq!(schedulers.len(), 1);
    // 20h × 1.5 = 30h = "1-06:00:00"
    assert_eq!(
        schedulers[0].walltime, "1-06:00:00",
        "Walltime should be 20h × 1.5 = 30h, not the partition max of 48h"
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
            // On 104 CPU / 246,064MB node: 104/52=2 by CPU, 246064/61440=4 by memory -> CPU wins (2 per node)
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
    // Concurrent by memory: 246064/61440 = 4 jobs per node
    // Concurrent = min(2, 4) = 2 jobs per node (CPU-limited)
    // Allocation walltime: 1h × 1.5 multiplier = 1.5h
    // Time slots: floor(1.5h / 1h) = 1 sequential batch
    // Jobs per allocation: 2 concurrent × 1 time slot = 2 jobs
    // Allocations needed: ceil(4 / 2) = 2
    assert_eq!(
        action.num_allocations,
        Some(2),
        "Should allocate 2 nodes for 4 CPU-heavy jobs (2 concurrent × 1 time slot = 2 jobs per allocation)"
    );
}

// ============== Dynamic Slurm Profile Tests ==============

#[rstest]
#[serial(slurm)]
fn test_detect_slurm_profile() {
    use std::path::Path;
    use torc::client::hpc::slurm::detect_slurm_profile;

    let sinfo_path = Path::new("tests/scripts/fake_sinfo.sh")
        .canonicalize()
        .unwrap();
    let scontrol_path = Path::new("tests/scripts/fake_scontrol.sh")
        .canonicalize()
        .unwrap();

    let mut overrides = HashMap::new();
    overrides.insert("TORC_FAKE_SINFO", sinfo_path.to_str().unwrap());
    overrides.insert("TORC_FAKE_SCONTROL", scontrol_path.to_str().unwrap());
    let _guard = EnvGuard::new(overrides);

    let profile = detect_slurm_profile().expect("Should detect slurm profile");

    assert_eq!(profile.name, "test_cluster");
    assert!(profile.display_name.contains("Test_cluster"));
    assert_eq!(profile.partitions.len(), 2);

    let standard = profile.get_partition("standard").unwrap();
    assert_eq!(standard.cpus_per_node, 104);
    assert_eq!(standard.memory_mb, 246064);

    let gpu = profile.get_partition("gpu").unwrap();
    assert_eq!(gpu.gpus_per_node, Some(4));
    assert_eq!(gpu.gpu_type, Some("h100".to_string()));
}

#[rstest]
#[serial(slurm)]
fn test_registry_detect_dynamic_slurm() {
    use std::path::Path;

    let sinfo_path = Path::new("tests/scripts/fake_sinfo.sh")
        .canonicalize()
        .unwrap();
    let scontrol_path = Path::new("tests/scripts/fake_scontrol.sh")
        .canonicalize()
        .unwrap();

    let mut overrides = HashMap::new();
    overrides.insert("TORC_FAKE_SINFO", sinfo_path.to_str().unwrap());
    overrides.insert("TORC_FAKE_SCONTROL", scontrol_path.to_str().unwrap());
    let _guard = EnvGuard::new(overrides);

    let registry = HpcProfileRegistry::new();
    let profile = registry
        .detect()
        .expect("Should detect dynamic slurm profile");

    assert_eq!(profile.name, "test_cluster");
}

#[rstest]
#[serial(slurm)]
fn test_registry_get_slurm() {
    use std::path::Path;

    let sinfo_path = Path::new("tests/scripts/fake_sinfo.sh")
        .canonicalize()
        .unwrap();
    let scontrol_path = Path::new("tests/scripts/fake_scontrol.sh")
        .canonicalize()
        .unwrap();

    let mut overrides = HashMap::new();
    overrides.insert("TORC_FAKE_SINFO", sinfo_path.to_str().unwrap());
    overrides.insert("TORC_FAKE_SCONTROL", scontrol_path.to_str().unwrap());
    let _guard = EnvGuard::new(overrides);

    let registry = HpcProfileRegistry::new();
    let profile = registry.get("slurm").expect("Should return slurm profile");

    assert_eq!(profile.name, "test_cluster");
}

use torc::client::hpc::slurm::parse_sinfo_string;

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
    // Allocation walltime: 1h × 1.5 multiplier = 1.5h
    // Time slots: floor(1.5h / 1h) = 1 sequential batch
    // Jobs per allocation: 2 concurrent × 1 time slot = 2 jobs
    // Allocations needed: ceil(8 / 2) = 4
    assert_eq!(
        action.num_allocations,
        Some(4),
        "Should allocate 4 nodes for 8 GPU jobs (2 concurrent × 1 time slot = 2 jobs per allocation)"
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

// ============== Comprehensive Allocation Calculation Tests ==============
//
// These tests systematically verify that compute node allocation correctly
// identifies the limiting resource (CPU, memory, GPU) and calculates the
// right number of Slurm allocations across typical HPC usage patterns.
//
// Coverage matrix:
//   - CPU-limited: varying CPU count (4..104) with low memory
//   - Memory-limited: varying memory (30g..240g) with low CPUs
//   - GPU-limited: varying GPU count (1..4) with low CPU/memory
//   - Mixed bottleneck: CPU vs memory, GPU vs CPU vs memory
//   - MaxPartitionTime strategy: time_slots > 1 (sequential batches)
//   - Edge cases: single job, exact fit, multi-node, large batches
//   - Regression: walltime strategy uses allocation walltime, not partition max
//
// Kestrel partition reference:
//   - short: 104 CPUs, 246,064 MB, 4h max, no GPUs
//   - standard: 104 CPUs, 246,064 MB, 48h max, no GPUs
//   - long: 104 CPUs, 246,064 MB, 10d max, no GPUs
//   - gpu-h100s: 128 CPUs, 360,000 MB, 4h max, 4 GPUs
//   - gpu-h100: 128 CPUs, 360,000 MB, 48h max, 4 GPUs

/// Helper to create a WorkflowSpec, generate schedulers, and return num_allocations.
#[allow(clippy::too_many_arguments)]
fn compute_allocations(
    job_count: usize,
    num_cpus: i64,
    num_gpus: i64,
    num_nodes: i64,
    memory: &str,
    runtime: &str,
    strategy: WalltimeStrategy,
    multiplier: f64,
) -> i64 {
    let jobs: Vec<JobSpec> = (0..job_count)
        .map(|i| JobSpec {
            name: format!("job_{i}"),
            command: "echo test".to_string(),
            resource_requirements: Some("rr".to_string()),
            ..Default::default()
        })
        .collect();

    let mut spec = WorkflowSpec {
        name: "alloc_test".to_string(),
        jobs,
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "rr".to_string(),
            num_cpus,
            num_gpus,
            num_nodes,

            memory: memory.to_string(),
            runtime: runtime.to_string(),
        }]),
        ..Default::default()
    };

    let profile = kestrel_profile();
    generate_schedulers_for_workflow(
        &mut spec,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        strategy,
        multiplier,
        true,
        false,
    )
    .unwrap();

    spec.actions.as_ref().unwrap()[0].num_allocations.unwrap()
}

// --------------- CPU-limited allocation tests ---------------
//
// Using 4g memory (4,096 MB) so CPU is always the bottleneck.
// concurrent_mem = 246,064 / 4,096 = 60 (always larger than concurrent_cpu).
// With MaxJobRuntime 1.5x: time_slots = 1 always.
// Allocation count = ceil(job_count / (104 / num_cpus)).

/// CPU-limited: short walltime (PT10M) → routed to short partition (4h max).
#[rstest]
#[case::tiny_cpu(4, 1)] // 104/4=26 concurrent, ceil(20/26)=1
#[case::small_cpu(13, 3)] // 104/13=8, ceil(20/8)=3
#[case::medium_cpu(26, 5)] // 104/26=4, ceil(20/4)=5
#[case::high_cpu(52, 10)] // 104/52=2, ceil(20/2)=10
#[case::whole_node(104, 20)] // 104/104=1, ceil(20/1)=20
fn test_alloc_cpu_limited_short_walltime(#[case] num_cpus: i64, #[case] expected: i64) {
    let result = compute_allocations(
        20,
        num_cpus,
        0,
        1,
        "4g",
        "PT10M",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    assert_eq!(result, expected);
}

/// CPU-limited: medium walltime (PT8H) → routed to standard partition (48h max).
#[rstest]
#[case::tiny_cpu(4, 1)]
#[case::small_cpu(13, 3)]
#[case::medium_cpu(26, 5)]
#[case::high_cpu(52, 10)]
#[case::whole_node(104, 20)]
fn test_alloc_cpu_limited_medium_walltime(#[case] num_cpus: i64, #[case] expected: i64) {
    let result = compute_allocations(
        20,
        num_cpus,
        0,
        1,
        "4g",
        "PT8H",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    assert_eq!(result, expected);
}

/// CPU-limited: long walltime (PT36H) → routed to standard partition (48h max).
/// Walltime capped: 36h × 1.5 = 54h > 48h partition max → walltime = 48h.
/// time_slots = floor(48h/36h) = 1, so allocations match shorter cases.
#[rstest]
#[case::tiny_cpu(4, 1)]
#[case::medium_cpu(26, 5)]
#[case::high_cpu(52, 10)]
#[case::whole_node(104, 20)]
fn test_alloc_cpu_limited_long_walltime(#[case] num_cpus: i64, #[case] expected: i64) {
    let result = compute_allocations(
        20,
        num_cpus,
        0,
        1,
        "4g",
        "PT36H",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    assert_eq!(result, expected);
}

// --------------- Memory-limited allocation tests ---------------
//
// Using 4 CPUs so memory is always the bottleneck.
// concurrent_cpu = 104/4 = 26 (always larger than concurrent_mem for ≥30g).
// Allocation count = ceil(20 / (246,064 / memory_mb)).

/// Memory-limited: medium walltime (PT8H), standard partition.
#[rstest]
#[case::medium_mem("30g", 3)] // 246064/30720=8, ceil(20/8)=3
#[case::high_mem("60g", 5)] // 246064/61440=4, ceil(20/4)=5
#[case::very_high_mem("120g", 10)] // 246064/122880=2, ceil(20/2)=10
#[case::near_whole_node("240g", 20)] // 246064/245760=1, ceil(20/1)=20
fn test_alloc_memory_limited(#[case] memory: &str, #[case] expected: i64) {
    let result = compute_allocations(
        20,
        4,
        0,
        1,
        memory,
        "PT8H",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    assert_eq!(result, expected);
}

/// Memory-limited with short walltime: same concurrent capacity, different partition.
#[rstest]
#[case::medium_mem("30g", 3)]
#[case::high_mem("60g", 5)]
#[case::very_high_mem("120g", 10)]
fn test_alloc_memory_limited_short_walltime(#[case] memory: &str, #[case] expected: i64) {
    let result = compute_allocations(
        20,
        4,
        0,
        1,
        memory,
        "PT10M",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    assert_eq!(result, expected);
}

/// Memory-limited with long walltime: walltime capped at partition max.
#[rstest]
#[case::medium_mem("30g", 3)]
#[case::very_high_mem("120g", 10)]
#[case::near_whole_node("240g", 20)]
fn test_alloc_memory_limited_long_walltime(#[case] memory: &str, #[case] expected: i64) {
    let result = compute_allocations(
        20,
        4,
        0,
        1,
        memory,
        "PT36H",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    assert_eq!(result, expected);
}

// --------------- GPU-limited allocation tests ---------------
//
// GPU partition (gpu-h100s): 128 CPUs, 360,000 MB, 4 GPUs.
// Using 16 CPUs, 10g so GPU is always the bottleneck:
//   concurrent_cpu = 128/16 = 8, concurrent_mem = 360000/10240 = 35.
// Allocation count = ceil(12 / (4 / num_gpus)).

/// GPU-limited: short walltime (PT1H) → gpu-h100s partition.
#[rstest]
#[case::one_gpu(1, 3)] // 4/1=4 concurrent, ceil(12/4)=3
#[case::two_gpus(2, 6)] // 4/2=2, ceil(12/2)=6
#[case::four_gpus(4, 12)] // 4/4=1, ceil(12/1)=12
fn test_alloc_gpu_limited_short_walltime(#[case] num_gpus: i64, #[case] expected: i64) {
    let result = compute_allocations(
        12,
        16,
        num_gpus,
        1,
        "10g",
        "PT1H",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    assert_eq!(result, expected);
}

/// GPU-limited: medium walltime (PT8H) → gpu-h100 partition (48h max).
#[rstest]
#[case::one_gpu(1, 3)]
#[case::two_gpus(2, 6)]
#[case::four_gpus(4, 12)]
fn test_alloc_gpu_limited_medium_walltime(#[case] num_gpus: i64, #[case] expected: i64) {
    let result = compute_allocations(
        12,
        16,
        num_gpus,
        1,
        "10g",
        "PT8H",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    assert_eq!(result, expected);
}

// --------------- Mixed bottleneck tests (CPU vs Memory) ---------------
//
// Standard partition: 104 CPUs, 246,064 MB.
// PT8H → standard partition. MaxJobRuntime 1.5x, time_slots = 1.

/// Mixed CPU/memory: the limiting resource shifts with different combinations.
#[rstest]
//            CPUs  Memory  Expected  Bottleneck
#[case::cpu_bottleneck(26, "10g", 5)] // cpu=4, mem=24 → 4 (CPU)
#[case::mem_bottleneck(26, "120g", 10)] // cpu=4, mem=2 → 2 (Memory)
#[case::cpu_half_node(52, "60g", 10)] // cpu=2, mem=4 → 2 (CPU)
#[case::both_equal(52, "120g", 10)] // cpu=2, mem=2 → 2 (Both)
#[case::mem_whole_node(4, "240g", 20)] // cpu=26, mem=1 → 1 (Memory)
#[case::cpu_whole_node(104, "4g", 20)] // cpu=1, mem=60 → 1 (CPU)
fn test_alloc_mixed_cpu_memory(#[case] num_cpus: i64, #[case] memory: &str, #[case] expected: i64) {
    let result = compute_allocations(
        20,
        num_cpus,
        0,
        1,
        memory,
        "PT8H",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    assert_eq!(result, expected);
}

// --------------- GPU + CPU/Memory mixed bottleneck tests ---------------
//
// GPU partition (gpu-h100s): 128 CPUs, 360,000 MB, 4 GPUs.
// PT1H → gpu-h100s. MaxJobRuntime 1.5x, time_slots = 1.

/// GPU + CPU/memory: the bottleneck shifts across GPU, CPU, and memory.
#[rstest]
//            CPUs  Memory  GPUs  Expected  Bottleneck
#[case::gpu_bottleneck(16, "10g", 1, 3)] // cpu=8, mem=35, gpu=4 → 4 (GPU), ceil(12/4)=3
#[case::cpu_bottleneck(64, "10g", 1, 6)] // cpu=2, mem=35, gpu=4 → 2 (CPU), ceil(12/2)=6
#[case::mem_bottleneck(16, "180g", 1, 12)] // cpu=8, mem=1, gpu=4 → 1 (Memory), ceil(12/1)=12
#[case::gpu_with_high_cpu(32, "90g", 2, 6)] // cpu=4, mem=3, gpu=2 → 2 (GPU), ceil(12/2)=6
#[case::cpu_with_high_gpu(128, "90g", 2, 12)] // cpu=1, mem=3, gpu=2 → 1 (CPU), ceil(12/1)=12
#[case::mem_with_high_gpu(32, "180g", 2, 12)] // cpu=4, mem=1, gpu=2 → 1 (Memory), ceil(12/1)=12
fn test_alloc_mixed_gpu_cpu_memory(
    #[case] num_cpus: i64,
    #[case] memory: &str,
    #[case] num_gpus: i64,
    #[case] expected: i64,
) {
    let result = compute_allocations(
        12,
        num_cpus,
        num_gpus,
        1,
        memory,
        "PT1H",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    assert_eq!(result, expected);
}

// --------------- MaxPartitionTime strategy (time_slots > 1) ---------------
//
// With MaxPartitionTime, walltime = partition max, allowing multiple
// sequential job batches per allocation.
// This tests the core of the bug fix: time_slots must use the correct walltime.

/// MaxPartitionTime: short jobs (PT10M) on short partition (4h max).
/// time_slots = floor(14400/600) = 24. Very high throughput per allocation.
#[rstest]
//            CPUs  Jobs   Expected
#[case::tiny_cpu(4, 1000, 2)] // concurrent=26, 26×24=624/alloc, ceil(1000/624)=2
#[case::medium_cpu(26, 1000, 11)] // concurrent=4, 4×24=96/alloc, ceil(1000/96)=11
#[case::high_cpu(52, 1000, 21)] // concurrent=2, 2×24=48/alloc, ceil(1000/48)=21
#[case::whole_node(104, 1000, 42)] // concurrent=1, 1×24=24/alloc, ceil(1000/24)=42
fn test_alloc_max_partition_time_short_jobs(
    #[case] num_cpus: i64,
    #[case] job_count: usize,
    #[case] expected: i64,
) {
    let result = compute_allocations(
        job_count,
        num_cpus,
        0,
        1,
        "4g",
        "PT10M",
        WalltimeStrategy::MaxPartitionTime,
        1.5,
    );
    assert_eq!(result, expected);
}

/// MaxPartitionTime: medium jobs (PT1H) on short partition (4h max).
/// time_slots = floor(14400/3600) = 4.
#[rstest]
#[case::medium_cpu(26, 100, 7)] // 4×4=16/alloc, ceil(100/16)=7
#[case::high_cpu(52, 100, 13)] // 2×4=8/alloc, ceil(100/8)=13
#[case::whole_node(104, 100, 25)] // 1×4=4/alloc, ceil(100/4)=25
fn test_alloc_max_partition_time_medium_jobs(
    #[case] num_cpus: i64,
    #[case] job_count: usize,
    #[case] expected: i64,
) {
    let result = compute_allocations(
        job_count,
        num_cpus,
        0,
        1,
        "4g",
        "PT1H",
        WalltimeStrategy::MaxPartitionTime,
        1.5,
    );
    assert_eq!(result, expected);
}

/// MaxPartitionTime: long jobs (PT20H) on standard partition (48h max).
/// time_slots = floor(172800/72000) = 2.
#[rstest]
#[case::medium_cpu(26, 100, 13)] // 4×2=8/alloc, ceil(100/8)=13
#[case::high_cpu(52, 100, 25)] // 2×2=4/alloc, ceil(100/4)=25
#[case::whole_node(104, 100, 50)] // 1×2=2/alloc, ceil(100/2)=50
fn test_alloc_max_partition_time_long_jobs(
    #[case] num_cpus: i64,
    #[case] job_count: usize,
    #[case] expected: i64,
) {
    let result = compute_allocations(
        job_count,
        num_cpus,
        0,
        1,
        "4g",
        "PT20H",
        WalltimeStrategy::MaxPartitionTime,
        1.5,
    );
    assert_eq!(result, expected);
}

// --------------- Edge cases ---------------

/// Single job always gets exactly 1 allocation regardless of resources.
#[rstest]
#[case::small_cpu(4, 0, "4g", "PT10M")]
#[case::whole_node_cpu(104, 0, "4g", "PT20H")]
#[case::whole_node_mem(4, 0, "240g", "PT8H")]
#[case::single_gpu(16, 1, "10g", "PT1H")]
#[case::four_gpus(16, 4, "10g", "PT1H")]
fn test_alloc_single_job(
    #[case] num_cpus: i64,
    #[case] num_gpus: i64,
    #[case] memory: &str,
    #[case] runtime: &str,
) {
    let result = compute_allocations(
        1,
        num_cpus,
        num_gpus,
        1,
        memory,
        runtime,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    assert_eq!(result, 1);
}

/// Jobs that exactly fill concurrent capacity → exactly N allocations.
#[rstest]
#[case::exact_one_alloc(4, 1)] // 104/26=4 concurrent, 4 jobs = 1 alloc
#[case::exact_two_allocs(8, 2)] // 4 concurrent, 8 jobs = 2 allocs
#[case::exact_five_allocs(20, 5)] // 4 concurrent, 20 jobs = 5 allocs
fn test_alloc_exact_fit(#[case] job_count: usize, #[case] expected: i64) {
    // 26 CPUs → 4 jobs per node on standard partition
    let result = compute_allocations(
        job_count,
        26,
        0,
        1,
        "4g",
        "PT8H",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    assert_eq!(result, expected);
}

/// Jobs off-by-one from concurrent capacity → one extra allocation.
#[rstest]
#[case::one_over(5, 2)] // 4 concurrent, 5 jobs = 2 allocs (not 1)
#[case::one_over_two(9, 3)] // 4 concurrent, 9 jobs = 3 allocs (not 2)
fn test_alloc_off_by_one(#[case] job_count: usize, #[case] expected: i64) {
    let result = compute_allocations(
        job_count,
        26,
        0,
        1,
        "4g",
        "PT8H",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    assert_eq!(result, expected);
}

/// Multi-node jobs: allocations = ceil(jobs/concurrent) × nodes_per_job.
#[rstest]
#[case::two_nodes(2, 10)] // concurrent=2, ceil(10/2)×2 = 5×2 = 10
#[case::four_nodes(4, 20)] // concurrent=2, ceil(10/2)×4 = 5×4 = 20
fn test_alloc_multi_node_jobs(#[case] num_nodes: i64, #[case] expected: i64) {
    // 52 CPUs → 2 per node on standard partition
    let result = compute_allocations(
        10,
        52,
        0,
        num_nodes,
        "4g",
        "PT8H",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    assert_eq!(result, expected);
}

/// Large batch of jobs (500 jobs).
#[rstest]
fn test_alloc_large_batch() {
    // 52 CPUs, 2 per node, time_slots=1, ceil(500/2)=250
    let result = compute_allocations(
        500,
        52,
        0,
        1,
        "4g",
        "PT10M",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    assert_eq!(result, 250);
}

/// Large batch with time slots: 1000 very short jobs, MaxPartitionTime.
#[rstest]
fn test_alloc_large_batch_with_time_slots() {
    // 1 CPU, 1g memory on short partition (4h max):
    // concurrent_cpu = 104/1 = 104
    // concurrent_mem = 246064/1024 = 240
    // concurrent = min(104, 240) = 104
    // time_slots = 14400/60 = 240
    // jobs_per_alloc = 104 × 240 = 24960
    // ceil(1000/24960) = 1
    let result = compute_allocations(
        1000,
        1,
        0,
        1,
        "1g",
        "PT1M",
        WalltimeStrategy::MaxPartitionTime,
        1.5,
    );
    assert_eq!(result, 1);
}

/// Runtime at partition boundary: job runtime equals partition max walltime.
#[rstest]
fn test_alloc_runtime_at_partition_max() {
    // PT48H = 48h = standard partition max (172,800s).
    // MaxJobRuntime 1.5x: walltime = min(72h, 48h) = 48h.
    // time_slots = floor(48h/48h) = 1.
    // 26 CPUs → 4 concurrent, ceil(20/4) = 5.
    let result = compute_allocations(
        20,
        26,
        0,
        1,
        "4g",
        "P2D",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    assert_eq!(result, 5);
}

// --------------- Regression tests for walltime bug (82cbdc28) ---------------
//
// The bug: calculate_allocations used partition.max_walltime_secs for time_slots
// instead of the actual allocation_walltime_secs. With MaxJobRuntime strategy,
// this inflated time_slots, causing under-allocation.
//
// Example: 20h job on standard partition (48h max):
//   Correct: walltime = 30h, time_slots = floor(30h/20h) = 1
//   Bug:     time_slots = floor(48h/20h) = 2 → half the allocations needed

/// Regression: long jobs must not use partition max for time_slots.
///
/// With MaxJobRuntime 1.5x and 20h runtime:
///   Correct: walltime=30h, slots=1, allocs = ceil(100/4) = 25
///   Bug:     slots = floor(48h/20h) = 2, allocs = ceil(100/8) = 13
#[rstest]
fn test_alloc_regression_long_runtime_correct_timeslots() {
    let result = compute_allocations(
        100,
        26,
        0,
        1,
        "4g",
        "PT20H",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    // If the bug recurs, this would be 13 instead of 25.
    assert_eq!(result, 25);
}

/// Regression: whole-node long jobs must each get their own allocation.
///
/// 10 memory-heavy 20h jobs that fill an entire node:
///   Correct: walltime=30h, slots=1, 1 job/alloc → 10 allocations
///   Bug:     slots = floor(48h/20h) = 2, 2 jobs/alloc → 5 allocations
#[rstest]
fn test_alloc_regression_whole_node_long_runtime() {
    let result = compute_allocations(
        10,
        12,
        0,
        1,
        "160g",
        "PT20H",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    // If the bug recurs, this would be 5 instead of 10.
    assert_eq!(result, 10);
}

/// Regression: 12h jobs shouldn't get 4 time slots from 48h partition max.
///
///   Correct: walltime=18h, slots=floor(18/12)=1, allocs=ceil(40/4)=10
///   Bug:     slots=floor(48/12)=4, allocs=ceil(40/16)=3
#[rstest]
fn test_alloc_regression_12h_jobs() {
    let result = compute_allocations(
        40,
        26,
        0,
        1,
        "4g",
        "PT12H",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    assert_eq!(result, 10);
}

/// Regression: GPU long jobs must also use correct walltime for time_slots.
///
/// 12 GPU jobs, 12h runtime on gpu-h100 partition (48h max):
///   Correct: walltime=18h, slots=1, concurrent=2 (GPU), allocs=ceil(12/2)=6
///   Bug:     slots=floor(48/12)=4, allocs=ceil(12/8)=2
#[rstest]
fn test_alloc_regression_gpu_long_runtime() {
    let result = compute_allocations(
        12,
        16,
        2,
        1,
        "10g",
        "PT12H",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    // If the bug recurs, this would be 2 instead of 6.
    assert_eq!(result, 6);
}

/// Verify MaxJobRuntime vs MaxPartitionTime produce different allocations for long jobs.
///
/// This confirms the two strategies diverge when they should:
/// - MaxJobRuntime: walltime = 30h, slots = 1
/// - MaxPartitionTime: walltime = 48h, slots = 2
#[rstest]
fn test_alloc_strategy_divergence_long_jobs() {
    let max_job = compute_allocations(
        100,
        26,
        0,
        1,
        "4g",
        "PT20H",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    let max_partition = compute_allocations(
        100,
        26,
        0,
        1,
        "4g",
        "PT20H",
        WalltimeStrategy::MaxPartitionTime,
        1.5,
    );

    // MaxJobRuntime: slots=1, allocs=ceil(100/4)=25
    assert_eq!(max_job, 25);
    // MaxPartitionTime: slots=2, allocs=ceil(100/8)=13
    assert_eq!(max_partition, 13);
    // The two strategies must produce different results here
    assert_ne!(max_job, max_partition);
}

/// Verify MaxJobRuntime and MaxPartitionTime converge for very short jobs
/// where the multiplied runtime is still much less than partition max.
///
/// Both strategies give time_slots=1 with MaxJobRuntime 1.5x on short jobs
/// (since 10min × 1.5 = 15min, and floor(15/10) = 1 = floor(4h/10min) would be 24).
/// So they should NOT converge - MaxPartitionTime should give fewer allocations.
#[rstest]
fn test_alloc_strategy_divergence_short_jobs() {
    let max_job = compute_allocations(
        100,
        26,
        0,
        1,
        "4g",
        "PT10M",
        WalltimeStrategy::MaxJobRuntime,
        1.5,
    );
    let max_partition = compute_allocations(
        100,
        26,
        0,
        1,
        "4g",
        "PT10M",
        WalltimeStrategy::MaxPartitionTime,
        1.5,
    );

    // MaxJobRuntime: slots=1, allocs=ceil(100/4)=25
    assert_eq!(max_job, 25);
    // MaxPartitionTime: time_slots=24, allocs=ceil(100/96)=2
    assert_eq!(max_partition, 2);
}

// ============== resolve_hpc_profile Tests ==============

use torc::client::commands::hpc::resolve_hpc_profile;

#[rstest]
fn test_resolve_hpc_profile_by_name() {
    let mut registry = HpcProfileRegistry::new();
    registry.register(create_test_profile(
        "myprofile",
        vec![create_test_partition("standard", 64, 128000, 86400, None)],
    ));

    let result = resolve_hpc_profile(&registry, Some("myprofile"));
    assert!(result.is_ok());
    assert_eq!(result.unwrap().name, "myprofile");
}

#[rstest]
fn test_resolve_hpc_profile_unknown_name() {
    let registry = HpcProfileRegistry::new();
    let result = resolve_hpc_profile(&registry, Some("nonexistent"));
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unknown HPC profile"));
}

#[rstest]
#[serial(slurm)]
fn test_resolve_hpc_profile_dynamic_slurm_fallback() {
    use std::path::Path;

    let sinfo_path = Path::new("tests/scripts/fake_sinfo.sh")
        .canonicalize()
        .unwrap();
    let scontrol_path = Path::new("tests/scripts/fake_scontrol.sh")
        .canonicalize()
        .unwrap();

    let mut overrides = HashMap::new();
    overrides.insert("TORC_FAKE_SINFO", sinfo_path.to_str().unwrap());
    overrides.insert("TORC_FAKE_SCONTROL", scontrol_path.to_str().unwrap());
    let _guard = EnvGuard::new(overrides);

    // Empty registry with no built-in profiles → should fall back to dynamic Slurm
    let registry = HpcProfileRegistry::new();
    let result = resolve_hpc_profile(&registry, None);
    assert!(result.is_ok());
    let profile = result.unwrap();
    assert_eq!(profile.name, "test_cluster");
    assert!(!profile.partitions.is_empty());
}

#[rstest]
#[serial(slurm)]
fn test_resolve_hpc_profile_no_detection_no_slurm() {
    // Point fake executables to nonexistent paths so Slurm detection always fails
    let mut overrides = HashMap::new();
    overrides.insert("TORC_FAKE_SINFO", "/nonexistent/sinfo");
    overrides.insert("TORC_FAKE_SCONTROL", "/nonexistent/scontrol");
    let _guard = EnvGuard::new(overrides);

    // Empty registry, no Slurm available → should return error
    let registry = HpcProfileRegistry::new();
    let result = resolve_hpc_profile(&registry, None);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("No HPC profile specified"));
}

#[rstest]
#[serial(slurm)]
fn test_resolve_hpc_profile_slurm_special_name() {
    use std::path::Path;

    let sinfo_path = Path::new("tests/scripts/fake_sinfo.sh")
        .canonicalize()
        .unwrap();
    let scontrol_path = Path::new("tests/scripts/fake_scontrol.sh")
        .canonicalize()
        .unwrap();

    let mut overrides = HashMap::new();
    overrides.insert("TORC_FAKE_SINFO", sinfo_path.to_str().unwrap());
    overrides.insert("TORC_FAKE_SCONTROL", scontrol_path.to_str().unwrap());
    let _guard = EnvGuard::new(overrides);

    // Using "slurm" as profile name should trigger dynamic detection
    let registry = HpcProfileRegistry::new();
    let result = resolve_hpc_profile(&registry, Some("slurm"));
    assert!(result.is_ok());
    assert_eq!(result.unwrap().name, "test_cluster");
}

// ============== Partition Selection (tightest-fit) Tests ==============

/// Build a dynamic-style profile where "bigmem" sorts alphabetically before "standard".
/// This reproduces the bug where `.find()` picked the first match (bigmem) instead of
/// the smallest sufficient partition (standard).
fn dynamic_profile_with_bigmem() -> HpcProfile {
    HpcProfile {
        name: "test_dynamic".to_string(),
        display_name: "Test Dynamic Cluster".to_string(),
        description: "Profile for testing tightest-fit partition selection".to_string(),
        detection: vec![],
        default_account: None,
        charge_factor_cpu: 1.0,
        charge_factor_gpu: 10.0,
        metadata: HashMap::new(),
        partitions: vec![
            // Alphabetically first — should NOT be selected for small jobs
            HpcPartition {
                name: "bigmem".to_string(),
                description: "Big memory partition".to_string(),
                cpus_per_node: 104,
                memory_mb: 2_000_000, // ~1953g
                max_walltime_secs: 172800,
                max_nodes: None,
                max_nodes_per_user: None,
                min_nodes: None,
                gpus_per_node: None,
                gpu_type: None,
                gpu_memory_gb: None,
                local_disk_gb: None,
                shared: false,
                requires_explicit_request: false,
                default_qos: Some("p_bigmem".to_string()),
                features: vec![],
            },
            // Alphabetically second — should be selected for small/medium jobs
            HpcPartition {
                name: "standard".to_string(),
                description: "Standard partition".to_string(),
                cpus_per_node: 104,
                memory_mb: 246_064, // ~240g
                max_walltime_secs: 172800,
                max_nodes: None,
                max_nodes_per_user: None,
                min_nodes: None,
                gpus_per_node: None,
                gpu_type: None,
                gpu_memory_gb: None,
                local_disk_gb: None,
                shared: false,
                requires_explicit_request: false,
                default_qos: None,
                features: vec![],
            },
        ],
    }
}

#[rstest]
fn test_find_best_partition_prefers_smallest_memory() {
    let profile = dynamic_profile_with_bigmem();

    // A job needing 50g should land on "standard" (240g), not "bigmem" (1953g)
    let best = profile
        .find_best_partition(4, 50_000, 3600, None)
        .expect("Should find a partition");
    assert_eq!(
        best.name, "standard",
        "Should pick standard (240g) over bigmem (1953g) for a 50g job"
    );
}

#[rstest]
fn test_find_best_partition_uses_bigmem_when_needed() {
    let profile = dynamic_profile_with_bigmem();

    // A job needing 500g must go to bigmem — standard (240g) can't satisfy it
    let best = profile
        .find_best_partition(4, 500_000, 3600, None)
        .expect("Should find a partition");
    assert_eq!(
        best.name, "bigmem",
        "Should pick bigmem when standard can't satisfy memory requirement"
    );
}

#[rstest]
fn test_find_best_partition_gpu_prefers_smallest() {
    // Profile with two GPU partitions of different sizes, alphabetically ordered
    let profile = HpcProfile {
        name: "test_gpu".to_string(),
        display_name: "Test GPU Cluster".to_string(),
        description: String::new(),
        detection: vec![],
        default_account: None,
        charge_factor_cpu: 1.0,
        charge_factor_gpu: 10.0,
        metadata: HashMap::new(),
        partitions: vec![
            HpcPartition {
                name: "gpu-a100".to_string(),
                description: "A100 GPU partition".to_string(),
                cpus_per_node: 64,
                memory_mb: 500_000,
                max_walltime_secs: 172800,
                max_nodes: None,
                max_nodes_per_user: None,
                min_nodes: None,
                gpus_per_node: Some(4),
                gpu_type: Some("a100".to_string()),
                gpu_memory_gb: Some(80),
                local_disk_gb: None,
                shared: false,
                requires_explicit_request: false,
                default_qos: None,
                features: vec![],
            },
            HpcPartition {
                name: "gpu-h100".to_string(),
                description: "H100 GPU partition".to_string(),
                cpus_per_node: 64,
                memory_mb: 200_000,
                max_walltime_secs: 172800,
                max_nodes: None,
                max_nodes_per_user: None,
                min_nodes: None,
                gpus_per_node: Some(4),
                gpu_type: Some("h100".to_string()),
                gpu_memory_gb: Some(80),
                local_disk_gb: None,
                shared: false,
                requires_explicit_request: false,
                default_qos: None,
                features: vec![],
            },
        ],
    };

    // Should pick gpu-h100 (200g) over gpu-a100 (500g) for a small GPU job
    let best = profile
        .find_best_partition(4, 100_000, 3600, Some(1))
        .expect("Should find a GPU partition");
    assert_eq!(
        best.name, "gpu-h100",
        "Should pick the GPU partition with smallest sufficient memory"
    );
}

// ============== Scheduler Override Tests ==============

/// Helper to create a simple workflow spec for override tests
fn create_override_test_spec() -> WorkflowSpec {
    WorkflowSpec {
        name: "override_test".to_string(),
        description: Some("Test workflow for scheduler overrides".to_string()),
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
                resource_requirements: Some("small".to_string()),
                ..Default::default()
            },
        ],
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "small".to_string(),
            num_cpus: 4,
            num_gpus: 0,
            num_nodes: 1,
            memory: "8g".to_string(),
            runtime: "PT1H".to_string(), // 1 hour
        }]),
        ..Default::default()
    }
}

/// Helper to create a profile with two partitions for override tests
fn create_override_test_profile() -> HpcProfile {
    create_test_profile(
        "test_cluster",
        vec![
            // "standard" partition: 104 CPUs, 240GB, 48h max walltime
            create_test_partition("standard", 104, 245760, 172800, None),
            // "short" partition: 104 CPUs, 240GB, 4h max walltime
            create_test_partition("short", 104, 245760, 14400, None),
        ],
    )
}

#[rstest]
fn test_partition_override_uses_specified_partition() {
    let spec = create_override_test_spec();
    let profile = create_override_test_profile();

    let graph = WorkflowGraph::from_spec(&spec).unwrap();
    let rr_vec = spec.resource_requirements.as_ref().unwrap();
    let rr_map: HashMap<&str, &ResourceRequirementsSpec> =
        rr_vec.iter().map(|rr| (rr.name.as_str(), rr)).collect();

    let overrides = SchedulerOverrides {
        partition: Some("short".to_string()),
        walltime_secs: None,
    };

    let plan = generate_scheduler_plan(
        &graph,
        &rr_map,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        None,
        false,
        &overrides,
    );

    assert!(plan.warnings.is_empty(), "Expected no warnings");
    assert_eq!(plan.schedulers.len(), 1);

    let scheduler = &plan.schedulers[0];
    // Partition should be explicitly set to "short" even though it doesn't require explicit request
    assert_eq!(scheduler.partition.as_deref(), Some("short"));
}

#[rstest]
fn test_walltime_override_uses_specified_walltime() {
    let spec = create_override_test_spec();
    let profile = create_override_test_profile();

    let graph = WorkflowGraph::from_spec(&spec).unwrap();
    let rr_vec = spec.resource_requirements.as_ref().unwrap();
    let rr_map: HashMap<&str, &ResourceRequirementsSpec> =
        rr_vec.iter().map(|rr| (rr.name.as_str(), rr)).collect();

    // Override walltime to exactly 2 hours (7200 seconds)
    let overrides = SchedulerOverrides {
        partition: None,
        walltime_secs: Some(7200),
    };

    let plan = generate_scheduler_plan(
        &graph,
        &rr_map,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        None,
        false,
        &overrides,
    );

    assert!(plan.warnings.is_empty(), "Expected no warnings");
    assert_eq!(plan.schedulers.len(), 1);
    assert_eq!(plan.schedulers[0].walltime, "02:00:00");
}

#[rstest]
fn test_partition_and_walltime_override_together() {
    let spec = create_override_test_spec();
    let profile = create_override_test_profile();

    let graph = WorkflowGraph::from_spec(&spec).unwrap();
    let rr_vec = spec.resource_requirements.as_ref().unwrap();
    let rr_map: HashMap<&str, &ResourceRequirementsSpec> =
        rr_vec.iter().map(|rr| (rr.name.as_str(), rr)).collect();

    let overrides = SchedulerOverrides {
        partition: Some("short".to_string()),
        walltime_secs: Some(14400), // 4 hours
    };

    let plan = generate_scheduler_plan(
        &graph,
        &rr_map,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        None,
        false,
        &overrides,
    );

    assert!(plan.warnings.is_empty(), "Expected no warnings");
    assert_eq!(plan.schedulers.len(), 1);
    assert_eq!(plan.schedulers[0].partition.as_deref(), Some("short"));
    assert_eq!(plan.schedulers[0].walltime, "04:00:00");
}

#[rstest]
fn test_invalid_partition_override_returns_warning() {
    let spec = create_override_test_spec();
    let profile = create_override_test_profile();

    let graph = WorkflowGraph::from_spec(&spec).unwrap();
    let rr_vec = spec.resource_requirements.as_ref().unwrap();
    let rr_map: HashMap<&str, &ResourceRequirementsSpec> =
        rr_vec.iter().map(|rr| (rr.name.as_str(), rr)).collect();

    let overrides = SchedulerOverrides {
        partition: Some("nonexistent".to_string()),
        walltime_secs: None,
    };

    let plan = generate_scheduler_plan(
        &graph,
        &rr_map,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        None,
        false,
        &overrides,
    );

    // Should return no schedulers and a single warning
    assert!(plan.schedulers.is_empty());
    assert_eq!(plan.warnings.len(), 1);
    assert!(plan.warnings[0].contains("nonexistent"));
    assert!(plan.warnings[0].contains("not found"));
}

#[rstest]
fn test_no_overrides_uses_auto_selection() {
    let spec = create_override_test_spec();
    let profile = create_override_test_profile();

    let graph = WorkflowGraph::from_spec(&spec).unwrap();
    let rr_vec = spec.resource_requirements.as_ref().unwrap();
    let rr_map: HashMap<&str, &ResourceRequirementsSpec> =
        rr_vec.iter().map(|rr| (rr.name.as_str(), rr)).collect();

    let plan = generate_scheduler_plan(
        &graph,
        &rr_map,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        None,
        false,
        &SchedulerOverrides::default(),
    );

    assert!(plan.warnings.is_empty(), "Expected no warnings");
    assert_eq!(plan.schedulers.len(), 1);

    // With default overrides, partition should NOT be explicitly set
    // (since neither test partition requires explicit request)
    assert!(plan.schedulers[0].partition.is_none());

    // Walltime should be auto-calculated: 1h * 1.5 = 1.5h = 01:30:00
    assert_eq!(plan.schedulers[0].walltime, "01:30:00");
}

#[rstest]
fn test_walltime_override_affects_allocation_count() {
    // With a longer walltime, more jobs fit per allocation → fewer allocations needed
    let mut spec = WorkflowSpec {
        name: "alloc_test".to_string(),
        jobs: (0..10)
            .map(|i| JobSpec {
                name: format!("job_{}", i),
                command: "echo hello".to_string(),
                resource_requirements: Some("compute".to_string()),
                ..Default::default()
            })
            .collect(),
        resource_requirements: Some(vec![ResourceRequirementsSpec {
            name: "compute".to_string(),
            num_cpus: 104, // uses full node
            num_gpus: 0,
            num_nodes: 1,
            memory: "240g".to_string(),
            runtime: "PT1H".to_string(), // 1 hour per job
        }]),
        ..Default::default()
    };

    // Expand parameters so the spec is ready
    spec.expand_parameters().unwrap();

    let profile = create_override_test_profile();
    let graph = WorkflowGraph::from_spec(&spec).unwrap();
    let rr_vec = spec.resource_requirements.as_ref().unwrap();
    let rr_map: HashMap<&str, &ResourceRequirementsSpec> =
        rr_vec.iter().map(|rr| (rr.name.as_str(), rr)).collect();

    // With 1h walltime: 1 job per allocation → 10 allocations
    let plan_short = generate_scheduler_plan(
        &graph,
        &rr_map,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.0, // no multiplier
        true,
        None,
        false,
        &SchedulerOverrides {
            partition: Some("standard".to_string()),
            walltime_secs: Some(3600), // 1 hour
        },
    );

    // With 4h walltime: 4 jobs per allocation → ceil(10/4) = 3 allocations
    let plan_long = generate_scheduler_plan(
        &graph,
        &rr_map,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::ResourceRequirements,
        WalltimeStrategy::MaxJobRuntime,
        1.0,
        true,
        None,
        false,
        &SchedulerOverrides {
            partition: Some("standard".to_string()),
            walltime_secs: Some(14400), // 4 hours
        },
    );

    assert_eq!(plan_short.total_allocations(), 10);
    assert_eq!(plan_long.total_allocations(), 3);
}

#[rstest]
fn test_find_partition_by_name() {
    let profile = create_override_test_profile();

    assert!(profile.find_partition_by_name("standard").is_some());
    assert!(profile.find_partition_by_name("short").is_some());
    assert!(profile.find_partition_by_name("nonexistent").is_none());

    let partition = profile.find_partition_by_name("standard").unwrap();
    assert_eq!(partition.cpus_per_node, 104);
    assert_eq!(partition.max_walltime_secs, 172800);
}

#[rstest]
fn test_partition_override_with_group_by_partition() {
    let spec = WorkflowSpec {
        name: "group_by_test".to_string(),
        jobs: vec![
            JobSpec {
                name: "small_job".to_string(),
                command: "echo small".to_string(),
                resource_requirements: Some("small".to_string()),
                ..Default::default()
            },
            JobSpec {
                name: "medium_job".to_string(),
                command: "echo medium".to_string(),
                resource_requirements: Some("medium".to_string()),
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
                runtime: "PT2H".to_string(),
            },
        ]),
        ..Default::default()
    };

    let profile = create_override_test_profile();
    let graph = WorkflowGraph::from_spec(&spec).unwrap();
    let rr_vec = spec.resource_requirements.as_ref().unwrap();
    let rr_map: HashMap<&str, &ResourceRequirementsSpec> =
        rr_vec.iter().map(|rr| (rr.name.as_str(), rr)).collect();

    // With partition override + group-by-partition, all jobs go to the specified partition
    let overrides = SchedulerOverrides {
        partition: Some("short".to_string()),
        walltime_secs: Some(14400),
    };

    let plan = generate_scheduler_plan(
        &graph,
        &rr_map,
        &profile,
        "testaccount",
        false,
        GroupByStrategy::Partition,
        WalltimeStrategy::MaxJobRuntime,
        1.5,
        true,
        None,
        false,
        &overrides,
    );

    assert!(plan.warnings.is_empty(), "Expected no warnings");

    // All schedulers should use the "short" partition
    for scheduler in &plan.schedulers {
        assert_eq!(scheduler.partition.as_deref(), Some("short"));
        assert_eq!(scheduler.walltime, "04:00:00");
    }
}
